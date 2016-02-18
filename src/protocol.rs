use mio::TryRead;
use mio::tcp::TcpStream;
use std::os::unix::io::{RawFd, AsRawFd};
use std::net::SocketAddr;
use std::mem;
use std::io::Read;
use rpc;


pub const MAX_STATIC_SZ: usize = 2048;

/// RawMessage is raw in so far that we have the message in it's entirety
/// we know the general type but we don't necessarily know what the contents in
/// the payload is.
pub struct RawMessage {
    pub m_type: u8,
    pub length: usize,
    pub payload: [u8; MAX_STATIC_SZ],
    pub raw_fd: RawFd,
    pub socket_addr: SocketAddr
}

/// Gets a message from the socket
pub fn get_message (socket: &mut TcpStream) -> Option<RawMessage>{
    let mut preamble: [u8; 5] = unsafe { mem::uninitialized() };
    let mut preamble_read = 0;
    let payl_size;
    let m_type;

    loop {
        match socket.try_read(&mut preamble[preamble_read..]) {
            Ok(Some(0)) if preamble_read == 0 => {
                return None
            }
            Ok(Some(num_read)) => {
                preamble_read += num_read;
                if preamble_read == 5 {
                    payl_size = u8_4_to_u32(&preamble[0..4]);
                    m_type = preamble[4];
                    break;
                }
            }
            _ => return None
        };
    }

    let mut payload: [u8; MAX_STATIC_SZ] = unsafe { mem::uninitialized() };
    //let mut retries = 0;
    let mut curr_index = 0;
    loop { //this loop might be bad for the event loop. might be able to abstract into a
           //coroutine powered by mio
        match socket.try_read(&mut payload[curr_index..payl_size]) {
            Ok(Some(read)) => {
                curr_index += read;
                if curr_index == payl_size {
                    return Some(RawMessage {
                        m_type: m_type,
                        length: curr_index,
                        payload: payload,
                        raw_fd: socket.as_raw_fd(),
                        socket_addr: socket.peer_addr().unwrap()
                    })
                } else { /*retries += 1; */}
            }
            _ => { /*retries += 1; */}
        }
    }
}

//intended to be used as a client library. someday create an api lib
//for now these are used for testing
pub fn notify_message(topic: &[u8], content: &[u8]) -> Vec<u8> {
    let mut vec = Vec::new();
    let topic_len = [topic.len() as u8];
    let sz = (content.len() + topic.len() + topic_len.len()) as u32;
    let len:[u8; 4] = unsafe {mem::transmute(sz.to_be())};

    vec.extend(len.iter()
               .chain([rpc::NOTIFICATION].iter())
               .chain(topic_len.iter())
               .chain(topic.iter())
               .chain(content.iter())
               );
    vec
}

/// creates a byte representation of a publish message
pub fn publish_message(topic: &[u8], content: &[u8]) -> Vec<u8> {
    let mut vec = Vec::new();
    let payload = notify_message(topic, content);
    let sz = payload.len() as u32;
    let len:[u8; 4] = unsafe {mem::transmute(sz.to_be())};

    //publish is composed with a notification message to avoid allocations
    //the payload will need a length prefix because will be forwarded along
    //to other tcp listeners.

    //e.g. [len x 4, type x 1,  PAYLOAD] ->
    //     [len x 4, type x 1, [len x 4, type x 1, INNER PAYLOAD]

    vec.extend(len.iter()
               .chain([rpc::PUBLISH].iter())
               .chain(payload.iter()));

    vec
}

/// creates a byte representation of a subscribe message
pub fn subscribe_message(topic: &[u8]) -> Vec<u8> {
    let mut vec = Vec::new();
    let topic_len = [topic.len() as u8];
    let sz = (topic.len() + topic_len.len()) as u32;
    let len:[u8; 4] = unsafe {mem::transmute(sz.to_be())};

    vec.extend(len.iter()
               .chain([rpc::SUBSCRIBE].iter())
               .chain(topic_len.iter())
               .chain(topic.iter()));
    vec
}

pub fn u8_4_to_u32 (bytes: &[u8]) -> usize {
    (bytes[3] as usize
        | ((bytes[2] as usize) << 8)
        | ((bytes[1] as usize) << 16)
        | ((bytes[0] as usize) << 24))
}
