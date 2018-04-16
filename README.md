# tungstenite-dispatch-server

A simple websocket message dispatcher based on
[tokio](https://tokio.rs) and
[tungstenite](https://docs.rs/tungstenite/).

[![Build Status](https://travis-ci.org/slowtec/tungstenite-dispatch-server.svg?branch=master)](https://travis-ci.org/slowtec/tungstenite-dispatch-server)

## Example

```rust
let (channels, server) = serve(&handle, &addr).unwrap();
let (mut tx, rx) = channels;
let receiver = rx.for_each(|ev| {
    match ev {
        ClientEvent::Connected(addr) => {
            info!("{} connected", addr);
            tx.start_send((Message::Text("Welcome"), addr)).unwrap();
        }
        ClientEvent::Message(addr, msg) => {
            info!("received message {} from {}", msg, addr);
        }
        _ => { }
    }
    Ok(())
});
core.run(server.select(receiver).map(|_| ()).map_err(|_| ())).unwrap();
```

## License

MIT/Apache-2.0
