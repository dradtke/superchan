extern crate msg;
extern crate superchan;

use msg::{Message, Response};
use superchan::tcp::server_channel;

fn new_client(id: uint) {
    println!("client {} has joined the fray", id);
}

fn handle_client(id: uint, msg: Message) -> Response {
    match msg {
        Message::Blank => { println!("[{}] received blank message", id); Response::NotOk },
        Message::Int(i) => { println!("[{}] received int message: {}", id, i); Response::Ok },
        Message::String(s) => { println!("[{}] received string message: {}", id, s); Response::Ok },
    }
}

fn dropped_client(id: uint) {
    println!("client {} has left us =(", id);
}

#[allow(unused_must_use)]
fn main() {
    println!("Starting server...");
    server_channel("127.0.0.1:8080", handle_client, new_client, dropped_client);
}
