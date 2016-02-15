use buffered_reader::RawMessage;
use std::sync::Arc;
use concurrent_hash_map::ConcurrentHashMap;
use std::net::TcpStream;

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
pub fn parse <'a> (work: &'a RawMessage, state_map: &Arc<ConcurrentHashMap<&[u8], TcpStream>>) -> Actions<'a> {
    match work.m_type {
        PUBLISH => {
            //topic len is a one byte value (<255)
            let topic_len = work.payload[0] as usize;
            let topic = &work.payload[1..topic_len+1];
            let message = &work.payload[topic_len+1..work.length];

            println!("pub topic: {:?}", topic);
            println!("message: {:?}", message);

            Actions::Publish {topic: topic, content: message}
        }
        SUBSCRIBE => {
            let topic_len = work.payload[0] as usize;
            let topic = &work.payload[1..];

            println!("sub topic: {:?}", &topic[..work.length]);

            //let res = state_map.get(topic);

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
