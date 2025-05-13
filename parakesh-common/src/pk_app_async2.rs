// use crate::async_queue::{Queue, QueueSender};
use crate::pk_app::{BalanceInfo, MintFromLnIntermediaryResult, MintInfo, PKApp, WalletInfo};
use crate::simple_queue::{Queue, QueueSender};

/// Events delivered to the callback.
#[derive(Clone, Debug)]
pub enum AppEvent {
    GetBalance,
    CreateResult(Result<(), String>),
    WalletInfo(Result<WalletInfo, String>),
    BalanceChange(Result<BalanceInfo, String>),
    MintsInfo(Result<Vec<MintInfo>, String>),
    MintSelected(Result<(), String>),
    MintFromLnInvoice(String),
    MintFromLnRes(Result<u64, String>),
    ReceivedEC(Result<u64, String>),
    MeltToLnRes(Result<u64, String>),
    SendECRes(Result<String, String>),
}

/// Type for callback function from the app
type AppCallback = fn(app: &AppEvent);

/// Requests, used internally to pass requests to processing thread.
#[derive(Clone)]
enum AppRequest {
    GetWalletInfo,
    GetBalance,
    GetMintsInfo,
    SelectMint(String),
    MintFromLn(u64),
    MintFromLnCheck(MintFromLnIntermediaryResult),
    ReceiveEC(String),
    MeltToLn(String),
    SendEC(u64),
}

/// App (PKApp) done in an async way with callbacks, not with async/await,
/// for use in environments without async/await (e.g. iced)
pub struct PKAppAsync2 {
    sender: QueueSender<AppRequest>,
}

impl PKAppAsync {
    /// Create instance asynchronously, start background thread
    pub fn new(callback: AppCallback) -> Result<Self, String> {
        let mut queue = Queue::new();
        let sender = queue.get_sender_clone();
        let sender2 = queue.get_sender_clone();
        let app_async = PKAppAsync { sender };

        // Start background processor thread
        let _handle = tokio::task::spawn(async move {
            match PKApp::new().await {
                Err(err) => {
                    let err_msg = format!("Could not create app, {}", err.to_string());
                    eprint!("{}", err_msg);
                    (callback)(&AppEvent::CreateResult(Err(err_msg)));
                }
                Ok(ref mut app) => {
                    Self::process_app_requests_loop(app, &mut queue, callback, &sender2).await;
                }
            }
        });

        Ok(app_async)
    }

    async fn process_app_requests_loop(
        app: &mut PKApp,
        queue: &mut Queue<AppRequest>,
        callback: AppCallback,
        sender: &QueueSender<AppRequest>,
    ) {
        loop {
            match queue.recv().await {
                None => {}
                Some(req) => {
                    // Took a request
                    Self::process_one_request(req, app, callback, sender).await;
                }
            }
        }
    }

    async fn process_one_request(
        req: AppRequest,
        app: &mut PKApp,
        callback: AppCallback,
        sender: &QueueSender<AppRequest>,
    ) {
        match req {
            AppRequest::GetWalletInfo => {
                let res = app.get_wallet_info().await;
                (callback)(&AppEvent::WalletInfo(res));
            }
            AppRequest::GetBalance => {
                let res = app.get_balance().await;
                (callback)(&AppEvent::BalanceChange(res));
            }
            AppRequest::GetMintsInfo => {
                let res = app.get_mints_info().await;
                (callback)(&AppEvent::MintsInfo(res));
            }
            AppRequest::SelectMint(url) => {
                let res = app.select_mint(url.as_str()).await;
                (callback(&AppEvent::MintSelected(res)));
            }
            AppRequest::MintFromLn(amount) => {
                match app.mint_from_ln_start(amount).await {
                    Err(err) => {
                        (callback)(&AppEvent::MintFromLnRes(Err(err)));
                    }
                    Ok((invoice, intermediary_result)) => {
                        (callback)(&AppEvent::MintFromLnInvoice(invoice.to_owned()));
                        if let Some(res) = intermediary_result.paid_result {
                            (callback)(&AppEvent::MintFromLnRes(res));
                        } else {
                            let next_check_time = intermediary_result.next_check_time;
                            let _res = sender.send(
                                AppRequest::MintFromLnCheck(intermediary_result),
                                // Some(next_check_time),
                            );
                        }
                    }
                };
            }
            AppRequest::MintFromLnCheck(intermediary_result) => {
                if let Some(res) = intermediary_result.paid_result {
                    (callback)(&AppEvent::MintFromLnRes(res));
                } else {
                    let next_check_time = intermediary_result.next_check_time;
                    let _res = sender.send(
                        AppRequest::MintFromLnCheck(intermediary_result),
                        // Some(next_check_time),
                    );
                }
            }
            AppRequest::MeltToLn(invoice) => {
                let res = app.melt_to_ln(&invoice).await;
                (callback)(&AppEvent::MeltToLnRes(res));
            }
            AppRequest::ReceiveEC(token) => {
                let res = app.receive_ecash(&token).await;
                (callback)(&AppEvent::ReceivedEC(res));
            }
            AppRequest::SendEC(amount) => {
                let res = app.send_ecash(amount).await;
                (callback)(&&AppEvent::SendECRes(res));
            }
        }
    }

    pub fn get_wallet_info_async(&self) -> Result<(), String> {
        self.sender.send(AppRequest::GetWalletInfo)
    }

    pub fn get_balance_async(&self) -> Result<(), String> {
        self.sender.send(AppRequest::GetBalance)
    }

    pub fn get_mints_info_async(&self) -> Result<(), String> {
        self.sender.send(AppRequest::GetMintsInfo)
    }

    pub fn select_mint(&mut self, mint_url_str: String) -> Result<(), String> {
        self.sender.send(AppRequest::SelectMint(mint_url_str))
    }

    pub fn mint_from_ln(&mut self, amount_sats: u64) -> Result<(), String> {
        self.sender.send(AppRequest::MintFromLn(amount_sats))
    }

    pub fn receive_ec(&mut self, ecash_token: String) -> Result<(), String> {
        self.sender.send(AppRequest::ReceiveEC(ecash_token))
    }

    pub fn melt_to_ln(&mut self, invoice_to_pay: String) -> Result<(), String> {
        self.sender.send(AppRequest::MeltToLn(invoice_to_pay))
    }
    pub fn send_ec(&mut self, amount_sats: u64) -> Result<(), String> {
        self.sender.send(AppRequest::SendEC(amount_sats))
    }
}
