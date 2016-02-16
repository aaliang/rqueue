use buffered_reader::RawMessage;
use std::sync::mpsc::{Sender};
use slice_map::SliceMap;
use std::net::{TcpStream, SocketAddr};
use net2::TcpBuilder;
use std::os::unix::io::{FromRawFd, RawFd};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::{ptr};

/* for reference
pub struct RawMessage {
    pub m_type: u8,
    pub length: usize,
    pub payload: [u8; MAX_STATIC_SZ],
    pub raw_fd: RawFd
}
 */

pub const PUBLISH         : u8 = 0;
pub const SUBSCRIBE       : u8 = 1;
pub const REMOVE          : u8 = 2;
pub const SUBSCRIBE_ONCE  : u8 = 3;
pub const REMOVE_ONCE     : u8 = 4;
pub const DEREGISTER      : u8 = 5;
pub const DEREGISTER_ONCE : u8 = 6;

//TODO: keying by SocketAddr alone seems like it could potentially be dangerous. perhaps should add a uid. if the socket is reused by a different subscriber and
//ends up mapping to a previous fd - it might be able to receive traffic that it was not interested in. e.g. a client disconnects then reconnects and happens
//to get accepted to the same socket, and receives messages before the server can purge interests. Think about this more.
// the way we're currently handling disconnects right now, sending deregisters from the EL thread makes this a non-issue though
pub fn parse(work: RawMessage, contacts: &[Sender<RawMessage>], state_map: &mut SliceMap<HashMap<SocketAddr, TcpStream>>, interest_map: &mut HashMap<SocketAddr, HashSet<Vec<u8>>>) {
    match work.m_type {
        PUBLISH => {
            //topic len is a one byte value (<255)
            let topic_len = work.payload[0] as usize;
            let topic = &work.payload[1..topic_len+1];
            let message = &work.payload[topic_len+1..work.length];

            println!("pub topic: {:?}", topic);
            println!("message: {:?}", message);

            state_map.apply(topic, |ref mut h_entry| {
                for (addr, tcp_stream) in h_entry.iter_mut() {
                    //TcpStreams in Rust are wrappers around raw file descriptors
                    //thus, it is possible for us to get here with stale TcpStreams that have yet to be removed
                    if addr == &tcp_stream.peer_addr().unwrap() {
                        let _ = tcp_stream.write(message);
                    }
                }
            });
        }

        SUBSCRIBE |  SUBSCRIBE_ONCE => {
            let topic_len = work.payload[0] as usize;
            let topic = &work.payload[1..topic_len+1];
            let c = interest_map.entry(work.socket_addr).or_insert(HashSet::new());
            c.insert(topic.to_owned());

            state_map.modify_or_else(topic, |ref mut map| {
                let tcp_stream = to_std_tcpstream_from_raw(work.raw_fd.clone());
                if tcp_stream.peer_addr().unwrap() == work.socket_addr {
                    map.insert(tcp_stream.peer_addr().unwrap(), tcp_stream);
                }
            }, || {
                let mut map = HashMap::new();
                let tcp_stream = to_std_tcpstream_from_raw(work.raw_fd.clone());
                if tcp_stream.peer_addr().unwrap() == work.socket_addr {
                    map.insert(tcp_stream.peer_addr().unwrap(), tcp_stream);
                }
                map
            });

            if work.m_type == SUBSCRIBE {
                println!("sub topic: {:?}", &topic[..]);
                for sender in contacts.iter() {
                    let mut u = unsafe{ ptr::read(&work) };
                    u.m_type = SUBSCRIBE_ONCE;
                    let _ = sender.send(u);
                }
            }
        }

        REMOVE | REMOVE_ONCE => {
            let topic_len = work.payload[0] as usize;
            let topic = &work.payload[1..topic_len+1];
            let remove_topic = state_map.modify(topic, |ref mut map| {
                map.remove(&work.socket_addr);
                match map.is_empty() {
                    true => Some(true),
                    _ => None
                }
            });
            if remove_topic == Some(true) {
                state_map.delete(topic);
            }
            if work.m_type == REMOVE {
                println!("removing, {:?}", &topic[..]);
                for sender in contacts.iter() {
                    let mut u = unsafe{ ptr::read(&work) };
                    u.m_type = REMOVE_ONCE;
                    let _ = sender.send(u);
                }
            }
        }

        DEREGISTER | DEREGISTER_ONCE => {
            if let Some(set) = interest_map.remove(&work.socket_addr) {
                for topic in set.iter() {
                    let remove_topic = state_map.modify(topic, |ref mut map| {
                        map.remove(&work.socket_addr);
                        match map.is_empty() {
                            true => Some(true),
                            _ => None
                        }
                    });
                    if remove_topic == Some(true) {
                        println!("removing topic {:?}", &topic[..]);
                        state_map.delete(topic);
                    }
                }
            }

            if work.m_type == DEREGISTER {
                for sender in contacts.iter() {
                    let mut u = unsafe {ptr::read(&work) };
                    u.m_type = DEREGISTER_ONCE;
                    let _ = sender.send(u);
                }
            }
        }
        _ => {
            println!("none");
            //Actions::Nil
        }
    }
}

fn to_std_tcpstream_from_raw(fd: RawFd) -> TcpStream {
    let builder = unsafe { TcpBuilder::from_raw_fd(fd.clone()) };
    builder.to_tcp_stream().unwrap()
}
