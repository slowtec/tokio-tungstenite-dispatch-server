extern crate futures;
extern crate tokio_core;
extern crate tokio_tungstenite_dispatch_server;

use tokio_core::reactor::Core;
use tokio_tungstenite_dispatch_server::{serve, Message};
use std::net::SocketAddr;
use futures::stream::Stream;
use futures::Sink;
use futures::Future;

pub fn main() {
    let addr = "127.0.0.1:8080".parse().unwrap();
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let (client_tx, client_rx) = futures::sync::mpsc::unbounded::<(Message, SocketAddr)>();
    let (mut server_tx, server_rx) = futures::sync::mpsc::unbounded::<(Message, SocketAddr)>();

    let server = serve(&handle, &addr, client_tx, server_rx).unwrap();

    let receiver = client_rx.for_each(|(msg, addr)| {
        println!("received message {} from {}", msg, addr);
        server_tx.start_send((msg, addr)).unwrap();
        Ok(())
    });

    let task = server.select(receiver).map(|_| ()).map_err(|_| ());

    core.run(task).unwrap();
}
