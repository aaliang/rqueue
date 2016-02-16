extern crate time;
extern crate rqueue;

use std::net::{TcpStream};
use std::thread;
use std::io::{Read, Write};
use rqueue::protocol;

fn main () {
    let mut stream = TcpStream::connect("127.0.0.1:6567").unwrap();

    /*
    let sub_msg = subscribe_message(&[3,3,3,3]);
    stream.write_all(&sub_msg);

    let sub_msg_2 = subscribe_message(&[4,4,4,4]);
    stream.write_all(&sub_msg_2);
    */
    let pub_msg = protocol::publish_message(&[3,3,3,3], &[9]);

    let mut f = 0;
    loop {
        f += 1;
        stream.write(&pub_msg);
        if f == 1000000 {
            return
        }
    }

    let sub_msg = protocol::subscribe_message(&[3,3,3,3]);
    stream.write(&sub_msg);

    if let Ok(b) = stream.read(&mut [0; 1000]) {
        let mut bytes = b;
        let mut count = 1;
        let start = time::precise_time_ns();
        loop {
            let mut mut_buf = [0; 1000];
            match stream.read(&mut mut_buf) {
                Ok(bytes_read) => {bytes+=bytes_read;}, //println!("{:?}", &mut_buf[..bytes_read]),
                _ => ()
            };
            count += 1;
            if count % 2000 == 0 {
                let seconds = ((time::precise_time_ns() - start) as f64)/ 1000000000.0;
                println!("{} bytes/sec", (bytes as f64)/seconds);
                println!("count: {}, over {} secs", count, seconds);
            }
        }

    }
    /*println!("{:?}", r);
    println!("{:?}", &mut_buf);
    thread::park();*/
}
