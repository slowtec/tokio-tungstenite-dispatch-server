extern crate futures;
extern crate tokio_core;
extern crate tokio_tungstenite_dispatch_server;

use tokio_core::reactor::Core;
use tokio_tungstenite_dispatch_server::serve;
use futures::stream::Stream;
use futures::Sink;
use futures::Future;

pub fn main() {
    let addr = "127.0.0.1:8080".parse().unwrap();
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let (channels, server) = serve(&handle, &addr).unwrap();

    let (mut tx, rx) = channels;

    let receiver = rx.for_each(|(msg, addr)| {
        println!("received message {} from {}", msg, addr);
        tx.start_send((msg, addr)).unwrap();
        Ok(())
    });

    let task = server.select(receiver).map(|_| ()).map_err(|_| ());

    core.run(task).unwrap();
}
