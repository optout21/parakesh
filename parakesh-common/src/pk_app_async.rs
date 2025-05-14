use crate::pk_app::{BalanceInfo, MintFromLnIntermediaryResult, MintInfo, PKApp, WalletInfo};
use futures::channel::mpsc::{self, Receiver, Sender};
use futures::task::AtomicWaker;
use futures::{stream, SinkExt, Stream, StreamExt};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};

/// Events delivered to the callback.
#[derive(Clone, Debug)]
pub enum AppEvent {
    // GetBalance,
    // CreateResult(Result<(), String>),
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
    MintFromLnCheck(MintFromLnIntermediaryResult),
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
    p: Arc<RwLock<Vec<PendingPoll>>>,
    waker: Arc<RwLock<AtomicWaker>>,
}

/// App (PKApp) done in an async way with callbacks, not with async/await,
/// for use in environments without async/await (e.g. iced)
#[derive(Clone)]
pub struct PKAppAsync {
    // sender: QueueSender<AppRequest>,
    incoming_sender: Sender<AppRequest>,
    // incoming_receiver: Receiver<AppRequest>,
    // app: Option<PKApp>,
    // /// Pending poll operations
    // pending_polls: Arc<PendingPolls>,
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
            p: Arc::new(RwLock::new(Vec::new())),
            waker: Arc::new(RwLock::new(AtomicWaker::new())),
        }
    }

    pub fn count(&self) -> usize {
        self.p.read().unwrap().len()
    }

    pub fn add(&mut self, poll: PendingPoll) {
        self.p.write().unwrap().push(poll);
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
    fn get_runnable(&mut self) -> (Option<AppRequest>, Option<Duration>) {
        let now = SystemTime::now();
        let mut earliest: Option<(SystemTime, usize)> = None;
        for (i, p) in self.p.write().unwrap().iter().enumerate() {
            let is_earlier = if let Some((earliest, _i)) = earliest {
                p.next_time < earliest
            } else {
                true
            };
            if is_earlier {
                earliest = Some((p.next_time, i));
            }
        }
        if let Some((earliest, index)) = earliest {
            // check if this earliest is already runnable (in the past)
            let more_runs = {
                let pr = &self.p.read().unwrap()[index];
                if pr.next_time <= now {
                    // check if last run
                    if pr.next_time > pr.stop_time {
                        // no more runs
                        false
                    } else {
                        // more runs
                        true
                    }
                } else {
                    // no runnable
                    return (None, Some(earliest.duration_since(now).unwrap_or_default()));
                }
            };
            if more_runs {
                // more runs, update next time
                self.p.write().unwrap()[index].advance();
                (
                    Some(AppRequest::Poll(
                        self.p.read().unwrap()[index].result.clone(),
                    )),
                    None,
                )
            } else {
                // no more runs
                let pp = self.remove(index);
                (Some(AppRequest::Poll(pp.result)), None)
            }
        } else {
            // no operation
            (None, None)
        }
    }

    fn remove(&mut self, index: usize) -> PendingPoll {
        self.p.write().unwrap().remove(index)
    }
}

impl Stream for PendingPolls {
    type Item = AppRequest;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<AppRequest>> {
        let (runnable, to_wait) = self.get_runnable();
        if let Some(runnable) = runnable {
            return Poll::Ready(Some(runnable));
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
        let (runnable, _to_wait) = self.get_runnable();
        if let Some(runnable) = runnable {
            return Poll::Ready(Some(runnable));
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
        let instance = Self {
            incoming_sender,
            // incoming_receiver,
            // app: None,
            // pending_polls: Arc::new(PendingPolls::new()),
        };
        // let mut queue = Queue::new();
        // let sender = queue.get_sender_clone();
        // let sender2 = queue.get_sender_clone();
        // let app_async = PKAppAsync { sender };

        // Start background processor thread
        let mut instance_clone = instance.clone();
        let _handle = tokio::task::spawn(async move {
            instance_clone
                .process_app_requests_loop(incoming_receiver)
                .await;
            // match PKApp::new().await {
            //     Err(err) => {
            //         let err_msg = format!("Could not create app, {}", err.to_string());
            //         eprint!("{}", err_msg);
            //         (callback)(&AppEvent::CreateResult(Err(err_msg)));
            //     }
            //     Ok(ref mut app) => {
            //         Self::process_app_requests_loop(app, &mut queue, callback, &sender2).await;
            //     }
            // }
        });

        Ok(instance)
    }

    /// Initialize with a callback, response events will be delivered there
    pub fn init_with_callback<F: Fn(AppEvent) + std::marker::Send + 'static>(
        &mut self,
        callback: F,
    ) -> Result<(), String> {
        let mut event_receiver = self.init_channels()?;

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

        Ok(())
    }

    /// Initialize with a sender (channel), response events will be delivered there
    pub fn init_with_sender(&mut self, outgoing_sender: Sender<AppEvent>) -> Result<(), String> {
        self.send_to_incoming(AppRequest::InitApp(outgoing_sender))
    }

    async fn process_app_requests_loop(
        &mut self,
        incoming_receiver: Receiver<AppRequest>,
        // pending_requests: Arc<PendingPolls>,
        // app: &mut PKApp,
        // queue: &mut Queue<AppRequest>,
        // callback: AppCallback,
        // sender: &QueueSender<AppRequest>,
    ) {
        // placeholder for app
        let mut app: Option<PKApp> = None;
        let mut outgoing_sender: Option<Sender<AppEvent>> = None;
        // Pending poll operations
        let mut pending_polls = PendingPolls::new();
        let mut select_stream = stream::select(incoming_receiver, pending_polls.clone());
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
                                Self::process_one_request(app, out_sender, req, &mut pending_polls)
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
                        /*
                        // TODO non-blocking
                        let res = app.mint_from_ln_wait(intermediary_result).await;
                        let _res =
                            Self::send_out_event(out_sender, AppEvent::MintFromLnRes(res)).await;
                        // (callback)(&AppEvent::MintFromLnInvoice(invoice.to_owned()));
                        if let Some(res) = intermediary_result.paid_result {
                            // (callback)(&AppEvent::MintFromLnRes(res));
                        } else {
                            let next_check_time = intermediary_result.next_check_time;
                            let _res = sender.send(
                                AppRequest::MintFromLnCheck(intermediary_result),
                                // Some(next_check_time),
                            );
                        }
                        */
                    }
                };
            }
            AppRequest::MintFromLnCheck(intermediary_result) => {
                if let Some(res) = intermediary_result.paid_result {
                    let _res = Self::send_out_event(out_sender, AppEvent::MintFromLnRes(res)).await;
                } else {
                    /*/
                    let next_check_time = intermediary_result.next_check_time;
                    let _res = sender.send(
                        AppRequest::MintFromLnCheck(intermediary_result),
                        // Some(next_check_time),
                    );
                    */
                }
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
                // TODO check
                let res = app.mint_from_ln_check(intermediary_result).await;
                if let Ok(res) = res {
                    if let Some(result) = res.paid_result {
                        let _res =
                            Self::send_out_event(out_sender, AppEvent::MintFromLnRes(result)).await;
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
}
