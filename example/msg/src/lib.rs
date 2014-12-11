extern crate serialize;

#[deriving(Encodable, Decodable)]
pub enum Message {
    Blank,
    Int(int),
    String(String),
}
