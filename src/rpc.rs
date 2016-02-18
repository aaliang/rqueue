use std::sync::mpsc::{Sender};
use std::net::{TcpStream, SocketAddr};
use std::os::unix::io::{FromRawFd, RawFd};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::ptr;
use net2::TcpBuilder;
use slice_map::SliceMap;
use protocol::{RawMessage, PREAMBLE_SZ};
use protocol::{NOTIFICATION, SUBSCRIBE, SUBSCRIBE_ONCE, REMOVE, REMOVE_ONCE, DEREGISTER, DEREGISTER_ONCE};

/* for reference
pub struct RawMessage {
    pub m_type: u8,
    pub length: usize,
    pub bytes: [u8; MAX_STATIC_SZ],
    pub raw_fd: RawFd
}
 */

//TODO: keying by SocketAddr alone seems like it could potentially be dangerous. perhaps should add a uid. if the socket is reused by a different subscriber and
//ends up mapping to a previous fd - it might be able to receive traffic that it was not interested in. e.g. a client disconnects then reconnects and happens
//to get accepted to the same socket, and receives messages before the server can purge interests. Think about this more.
// the way we're currently handling disconnects right now, sending deregisters from the EL thread makes this a non-issue though
/// does something, given work denoted as a RawMessage. Many operations are on a SliceMap, which is
/// a handrolled specialized datastructure
pub fn parse(work: RawMessage, contacts: &[Sender<RawMessage>], state_map: &mut SliceMap<HashMap<SocketAddr, TcpStream>>, interest_map: &mut HashMap<SocketAddr, HashSet<Vec<u8>>>) {

    //the message excluding the preamble
    let payload = &work.bytes[PREAMBLE_SZ..];

    match work.m_type {
        NOTIFICATION => {
            //topic len is a one byte value (<255)
            let topic_len = payload[0] as usize;
            let topic = &payload[1..topic_len+1];

            // forwards the NOTIFICATION to each interested party
            state_map.apply(topic, |ref mut h_entry| {
                for (addr, tcp_stream) in h_entry.iter_mut() {
                    //TcpStreams in Rust are wrappers around raw file descriptors
                    //this outer check is for runtime protection
                    match tcp_stream.peer_addr() {
                        Ok(ref a) if a == addr => {
                            let mut index = 0;
                            loop {
                                match tcp_stream.write(&work.bytes[index..work.length]) {
                                    Ok(just_written) => {
                                        index += just_written;
                                        if index == work.length {
                                            break;
                                        } else {
                                            //this is potentially bad, but does not seem to happen
                                            //if our buffer is < 4KB. (possibly because of ethernet
                                            //MTU?)
                                            //
                                            //not sure what happens with multithreadedness
                                            //but while send() syscalls are considered atomic partials are
                                            //possible in theory -> might have to break our rqueue
                                            //protocol to allow multi-part
                                            println!("partial");
                                        }
                                    },
                                    _ => {
                                        //this is dangerous as it will retry even from a
                                        //potentially unrecoverable error. perhaps enforce a max
                                        //timeout or retry limit
                                        //println!("err writing: {:?}", e);
                                    }
                                };
                            }
                        },
                        _ => ()
                    };
                }
            });
        }

        // subscribes the client sender to one topic
        SUBSCRIBE |  SUBSCRIBE_ONCE => {
            let topic_len = payload[0] as usize;
            let topic = &payload[1..topic_len+1];
            let c = interest_map.entry(work.socket_addr).or_insert(HashSet::new());
            c.insert(topic.to_owned());

            // add the subscribe to our map
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
                }
            );

            if work.m_type == SUBSCRIBE { //broadcast a sub once to the other workers
                println!("sub topic: {:?}", &topic[..]);
                for sender in contacts.iter() {
                    let mut u = unsafe{ ptr::read(&work) };
                    u.m_type = SUBSCRIBE_ONCE;
                    let _ = sender.send(u);
                }
            }
        }

        // removes one topic from a clients subscriptions
        REMOVE | REMOVE_ONCE => {
            let topic_len = payload[0] as usize;
            let topic = &payload[1..topic_len+1];
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
            if work.m_type == REMOVE { //broadcast a remove once to the other workers
                println!("removing, {:?}", &topic[..]);
                for sender in contacts.iter() {
                    let mut u = unsafe{ ptr::read(&work) };
                    u.m_type = REMOVE_ONCE;
                    let _ = sender.send(u);
                }
            }
        }

        // purges all subscriptions for a client
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

            if work.m_type == DEREGISTER { //broadcast is done from the event loop for now
                for sender in contacts.iter() {
                    let mut u = unsafe {ptr::read(&work) };
                    u.m_type = DEREGISTER_ONCE;
                    let _ = sender.send(u);
                }
            }
        }
        _ => {
            println!("none");
        }
    }
}

// converts a raw fd (a 32-bit C integer) to std::net::TcpStream
fn to_std_tcpstream_from_raw(fd: RawFd) -> TcpStream {
    let builder = unsafe { TcpBuilder::from_raw_fd(fd.clone()) };
    builder.to_tcp_stream().unwrap()
}
