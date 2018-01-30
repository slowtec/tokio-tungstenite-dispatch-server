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
use futures::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use tokio_core::reactor::Handle;
use tokio_core::net::TcpListener;

use tokio_tungstenite::accept_async;
pub use tungstenite::protocol::Message;

pub fn serve(
    handle: &Handle,
    addr: &SocketAddr,
    in_msg_tx: UnboundedSender<(Message, SocketAddr)>,
    out_msg_rx: UnboundedReceiver<(Message, SocketAddr)>,
) -> Result<Box<Future<Item = (), Error = ()>>> {
    let socket = TcpListener::bind(&addr, &handle)?;
    let handle = handle.clone();
    let connections = Rc::new(RefCell::new(HashMap::new()));

    let connections_inner = connections.clone();

    let srv = socket.incoming().for_each(move |(stream, addr)| {
        let connections_inner = connections_inner.clone();
        let handle_inner = handle.clone();
        let mut in_msg_tx = in_msg_tx.clone();

        accept_async(stream)
            .and_then(move |ws_stream| {
                debug!("New websocket client connected: {}", addr);

                let (tx, rx) = futures::sync::mpsc::unbounded::<Message>();
                connections_inner.borrow_mut().insert(addr, tx);

                let (sink, stream) = ws_stream.split();

                let ws_reader = stream.for_each(move |msg: Message| {
                    if let Err(err) = in_msg_tx.start_send((msg, addr)) {
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

    Ok(Box::new(server))
}
