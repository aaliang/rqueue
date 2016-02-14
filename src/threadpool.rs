use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread;
use std::sync::Arc;

pub struct Pool <T, R> {
    workers: Vec<Sender<T>>,
    wait_rx: Receiver<R>
}

pub trait Pooled <T, R> where T: Send + 'static, R: Send + 'static {
    fn func(task: T) -> R;
    fn new (num_threads: usize) -> Pool <T, R> {
        let (done, wait) = channel();
        let workers = (0..num_threads).map(|_| {
            let (tx_worker, rx_worker) = channel();
            let _done = done.clone();
            let _ = thread::spawn(move || {
                loop {
                    let work = rx_worker.recv().unwrap();
                    let res = Self::func(work);
                    let _ = _done.send(res);
                }
            });
            tx_worker
        }).collect();
        Pool {
            workers: workers,
            wait_rx: wait
        }
    }
}
