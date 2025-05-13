use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tokio::sync::mpsc::{Receiver, Sender};

struct MessageWithMaturity<M> {
    msg: M,
    maturity_time: Option<SystemTime>,
}

#[derive(Clone)]
pub(crate) struct QueueSender<M>
where
    M: Clone,
{
    sender: Sender<u8>,
    queue: Arc<RwLock<Vec<MessageWithMaturity<M>>>>,
}

struct QueueRecv<M> {
    recv: Receiver<u8>,
    queue: Arc<RwLock<Vec<MessageWithMaturity<M>>>>,
}

/// A Queue, to send events to another thread; events optionaly have a delay (maturity time)
/// The mpsc::Sender and Receivers are just used as a semaphore.
pub(crate) struct Queue<M>
where
    M: Clone,
{
    sender: QueueSender<M>,
    recver: QueueRecv<M>,
}

impl<M> QueueSender<M>
where
    M: Clone,
{
    fn new(sender: Sender<u8>, queue: Arc<RwLock<Vec<MessageWithMaturity<M>>>>) -> Self {
        Self { sender, queue }
    }

    pub fn send(&self, msg: M, maturity_time: Option<SystemTime>) -> Result<(), String> {
        self.queue
            .write()
            .unwrap()
            .push(MessageWithMaturity { maturity_time, msg });
        let _res = self.sender.blocking_send(1);
        //.map_err(|e| e.to_string())
        println!("Sent sg {}", self.queue.read().unwrap().len());
        Ok(())
    }
}

impl<M> QueueRecv<M> {
    fn new(recv: Receiver<u8>, queue: Arc<RwLock<Vec<MessageWithMaturity<M>>>>) -> Self {
        Self { recv, queue }
    }

    async fn recv(&mut self) -> Option<M> {
        loop {
            println!("recv {} ...", self.queue.read().unwrap().len());
            let now = SystemTime::now();
            // find the closes valid time
            let earliest = self
                .queue
                .read()
                .unwrap()
                .iter()
                .map(|tm| tm.maturity_time.unwrap_or(now))
                .min();
            // println!(
            //     "                {} in queue, earliest in {}",
            //     self.queue.read().unwrap().len(),
            //     earliest
            //         .unwrap_or(now)
            //         .duration_since(now)
            //         .unwrap_or_default()
            //         .as_millis(),
            // );
            if let Some(earliest) = earliest {
                // we have an earliest, check if it is past
                if earliest <= now {
                    // we have at least a past one, return it
                    let mut queue_lock = self.queue.write().unwrap();
                    for i in 0..queue_lock.len() {
                        let valid = if let Some(maturity_time) = queue_lock[i].maturity_time {
                            maturity_time < now
                        } else {
                            true
                        };
                        if valid {
                            let m = queue_lock.remove(i);
                            // println!("                earliest in past");
                            return Some(m.msg);
                        }
                    }
                    // should not get here, continue
                } else {
                    // we have an earliest, from the future
                    let to_wait = earliest.duration_since(now).unwrap_or_default();
                    // println!(
                    //     "                earliest in {}, waiting",
                    //     to_wait.as_millis()
                    // );
                    // wait until earliest happens OR new event received
                    match tokio::time::timeout(to_wait, self.recv.recv()).await {
                        Err(_) => {
                            // did not receive in time
                            if let Ok(to_wait2) = earliest.duration_since(SystemTime::now()) {
                                // println!("                just wait {}", to_wait2.as_millis());
                                // just wait to avoid busy loop
                                tokio::time::sleep(to_wait2).await;
                                // continue
                            } else {
                                // continue
                            }
                        }
                        Ok(_res) => {
                            // received a new change, continue
                        }
                    }
                }
            } else {
                // no earliest, maybe no data
                let _res = self.recv.recv().await;
                // continue
            }
        }
    }
}

impl<M> Queue<M>
where
    M: Clone,
{
    pub fn new() -> Self {
        let (sender0, recver0) = tokio::sync::mpsc::channel::<u8>(100);
        let queue = Arc::new(RwLock::new(Vec::new()));
        let sender = QueueSender::new(sender0, queue.clone());
        let recver = QueueRecv::new(recver0, queue);
        Self { sender, recver }
    }

    pub async fn recv(&mut self) -> Option<M> {
        self.recver.recv().await
    }

    pub fn get_sender_clone(&self) -> QueueSender<M> {
        self.sender.clone()
    }
}
