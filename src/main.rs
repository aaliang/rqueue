extern crate mio;
extern crate rqueue;

use mio::tcp::{TcpStream, TcpListener};
use mio::{Token, EventSet, EventLoop, PollOpt, Handler};
use std::collections::{HashMap};
use rqueue::buffered_reader::{RawMessage, get_message};
use rqueue::threadpool::{Pool, Pooled};
use std::sync::Arc;
use rqueue::concurrent_hash_map::ConcurrentHashMap;

const SERVER: mio::Token = mio::Token(0);

struct Client {
    socket: TcpStream
}

impl Client {
    fn new(socket: TcpStream) -> Client {
        Client {
            socket: socket
        }
    }
    fn onread(&mut self, pool: &mut Pool<RawMessage, ()>) {
        loop {
            match get_message(&mut self.socket) {
                some @ Some(_) => {
                    pool.send_rr(some.unwrap());
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
    worker_pool: Pool<RawMessage, ()>
}

impl Handler for RQueueServer {
    type Timeout = ();
    type Message = ();

    fn ready(&mut self, event_loop: &mut EventLoop<RQueueServer>, token: Token, events: EventSet) {
        match token {
            SERVER => {
                assert!(events.is_readable());
                println!("server is up");
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
                self.clients.insert(ntoken, Client::new(client_socket));

                event_loop.register(&self.clients[&ntoken].socket, ntoken, EventSet::readable(),
                                    PollOpt::edge()).unwrap();
            }
            token => {
                if events.is_hup() {
                    println!("removing token {:?}", token);
                    self.clients.remove(&token);
                } else {
                    let mut client = self.clients.get_mut(&token).unwrap();
                    client.onread(&mut self.worker_pool);
                    let _ = event_loop.reregister(&client.socket, token, EventSet::readable(), PollOpt::edge());
                }
            }
        }
    }
}

//we could probably couple a pointer to the RWorker, but things will have to change
//RWorker is poorly named right now. it's static. could convert to instance
struct RWorker {
    global_context: Arc<ConcurrentHashMap<u8, u8>>
}

impl Pooled<RawMessage, ()> for RWorker {
    /*fn context(&self) -> Arc<ConcurrentHashMap<u8, u8>> {
        self.global_context().clone()
    }*/
    fn func(work: RawMessage) {
        println!("{:?}", &work.payload[..work.length]);
    }
}


fn main() {
    let address = "0.0.0.0:6567".parse().unwrap();
    let server = TcpListener::bind(&address).unwrap();

    let mut event_loop = EventLoop::new().unwrap();
    event_loop.register(&server, SERVER, EventSet::all(), PollOpt::edge()).unwrap();

    println!("running server");
    let hm: ConcurrentHashMap<u8, std::net::TcpStream> = ConcurrentHashMap::new();
    let _ = event_loop.run(&mut RQueueServer { server: server,
                                               clients: HashMap::new(),
                                               token_counter: 0,
                                               worker_pool: Pool::new(4, hm, |work: RawMessage, _| {
                                                   println!("{:?}", &work.payload[..work.length]);
                                               })
    });
}
