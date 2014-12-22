//! # Superchan!
//!
//! This crate provides a set of types that mimick Rust's native channels,
//! but which can be used to communicate over a network.
//!
//! Example of using `superchan` to spin up a server:
//!
//! ```
//! // server.rs
//! extern crate serialize;
//! extern crate superchan;
//! use superchan::tcp::server_channel;
//!
//! #[deriving(Encodable, Decodable)]
//! enum Message {
//!     Good,
//!     Bad,
//! }
//!
//! #[deriving(Encodable, Decodable)]
//! enum Response {
//!     Ok,
//!     NotOk,
//! }
//!
//! // Take the client's message and return a response.
//! // This version is obviously pretty contrived, but
//! // you get the idea.
//! fn on_msg(client_id: uint, msg: Message) -> Response {
//!     match msg {
//!         Message::Good => Response::Ok,
//!         Message::Bad => Response::NotOk,
//!     }
//! }
//!
//! fn on_new(client_id: uint) {
//!     println!("New client has connected: {}", client_id);
//! }
//!
//! fn on_drop(client_id: uint) {
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
//! ```
//! // client.rs
//! extern crate serialize;
//! extern crate superchan;
//! use superchan::{Sender, Receiver};
//! use superchan::tcp::client_channel;
//!
//! #[deriving(Encodable, Decodable)]
//! enum Message {
//!     Good,
//!     Bad,
//! }
//!
//! #[deriving(Encodable, Decodable)]
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
#![experimental]
#![feature(globs, unboxed_closures, unsafe_destructor)]
#![allow(dead_code)]
extern crate serialize;

use serialize::{Decodable, Encodable};
use serialize::json::{Decoder, DecoderError, Encoder};
use std::comm;
use std::error::{Error, FromError};
use std::io::{IoError, IoErrorKind, IoResult};
use std::sync::Future;

pub mod tcp;

/// Sender is a generic trait for objects that are able to send values
/// across a network.
pub trait Sender<T> where T: Encodable<Encoder<'static>, IoError> + Send {
    fn send(&mut self, t: T) -> Future<IoResult<()>>;
}

/// Receiver is a generic trait for objects that are able to receive
/// values from across a network.
pub trait Receiver<S> where S: Decodable<Decoder, DecoderError> + Send {
    fn try_recv(&mut self) -> Result<S, ReceiverError>;

    /// Receive a server response. Unlike `try_recv()`, this method panics
    /// if an error is encountered.
    fn recv(&mut self) -> S {
        match self.try_recv() {
            Ok(val) => val,
            Err(e) => panic!("{}", e),
        }
    }
}

/// ReceiverError is an enumeration of the various types of errors that
/// a Receiver could run in to.
#[deriving(Show)]
pub enum ReceiverError {
    EndOfFile,
    IoError(IoError),
    ConversionError(Vec<u8>),
    DecoderError(DecoderError),
}

impl Error for ReceiverError {
    fn description(&self) -> &str {
        match *self {
            ReceiverError::EndOfFile => "end of file",
            ReceiverError::IoError(_) => "io error",
            ReceiverError::ConversionError(_) => "conversion error",
            ReceiverError::DecoderError(_) => "decoder error",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            ReceiverError::EndOfFile => None,
            ReceiverError::IoError(ref err) => Some(err as &Error),
            ReceiverError::ConversionError(_) => None,
            ReceiverError::DecoderError(ref err) => Some(err as &Error),
        }
    }
}

impl FromError<IoError> for ReceiverError {
    fn from_error(err: IoError) -> ReceiverError { ReceiverError::IoError(err) }
}
impl FromError<Vec<u8>> for ReceiverError {
    fn from_error(err: Vec<u8>) -> ReceiverError { ReceiverError::ConversionError(err) }
}
impl FromError<DecoderError> for ReceiverError {
    fn from_error(err: DecoderError) -> ReceiverError { ReceiverError::DecoderError(err) }
}

impl ReceiverError {
    /// Returns true iff the error was caused by an EOF IoError.
    fn is_eof(&self) -> bool {
        match *self {
            ReceiverError::IoError(ref err) => err.kind == IoErrorKind::EndOfFile,
            _ => false,
        }
    }
}

/// Contains a type to be sent and a channel for sending the response.
type SendRequest<T> = (T, comm::Sender<IoResult<()>>);
