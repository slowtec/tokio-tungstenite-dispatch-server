extern crate futures;
#[macro_use]
extern crate log;
extern crate tokio_core;
extern crate tokio_tungstenite;
extern crate tungstenite;

use std::net::SocketAddr;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::io::{Error, ErrorKind, Result};

use futures::stream::Stream;
use futures::{Future, Sink};
use futures::sync::mpsc;

use tokio_core::reactor::Handle;
use tokio_core::net::TcpListener;

use tokio_tungstenite::accept_async;
use tungstenite::Error as WsError;
pub use tungstenite::protocol::Message;

#[derive(Debug, Clone, PartialEq)]
pub enum ClientEvent {
    Connected(SocketAddr),
    Disconnected(SocketAddr),
    Message(SocketAddr, Message),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerEvent {
    Message(SocketAddr, Message),
    Broadcast(Message),
}

type ServerFuture = Box<Future<Item = (), Error = ()>>;
type Out = mpsc::UnboundedSender<ServerEvent>;
type In = mpsc::UnboundedReceiver<ClientEvent>;

pub fn serve(addr: &SocketAddr, handle: &Handle) -> Result<((Out, In), ServerFuture)> {
    let socket = TcpListener::bind(&addr, &handle)?;
    let handle = handle.clone();
    let connections = Rc::new(RefCell::new(HashMap::new()));

    let (in_msg_tx, in_msg_rx) = mpsc::unbounded();
    let (out_msg_tx, out_msg_rx) = mpsc::unbounded();

    let connections_inner = connections.clone();

    let srv = socket.incoming().for_each(move |(stream, addr)| {
        let connections_inner = connections_inner.clone();
        let handle_inner = handle.clone();
        let in_msg_tx = in_msg_tx.clone();
        accept_async(stream)
            .and_then(move |ws_stream| {
                debug!("New websocket client connected: {}", addr);

                in_msg_tx
                    .unbounded_send(ClientEvent::Connected(addr))
                    .map_err(|err| {
                        WsError::Io(Error::new(
                            ErrorKind::Other,
                            format!("Could not send connection event for {}: {}", addr, err),
                        ))
                    })?;

                let (tx, rx) = mpsc::unbounded();
                connections_inner.borrow_mut().insert(addr, tx);

                let (sink, stream) = ws_stream.split();

                let in_tx = in_msg_tx.clone();

                let ws_reader = stream.for_each(move |msg| {
                    in_tx
                        .unbounded_send(ClientEvent::Message(addr, msg))
                        .map_err(|err| {
                            WsError::Io(Error::new(
                                ErrorKind::Other,
                                format!("Could not receive message from {}: {}", addr, err),
                            ))
                        })
                });

                let ws_writer = rx.map_err(|_| {
                    WsError::Io(Error::new(
                        ErrorKind::Other,
                        "Could not receive server event",
                    ))
                }).fold(sink, move |sink, msg| sink.send(msg));

                let connection = ws_reader.select(ws_writer.map(|_| ()));

                handle_inner.spawn(connection.then(move |_| {
                    connections_inner.borrow_mut().remove(&addr);
                    debug!("Connection {} closed.", addr);
                    in_msg_tx
                        .unbounded_send(ClientEvent::Disconnected(addr))
                        .map_err(|err| {
                            error!("Could not send disconnection event for {}: {}", addr, err)
                        })
                }));
                Ok(())
            })
            .or_else(move |err| {
                error!("Error during the websocket handshake occurred: {}", err);
                Ok(()) // don't stop TCP server
            })
    });

    let dispatcher = out_msg_rx.for_each(move |ev| {
        match ev {
            ServerEvent::Broadcast(msg) => for (addr, mut tx) in connections.borrow().iter() {
                if let Err(err) = tx.unbounded_send(msg.clone()) {
                    error!("Could not send message to {}: {}", addr, err);
                }
            },
            ServerEvent::Message(addr, msg) => {
                if let Some(mut tx) = connections.borrow().get(&addr) {
                    if let Err(err) = tx.unbounded_send(msg) {
                        error!("Could not send message to {}: {}", addr, err);
                    }
                }
            }
        }
        Ok(())
    });

    let server = srv.map_err(|e| error!("{}", e))
        .select(dispatcher)
        .map(|_| ())
        .map_err(|_| ());

    Ok(((out_msg_tx, in_msg_rx), Box::new(server)))
}
