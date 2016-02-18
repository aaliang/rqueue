use mio::TryRead;
use mio::tcp::TcpStream;
use std::os::unix::io::{RawFd, AsRawFd};
use std::net::SocketAddr;
use std::mem;
use std::io::Read;

/// fixed stack space for each message
pub const MAX_STATIC_SZ   : usize = 2048;

/// a fixed {PREAMBLE_SZ} sized stream of bytes precedes every message
pub const PREAMBLE_SZ     : usize = 3;
pub const PREAMBLE_LEN_SZ : usize = 2;

// an enumeration on possible message types
pub const SUBSCRIBE       : u8 = 1; // subscribes a client to a topic, broadcasts to other workers
pub const REMOVE          : u8 = 2; // removes client intent on topic, broadcasts to other workers
pub const SUBSCRIBE_ONCE  : u8 = 3; // same as SUBSCRIBE, but no broadcast
pub const REMOVE_ONCE     : u8 = 4; // same as REMOVE, but no broadcast
pub const DEREGISTER      : u8 = 5; // purges all subscriptions for a client, and broadcasts
pub const DEREGISTER_ONCE : u8 = 6; // same as REGISTER, but no broadcast
pub const NOTIFICATION    : u8 = 7; // message pertaining to topic sent from a client to the server,
                                    // forwarded directly to interested clients 


/// RawMessage is raw in so far that we have the message in it's entirety
/// we know the general type but we don't necessarily know what the contents in
/// the payload is.
pub struct RawMessage {
    pub m_type: u8,

    /// the length of the message, in bytes. Includes the preamble
    pub length: usize,

    /// the byte representation of the message.
    /// only the first {length} bytes truly represent the message
    /// the rest are considered garbage and should not be used
    pub bytes: [u8; MAX_STATIC_SZ],
    pub raw_fd: RawFd,
    pub socket_addr: SocketAddr
}

/// Gets a message from the socket
pub fn get_message (socket: &mut TcpStream) -> Option<RawMessage> {
    let mut message_raw: [u8; MAX_STATIC_SZ] = unsafe { mem::uninitialized() };

    let mut preamble_read = 0;
    let payl_size;
    let m_type;

    loop {
        match socket.try_read(&mut message_raw[preamble_read..PREAMBLE_SZ]) {
            Ok(Some(0)) if preamble_read == 0 => return None,
            Ok(Some(num_read)) => {
                preamble_read += num_read;
                if preamble_read == PREAMBLE_SZ {
                    payl_size = u8_2_to_usize(&message_raw[..PREAMBLE_LEN_SZ]);
                    m_type = message_raw[PREAMBLE_LEN_SZ];
                    break;
                }
            }
            _ => return None
        };
    }
    let mut curr_index = 0;
    //let mut retries = 0;
    {
        let mut payload = &mut message_raw[PREAMBLE_SZ..(payl_size + PREAMBLE_SZ)];
        loop { //this loop might be bad for the event loop. might be able to abstract into a
               //coroutine powered by mio
            match socket.try_read(&mut payload[curr_index..]) {
                Ok(Some(read)) => {
                    curr_index += read;
                    if curr_index == payl_size {
                        break;
                    } else { /*retries += 1; */}
                }
                _ => { /*retries += 1; */}
            }
        }
    }

    return Some(RawMessage {
        m_type: m_type,
        length: payl_size + PREAMBLE_SZ,
        bytes: message_raw,
        raw_fd: socket.as_raw_fd(),
        socket_addr: socket.peer_addr().unwrap()
    })
}

//intended to be used as a client library. someday create an api lib
//for now these are used for testing
pub fn notify_message(topic: &[u8], content: &[u8]) -> Vec<u8> {
    let mut vec = Vec::new();
    let topic_len = [topic.len() as u8];
    let sz = (content.len() + topic.len() + topic_len.len()) as u16;
    let len:[u8; PREAMBLE_LEN_SZ] = unsafe {mem::transmute(sz.to_be())};

    vec.extend(len.iter()
               .chain([NOTIFICATION].iter())
               .chain(topic_len.iter())
               .chain(topic.iter())
               .chain(content.iter())
               );
    vec
}

/// creates a byte representation of a subscribe message
pub fn subscribe_message(topic: &[u8]) -> Vec<u8> {
    let mut vec = Vec::new();
    let topic_len = [topic.len() as u8];
    let sz = (topic.len() + topic_len.len()) as u16;
    let len:[u8; PREAMBLE_LEN_SZ] = unsafe {mem::transmute(sz.to_be())};

    vec.extend(len.iter()
               .chain([SUBSCRIBE].iter())
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

pub fn u8_2_to_usize (bytes: &[u8]) -> usize {
    (bytes[1] as usize
        | ((bytes[0] as usize) << 8))
}
