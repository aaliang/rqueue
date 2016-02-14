/*
extern crate rqueue;
extern crate mio;

use rqueue::rqueue::RQueue;
use rqueue::threadpool::Pooled;

use mio::tcp::*;

const SERVER: mio::Token = mio::Token(0);


fn main () {
    let address = "0.0.0.0:6567".parse().unwrap();
    let server = TcpListener::bind(&address).unwrap();

    let mut event_loop = mio::EventLoop::new().unwrap();
    event_loop.register(&server, SERVER);
}
 */
extern crate mio;

use mio::tcp::{TcpStream, TcpListener};
use mio::{Token, EventSet, EventLoop, PollOpt, Handler};
use std::collections::HashMap;

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
}

struct RQueueServer {
    server: TcpListener,
    clients: HashMap<mio::Token, Client>,
    token_counter: usize
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
                let token = Token(self.token_counter);
                self.clients.insert(token, Client::new(client_socket));

                event_loop.register(&self.clients[&token].socket, token, EventSet::readable(),
                                    PollOpt::edge()).unwrap();
            }
            token => {
                if events.is_hup() {
                    println!("removing token {:?}", token);
                    self.clients.remove(&token);
                } else {
                    let mut client = self.clients.get_mut(&token).unwrap();
                    //client.read();
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

    println!("running pingpong server");
    let _ = event_loop.run(&mut RQueueServer { server: server, clients: HashMap::new(), token_counter: 0});
}
