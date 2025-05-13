use cdk::amount::SplitTarget;
use cdk::mint_url::MintUrl;
use cdk::nuts::nut00::ProofsMethods;
use cdk::nuts::{CurrencyUnit, MintQuoteState};
use cdk::wallet::multi_mint_wallet::MultiMintWallet;
use cdk::wallet::types::WalletKey;
use cdk::wallet::{SendOptions, Wallet, WalletBuilder};
use cdk::Amount;
use cdk_common::database::WalletDatabase;
use cdk_redb::WalletRedbDatabase;
// use cdk_sqlite::wallet::memory;
// use cdk_sqlite::WalletSqliteDatabase;

use seedstore::{ChildSpecifier, SeedStore, SeedStoreCreator};

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const KEY_DERIVATION_PATH: &str = "m/84'/0'/0'/0/0";

/// Parakesh application, based on CDK.
pub struct PKApp {
    /// Stores the seed
    seedstore: SeedStore,
    unit: CurrencyUnit,
    /// CDK ecash store
    store: Arc<WalletRedbDatabase>,
    /// CDK multi-mint wallet
    multi_mint_wallet: MultiMintWallet,
    // Current mint, to use with operations
    selected_mint: Option<MintUrl>,
}

/// Summary info about the mints
#[derive(Clone, Debug, Default)]
pub enum MintsSummary {
    #[default]
    None,
    Single(String),
    Multiple(usize),
}

/// Ecash wallet struct
#[derive(Clone, Debug, Default)]
pub struct WalletInfo {
    pub is_inititalized: bool,
    pub mint_count: usize,
    pub mints_summary: MintsSummary,
    pub selected_mint_url: String,
}

#[derive(Clone, Debug, Default)]
pub struct BalanceInfo(pub u64);

#[derive(Clone, Debug)]
pub struct MintInfo {
    pub url: String,
    pub balance: u64,
}

/// Used with CDK error
#[derive(Debug, PartialEq, Eq)]
pub struct StringError(String);

impl std::error::Error for StringError {}

impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

const CHECK_STEP_INCREASE: f64 = 1.05;

/// Intermediary result used in `mint_from_ln_start` and `mint_from_ln_wait`.
#[derive(Clone, Debug)]
pub struct MintFromLnIntermediaryResult {
    mint_quote: cdk::wallet::MintQuote,
    /// Set if complete (paid)
    pub paid_result: Option<Result<u64, String>>,
    timeout_time: SystemTime,
    pub next_check_time: SystemTime,
    step: f64,
}

pub struct PKAppLazy {
    app: Option<PKApp>,
}

impl PKApp {
    /// Create new app instance
    pub async fn new() -> Result<PKApp, String> {
        // TODO should be in config dir
        let secret_seed_file_name = "./parakesh.secret";
        // TODO should be user input
        let seed_encryption_password = "Parakesh+Password1337";
        let seedstore = match SeedStore::new_from_encrypted_file(
            secret_seed_file_name,
            &seed_encryption_password.to_owned(),
            None,
        ) {
            Ok(seedstore) => seedstore,
            Err(_e) => {
                // Could not read seed, generate a new one
                // TODO do this with init, read PW, etc.
                // TODO Should be random!
                let secret_key = [43u8; 16].to_vec();
                match SeedStoreCreator::new_from_data(&secret_key, None, None) {
                    Ok(seedstore) => {
                        let _res = SeedStoreCreator::write_to_file(
                            &seedstore,
                            secret_seed_file_name,
                            seed_encryption_password,
                            None, // allow weak pw
                        )?;
                        println!("Seed written to secret file {}", secret_seed_file_name);
                        // Try to open again
                        match SeedStore::new_from_encrypted_file(
                            secret_seed_file_name,
                            &seed_encryption_password.to_owned(),
                            None,
                        ) {
                            Ok(seedstore) => seedstore,
                            Err(e2) => {
                                return Err(format!(
                                    "Could not read seed from freshly-generated secret file ({})",
                                    e2
                                ));
                            }
                        }
                    }
                    Err(e3) => {
                        return Err(format!("Could not read seed from secret file, and could not save newly-generated seed ({})", e3));
                    }
                }
            }
        };

        let unit = CurrencyUnit::Sat;

        // Initialize the memory store
        // let store = memory::empty().await?;
        // TODO should be in config dir
        let path = std::path::Path::new("./parakesh_data.dedb");
        let store = Arc::new(WalletRedbDatabase::new(&path).unwrap());

        // read the wallets, create Wallet instances
        let mut wallets: Vec<Wallet> = Vec::new();
        let db_mints = store.get_mints().await.map_err(|e| e.to_string())?;
        let seed_privkey = seedstore.get_secret_child_private_key(&ChildSpecifier::Derivation(
            KEY_DERIVATION_PATH.into(),
        ))?;
        for (mint_url, _) in db_mints {
            let builder = WalletBuilder::new()
                .mint_url(mint_url.clone())
                .unit(unit.clone())
                .localstore(store.clone())
                .seed(seed_privkey.as_ref());
            let wallet = builder.build().map_err(|e| e.to_string())?;
            wallets.push(wallet);
        }

        let wallets_len = wallets.len();
        let multi_mint_wallet = MultiMintWallet::new(wallets);

        let mut app = PKApp {
            seedstore,
            unit,
            store,
            multi_mint_wallet,
            selected_mint: None, // set below
        };

        // Select first wallet
        if wallets_len > 0 {
            let _res = app
                .select_mint_by_number(1)
                .await
                .map_err(|e| e.to_string())?;
        }

        Ok(app)
    }

    pub async fn get_wallet_info(&self) -> Result<WalletInfo, String> {
        let wallets = self.multi_mint_wallet.get_wallets().await;
        let mint_count = wallets.len();
        let selected_mint_url = if let Some(mint) = &self.selected_mint {
            mint.to_string()
        } else {
            "".to_string()
        };
        let mints_summary = match wallets.len() {
            0 => MintsSummary::None,
            1 => MintsSummary::Single(wallets[0].mint_url.to_string()),
            _ => MintsSummary::Multiple(wallets.len()),
        };
        Ok(WalletInfo {
            is_inititalized: true,
            mint_count,
            mints_summary,
            selected_mint_url,
        })
    }

    pub async fn get_balance(&self) -> Result<BalanceInfo, String> {
        let wallet_balances: BTreeMap<MintUrl, Amount> = self
            .multi_mint_wallet
            .get_balances(&CurrencyUnit::Sat)
            .await
            .map_err(|e| e.to_string())?;
        let total_balance: u64 = wallet_balances
            .iter()
            .map(|(_url, a)| {
                let u: u64 = (*a).into();
                u
            })
            .sum();
        Ok(BalanceInfo(total_balance))
    }

    async fn get_mint_wallet(
        &self,
        mint_url: MintUrl,
    ) -> Result<Wallet, Box<dyn std::error::Error>> {
        let wallet_key = WalletKey::new(mint_url.clone(), self.unit.clone());
        match self.multi_mint_wallet.get_wallet(&wallet_key).await {
            Some(wallet) => Ok(wallet.clone()),
            None => {
                return Err(Box::new(StringError(format!(
                    "Mint not found, {}",
                    mint_url.to_string()
                ))))
            }
        }
    }

    // async fn get_mint_wallet_str(
    //     &mut self,
    //     mint_url_str: &str,
    // ) -> Result<Wallet, Box<dyn std::error::Error>> {
    //     let mint_url = MintUrl::from_str(mint_url_str)?;
    //     self.get_mint_wallet(mint_url).await
    // }

    fn get_seed(&self) -> Result<[u8; 32], Box<dyn std::error::Error>> {
        let seed_privkey =
            self.seedstore
                .get_secret_child_private_key(&ChildSpecifier::Derivation(
                    KEY_DERIVATION_PATH.into(),
                ))?;
        Ok(seed_privkey.as_ref().clone())
    }

    pub async fn add_mint(&mut self, mint_url_str: &str) -> Result<(), Box<dyn std::error::Error>> {
        let wallet = Wallet::new(
            &mint_url_str.to_string(),
            self.unit.clone(),
            self.store.clone(),
            &self.get_seed()?,
            None,
        )?;
        // This is needed to store the mint in the store
        let mint_info = wallet.get_mint_info().await?;
        if let Some(_info) = mint_info {
        } else {
            return Err(Box::new(StringError(format!(
                "Could not obtain mint info for {}",
                mint_url_str
            ))));
        }
        self.multi_mint_wallet.add_wallet(wallet).await;
        let _res = self
            .select_mint(mint_url_str)
            .await
            .map_err(|e| Box::new(StringError(e)))?;
        Ok(())
    }

    pub fn selected_mint(&self) -> String {
        match &self.selected_mint {
            Some(mint) => mint.to_string(),
            None => "(none)".to_string(),
        }
    }

    pub async fn select_mint(&mut self, mint_url_str: &str) -> Result<(), String> {
        let mint_url = MintUrl::from_str(mint_url_str).map_err(|e| e.to_string())?;
        let _wallet = self
            .get_mint_wallet(mint_url.clone())
            .await
            .map_err(|e| e.to_string())?;
        self.selected_mint = Some(mint_url);
        Ok(())
    }

    pub async fn select_mint_by_number(
        &mut self,
        mint_index_1_based: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let wallets = &self.multi_mint_wallet.get_wallets().await;
        if mint_index_1_based == 0 {
            return Err(Box::new(StringError(format!(
                "Invalid mint index {}, the first is 1",
                mint_index_1_based
            ))));
        }
        let max_index = wallets.len();
        if mint_index_1_based > max_index {
            return Err(Box::new(StringError(format!(
                "Invalid mint index {}, maximum is {}",
                mint_index_1_based, max_index
            ))));
        }
        let mint_url = &wallets[mint_index_1_based - 1].mint_url;
        self.selected_mint = Some(mint_url.clone());
        Ok(())
    }

    pub async fn get_mints_info(&self) -> Result<Vec<MintInfo>, String> {
        let wallets = &self.multi_mint_wallet.get_wallets().await;
        let mut info = Vec::new();
        for wallet in wallets.iter() {
            let balance: u64 = wallet.total_balance().await.unwrap_or_default().into();
            info.push(MintInfo {
                url: wallet.mint_url.to_string(),
                balance,
            });
        }
        Ok(info)
    }

    pub async fn receive_ecash(&mut self, token: &str) -> Result<u64, String> {
        if let Some(sel_mint) = &self.selected_mint {
            let wallet = self
                .get_mint_wallet(sel_mint.clone())
                .await
                .map_err(|e| e.to_string())?;

            // Receive the token
            let received = wallet
                .receive(token, SplitTarget::default(), &[], &[])
                .await
                .map_err(|e| e.to_string())?;
            Ok(received.into())
        } else {
            Err("No selected mint!".to_owned())
        }
    }

    pub async fn send_ecash(&mut self, amount_sats: u64) -> Result<String, String> {
        if let Some(sel_mint) = &self.selected_mint {
            let wallet = self
                .get_mint_wallet(sel_mint.clone())
                .await
                .map_err(|e| e.to_string())?;
            // Send the token
            let prepared_send = wallet
                .prepare_send(Amount::from(amount_sats), SendOptions::default())
                .await
                .map_err(|e| e.to_string())?;
            let token = wallet
                .send(prepared_send, None)
                .await
                .map_err(|e| e.to_string())?;

            Ok(token.to_v3_string())
        } else {
            Err("No selected mint!".to_string())
        }
    }

    /// Run `mint_from_ln_start` and `mint_from_ln_wait` in sequence.
    /// Return the invoice in a callback.
    /// - `callback`: This callback is called with the invoice to be paid.
    pub async fn mint_from_ln<F: FnOnce(&str)>(
        &mut self,
        amount_sats: u64,
        callback: F,
    ) -> Result<u64, String> {
        let (invoice, intermediary_res) = self.mint_from_ln_start(amount_sats).await?;
        (callback)(invoice.as_str());
        self.mint_from_ln_wait(intermediary_res).await
    }

    /// Receive Lightning: perform Mint from Lightning invoice.
    /// First part, generate the invoice to be paid, and return it, alongside an
    /// intermediary result with which `mint_from_ln_wait` should be invoked.
    pub async fn mint_from_ln_start(
        &mut self,
        amount_sats: u64,
    ) -> Result<(String, MintFromLnIntermediaryResult), String> {
        if let Some(sel_mint) = &self.selected_mint {
            let wallet = self
                .get_mint_wallet(sel_mint.clone())
                .await
                .map_err(|e| e.to_string())?;

            // Request a mint quote from the wallet
            let mint_quote = wallet
                .mint_quote(Amount::from(amount_sats), None)
                .await
                .map_err(|e| e.to_string())?;

            // println!("Pay request: {}", quote.request);
            let invoice_to_be_paid = mint_quote.request.clone();
            let now = SystemTime::now();
            let step = 2000f64; // ms
            Ok((
                invoice_to_be_paid,
                MintFromLnIntermediaryResult {
                    mint_quote,
                    paid_result: None,
                    timeout_time: now.checked_add(Duration::from_secs(5 * 60)).unwrap_or(now),
                    next_check_time: now
                        .checked_add(Duration::from_millis(step as u64))
                        .unwrap_or(now),
                    step,
                },
            ))
        } else {
            Err("No selected mint!".to_string())
        }
    }

    /// Check for the status once
    /// Returns an intermediary result, or the amount if paid
    pub async fn mint_from_ln_check(
        &mut self,
        intermediary_result: MintFromLnIntermediaryResult,
    ) -> Result<MintFromLnIntermediaryResult, String> {
        if intermediary_result.paid_result.is_some() {
            return Ok(intermediary_result);
        }
        if let Some(sel_mint) = &self.selected_mint {
            let wallet = self
                .get_mint_wallet(sel_mint.clone())
                .await
                .map_err(|e| e.to_string())?;

            let quote_id = &intermediary_result.mint_quote.id;
            let status = wallet
                .mint_quote_state(quote_id)
                .await
                .map_err(|e| e.to_string())?;
            if status.state == MintQuoteState::Paid {
                // Mint the received amount
                let proofs = wallet
                    .mint(quote_id, SplitTarget::default(), None)
                    .await
                    .map_err(|e| e.to_string())?;
                let receive_amount = proofs.total_amount().map_err(|e| e.to_string())?;
                let mut res2 = intermediary_result;
                res2.paid_result = Some(Ok(receive_amount.into()));
                return Ok(res2);
            }
            // not paid yet
            let now = SystemTime::now();
            if now > intermediary_result.timeout_time {
                return Err("Timeout while waiting for mint quote to be paid".into());
            }
            // still need to wait
            println!("Quote state: {}", status.state);

            let mut res2 = intermediary_result;
            res2.next_check_time = res2
                .next_check_time
                .checked_add(Duration::from_millis(res2.step as u64))
                .unwrap_or(res2.next_check_time);
            res2.step = res2.step * CHECK_STEP_INCREASE;
            Ok(res2)
        } else {
            Err("No selected mint!".to_string())
        }
    }

    /// Second part of `mint_from_ln_start`, should be invoked with the intermediary result.
    /// Polls for result, waits until a result is available (invoice had been paid), or timeout.
    /// Returns the amount received.
    /// Warning: Returns in a long time (waits until user action)
    pub async fn mint_from_ln_wait(
        &mut self,
        intermediary_result: MintFromLnIntermediaryResult,
    ) -> Result<u64, String> {
        // Check the quote state in a loop with a timeout
        let mut int_res = intermediary_result;
        loop {
            let res2 = self.mint_from_ln_check(int_res).await?;
            if let Some(res) = res2.paid_result {
                return res;
            }
            // not paid, wait some more
            let now = SystemTime::now();
            let to_wait = res2
                .next_check_time
                .duration_since(now)
                .unwrap_or(Duration::from_millis(10));
            int_res = res2;
            // sleep(to_wait).await;
            tokio::time::sleep(to_wait).await;
        }
    }

    pub async fn melt_to_ln(&mut self, ln_invoice: &str) -> Result<u64, String> {
        if let Some(sel_mint) = &self.selected_mint {
            let wallet = self
                .get_mint_wallet(sel_mint.clone())
                .await
                .map_err(|e| e.to_string())?;

            println!("About to melt_quote...");
            // Request a melt quote from the wallet
            let quote = wallet
                .melt_quote(ln_invoice.to_string(), None)
                .await
                .map_err(|e| e.to_string())?;
            println!("Melt quote: {} {} {:?}", quote.amount, quote.state, quote,);

            /*
            // Check the quote state in a loop with a timeout
            let timeout = Duration::from_secs(60); // Set a timeout duration
            let start = std::time::Instant::now();

            loop {
                let status = wallet.melt_quote_status(&quote.id).await?;
                println!("status {:?}", status);

                if status.state == MeltQuoteState::Paid {
                    break;
                }
                if start.elapsed() >= timeout {
                    return Err("Timeout while waiting for mint quote to be paid".into());
                }

                println!("Quote state: {}", status.state);
                sleep(Duration::from_millis(1500)).await;
            }
            */

            // Melt the sent amount
            let melted = wallet.melt(&quote.id).await.map_err(|e| e.to_string())?;
            Ok(melted.amount.into())
        } else {
            Err("No selected mint!".to_owned())
        }
    }
}

impl PKAppLazy {
    pub fn new() -> Self {
        Self { app: None }
    }

    pub async fn init(&mut self) -> Result<(), String> {
        if self.app.is_some() {
            return Ok(());
        }
        self.app = Some(PKApp::new().await?);
        Ok(())
    }

    pub async fn get_balance(&self) -> Result<BalanceInfo, String> {
        self.app.as_ref().expect("Not inited").get_balance().await
    }
}
