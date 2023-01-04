# ddp-rs

Client Library for [Meteor.js](https://www.meteor.com/)' [DDP protocol](https://github.com/meteor/meteor/blob/master/packages/ddp/DDP.md)


[![MIT licensed][mit-badge]][mit-url]

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/mhutter/ddp-rs/blob/main/LICENSE


```rust
let conn = ddp::connect("wss://open.rocket.chat/websocket").await?;

let res = conn
    .call(
        "login",
        Some(json!([{ "resume": "your-personal-access-token" }])),
    )
    .await?;

println!("Login response: {res:?}");
```


## Features

* Server/Client handshake/connection establishment
* Ping/Pong
* RPC implementation
* Automatic random ID generation
* Logging via `log` crate

For what's planned next, see "Project status" below.


## Crate Features

In order to support TLS connections, one of the following features must be enabled:

* `native-tls`
* `native-tls-vendored`
* `rustls-tls-native-roots`
* `rustls-tls-webpki-roots`

This will in turn enable the respective feature in the underlying [tungstenite](https://lib.rs/crates/tungstenite) crate.


## Project status

This project is mostly driven by my own needs, so naturally things I need were implemented first.

- [x] Server/Client handshake/connection establishment
- [x] Ping/Pong
- [x] RPC implementation
- [x] Random ID generation
- [x] Logging via `log` crate
- [ ] Documentation
- [ ] Tests
- [ ] Communicate errors while serializing/deserializing messages back to the caller
- [ ] Reconnect on connection loss
- [ ] Data features (PubSub)


## License

This project is licensed unter the [MIT license](LICENSE).
