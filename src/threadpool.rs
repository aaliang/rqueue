use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread;
use std::sync::Arc;
use buffered_reader::RawMessage;
use slice_map::SliceMap;
use std::collections::{HashSet, HashMap};
use std::net::{SocketAddr, TcpStream};
use rpc::parse;

pub struct Pool <T, R> {
    pub workers: Vec<Sender<T>>,
    pub wait_rx: Receiver<R>,
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

pub trait PoolWorker <T, R> {
    fn new (Vec<Sender<T>>) -> Self;
    fn func(&mut self, T) -> R;
}

pub struct StatePool <T, R> {
    pub workers: Vec<Sender<T>>,
    pub wait_rx: Receiver<R>,
    curr_index: usize
}

pub struct QueuePoolWorker {
    topic_map: SliceMap<HashMap<SocketAddr, TcpStream>>,
    contacts: Vec<Sender<RawMessage>>,
    interest_map: HashMap<SocketAddr, HashSet<Vec<u8>>>
}

impl PoolWorker<RawMessage, ()> for QueuePoolWorker {
    fn new (contacts: Vec<Sender<RawMessage>>) -> QueuePoolWorker {
        QueuePoolWorker {
            topic_map: SliceMap::new(),
            contacts: contacts,
            interest_map: HashMap::new()
        }
    }
    fn func (&mut self, message: RawMessage) {
        parse(message, &self.contacts, &mut self.topic_map, &mut self.interest_map);
    }
}

//lose generics here because unable to return traits in impl generics right now
impl StatePool <RawMessage, ()> {
    //sends a value to the pool round robin
    pub fn send_rr(&mut self, task: RawMessage) {
        let worker = &self.workers[self.curr_index];
        self.curr_index = (self.curr_index + 1) % self.workers.len();
        worker.send(task).unwrap();
    }

    pub fn new <W> (num_threads: usize, new_worker: W) -> StatePool<RawMessage, ()> where W: Fn(Vec<Sender<RawMessage>>) -> QueuePoolWorker {

        let (done, wait) = channel();
        let _workers = (0..num_threads).map(|_| {
            channel()
        }).collect::<Vec<_>>();

        let contacts = _workers.iter().map(|&(ref tx, _)| tx.clone()).collect::<Vec<_>>();

        for (i, (_, _work)) in _workers.into_iter().enumerate() {
            let _done = done.clone();

            //exclude own sender from contact info
            let mut other_contacts = contacts.clone();
            other_contacts.remove(i);
            let mut worker = new_worker(other_contacts);

            let _ = thread::spawn(move || {
                loop {
                    let _ = match _work.recv() {
                        Ok(task) => {
                            let res = worker.func(task);
                            let _ = _done.send(res);
                        }
                        _ => ()
                    };
                }
            });
        }

        StatePool {
            workers: contacts,
            wait_rx: wait,
            curr_index: 0
        }
    }
}

pub trait Pooled <T, R> where T: Send + 'static, R: Send + 'static {
    fn func(task: T) -> R;
    //fn context() -> G;
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
            wait_rx: wait,
            curr_index: 0
        }
    }
}
