use buffered_reader::RawMessage;
use std::sync::Arc;
use concurrent_hash_map::ConcurrentHashMap;
use std::net::{TcpStream, SocketAddr};
use net2::TcpBuilder;
use std::os::unix::io::{FromRawFd, AsRawFd, RawFd};
use std::collections::HashMap;
use std::io::Write;

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

//in this case this is an antipattern. can inline the handling
//todo: don't use a vec
pub fn parse <'a, 'b> (work: &'a RawMessage, state_map: &Arc<ConcurrentHashMap<HashMap<SocketAddr, TcpStream>>>) -> Actions<'a> {
    match work.m_type {
        PUBLISH => {
            //topic len is a one byte value (<255)
            let topic_len = work.payload[0] as usize;
            let topic = &work.payload[1..topic_len+1];
            let message = &work.payload[topic_len+1..work.length];

            println!("pub topic: {:?}", topic);
            println!("message: {:?}", message);

            /*let x = state_map.get_apply(topic, |ref mut h_entry| {
                for (_, tcp_stream) in h_entry.val.iter_mut() {
                    println!("writing");
                    tcp_stream.write(message);
                }
            });*/

            //println!("{:?}", x);
            Actions::Publish {topic: topic, content: message}

        }
        SUBSCRIBE => {
            let topic_len = work.payload[0] as usize;
            let topic = &work.payload[1..topic_len+1];

            println!("sub topic: {:?}", &topic[..]);

            state_map.modify_or_else(topic, |ref mut map| {
                let tcp_stream = to_std_tcpstream_from_raw(work.raw_fd.clone());
                map.insert(tcp_stream.peer_addr().unwrap(), tcp_stream);
            }, || {
                let mut map = HashMap::new();
                let tcp_stream = to_std_tcpstream_from_raw(work.raw_fd.clone());
                map.insert(tcp_stream.peer_addr().unwrap(), tcp_stream);
                map
            });
            Actions::Subscribe {topic: topic}
        }
        REMOVE => {
            println!("removing");
            Actions::Nil
        }
        _ => {
            println!("none");
            Actions::Nil
        }
    }
} 




fn to_std_tcpstream_from_raw(fd: RawFd) -> TcpStream {
    let builder = unsafe { TcpBuilder::from_raw_fd(fd.clone()) };
    builder.to_tcp_stream().unwrap()
}
