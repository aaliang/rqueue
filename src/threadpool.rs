use std::sync::mpsc::{Sender, Receiver, channel};
use std::thread;
use protocol::RawMessage;
use slice_map::SliceMap;
use std::collections::{HashSet, HashMap};
use std::net::{SocketAddr, TcpStream};
use rpc::parse;

/// an interface for a stateful worker capable of acting in a threadpool
pub trait PoolWorker <T, R> {
    fn new (Vec<Sender<T>>) -> Self;

    /// does some arbitrary unit of work
    fn func(&mut self, T) -> R;
}

/// backs a concrete implementation of a PoolWorker
pub struct QueuePoolWorker {

    /// locally cached mapping of topics (a bunch of bytes) to a collection of subcriber info (tcp
    /// sockets) 
    topic_map: SliceMap<HashMap<SocketAddr, TcpStream>>,

    /// locally cached mapping of socket addresses to a collection of topics 
    interest_map: HashMap<SocketAddr, HashSet<Vec<u8>>>,

    /// channels to other threads to broadcast messages on (relatively low priority i.e. does not
    /// interrupt)
    contacts: Vec<Sender<RawMessage>>,
}

impl PoolWorker<RawMessage, ()> for QueuePoolWorker {
    fn new (contacts: Vec<Sender<RawMessage>>) -> QueuePoolWorker {
        QueuePoolWorker {
            topic_map: SliceMap::new(),
            contacts: contacts,
            interest_map: HashMap::new()
        }
    }

    /// does something with a message
    fn func (&mut self, message: RawMessage) {
        //parse(message, &self.contacts, &mut self.topic_map, &mut self.interest_map);
    }
}

pub struct StatePool <T, R> {
    /// handles of channels to workers, you can send work to them from here
    pub workers: Vec<Sender<T>>,

    /// used to receive messages from worker threads
    pub wait_rx: Receiver<R>,

    /// the last worker that we sent work to, used for certain strats e.g. round robin
    curr_index: usize
}

//fyi lose generics here because unable to return traits in impl generics right now

impl StatePool <RawMessage, ()> {
    /// sends a value to the pool round robin
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
