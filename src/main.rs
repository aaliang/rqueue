extern crate mio;
extern crate rqueue;
extern crate getopts;

use std::{mem, env};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::thread;
use std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicBool};
use mio::tcp::{TcpStream, TcpListener};
use mio::{Token, EventSet, EventLoop, PollOpt, Handler, Sender};
use getopts::Options;
use rqueue::protocol::{RawMessage, get_message};
use rqueue::threadpool::{StatePool, QueuePoolWorker, PoolWorker};
use rqueue::protocol::DEREGISTER_ONCE;
use rqueue::rpc::{Events, parse};
use rqueue::slice_map::SliceMap;

const SERVER: mio::Token = mio::Token(0);

struct RQueueServer {
    server: TcpListener,
    clients: HashMap<Token, Client>, // just a regular slow hm for now
    token_counter: usize,
    slave_loops: Vec<Sender<Events>>,
    rr_index: usize
}

// implements a vanilla-ish mio event loop
impl Handler for RQueueServer {
    type Timeout = ();
    type Message = ();

    fn ready(&mut self, event_loop: &mut EventLoop<RQueueServer>, token: Token, events: EventSet) {
        match token {
            SERVER => {
                assert!(events.is_readable());
                let client_socket = match self.server.accept() {
                    Ok(Some((socket, _))) => socket,
                    Ok(None) => unreachable!(),
                    Err(e) => {
                        println!("listener.accept() errored: {}", e);
                        return;
                    }
                };
                //for now just increment the token (FAIP the client id)
                self.token_counter += 1;
                let ntoken = Token(self.token_counter);

                let arcBool = Arc::new(AtomicBool::new(false));


                for x in self.slave_loops.iter() {
                    x.send(Events::NewSocket(ntoken, client_socket.try_clone().unwrap(), arcBool.clone()));
                }
                println!("new token {:?}", ntoken);
                self.clients.insert(ntoken, Client::new(client_socket));

                /*for i in self.slave_loops.iter() {
                    i.send(Events::Read())
                }*/

                event_loop.register(&self.clients[&ntoken].socket, ntoken, EventSet::readable(),
                                    PollOpt::edge()).unwrap();
            }
            token => {
                if events.is_hup() { // on client hangup
                    println!("removing token {:?}", token);
                    let client = self.clients.remove(&token).unwrap();
                    //client.disconnect(&mut self.worker_pool);
                } else {
                    let client = self.clients.get(&token).unwrap();

                    self.slave_loops[self.rr_index].send(Events::Read(token));

                    self.rr_index = (self.rr_index + 1) % self.slave_loops.len();

                    let _ = event_loop.reregister(&client.socket, token, EventSet::readable(), PollOpt::edge());
                }
            }
        }
    }
}

struct XWorker {
    id: usize,
    clients: HashMap<Token, (TcpStream, Arc<AtomicBool>)>,
    state_map: SliceMap<HashMap<SocketAddr, std::net::TcpStream>>,
    interest_map: HashMap<SocketAddr, HashSet<Vec<u8>>>,
    friends: Vec<Sender<Events>>
}

impl Handler for XWorker {
    type Timeout = ();
    type Message = Events;

    fn notify(&mut self, event_loop: &mut EventLoop<XWorker>, event: Events) {
        match event {
            Events::Read(token) => {
                let &mut (ref mut client, ref mut arc_bool) = self.clients.get_mut(&token).unwrap();
                loop {
                    let is_occupied = arc_bool.compare_and_swap(false, true, Ordering::SeqCst);
                    if !is_occupied {
                        match get_message(client) {
                            Some(s) => {
                                arc_bool.store(false, Ordering::SeqCst);
                                //println!("{}", self.id);
                                parse(s, &mut self.state_map, &mut self.interest_map, &self.friends)
                            },
                            None => {
                                arc_bool.store(false, Ordering::SeqCst);
                                return
                            }
                        }
                    }
                }
            }
            Events::NewSocket(token, socket, arc_bool) =>  {
                self.clients.insert(token, (socket, arc_bool));
            }
            Events::Broadcast(message) => {
                parse(message, &mut self.state_map, &mut self.interest_map, &self.friends);
            }
        }
    }
}

/// wrapper over client sockets
struct Client {
    socket: TcpStream,
    socket_addr: SocketAddr
}

impl Client {
    fn new(socket: TcpStream) -> Client {
        let addr = socket.peer_addr().unwrap();
        Client {
            socket: socket,
            socket_addr: addr//cache this value
        }
    }
    /// disconnects a client
    fn disconnect(&self, pool: &mut StatePool<RawMessage, ()>) {
        //sends a DEREGISTER_ONCE message to each worker
        for sender in pool.workers.iter() {
            let message = RawMessage {
                m_type: DEREGISTER_ONCE,
                length: 0,
                socket_addr: self.socket_addr.clone(),
                bytes: unsafe {mem::uninitialized()},
                raw_fd: -1
            };
            let _ = sender.send(message);
        }
    }
    /// called on every message after connecting
    fn onread(&mut self, pool: &mut StatePool<RawMessage, ()>) {
        loop {
            match get_message(&mut self.socket) {
                Some(s) => {
                    pool.send_rr(s);
                },
                None => return
            }
        }
    }
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let mut opts = Options::new();

    opts.optopt("p", "port", "tcp server port", "PORT_NUM");
    opts.optopt("t", "threads", "auxiliary worker threads", "NUM_THREADS");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };

    let port = match matches.opt_str("p") {
        Some(e) => e.parse::<usize>().unwrap_or(6567),
        _ => 6567
    };

    let aux_threads = match matches.opt_str("t") {
        Some(e) => e.parse::<usize>().unwrap_or(8),
        _ => 8
    };

    let address = format!("0.0.0.0:{}", port).parse().unwrap();
    let server = TcpListener::bind(&address).unwrap();

    let mut main_event_loop = EventLoop::new().unwrap();
    main_event_loop.register(&server, SERVER, EventSet::all(), PollOpt::edge()).unwrap();

    let num_slaves = aux_threads.clone();

    let slave_loops = (0..num_slaves).map(|_| {
        EventLoop::new().unwrap()
    }).collect::<Vec<_>>();

    let contacts = slave_loops.iter().map(|el|
        el.channel()).collect::<Vec<_>>();

    for (i, mut slave) in slave_loops.into_iter().enumerate() {
        let mut friends = contacts.clone();
        friends.remove(i);
        thread::spawn(move || {
            slave.run(&mut XWorker {
                id: i,
                clients: HashMap::new(),
                state_map: SliceMap::new(),
                interest_map: HashMap::new(),
                friends: friends
            });
        });
    }

    /*let slaves = (0..num_slaves).map(|x| {
        let mut slave_loop = EventLoop::new().unwrap();
        let chan = slave_loop.channel();
        let workers = thread::spawn(move || {
            let _ = slave_loop.run(&mut XWorker {
                clients: HashMap::new(),
                state_map: SliceMap::new(),
                interest_map: HashMap::new()
            });
        });
        chan
    }).collect::<Vec<_>>();*/


    println!("running server on port {}", port);
    println!("   with {} workers", aux_threads);

    thread::spawn(move || {
    //start the event loop
        let _ = main_event_loop.run(&mut RQueueServer { server: server,
                                                   clients: HashMap::new(),
                                                   token_counter: 0,
                                                   slave_loops: contacts.clone(),
                                                   rr_index: 0,
                                                   // decoupled worker pool with configurable # of
                                                   // threads
        });
    }).join();
}

//for debug
/*fn to_std_tcpstream(stream: &TcpStream) -> std::net::TcpStream {
    let builder = unsafe {
        TcpBuilder::from_raw_fd(stream.as_raw_fd())
    };
    builder.to_tcp_stream().unwrap()
}*/
