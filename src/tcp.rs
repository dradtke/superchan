//! Module `tcp` provides support for channels that communicate
//! over TCP.

use serialize::{Decodable, Encodable};
use serialize::json::{Decoder, DecoderError, decode, Encoder};
use std::collections::ring_buf::RingBuf;
use std::comm;
use std::io::{IoError, IoErrorKind, IoResult, TcpStream};
use std::io::net::ip::ToSocketAddr;
use std::sync::{Arc, Future, Mutex};
use std::thread::Thread;
use super::{SendRequest, ReceiverError};

/// A client sender for sending messages over TCP.
#[deriving(Clone)]
pub struct ClientSender<T: Encodable<Encoder<'static>, IoError> + Send>(comm::Sender<SendRequest<T>>);

impl<T> super::Sender<T> for ClientSender<T> where T: Encodable<Encoder<'static>, IoError> + Send {
    /// Send a value along the channel.
    ///
    /// The returned Future will only have a value available after the send has either
    /// succeeded or failed.
    fn send(&mut self, t: T) -> Future<IoResult<()>> {
        let (fi, fo) = comm::channel();
        self.0.send((t, fi));
        // Future::from_receiver() doesn't work here. It causes the `fi` Sender to close before it
        // gets a chance to send the response. For some reason though, spawning it like this works.
        Future::spawn(move || { fo.recv() })
    }
}

/// A client receiver for receiving server responses over TCP.
pub struct ClientReceiver<S: Decodable<Decoder, DecoderError> + Send>(comm::Receiver<Result<S, ReceiverError>>);

impl<S> super::Receiver<S> for ClientReceiver<S> where S: Decodable<Decoder, DecoderError> + Send {
    /// Try to receive a server response.
    fn try_recv(&mut self) -> Result<S, ReceiverError> {
        self.0.recv()
    }
}

/// Create a channel over a new TCP connection.
///
/// This method attempts to connect to an existing server at the specified
/// address, and returns a sender/receiver pair if the connection was made.
#[allow(unused_must_use)]
pub fn client_channel<A: ToSocketAddr, T: Encodable<Encoder<'static>, IoError> + Send, S: Decodable<Decoder, DecoderError> + Send>(addr: A) -> IoResult<(ClientSender<T>, ClientReceiver<S>)> {
    let stream = Arc::new(Mutex::new(try!(TcpStream::connect(addr))));
    let (ss, sr) = comm::channel::<SendRequest<T>>();
    {
        let stream = stream.clone();
        Thread::spawn(move || {
            for (t, fi) in sr.iter() {
                let mut stream = stream.lock();
                let e = Encoder::buffer_encode(&t);
                stream.write_le_uint(e.len());
                stream.write(e.as_slice());
                stream.flush();
                // TODO: use the responses from the above calls to send an error if appropriate
                fi.send(Ok(()));
            }
        });
    }
    let (rs, rr) = comm::channel::<Result<S, ReceiverError>>();
    {
        let stream = stream.clone();
        Thread::spawn(move || {
            loop {
                {
                    let mut stream = stream.lock();
                    stream.set_read_timeout(Some(10)); // TODO: config value?
                    match stream.read_le_uint() {
                        Ok(size) => rs.send(read_item(&mut (*stream), size)),
                        Err(ref e) if e.kind == IoErrorKind::TimedOut => (), // no data available
                        Err(ref e) if e.kind == IoErrorKind::EndOfFile => return,
                        Err(e) => rs.send(Err(ReceiverError::IoError(e))),
                    }
                }
                Thread::yield_now();
            }
        });
    }
    Ok((ClientSender(ss), ClientReceiver(rr)))
}

/// Listen for incoming TCP connections.
///
/// The server side uses an event-based architecture, with the supported events:
///
///  * `on_msg`: notification of a client message
///  * `on_new`: notification of a new client connection
///  * `on_drop`: notification of a client hanging up
///
/// Events that you don't care about can be ignored by passing in `|_|{}`, which is an
/// empty closure.
#[allow(unused_must_use)]
pub fn server_channel<A, T, S, H, N, D>(addr: A, on_msg: H, on_new: N, on_drop: D) -> IoResult<()>
        where A: ToSocketAddr,
              T: Encodable<Encoder<'static>, IoError> + Send, // outgoing
              S: Decodable<Decoder, DecoderError> + Send,     // incoming
              H: Fn(uint, S) -> T + Copy + Send,              // handle client message
              N: Fn(uint) -> () + Copy + Send,                // new client
              D: Fn(uint) -> () + Copy + Send,                // client dropped
{
    use std::io::{Acceptor, Listener};
    use std::io::net::tcp::TcpListener;

    let listener = try!(TcpListener::bind(addr));
    let acceptor = try!(listener.listen());
    {
        let mut acceptor = acceptor.clone();
        let mut client_counter = 0u;
        let freed_clients = Arc::new(Mutex::new(RingBuf::new()));
        for conn in acceptor.incoming() {
            match conn {
                Ok(mut conn) => {
                    let client_id = match (*freed_clients.lock()).pop_front() {
                        Some(id) => id,
                        None => {
                            client_counter = client_counter + 1;
                            client_counter
                        },
                    };
                    on_new(client_id);
                    let freed_clients = freed_clients.clone();
                    Thread::spawn(move || {
                        loop {
                            let item = match conn.read_le_uint() {
                                Ok(size) => match read_item(&mut conn, size) {
                                    Ok(item) => item,
                                    Err(e) => panic!(e),
                                },
                                Err(ref e) if e.kind == IoErrorKind::TimedOut => {
                                    continue;
                                },
                                Err(ref e) if e.kind == IoErrorKind::EndOfFile => {
                                    (*freed_clients.lock()).push_back(client_id);
                                    on_drop(client_id);
                                    return;
                                },
                                Err(e) => {
                                    (*freed_clients.lock()).push_back(client_id);
                                    panic!("{}", e);
                                },
                            };
                            let resp = on_msg(client_id, item);
                            let e = Encoder::buffer_encode(&resp);
                            conn.write_le_uint(e.len());
                            conn.write(e.as_slice());
                            conn.flush();
                        }
                    });
                },
                Err(ref e) if e.kind == IoErrorKind::EndOfFile => break,
                Err(e) => panic!(e),
            }
        }
    }
    Ok(())
}

/// Utility method for reading a value from a stream.
fn read_item<S: Decodable<Decoder, DecoderError> + Send>(stream: &mut TcpStream, size: uint) -> Result<S, ReceiverError> {
    let data = try!(stream.read_exact(size));
    let string = try!(String::from_utf8(data));
    Ok(try!(decode::<S>(string.as_slice())))
}
