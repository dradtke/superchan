extern crate msg;
extern crate superchan;

use msg::{Message, Response};
use superchan::tcp::server_channel;

fn handle_client(msg: Message) -> Response {
    match msg {
        Message::Blank => { println!("received blank message"); Response::NotOk },
        Message::Int(i) => { println!("received int message: {}", i); Response::Ok },
        Message::String(s) => { println!("received string message: {}", s); Response::Ok },
    }
}

#[allow(unused_must_use)]
fn main() {
    println!("Starting server...");
    server_channel("127.0.0.1:8080", handle_client);
}
