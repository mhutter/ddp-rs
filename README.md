# ddp-rs

Client Library for [Meteor.js](https://www.meteor.com/)' [DDP protocol](https://github.com/meteor/meteor/blob/master/packages/ddp/DDP.md)


[![MIT licensed][mit-badge]][mit-url]

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/mhutter/ddp-rs/blob/main/LICENSE


### What

Communicate with servers that implement the DDP protocol

### Why

Mostly to talk to [Rocket.Chat](https://www.rocket.chat/) instances via their [Realtime API](https://developer.rocket.chat/reference/api/realtime-api).


## Project status

This project is mostly driven by my own needs, so naturally things I need were implemented first.

- [x] Connect to servers
- [x] RPC features
- [ ] Tests
- [ ] Documentation
- [ ] Communicate errors while serializing/deserializing messages back to the caller
- [ ] Reconnect on connection loss
- [ ] Data features (PubSub)


## License

[MIT](LICENSE)
