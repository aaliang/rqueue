extern crate time;
extern crate rqueue;

use std::net::{TcpStream};
use std::thread;
use std::io::{Read, Write};
use rqueue::protocol;

fn get_message (socket: &mut TcpStream) -> Option<usize>{
    let mut preamble = [0; 5];
    let (payl_size, m_type) = match socket.read(&mut preamble) {
        Ok(5) => {
            let size = protocol::u8_4_to_u32(&preamble[0..4]);
            (size, preamble[4])
        },
        _ => return None
    };

    let mut payload = [0; 1024];
    match socket.read(&mut payload[0..payl_size]) {
        Ok(bytes_read) => {
            //println!("got: {:?}", &payload[..bytes_read]);
            Some(bytes_read)
        },
        _ => {
            println!("no payload found");
            None
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
                Some(0) => break,
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
