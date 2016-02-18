extern crate time;
extern crate rqueue;

use std::net::{TcpStream};
use std::io::{Read, Write};
use rqueue::protocol;
use rqueue::protocol::{u8_2_to_usize, MAX_STATIC_SZ, PREAMBLE_SZ, PREAMBLE_LEN_SZ};
use std::mem;

pub fn get_message (socket: &mut TcpStream) -> Option<([u8; MAX_STATIC_SZ], usize)>{
    let mut message_raw: [u8; MAX_STATIC_SZ] = unsafe{ mem::uninitialized() };
    let mut preamble_read = 0;
    let payl_size;
    let m_type;

    loop {
        match socket.read(&mut message_raw[preamble_read..PREAMBLE_SZ]) {
            Ok(0) if preamble_read == 0 => return None,
            Ok(num_read) => {
                preamble_read += num_read;
                if preamble_read == PREAMBLE_SZ {
                    payl_size = u8_2_to_usize(&message_raw[0..PREAMBLE_LEN_SZ]);
                    m_type = message_raw[PREAMBLE_LEN_SZ];
                    break;
                } else {
                    println!("only read: {}", num_read);
                }
            }
            _ => return None
        };
    }

    let mut retries = 0;
    let mut curr_index = 0;

    {
        let mut payload = &mut message_raw[PREAMBLE_SZ..(payl_size + PREAMBLE_SZ)];
        loop { //this loop might be bad for the event loop. might be able to abstract into a
               //coroutine powered by mio
            match socket.read(&mut payload[curr_index..]) {
                Ok(read) => {
                    curr_index += read;
                    if curr_index == payl_size {
                        break;
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

    return Some((message_raw, payl_size + PREAMBLE_SZ))
}

fn main () {
    let mut stream = TcpStream::connect("127.0.0.1:6567").unwrap();
    let sub_msg = protocol::subscribe_message(&[3,3,3,3]);
    println!("{:?}", sub_msg);
    let _ = stream.write_all(&sub_msg);

    let msg_opt = get_message(&mut stream);

    if let Some((_b, _bytes_read)) = msg_opt {
        let mut bytes = _bytes_read;
        let mut count = 1;
        let start = time::precise_time_ns();

        loop {
            match get_message(&mut stream) {
                Some((payload, bytes_read)) => {
                    bytes += bytes_read;
                    count += 1;
                    let topic_len = payload[0] as usize;
                    let topic = &payload[1..topic_len+1];

                    let message = &payload[5..bytes_read];
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

}
