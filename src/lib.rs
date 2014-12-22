//! # Superchan!
//!
//! This crate provides a set of types that mimick Rust's native channels,
//! but which can be used to communicate over a network.
//!
//! This is only an experimental crate, and its API is still subject to very drastic changes.
//! Use at your own risk. =)

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
