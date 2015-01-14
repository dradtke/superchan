//! # Superchan!
//!
//! This crate provides a set of types that mimick Rust's native channels,
//! but which can be used to communicate over a network.
//!
//! Example of using `superchan` to spin up a server:
//!
//! ```ignore
//! // server.rs
//! extern crate "rustc-serialize" as rustc_serialize;
//! extern crate superchan;
//! use superchan::tcp::server_channel;
//!
//! #[derive(Encodable, Decodable)]
//! enum Message {
//!     Good,
//!     Bad,
//! }
//!
//! #[derive(Encodable, Decodable)]
//! enum Response {
//!     Ok,
//!     NotOk,
//! }
//!
//! // Take the client's message and return a response.
//! // This version is obviously pretty contrived, but
//! // you get the idea.
//! fn on_msg(client_id: u32, msg: Message) -> Response {
//!     match msg {
//!         Message::Good => Response::Ok,
//!         Message::Bad => Response::NotOk,
//!     }
//! }
//!
//! fn on_new(client_id: u32) {
//!     println!("New client has connected: {}", client_id);
//! }
//!
//! fn on_drop(client_id: u32) {
//!     println!("Client has disconnected: {}", client_id);
//! }
//!
//! fn main() {
//!     if let Err(e) = server_channel("127.0.0.1:8080", on_msg, on_new, on_drop) {
//!         println!("Failed to start server: {}", e);
//!     }
//! }
//! ```
//!
//! And creating a client to connect to it (ideally, the shared `Message` and `Response`
//! enums would be in a separate crate that is referenced by both, but they're
//! duplicated here for simplicity):
//!
//! ```ignore
//! // client.rs
//! extern crate "rustc-serialize" as rustc_serialize;
//! extern crate superchan;
//! use superchan::{Sender, Receiver};
//! use superchan::tcp::client_channel;
//!
//! #[derive(Encodable, Decodable)]
//! enum Message {
//!     Good,
//!     Bad,
//! }
//!
//! #[derive(Encodable, Decodable)]
//! enum Response {
//!     Ok,
//!     NotOk,
//! }
//!
//! fn main() {
//!     let (mut sender, mut receiver) = match client_channel("127.0.0.1:8080") {
//!         Ok(chans) => chans,
//!         Err(e) => { println!("Failed to connect to server: {}", e); return; },
//!     };
//!
//!     // Now we can communicate with the server along the received channels.
//!     sender.send(Message::Good);
//!     match receiver.recv() {
//!         Response::Ok => println!("ok!"),
//!         Response::NotOk => println!("not ok..."),
//!     }
//! }
//! ```
//!
//! TCP is the only supported protocol right now, but UDP and maybe others will be added soon.
//! When that happens, the only difference needed should be the `use` statement by replacing
//! "tcp" with the protocol of your choice.

#![crate_name = "superchan"]
#![feature(unsafe_destructor)]
#![allow(dead_code)]
#![unstable = "waiting for the serialization dust to settle"]

extern crate "rustc-serialize" as rustc_serialize;
extern crate bincode;

use bincode::{encode, EncodingError, decode, DecodingError, SizeLimit};
use rustc_serialize::{Decodable, Encodable};
use std::sync::mpsc;
use std::error::{Error, FromError};
use std::io::{IoError, Reader, Writer};
use std::sync::Future;

pub mod tcp;

/// Sender is a generic trait for objects that are able to send values
/// across a network.
#[unstable = "waiting for the serialization dust to settle"]
pub trait Sender<T> where T: Encodable + Send {
    fn send(&mut self, t: T) -> Future<Result<(), SenderError<T>>>;
}

#[stable]
pub enum SenderError<T> {
    #[stable] Mpsc(mpsc::SendError<T>),
    #[stable] Io(IoError),
    #[stable] Encoding(EncodingError),
}

impl<T> Error for SenderError<T> where T: Send {
    fn description(&self) -> &str {
        // TODO: find a way to format!() the internal error's description into this one's
        match *self {
            SenderError::Mpsc(_) => "mpsc error",
            SenderError::Io(_) => "io error",
            SenderError::Encoding(_) => "encoding error",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            SenderError::Mpsc(_) => None,
            SenderError::Io(ref err) => Some(err as &Error),
            SenderError::Encoding(ref err) => Some(err as &Error),
        }
    }
}

impl<T> FromError<mpsc::SendError<T>> for SenderError<T> {
    fn from_error(err: mpsc::SendError<T>) -> SenderError<T> {
        SenderError::Mpsc(err)
    }
}

impl<T> FromError<IoError> for SenderError<T> {
    fn from_error(err: IoError) -> SenderError<T> {
        SenderError::Io(err)
    }
}

impl<T> FromError<EncodingError> for SenderError<T> {
    fn from_error(err: EncodingError) -> SenderError<T> {
        SenderError::Encoding(err)
    }
}

/// Contains a type to be sent and a channel for sending the response.
type SendRequest<T> = (T, mpsc::Sender<Result<(), SenderError<T>>>);

/// Receiver is a generic trait for objects that are able to receive
/// values from across a network.
#[unstable = "waiting for the serialization dust to settle"]
pub trait Receiver<S> where S: Decodable + Send {
    fn try_recv(&mut self) -> Result<S, ReceiverError<S>>;

    /// Receive a server response. Unlike `try_recv()`, this method panics
    /// if an error is encountered.
    fn recv(&mut self) -> S {
        match self.try_recv() {
            Ok(val) => val,
            Err(e) => panic!("{:?}", e.description()),
        }
    }
}

#[stable]
pub enum ReceiverError<S> {
    #[stable] Io(IoError),
    #[stable] Decoding(DecodingError),
}

impl<S> Error for ReceiverError<S> where S: Send {
    fn description(&self) -> &str {
        // TODO: find a way to format!() the internal error's description into this one's
        match *self {
            ReceiverError::Io(_) => "io error",
            ReceiverError::Decoding(_) => "decoding error",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            ReceiverError::Io(ref err) => Some(err as &Error),
            ReceiverError::Decoding(ref err) => Some(err as &Error),
        }
    }
}

impl<S> FromError<IoError> for ReceiverError<S> {
    fn from_error(err: IoError) -> ReceiverError<S> {
        ReceiverError::Io(err)
    }
}

impl<S> FromError<DecodingError> for ReceiverError<S> {
    fn from_error(err: DecodingError) -> ReceiverError<S> {
        ReceiverError::Decoding(err)
    }
}

/// Utility method for reading a value from a stream.
fn read_item<S, R>(r: &mut R, size: usize) -> Result<S, ReceiverError<S>> where S: Decodable, R: Reader {
    // ???: is it necessary to read the size first if we know what the type is?
    let data = try!(r.read_exact(size));
    Ok(try!(decode::<S>(data.as_slice())))
}

/// Utility method for writing a value to a stream.
fn write_item<T, W>(w: &mut W, val: &T) -> Result<(), SenderError<T>> where T: Encodable, W: Writer {
    let e = try!(encode(val, SizeLimit::Infinite));
    try!(w.write_le_uint(e.len()));
    try!(w.write(e.as_slice()));
    try!(w.flush());
    Ok(())
}
