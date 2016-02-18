extern crate time;
extern crate rqueue;

use std::net::{TcpStream};
use std::io::{Write};
use rqueue::protocol;
use std::mem;

use std::thread;

fn main () {
    let mut stream = TcpStream::connect("127.0.0.1:6567").unwrap();

    //let msg:[u8; 1000] = unsafe{mem::uninitialized()};//[0; 1000];
    let mut msg = [1; 2000];

    msg[1995] = 66;
    msg[1996] = 67;
    msg[1997] = 68;
    msg[1998] = 69;
    msg[1999] = 70;

    let max = 6000000;

    let pub_msg = protocol::notify_message(&[3,3,3,3], &msg);

    let msg_len = pub_msg.len();

    println!("{:?}", pub_msg);

    let mut f = 0;
    loop {
        let mut index = 0;
        loop {
            match stream.write(&pub_msg[index..]) {
                Ok(just_written) =>  {
                    index += just_written;
                    if index == msg_len {
                        f += 1;
                        break;
                    }
                },
                e => {
                    //println!("err writing: {:?}", e)
                }
            };
        }
        if f == max {
            break;
        }
    }

    thread::park();

}
