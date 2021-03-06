extern crate env_logger;
extern crate futures;
#[macro_use]
extern crate log;
extern crate tokio_core;
extern crate tungstenite_dispatch_server;

use futures::stream::Stream;
use futures::Future;
use futures::Sink;
use tokio_core::reactor::Core;
use tungstenite_dispatch_server::*;

pub fn main() {
    env_logger::init();
    let addr = "127.0.0.1:8080".parse().unwrap();
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let (channels, server) = serve(&addr, &handle).unwrap();

    let (mut tx, rx) = channels;

    let receiver = rx.for_each(|ev| {
        match ev {
            ClientEvent::Connected(addr) => {
                info!("{} connected", addr);
            }
            ClientEvent::Disconnected(addr) => {
                info!("{} disconnected", addr);
            }
            ClientEvent::Message(addr, msg) => {
                info!("received message {} from {}", msg, addr);
                tx.start_send(ServerEvent::Broadcast(msg)).unwrap();
            }
        }
        Ok(())
    });

    let task = server.select(receiver).map(|_| ()).map_err(|_| ());

    core.run(task).unwrap();
}
