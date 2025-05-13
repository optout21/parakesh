use crossbeam::channel::{Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

#[derive(Clone)]
pub(crate) struct QueueSender<M>
where
    M: Clone,
{
    sender: Sender<M>,
}

struct QueueRecv<M> {
    recv: Receiver<M>,
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
    fn new(sender: Sender<M>) -> Self {
        Self { sender }
    }

    pub fn send(&self, msg: M) -> Result<(), String> {
        let _res = self.sender.send(msg);
        //.map_err(|e| e.to_string())
        println!("Sent sg");
        Ok(())
    }
}

impl<M> QueueRecv<M> {
    fn new(recv: Receiver<M>) -> Self {
        Self { recv }
    }

    async fn recv(&mut self) -> Option<M> {
        loop {
            println!("recv ...");
            match self.recv.recv() {
                Err(_) => {}
                Ok(m) => return Some(m),
            }
        }
    }
}

impl<M> Queue<M>
where
    M: Clone,
{
    pub fn new() -> Self {
        let (sender0, recver0) = crossbeam::channel::bounded::<M>(100);
        let sender = QueueSender::new(sender0);
        let recver = QueueRecv::new(recver0);
        Self { sender, recver }
    }

    pub async fn recv(&mut self) -> Option<M> {
        self.recver.recv().await
    }

    pub fn get_sender_clone(&self) -> QueueSender<M> {
        self.sender.clone()
    }
}
