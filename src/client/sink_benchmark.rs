extern crate time;
extern crate rqueue;

use std::net::{TcpStream};
use std::thread;
use std::io::{Read, Write};
use rqueue::protocol;
use std::mem;

pub fn get_message (socket: &mut TcpStream) -> Option<usize>{
    let mut preamble = [0; 5];
    let mut preamble_read = 0;
    let payl_size;
    let m_type;

    loop {
        match socket.read(&mut preamble[preamble_read..]) {
            Ok(5) => {
                let size = protocol::u8_4_to_u32(&preamble[0..4]);
                payl_size = size;
                m_type = preamble[4];
                break;
            },
            Ok(0) if preamble_read == 0 => {
                return None
            }
            Ok(num_read) => {
                println!("only read {:?}", num_read);
                preamble_read += num_read;
            }
            _ => return None
        };
    }

    let mut payload: [u8; protocol::MAX_STATIC_SZ] = unsafe { mem::uninitialized() };
    let mut retries = 0;
    let mut curr_index = 0;
    loop { //this loop might be bad for the event loop. might be able to abstract into a
           //coroutine powered by mio
        match socket.read(&mut payload[curr_index..payl_size]) {
            Ok(read) => {
                curr_index += read;
                if curr_index == payl_size {
                    if retries > 0 {
                        println!("continuing after {} retries", retries);
                    }
                    return Some(curr_index + preamble_read)
                } else {
                    retries += 1;
                    println!("retrying #{}", retries);
                }
            }
            err => {
                println!("err {:?}", err);
                retries += 1;
                println!("retrying #{}", retries);
            }
        }
    }
}

fn main () {
    let mut stream = TcpStream::connect("127.0.0.1:6567").unwrap();
    let sub_msg = protocol::subscribe_message(&[3,3,3,3]);
    println!("{:?}", sub_msg);
    stream.write_all(&sub_msg);

    let msg_opt = get_message(&mut stream);

    if let Some(b) = msg_opt {

        let mut bytes = b;
        let mut count = 1;

        let start = time::precise_time_ns();

        loop {
            match get_message(&mut stream) {
               // Some(0) => break,
                Some(bytes_read) => {
                    bytes+=bytes_read;
                    count += 1;
                },
                _ => ()
            };

            if count % 10000 == 0 {
                let seconds = ((time::precise_time_ns() - start) as f64)/ 1000000000.0;
 
                println!("throughput: {} msg/sec @ {} bytes/sec",
                         (count as f64)/seconds,
                         (bytes as f64)/seconds);

            }
        }

    }

    //[length, type, payload]
    //[]


    /*println!("{:?}", r);
    println!("{:?}", &mut_buf);
    thread::park();*/
}
