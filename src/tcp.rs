//! Module `tcp` provides support for channels that communicate
//! over TCP.

use serialize::{Decodable, Encodable};
use serialize::json::{Decoder, DecoderError, decode, Encoder};
use std::comm;
use std::io::{IoError, IoErrorKind, IoResult, TcpStream};
use std::io::net::ip::ToSocketAddr;
use std::sync::{Arc, Future, Mutex};
use std::task;
use super::{SendRequest, ReceiverError};

#[deriving(Clone)]
pub struct ClientSender<T: Encodable<Encoder<'static>, IoError> + Send>(comm::Sender<SendRequest<T>>);

impl<T> super::Sender<T> for ClientSender<T> where T: Encodable<Encoder<'static>, IoError> + Send {
    fn send(&mut self, t: T) -> Future<IoResult<()>> {
        let (fi, fo) = comm::channel();
        self.0.send((t, fi));
        // Future::from_receiver() doesn't work here. It causes the `fi` Sender to close before it
        // gets a chance to send the response. For some reason though, spawning it into a new
        // proc works.
        Future::spawn(move || { fo.recv() })
    }
}

pub struct ClientReceiver<S: Decodable<Decoder, DecoderError> + Send>(comm::Receiver<Result<S, ReceiverError>>);

impl<S> super::Receiver<S> for ClientReceiver<S> where S: Decodable<Decoder, DecoderError> + Send {
    fn try_recv(&mut self) -> Result<S, ReceiverError> {
        self.0.recv()
    }
}

#[allow(unused_must_use)]
pub fn client_channel<A: ToSocketAddr, T: Encodable<Encoder<'static>, IoError> + Send, S: Decodable<Decoder, DecoderError> + Send>(addr: A) -> IoResult<(ClientSender<T>, ClientReceiver<S>)> {
    let stream = Arc::new(Mutex::new(try!(TcpStream::connect(addr))));
    let (ss, sr) = comm::channel::<SendRequest<T>>();
    {
        let stream = stream.clone();
        spawn(move || {
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
        spawn(move || {
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
                task::deschedule();
            }
        });
    }
    Ok((ClientSender(ss), ClientReceiver(rr)))
}

#[allow(unused_must_use)]
pub fn server_channel<A: ToSocketAddr, T: Encodable<Encoder<'static>, IoError> + Send, S: Decodable<Decoder, DecoderError> + Send, F: Fn(S) -> T + Copy + Send>(addr: A, handle: F) -> IoResult<()> {
    use std::io::{Acceptor, Listener};
    use std::io::net::tcp::TcpListener;

    let listener = try!(TcpListener::bind(addr));
    let acceptor = try!(listener.listen());
    {
        let mut acceptor = acceptor.clone();
        for conn in acceptor.incoming() {
            match conn {
                Ok(mut conn) => {
                    spawn(move || {
                        loop {
                            let item = match conn.read_le_uint() {
                                Ok(size) => match read_item(&mut conn, size) {
                                    Ok(item) => item,
                                    Err(e) => panic!(e),
                                },
                                Err(ref e) if e.kind == IoErrorKind::TimedOut => continue, // ignore
                                Err(ref e) if e.kind == IoErrorKind::EndOfFile => return,
                                Err(e) => panic!("{}", e),
                            };
                            let resp = handle(item); // TODO: add a client id
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

fn read_item<S: Decodable<Decoder, DecoderError> + Send>(stream: &mut TcpStream, size: uint) -> Result<S, ReceiverError> {
    let data = try!(stream.read_exact(size));
    let string = try!(String::from_utf8(data));
    Ok(try!(decode::<S>(string.as_slice())))
}
