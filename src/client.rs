use std::net::{TcpStream};
use std::thread;
fn main () {
    let mut stream = TcpStream::connect("127.0.0.1:6567").unwrap();
    thread::park();
}
