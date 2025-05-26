use crate::pk_app::{BalanceInfo, MintFromLnIntermediaryResult, MintInfo, PKApp, WalletInfo};
use futures::channel::mpsc::{self, Receiver, Sender};
use futures::task::AtomicWaker;
use futures::{stream, SinkExt, Stream, StreamExt};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};

/// Events delivered to the callback.
#[derive(Clone, Debug)]
pub enum AppEvent {
    WalletInfo(Result<WalletInfo, String>),
    BalanceChange(Result<BalanceInfo, String>),
    BalanceAndWalletInfo(Result<(BalanceInfo, WalletInfo), String>),
    MintsInfo(Result<Vec<MintInfo>, String>),
    MintSelectedByUrl(Result<String, String>),
    MintSelectedByIndex(Result<usize, String>),
    MintAdded(Result<(), String>),
    MintFromLnInvoice(String),
    MintFromLnRes(Result<u64, String>),
    ReceivedEC(Result<u64, String>),
    MeltToLnRes(Result<u64, String>),
    SendECRes(Result<(u64, String), String>),
}

/// Requests, used internally to pass requests to processing thread.
#[derive(Clone, Debug)]
pub enum AppRequest {
    InitApp(Sender<AppEvent>),
    GetWalletInfo,
    GetBalance,
    GetBalanceAndWalletInfo,
    GetMintsInfo,
    SelectMintByUrl(String),
    SelectMintByIndex(usize),
    AddMint(String),
    MintFromLn(u64),
    ReceiveEC(String),
    MeltToLn(String),
    SendEC(u64),
    /// A poll to execute
    Poll(MintFromLnIntermediaryResult),
}

const CHECK_STEP_INCREASE: f64 = 1.05;

/// An operations that needs periodic polling.
#[derive(Clone)]
pub struct PendingPoll {
    /// Later could be turned into enum if there are more types.
    result: MintFromLnIntermediaryResult,
    next_time: SystemTime,
    step: Duration,
    #[allow(unused)]
    start_time: SystemTime,
    stop_time: SystemTime,
    slowing_factor: f64,
}

/// Keeps pending poll operations, if there are any.
#[derive(Clone)]
pub struct PendingPolls {
    p: Arc<RwLock<HashMap<String, PendingPoll>>>,
    waker: Arc<RwLock<AtomicWaker>>,
}

/// App (PKApp) done in an async way with callbacks, not with async/await,
/// for use in environments without async/await (e.g. iced)
#[derive(Clone)]
pub struct PKAppAsync {
    incoming_sender: Sender<AppRequest>,
}

impl PendingPoll {
    fn advance(&mut self) {
        self.next_time = self.next_time.checked_add(self.step).unwrap();
        self.step = self.step.mul_f64(self.slowing_factor);
    }
}

impl PendingPolls {
    pub fn new() -> Self {
        Self {
            p: Arc::new(RwLock::new(HashMap::new())),
            waker: Arc::new(RwLock::new(AtomicWaker::new())),
        }
    }

    pub fn count(&self) -> usize {
        self.p.read().unwrap().len()
    }

    pub fn add(&mut self, poll: PendingPoll) {
        let id = poll.result.id();
        self.p.write().unwrap().insert(id, poll);
        self.waker.write().unwrap().wake();
    }

    pub fn add2(&mut self, result: MintFromLnIntermediaryResult, step_ms: u64, max_time_sec: u64) {
        let now = SystemTime::now();
        let poll = PendingPoll {
            result,
            next_time: now,
            step: Duration::from_millis(step_ms),
            start_time: now,
            stop_time: now.checked_add(Duration::from_secs(max_time_sec)).unwrap(),
            slowing_factor: CHECK_STEP_INCREASE,
        };
        self.add(poll);
    }

    /// Return the earliest runnable operation.
    /// Also return the duration till the earliest future time, if there is no runnable
    /// This should not write-lock
    fn get_runnable(&mut self) -> (Option<String>, Option<Duration>) {
        let mut earliest: Option<(String, SystemTime)> = None;
        {
            let pw = self.p.read().unwrap();
            for key in pw.keys() {
                let p = &pw[key];
                let is_earlier = if let Some((ref _key, earliest)) = earliest {
                    p.next_time < earliest
                } else {
                    true
                };
                if is_earlier {
                    earliest = Some((key.clone(), p.next_time));
                }
            }
        }
        if let Some((ref key, earliest)) = earliest {
            // check if this earliest is already runnable (in the past)
            let now = SystemTime::now();
            if let Some(ref poll) = &self.p.read().unwrap().get(key) {
                if poll.next_time <= now {
                    (Some(key.clone()), None)
                } else {
                    // no runnable
                    (None, Some(earliest.duration_since(now).unwrap_or_default()))
                }
            } else {
                (None, Some(earliest.duration_since(now).unwrap_or_default()))
            }
        } else {
            // no operation
            (None, None)
        }
    }

    fn prepare_for_run(&mut self, key: &String) -> Option<AppRequest> {
        // check if there should be more iterations
        let more_runs = {
            if let Some(ref poll) = self.p.read().unwrap().get(key) {
                if poll.next_time > poll.stop_time {
                    // no more runs
                    false
                } else {
                    // more runs
                    true
                }
            } else {
                // not runnable
                return None;
            }
        };
        if more_runs {
            // update `next time``
            if let Some(ref mut poll) = self.p.write().unwrap().get_mut(key) {
                poll.advance();
                Some(AppRequest::Poll(poll.result.clone()))
            } else {
                None
            }
        } else {
            // no more runs
            let removed = self.remove(key);
            removed.map(|poll| AppRequest::Poll(poll.result))
        }
    }

    fn remove(&mut self, key: &str) -> Option<PendingPoll> {
        self.p.write().unwrap().remove(key)
    }
}

impl Stream for PendingPolls {
    type Item = AppRequest;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<AppRequest>> {
        let (runnable_key, to_wait) = self.get_runnable();
        if let Some(key) = runnable_key {
            let req = self.prepare_for_run(&key);
            return Poll::Ready(req);
        }

        self.waker.write().unwrap().register(cx.waker());

        if let Some(to_wait) = to_wait {
            // Need to wake up after wait time
            // Do this with a wait in a background task
            let waker_clone = self.waker.clone();
            tokio::task::spawn(async move {
                tokio::time::sleep(to_wait).await;
                waker_clone.write().unwrap().wake();
            });
        }

        // Need to check condition **after** `register` to avoid a race
        // condition that would result in lost notifications.
        let (runnable_key, _to_wait) = self.get_runnable();
        if let Some(key) = runnable_key {
            let req = self.prepare_for_run(&key);
            return Poll::Ready(req);
        } else {
            Poll::Pending
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.count(), None)
    }
}

impl PKAppAsync {
    /// Create instance of the app shell.
    /// It has to be initialized for receiving response events:
    /// - by init_with_callback(), to get the events in a callback, OR
    /// - byinit_with_sender(), to get the events in a queue (Sender).
    /// Starts the background processing thread.
    pub fn new() -> Result<Self, String> {
        let (incoming_sender, incoming_receiver) = mpsc::channel::<AppRequest>(100);
        let instance = Self { incoming_sender };

        // Start background processor thread
        let mut instance_clone = instance.clone();
        let _handle = tokio::task::spawn(async move {
            instance_clone
                .process_app_requests_loop(incoming_receiver)
                .await;
        });

        Ok(instance)
    }

    /// Create instance of the app shell, initialized to receive events in a callback.
    /// Starts the background processing thread.
    pub fn new_with_callback<F: Fn(AppEvent) + std::marker::Send + 'static>(
        callback: F,
    ) -> Result<Self, String> {
        let mut instance = Self::new()?;

        let mut event_receiver = instance.init_channels()?;

        // Event receiver thread
        tokio::task::spawn(async move {
            loop {
                match event_receiver.next().await {
                    None => {
                        println!("Error in Subscription: None (310)");
                        break;
                    }
                    Some(event) => {
                        (callback)(event);
                    }
                }
            }
        });

        Ok(instance)
    }

    /// Initialize with a sender (channel), response events will be delivered there
    pub fn init_with_sender(&mut self, outgoing_sender: Sender<AppEvent>) -> Result<(), String> {
        self.send_to_incoming(AppRequest::InitApp(outgoing_sender))
    }

    async fn process_app_requests_loop(&mut self, incoming_receiver: Receiver<AppRequest>) {
        // placeholder for app
        let mut app: Option<PKApp> = None;
        let mut outgoing_sender: Option<Sender<AppEvent>> = None;
        // Pending poll operations
        let pending_polls = PendingPolls::new();
        let mut pending_polls2 = pending_polls.clone();
        let mut select_stream = stream::select(incoming_receiver, pending_polls);
        loop {
            match select_stream.next().await {
                None => {
                    println!("Error in Subscription: None (222)");
                    break;
                }
                Some(req) => {
                    // println!("Got request {:?}", req);
                    if let AppRequest::InitApp(sender) = req {
                        // initialize the app
                        let pk_app = PKApp::new().await.expect("App init error");
                        app = Some(pk_app);
                        outgoing_sender = Some(sender);
                        // also retrieve initial info
                        let _res = self.get_balance_and_wallet_info();
                        let _res = self.get_mints_info();
                    } else {
                        // Took a request
                        if let Some(ref mut app) = &mut app {
                            if let Some(out_sender) = &mut outgoing_sender {
                                Self::process_one_request(
                                    app,
                                    out_sender,
                                    req,
                                    &mut pending_polls2,
                                )
                                .await;
                            } else {
                                println!("Error: Request with missing out_sender, {:?}", req);
                            }
                        } else {
                            println!("Error: Request with missing app, {:?}", req);
                        }
                    }
                }
            }
        }
    }

    #[inline]
    async fn send_out_event(out_sender: &mut Sender<AppEvent>, ev: AppEvent) -> Result<(), String> {
        // println!("Sending out {:?} ...", ev);
        match out_sender.send(ev.clone()).await {
            Ok(_) => Ok(()),
            Err(e) => {
                println!("Error in send_out_event {:?} {}", ev, e);
                Err(e.to_string())
            }
        }
    }

    async fn process_one_request(
        app: &mut PKApp,
        out_sender: &mut Sender<AppEvent>,
        req: AppRequest,
        pending_polls: &mut PendingPolls,
    ) {
        // println!("process_one_request: {:?}", req);
        match req {
            AppRequest::InitApp(_) => {} // handled above
            AppRequest::GetWalletInfo => {
                let res = app.get_wallet_info().await;
                let _res = Self::send_out_event(out_sender, AppEvent::WalletInfo(res)).await;
            }
            AppRequest::GetBalance => {
                let res = app.get_balance().await;
                let _res = Self::send_out_event(out_sender, AppEvent::BalanceChange(res)).await;
            }
            AppRequest::GetBalanceAndWalletInfo => match app.get_balance().await {
                Err(err) => {
                    let _res =
                        Self::send_out_event(out_sender, AppEvent::BalanceAndWalletInfo(Err(err)))
                            .await;
                }
                Ok(balance_info) => match app.get_wallet_info().await {
                    Err(err) => {
                        let _res = Self::send_out_event(
                            out_sender,
                            AppEvent::BalanceAndWalletInfo(Err(err)),
                        )
                        .await;
                    }
                    Ok(wallet_info) => {
                        let _res = Self::send_out_event(
                            out_sender,
                            AppEvent::BalanceAndWalletInfo(Ok((balance_info, wallet_info))),
                        )
                        .await;
                    }
                },
            },
            AppRequest::GetMintsInfo => {
                let res = app.get_mints_info().await;
                let _res = Self::send_out_event(out_sender, AppEvent::MintsInfo(res)).await;
            }
            AppRequest::SelectMintByUrl(url) => {
                let res = app.select_mint(url.as_str()).await;
                let _res = Self::send_out_event(out_sender, AppEvent::MintSelectedByUrl(res)).await;
            }
            AppRequest::SelectMintByIndex(index) => {
                let res = app.select_mint_by_index(index).await;
                let _res =
                    Self::send_out_event(out_sender, AppEvent::MintSelectedByIndex(res)).await;
            }
            AppRequest::AddMint(url) => {
                let res = app.add_mint(url.as_str()).await;
                let _res = Self::send_out_event(out_sender, AppEvent::MintAdded(res)).await;
            }
            AppRequest::MintFromLn(amount) => {
                match app.mint_from_ln_start(amount).await {
                    Err(err) => {
                        let _res =
                            Self::send_out_event(out_sender, AppEvent::MintFromLnRes(Err(err)))
                                .await;
                    }
                    Ok((invoice, intermediary_result)) => {
                        let _res =
                            Self::send_out_event(out_sender, AppEvent::MintFromLnInvoice(invoice))
                                .await;
                        pending_polls.add2(
                            intermediary_result,
                            2000,
                            30, // TODO increase
                        );
                    }
                };
            }
            AppRequest::MeltToLn(invoice) => {
                let res = app.melt_to_ln(&invoice).await;
                let _res = Self::send_out_event(out_sender, AppEvent::MeltToLnRes(res)).await;
            }
            AppRequest::ReceiveEC(token) => {
                let res = app.receive_ecash(&token).await;
                let _res = Self::send_out_event(out_sender, AppEvent::ReceivedEC(res)).await;
            }
            AppRequest::SendEC(amount) => {
                let res = app.send_ecash(amount).await;
                let _res = Self::send_out_event(out_sender, AppEvent::SendECRes(res)).await;
            }
            AppRequest::Poll(intermediary_result) => {
                let res = app.mint_from_ln_check(intermediary_result).await;
                if let Ok(res) = res {
                    let id = res.id();
                    if let Some(result) = res.paid_result {
                        // we have a final result; remove from map and notify
                        pending_polls.remove(&id);
                        let _res =
                            Self::send_out_event(out_sender, AppEvent::MintFromLnRes(result)).await;
                        // also update balance
                        let res = app.get_balance().await;
                        let _res =
                            Self::send_out_event(out_sender, AppEvent::BalanceChange(res)).await;
                    }
                }
            }
        }
    }

    fn init_channels(&mut self) -> Result<Receiver<AppEvent>, String> {
        // Create channel for getting events from outside
        let (outgoing_sender, outgoing_receiver) = mpsc::channel::<AppEvent>(100);
        self.init_with_sender(outgoing_sender)?;
        Ok(outgoing_receiver)
    }

    #[inline]
    fn send_to_incoming(&mut self, req: AppRequest) -> Result<(), String> {
        self.incoming_sender
            .start_send(req)
            .map_err(|e| e.to_string())
    }

    pub fn get_wallet_info(&mut self) -> Result<(), String> {
        self.send_to_incoming(AppRequest::GetWalletInfo)
    }
    pub fn get_balance(&mut self) -> Result<(), String> {
        self.send_to_incoming(AppRequest::GetBalance)
    }
    pub fn get_balance_and_wallet_info(&mut self) -> Result<(), String> {
        self.send_to_incoming(AppRequest::GetBalanceAndWalletInfo)
    }
    pub fn get_mints_info(&mut self) -> Result<(), String> {
        self.send_to_incoming(AppRequest::GetMintsInfo)
    }
    pub fn select_mint(&mut self, mint_url_str: String) -> Result<(), String> {
        self.send_to_incoming(AppRequest::SelectMintByUrl(mint_url_str))
    }
    pub fn select_mint_by_index(&mut self, index: usize) -> Result<(), String> {
        self.send_to_incoming(AppRequest::SelectMintByIndex(index))
    }
    pub fn add_mint(&mut self, mint_url_str: String) -> Result<(), String> {
        self.send_to_incoming(AppRequest::AddMint(mint_url_str))
    }
    pub fn mint_from_ln(&mut self, amount_sats: u64) -> Result<(), String> {
        self.send_to_incoming(AppRequest::MintFromLn(amount_sats))
    }
    pub fn receive_ec(&mut self, ecash_token: String) -> Result<(), String> {
        self.send_to_incoming(AppRequest::ReceiveEC(ecash_token))
    }
    pub fn melt_to_ln(&mut self, invoice_to_pay: String) -> Result<(), String> {
        self.send_to_incoming(AppRequest::MeltToLn(invoice_to_pay))
    }
    pub fn send_ec(&mut self, amount_sats: u64) -> Result<(), String> {
        self.send_to_incoming(AppRequest::SendEC(amount_sats))
    }
    pub fn get_recommended_mint_list() -> Vec<(String, String)> {
        PKApp::get_recommended_mint_list()
    }
}
