extern crate time;
extern crate rqueue;

use std::net::{TcpStream};
use std::io::{Write};
use rqueue::protocol;
use std::mem;

use std::thread;

fn main () {
    let mut stream = TcpStream::connect("127.0.0.1:6567").unwrap();

    let msg:[u8; 200] = unsafe {mem::uninitialized()};
    let pub_msg = protocol::publish_message(&[3,3,3,3], &msg);
    println!("{:?}", pub_msg);

    let mut f = 0;
    loop {
        match stream.write(&pub_msg) {
            Ok(0) => {println!("failed")}
            Ok(_) => {f+=1;},
            _ => ()
        };
        if f == 1000000 {
            break;
        }
    }

    //thread::park();

}
