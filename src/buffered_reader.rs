use mio::{TryRead};
use mio::tcp::TcpStream;
use std::os::unix::io::{RawFd, AsRawFd};
use std::net::SocketAddr;

const MAX_STATIC_SZ: usize = 2048;

pub struct RawMessage {
    pub m_type: u8,
    pub length: usize,
    pub payload: [u8; MAX_STATIC_SZ],
    pub raw_fd: RawFd,
    pub socket_addr: SocketAddr
}

pub fn get_message(socket: &mut TcpStream) -> Option<RawMessage>{
    let mut preamble = [0; 5];
    let (payl_size, m_type) = match socket.try_read(&mut preamble) {
        Ok(Some(5)) => {
            let size = u8_4_to_u32(&preamble[0..4]);
            (size, preamble[4])
        },
        _ => return None
    };
    let mut payload = [0; MAX_STATIC_SZ];
    match socket.try_read(&mut payload[0..payl_size]) {
        Ok(Some(bytes_read)) => {
            Some(RawMessage {
                m_type: m_type,
                length: bytes_read,
                payload: payload,
                raw_fd: socket.as_raw_fd(),
                socket_addr: socket.peer_addr().unwrap()
            })
        },
        _ => {
            println!("no payload found");
            None
        }
    }
}

fn u8_4_to_u32 (bytes: &[u8]) -> usize {
    (bytes[3] as usize
        | ((bytes[2] as usize) << 8)
        | ((bytes[1] as usize) << 16)
        | ((bytes[0] as usize) << 24))
}
