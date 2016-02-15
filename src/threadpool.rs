use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread;
use std::sync::Arc;
use std::ptr::Unique;

pub struct Pool <T, R> {
    workers: Vec<Sender<T>>,
    wait_rx: Receiver<R>,
    curr_index: usize
}

impl <T, R> Pool <T, R> where T: Send + 'static, R: Send + 'static {
    //sends a value to the pool round robin
    pub fn send_rr(&mut self, task: T) {
        let worker = &self.workers[self.curr_index];
        self.curr_index = (self.curr_index + 1) % self.workers.len();
        worker.send(task).unwrap();
    }

    pub fn new <F, A> (num_threads: usize, hm: A, f: F) -> Pool <T, R>
    where F: Fn (T, &Arc<A>) -> R + Sync + Send + 'static, A: Send + Sync + 'static {
        let (done, wait) = channel();
        let func = Arc::new(f);
        let a = Arc::new(hm);
        let workers = (0..num_threads).map(|_| {
            let (snd, work) = channel();
            let _done = done.clone();
            let f = func.clone();
            let aaa = a.clone();
            let _ = thread::spawn(move || {
                let aa = aaa.clone();
                loop {
                    let _ = match work.recv() {
                        Ok(task) => {
                            let res = f(task, &aaa);
                            let _ = _done.send(res);
                        }
                        _ => ()
                    };
                }
            });
            snd
        }).collect();
        Pool {
            workers: workers,
            wait_rx: wait,
            curr_index: 0
        }
    }
}

pub trait Pooled <T, R> where T: Send + 'static, R: Send + 'static {
    fn func(task: T/*, context: &G*/) -> R;
    //fn context() -> G;
    fn new (num_threads: usize) -> Pool <T, R> {
        let (done, wait) = channel();
        let workers = (0..num_threads).map(|_| {
            let (tx_worker, rx_worker) = channel();
            let _done = done.clone();
            let _ = thread::spawn(move || {
               // let worker_context = Self::context();
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
            wait_rx: wait,
            curr_index: 0
        }
    }
}
