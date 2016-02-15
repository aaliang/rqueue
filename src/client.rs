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

    vec.extend(len.iter().chain([1].iter()).chain(topic.iter()));
    vec
}


fn main () {
    let mut stream = TcpStream::connect("127.0.0.1:6567").unwrap();


    let pub_msg = publish_message(&[3,3,3], &[4,5,6,7]);
    let sub_msg = subscribe_message(&[3,3,3]);
    println!("{:?}", &pub_msg[..]);
    stream.write_all(&pub_msg);
    println!("{:?}", &sub_msg[..]);
    stream.write_all(&sub_msg);


    let mut mut_buf = [0; 30];
    let r = stream.read(&mut mut_buf);
    println!("{:?}", r);
    println!("{:?}", &mut_buf);
    thread::park();
}
