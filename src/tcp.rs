//! Module `tcp` provides support for channels that communicate
//! over TCP.

use serialize::{Decodable, Encodable};
use serialize::json::encode;
use std::collections::ring_buf::RingBuf;
use std::sync::mpsc;
use std::io::{Acceptor, IoErrorKind, IoResult, Listener, TcpStream};
use std::io::net::ip::ToSocketAddr;
use std::io::net::tcp::TcpAcceptor;
use std::sync::{Arc, Future, Mutex};
use std::thread::Thread;
use super::{SendRequest, ReceiverError};

/// A client sender for sending messages over TCP.
#[derive(Clone)]
pub struct ClientSender<T: Encodable + Send>(mpsc::Sender<SendRequest<T>>);

impl<T> super::Sender<T> for ClientSender<T> where T: Encodable + Send {
    /// Send a value along the channel.
    ///
    /// The returned Future will only have a value available after the send has either
    /// succeeded or failed.
    fn send(&mut self, t: T) -> Future<IoResult<()>> {
        let (fi, fo) = mpsc::channel();
        self.0.send((t, fi)).unwrap();
        // Future::from_receiver() doesn't work here. It causes the `fi` Sender to close before it
        // gets a chance to send the response. For some reason though, spawning it like this works.
        Future::spawn(move || match fo.recv() {
                Ok(x) => x,
                Err(_) => panic!("sender hung up!"),
            }
        )
    }
}

/// A client receiver for receiving server responses over TCP.
pub struct ClientReceiver<S: Decodable + Send>(mpsc::Receiver<Result<S, ReceiverError>>);

impl<S> super::Receiver<S> for ClientReceiver<S> where S: Decodable + Send {
    /// Try to receive a server response.
    fn try_recv(&mut self) -> Result<S, ReceiverError> {
        match self.0.recv() {
            Ok(x) => x,
            Err(_) => panic!("sender hung up!"),
        }
    }
}

/// Create a channel over a new TCP connection.
///
/// This method attempts to connect to an existing server at the specified
/// address, and returns a sender/receiver pair if the connection was made.
#[allow(unused_must_use)]
pub fn client_channel<A: ToSocketAddr, T: Encodable + Send, S: Decodable + Send>(addr: A) -> IoResult<(ClientSender<T>, ClientReceiver<S>)> {
    let stream = try!(TcpStream::connect(addr));
    let (ss, sr) = mpsc::channel::<SendRequest<T>>();
    {
        let mut stream = stream.clone();
        Thread::spawn(move || {
            for (val, fi) in sr.iter() {
                fi.send(super::write_item(&mut stream, val));
            }
        });
    }
    let (rs, rr) = mpsc::channel::<Result<S, ReceiverError>>();
    {
        let mut stream = stream.clone();
        Thread::spawn(move || {
            loop {
                match stream.read_le_uint() {
                    Ok(size) => rs.send(super::read_item(&mut stream, size)).unwrap(),
                    Err(ref e) if e.kind == IoErrorKind::TimedOut => (),
                    Err(ref e) if e.kind == IoErrorKind::EndOfFile => return,
                    Err(e) => rs.send(Err(ReceiverError::IoError(e))).unwrap(),
                }
            }
        });
    }
    Ok((ClientSender(ss), ClientReceiver(rr)))
}

struct ClientAcceptor {
    inner: TcpAcceptor,
}

struct ServerSender<T: Encodable + Send>(mpsc::Sender<SendRequest<T>>);

struct ServerReceiver<S: Decodable + Send>(mpsc::Receiver<Result<S, ReceiverError>>);

type ClientConnection<T, S> = (ServerSender<T>, ServerReceiver<S>);

impl<T, S> Acceptor<ClientConnection<T, S>> for ClientAcceptor where T: Encodable + Send, S: Decodable + Send {
    fn accept(&mut self) -> IoResult<ClientConnection<T, S>> {
        let stream = try!(self.inner.accept());
        let (ss, sr) = mpsc::channel::<SendRequest<T>>();

        {
            let mut stream = stream.clone();
            Thread::spawn(move || {
                for val in sr.iter() {
                    super::write_item(&mut stream, val.0).unwrap();
                    // TODO: send result on val.1?
                }
            });
        }

        let (rs, rr) = mpsc::channel::<Result<S, ReceiverError>>();
        {
            let mut stream = stream.clone();
            Thread::spawn(move || {
                match stream.read_le_uint() {
                    Ok(size) => match super::read_item(&mut stream, size) {
                        Ok(val) => rs.send(Ok(val)).unwrap(),
                        Err(_) => return,
                    },
                    Err(ref e) if e.kind == IoErrorKind::TimedOut => (),
                    Err(ref e) if e.kind == IoErrorKind::EndOfFile => return,
                    Err(e) => rs.send(Err(ReceiverError::IoError(e))).unwrap(),
                }
            });
        }

        Ok((ServerSender(ss), ServerReceiver(rr)))
    }
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
              T: Encodable + Send, // outgoing
              S: Decodable + Send,     // incoming
              H: Fn(u32, S) -> T + Copy + Send,              // handle client message
              N: Fn(u32) -> () + Copy + Send,                // new client
              D: Fn(u32) -> () + Copy + Send,                // client dropped
{
    use std::io::net::tcp::TcpListener;

    let listener = try!(TcpListener::bind(addr));
    let acceptor = try!(listener.listen());
    {
        let mut acceptor = acceptor.clone();
        let mut client_counter = 0;
        let freed_clients = Arc::new(Mutex::new(RingBuf::new()));
        for conn in acceptor.incoming() {
            match conn {
                Ok(mut conn) => {
                    let client_id = match freed_clients.lock().unwrap().pop_front() {
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
                                Ok(size) => match super::read_item(&mut conn, size) {
                                    Ok(item) => item,
                                    Err(e) => panic!(e),
                                },
                                Err(ref e) if e.kind == IoErrorKind::TimedOut => {
                                    continue;
                                },
                                Err(ref e) if e.kind == IoErrorKind::EndOfFile => {
                                    freed_clients.lock().unwrap().push_back(client_id.clone());
                                    on_drop(client_id);
                                    return;
                                },
                                Err(e) => {
                                    freed_clients.lock().unwrap().push_back(client_id.clone());
                                    panic!("{}", e);
                                },
                            };
                            let resp = on_msg(client_id, item);
                            let e = encode(&resp);
                            conn.write_le_uint(e.len());
                            conn.write(e.as_bytes());
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
