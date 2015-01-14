extern crate "rustc-serialize" as rustc_serialize;

#[derive(RustcEncodable, RustcDecodable)]
pub enum Message {
    Blank,
    Int(i32),
    String(String),
}

#[derive(RustcEncodable, RustcDecodable, Show, Copy)]
pub enum Response {
    Ok,
    NotOk,
}
