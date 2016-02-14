use std::net::{TcpStream};
use std::thread;
use std::io::Write;
fn main () {
    let mut stream = TcpStream::connect("127.0.0.1:6567").unwrap();
    stream.write_all(&[0, 0, 0, 1, 2, 1]);
    thread::sleep_ms(500);
    stream.write_all(&[0, 0, 0, 1, 3, 3]);
    thread::park();
}
