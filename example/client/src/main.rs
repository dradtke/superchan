extern crate msg;
extern crate superchan;

use msg::Message;
use std::io;
use superchan::{Sender, TcpSender};

fn main() {
    println!("Connecting to server...");
    let mut sender: TcpSender<Message> = match TcpSender::new("127.0.0.1:8080") {
        Ok(sender) => sender,
        Err(e) => { println!("Failed to start server: {}", e); return; },
    };
    println!("Waiting for input.");

    let mut stdin = io::stdin();

    loop {
        print!("> ");
        match stdin.read_line() {
            Ok(line) => {
                let s = line.as_slice().trim().into_string();
                if s.len() == 0 {
                    sender.send(Message::Blank);
                } else if let Some(i) = from_str::<int>(s.as_slice()) {
                    sender.send(Message::Int(i));
                } else {
                    sender.send(Message::String(s));
                }
            },
            Err(e) => println!("error: {}", e),
        }
    }
}
