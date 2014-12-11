//! # Superchan!
//!
//! This crate provides a set of types that mimick Rust's native channels,
//! but which can be used to communicate over a network. For example, you may
//! have a `server.rs` that accepts incoming connections:
//!
//! ~~~ignore
//! // server.rs
//! let mut receiver: Receiver<...> = superchan::TcpReceiver::new(/* ip address, e.g. "127.0.0.1:8080" */);
//! let value = receiver.recv();
//! // do something with `value`
//! ~~~
//!
//! ...and then send values to it from some `client.rs` running in a different process,
//! or even on a different computer:
//!
//! ~~~ignore
//! // client.rs
//! let mut sender: Sender<...> = superchan::TcpSender::new(/* ip address */);
//! let value = ...;
//! sender.send(value);
//! ~~~
//!
//! The types used must be serializable and deserializable, so anything you send
//! must either natively support it, or derive `Encodable` and `Decodable`:
//!
//! ~~~ignore
//! extern crate serialize;
//!
//! #[deriving(Encodable, Decodable)]
//! pub struct Message {
//!     ...
//! }
//!
//! fn main() {
//!     let mut receiver: Receiver<Message> = superchan::TcpReceiver::new(/* ip address */);
//!     // now you can receive messages of type `Message`
//! }
//! ~~~
//!
//! Naturally, if custom types are sent across the wire, then both the client and server
//! will need access to the type definition. Attempting to receive a value of a different
//! type than the one that was sent is unsupported, untested, and may even go as far as to
//! release the hounds.
//!
//! # Protocols
//!
//! Right now the only supported protocol is TCP, but more will be added in the hopefully
//! not-too-distant future.
//!
//! # Formats
//!
//! The Rust standard library currently only comes with one general-purpose serialization format,
//! which is JSON, so currently all implemented channels encode and decode their data
//! to and from JSON. Hopefully the advent of protocol buffers will eventually render this
//! unnecessary. =)

#![crate_name = "superchan"]
#![experimental]
#![feature(globs, unsafe_destructor)]
#![allow(dead_code)]
extern crate serialize;

pub use tcp::TcpSender as TcpSender;
pub use tcp::TcpReceiver as TcpReceiver;

use serialize::{Decodable, Encodable};
use serialize::json::{Decoder, DecoderError, Encoder};
use std::error::{Error, FromError};
use std::io::{IoError, IoErrorKind};

pub mod tcp;

/// Sender is a generic trait for objects that are able to send values
/// across a network.
pub trait Sender<T> where T: Encodable<Encoder<'static>, IoError> + Send {
    fn send(&mut self, t: T);
}

/// Receiver is a generic trait for objects that are able to receive
/// values from across a network.
pub trait Receiver<T> where T: Decodable<Decoder, DecoderError> {
    fn try_recv(&mut self) -> Result<T, ReceiverError>;

    fn recv(&mut self) -> T {
        match self.try_recv() {
            Ok(val) => val,
            Err(e) => panic!("{}", e),
        }
    }
}

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
