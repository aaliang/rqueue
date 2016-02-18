extern crate mio;
extern crate rqueue;
extern crate getopts;

use std::{mem, env};
use std::collections::{HashMap};
use std::net::SocketAddr;
use mio::tcp::{TcpStream, TcpListener};
use mio::{Token, EventSet, EventLoop, PollOpt, Handler};
use getopts::Options;
use rqueue::protocol::{RawMessage, get_message};
use rqueue::threadpool::{StatePool, QueuePoolWorker, PoolWorker};
use rqueue::protocol::DEREGISTER_ONCE;

const SERVER: mio::Token = mio::Token(0);

struct RQueueServer {
    server: TcpListener,
    clients: HashMap<Token, Client>, // just a regular slow hm for now
    token_counter: usize,
    worker_pool: StatePool<RawMessage, ()>
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
                println!("new token {:?}", ntoken);
                self.clients.insert(ntoken, Client::new(client_socket));

                event_loop.register(&self.clients[&ntoken].socket, ntoken, EventSet::readable(),
                                    PollOpt::edge()).unwrap();
            }
            token => {
                if events.is_hup() { // on client hangup
                    println!("removing token {:?}", token);
                    let client = self.clients.remove(&token).unwrap();
                    client.disconnect(&mut self.worker_pool);
                } else {
                    let mut client = self.clients.get_mut(&token).unwrap();
                    client.onread(&mut self.worker_pool);
                    let _ = event_loop.reregister(&client.socket, token, EventSet::readable(), PollOpt::edge());
                }
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

    let mut event_loop = EventLoop::new().unwrap();
    event_loop.register(&server, SERVER, EventSet::all(), PollOpt::edge()).unwrap();

    println!("running server on port {}", port);
    println!("   with {} workers", aux_threads);

    //start the event loop
    let _ = event_loop.run(&mut RQueueServer { server: server,
                                               clients: HashMap::new(),
                                               token_counter: 0,
                                               // decoupled worker pool with configurable # of
                                               // threads
                                               worker_pool: StatePool::new(aux_threads, |contacts| QueuePoolWorker::new(contacts))
    });
}

//for debug
/*fn to_std_tcpstream(stream: &TcpStream) -> std::net::TcpStream {
    let builder = unsafe {
        TcpBuilder::from_raw_fd(stream.as_raw_fd())
    };
    builder.to_tcp_stream().unwrap()
}*/
