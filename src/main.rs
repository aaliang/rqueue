extern crate mio;
extern crate rqueue;

use mio::tcp::{TcpStream, TcpListener};
use mio::{Token, EventSet, EventLoop, PollOpt, Handler};
use std::collections::{HashMap};
use std::net::SocketAddr;
use rqueue::protocol::{RawMessage, get_message};
use rqueue::threadpool::{StatePool, Pooled, QueuePoolWorker, PoolWorker};
use rqueue::rpc::DEREGISTER_ONCE;
use std::mem;

const SERVER: mio::Token = mio::Token(0);

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
    fn disconnect(&self, pool: &mut StatePool<RawMessage, ()>) {
        for sender in pool.workers.iter() {
            let message = RawMessage {
                m_type: DEREGISTER_ONCE,
                length: 0,
                socket_addr: self.socket_addr.clone(),
                payload: unsafe {mem::uninitialized()},
                raw_fd: -1
            };
            let _ = sender.send(message);
        }
    }
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

struct RQueueServer {
    server: TcpListener,
    clients: HashMap<Token, Client>,
    token_counter: usize,
    worker_pool: StatePool<RawMessage, ()>
}

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
                self.token_counter += 1;
                let ntoken = Token(self.token_counter);
                println!("new token {:?}", ntoken);
                self.clients.insert(ntoken, Client::new(client_socket));

                event_loop.register(&self.clients[&ntoken].socket, ntoken, EventSet::readable(),
                                    PollOpt::edge()).unwrap();
            }
            token => {
                if events.is_hup() {
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



fn main() {
    let address = "0.0.0.0:6567".parse().unwrap();
    let server = TcpListener::bind(&address).unwrap();

    let mut event_loop = EventLoop::new().unwrap();
    event_loop.register(&server, SERVER, EventSet::all(), PollOpt::edge()).unwrap();

    println!("running server");

    let _ = event_loop.run(&mut RQueueServer { server: server,
                                               clients: HashMap::new(),
                                               token_counter: 0,
                                               worker_pool: StatePool::new(8, |contacts| QueuePoolWorker::new(contacts))
    });
}

/*
//for debug
fn to_std_tcpstream(stream: &TcpStream) -> std::TcpStream {
    let builder = unsafe {
        TcpBuilder::from_raw_fd(stream.as_raw_fd())
    };
    builder.to_tcp_stream().unwrap()
}*/
