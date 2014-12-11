extern crate msg;
extern crate superchan;

use msg::Message;
use std::error::Error;
use superchan::{Receiver, TcpReceiver};

fn main() {
    println!("Starting server...");
    let mut receiver: TcpReceiver<Message> = match TcpReceiver::new("127.0.0.1:8080") {
        Ok(receiver) => receiver,
        Err(e) => { println!("Failed to start server: {}", e); return; },
    };
    println!("Listening for clients.");

    loop {
        match receiver.try_recv() {
            Ok(Message::Blank) => println!("Received blank message."),
            Ok(Message::Int(i)) => println!("Received int message: {}.", i),
            Ok(Message::String(s)) => println!("Received string message: {}.", s),
            Err(ref e) => println!("Error: {}", (e as &Error).description()),
        }
    }
}
