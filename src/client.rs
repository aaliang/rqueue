use std::net::{TcpStream};
use std::thread;
use std::io::{Read, Write};
use std::mem;

//TODO: these do not assert length limits. could very well be illegal messages
fn publish_message(topic: &[u8], content: &[u8]) -> Vec<u8> {
    let mut vec = Vec::new();
    let topic_len = topic.len() as u8;
    let sz = (content.len() + topic.len() + 1) as u32;
    let len:[u8; 4] = unsafe { mem::transmute(sz.to_be())};
    
    vec.extend(len.iter().chain([0].iter())
        .chain([topic_len].iter())
        .chain(topic.iter())
        .chain(content.iter()));

    vec
}

fn subscribe_message(topic: &[u8]) -> Vec<u8> {
    let mut vec = Vec::new();
    let topic_len = topic.len() as u8;
    let sz = (topic.len() + 1) as u32;
    let len:[u8; 4] = unsafe {mem::transmute(sz.to_be())};

    vec.extend(len.iter().chain([1].iter()).chain([topic_len].iter()).chain(topic.iter()));
    vec
}

//use std::thread;


fn main () {
    let mut stream = TcpStream::connect("127.0.0.1:6567").unwrap();

    let mut stream_clone = stream.try_clone().unwrap();
    let _ = thread::spawn(move || {
        let pub_msg = publish_message(&[1,1,1], &[6,6,6,6,6,6]);
        loop {
            stream_clone.write_all(&pub_msg);
            //stream_clone.write_all(&[1,1,1,1,1,1,1,1,1,1]);
        }
    });


    loop {
        let pub_msg = publish_message(&[3,3,3], &[5,5,5,5,5,5]);
        //let sub_msg = subscribe_message(&[3,3,3]);
        //println!("{:?}", &sub_msg[..]);
        //stream.write_all(&sub_msg);

        //thread::sleep_ms(500);

        //println!("{:?}", &pub_msg[..]);
        stream.write_all(&pub_msg);
        //stream.write_all(&[8,8,8,8,8,8,8,8,8,8])
    }

    let mut mut_buf = [0; 30];
    let r = stream.read(&mut mut_buf);
    println!("{:?}", r);
    println!("{:?}", &mut_buf);
    thread::park();
}
