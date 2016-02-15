use buffered_reader::RawMessage;
use std::sync::Arc;
use std::sync::mpsc::{Sender};
use slice_map::SliceMap;
use std::net::{TcpStream, SocketAddr};
use net2::TcpBuilder;
use std::os::unix::io::{FromRawFd, AsRawFd, RawFd};
use std::collections::HashMap;
use std::io::Write;
use std::{mem, ptr};

/* for reference
pub struct RawMessage {
    pub m_type: u8,
    pub length: usize,
    pub payload: [u8; MAX_STATIC_SZ],
    pub raw_fd: RawFd
}
 */

//not to be used for now
pub enum Actions <'a> {
    Publish {topic: &'a [u8], content: &'a [u8]},
    Subscribe {topic: &'a [u8]},
    Nil
}

const PUBLISH  : u8 = 0;
const SUBSCRIBE: u8 = 1;
const REMOVE   : u8 = 2;
const SUBSCRIBE_FROM_WORKER: u8 = 3;


//in this case this is an antipattern. can inline the handling
//todo: don't use a vec
pub fn parse(work: RawMessage, contacts: &[Sender<RawMessage>], state_map: &mut SliceMap<HashMap<SocketAddr, TcpStream>>) {
    match work.m_type {
        PUBLISH => {
            //topic len is a one byte value (<255)
            let topic_len = work.payload[0] as usize;
            let topic = &work.payload[1..topic_len+1];
            let message = &work.payload[topic_len+1..work.length];

            println!("pub topic: {:?}", topic);
            println!("message: {:?}", message);

            state_map.apply(topic, |ref mut h_entry| {
                let mut n = 0;
                for (addr, tcp_stream) in h_entry.iter_mut() {
                    //TcpStreams in Rust are wrappers around raw file descriptors
                    //thus, it is possible for us to get here with stale TcpStreams that have yet to be removed
                    if addr == &tcp_stream.peer_addr().unwrap() {
                                            println!("writing #{} to {:?} - real {:?}", n, addr, tcp_stream.peer_addr());
                    n += 1;
                    tcp_stream.write(message);

                    }
                }
            });
        }
        SUBSCRIBE |  SUBSCRIBE_FROM_WORKER => {
            let topic_len = work.payload[0] as usize;
            let topic = &work.payload[1..topic_len+1];

            state_map.modify_or_else(topic, |ref mut map| {
                //println!("fd-1: {}", work.raw_fd);
                let tcp_stream = to_std_tcpstream_from_raw(work.raw_fd.clone());
                map.insert(tcp_stream.peer_addr().unwrap(), tcp_stream);
            }, || {
                //println!("fd-2: {}", work.raw_fd);
                let mut map = HashMap::new();
                let tcp_stream = to_std_tcpstream_from_raw(work.raw_fd.clone());
                map.insert(tcp_stream.peer_addr().unwrap(), tcp_stream);
                map
            });

            if work.m_type == SUBSCRIBE {
                println!("sub topic: {:?}", &topic[..]);
                for sender in contacts.iter() {
                    let mut u = unsafe{ ptr::read(&work) };
                    u.m_type = SUBSCRIBE_FROM_WORKER;
                    sender.send(u);
                }
            }


        }
        REMOVE => {
            println!("removing");
            //Actions::Nil
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