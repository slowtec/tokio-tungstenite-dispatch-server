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
pub use tungstenite::protocol::Message;

type ServerFuture = Box<Future<Item = (), Error = ()>>;
type Out = mpsc::UnboundedSender<(Message, SocketAddr)>;
type In = mpsc::UnboundedReceiver<ClientEvent>;

#[derive(Debug, Clone, PartialEq)]
pub enum ClientEvent {
    Connected(SocketAddr),
    Disconnected(SocketAddr),
    Message(SocketAddr, Message),
}

pub fn serve(handle: &Handle, addr: &SocketAddr) -> Result<((Out, In), ServerFuture)> {
    let socket = TcpListener::bind(&addr, &handle)?;
    let handle = handle.clone();
    let connections = Rc::new(RefCell::new(HashMap::new()));

    let (in_msg_tx, in_msg_rx) = mpsc::unbounded();
    let (out_msg_tx, out_msg_rx) = mpsc::unbounded();

    let connections_inner = connections.clone();

    let srv = socket.incoming().for_each(move |(stream, addr)| {
        let connections_inner = connections_inner.clone();
        let handle_inner = handle.clone();
        let mut in_msg_tx = in_msg_tx.clone();

        accept_async(stream)
            .and_then(move |ws_stream| {
                debug!("New websocket client connected: {}", addr);

                if let Err(err) = in_msg_tx.start_send(ClientEvent::Connected(addr)) {
                    warn!("Could not send connection event for {}: {}", addr, err);
                }

                let (tx, rx) = mpsc::unbounded();
                connections_inner.borrow_mut().insert(addr, tx);

                let (sink, stream) = ws_stream.split();

                let mut in_tx = in_msg_tx.clone();
                let ws_reader = stream.for_each(move |msg| {
                    if let Err(err) = in_tx.start_send(ClientEvent::Message(addr, msg)) {
                        error!("Could not receive message from {}: {}", addr, err);
                    }
                    Ok(())
                });

                let ws_writer = rx.fold(sink, move |mut sink, msg| {
                    if let Err(err) = sink.start_send(msg) {
                        error!("Could not send message to {}: {}", addr, err);
                    }
                    Ok(sink)
                });

                let connection = ws_reader.map_err(|_| ()).select(ws_writer.map(|_| ()));

                handle_inner.spawn(connection.then(move |_| {
                    connections_inner.borrow_mut().remove(&addr);
                    debug!("Connection {} closed.", addr);
                    if let Err(err) = in_msg_tx.start_send(ClientEvent::Disconnected(addr)) {
                        warn!("Could not send disconnection event for {}: {}", addr, err);
                    }
                    Ok(())
                }));
                Ok(())
            })
            .map_err(|e| {
                warn!("Error during the websocket handshake occurred: {}", e);
                Error::new(ErrorKind::Other, e)
            })
    });

    let dispatcher = out_msg_rx.for_each(move |(msg, addr)| {
        if let Some(mut tx) = connections.borrow().get(&addr) {
            if let Err(err) = tx.start_send(msg) {
                error!("Could not send message to {}: {}", addr, err);
            }
        }
        Ok(())
    });

    let server = srv.map_err(|_| ())
        .select(dispatcher)
        .map(|_| ())
        .map_err(|_| ());

    Ok(((out_msg_tx, in_msg_rx), Box::new(server)))
}
