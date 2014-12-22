extern crate msg;
extern crate superchan;

use msg::{Message, Response};
use std::io;
use superchan::{Sender, Receiver};
use superchan::tcp;

fn main() {
    let (mut sender, mut receiver) = match tcp::client_channel("127.0.0.1:8080") {
        Ok(chans) => chans,
        Err(e) => { println!("{}", e); return; },
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

                // Type annotation needed here because we're not matching on
                // specific Response::* values.
                let resp: Response = receiver.recv();
                println!("response: {}", resp);
            },
            Err(e) => println!("error: {}", e),
        }
    }
}
