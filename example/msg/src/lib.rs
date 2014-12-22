extern crate serialize;

#[deriving(Encodable, Decodable)]
pub enum Message {
    Blank,
    Int(int),
    String(String),
}

#[deriving(Encodable, Decodable, Show, Copy)]
pub enum Response {
    Ok,
    NotOk,
}
