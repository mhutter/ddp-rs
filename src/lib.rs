//! DDP client library via WebSockets.
//!
//! This library implements [Meteor](https://www.meteor.com/)'s [DDP protocol](https://github.com/meteor/meteor/blob/master/packages/ddp/DDP.md).
//!
//! # Getting started
//!
//! To get started, just add this library to your dependencies:
//!
//! ```sh
//! cargo add ddp
//! ```
//!
//! If you plan to (and I hope you do) connect via TLS (`wss://...`), also enable one of the
//! following features. They will in turn enable TLS support in the underlying
//! [tungstenite](https://https://github.com/snapview/tungstenite-rs) crate.
//!
//! * `native-tls`
//! * `native-tls-vendored`
//! * `rustls-tls-native-roots`
//! * `rustls-tls-webpki-roots`
//!
//! You probably also want to add either `serde` or `serde_json` to provide parameters for remote
//! procedure calls (see the example below).
//!
//! Once you added `ddp` to your dependencies, use [`connect`] to establish both the underlying
//! WebSocket connection and the DDP session on top (use [`Session::new`] if you already have a
//! Tungstenite websocket).
//!
//! ```no_run
//! # async {
//! let conn = ddp::connect("wss://example.com/websocket").await?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # };
//! ```
//!
//! ## Remote Procedure Calls
//!
//! Once the connection is established, use [`Session::call`] to execute remote procedure calls
//! and wait for the response.
//!
//! The params can be anything that implements [`serde::Serialize`], so you could use the
//! [`serde_json::json!()`] macro to construct ad-hoc params:
//!
//! ```no_run
//! use serde_json::json;
//!
//! // ...
//!
//! # async {
//! # let conn = ddp::connect("wss://example.com/websocket").await?;
//! conn.call("some-method", json!({ "some-param": 42 })).await?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # };
//! ```
//!
//! ## Logging/Debugging
//!
//! This library makes use of the [`log`](https://lib.rs/crates/log) facade. You can use a library
//! like [`env_logger`](https://lib.rs/crates/env_logger) to log to the console.
//!
//! **Warning** on the highest logging level (`trace`), all messages will be logged in plain text!
//! This includes any sensitive information such as passwords, tokens, etc.
//!
//! # Example
//!
//! ```no_run
//! use serde_json::json;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let conn = ddp::connect("wss://example.com/websocket").await?;
//!     let res = conn.call("some-method", json!({ "id": 42 })).await?;
//!     println!("Response to some-method: {res:?}");
//!
//!     // implement all your magic here!
//!
//!     // block as long as there is still a connection established. As soon as any of the worker
//!     // tasks exits, `select` will return.
//!     conn.select().await;
//!
//!     Ok(())
//! }
//! ```

#![deny(
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures_util::StreamExt;
use log::{error, info};
use message::{Connected, Failed, Message, RpcError, RpcResult};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;
use tokio::{
    net::TcpStream,
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    task::JoinHandle,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

pub mod message;
mod worker;

// Helper type aliases
type Websocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Callback = oneshot::Sender<RpcResult>;

/// The DDP protocol version used by the client
pub const PROTOCOL_VERSION: &str = "1";

/// A DDP connection
pub struct Session {
    // communication channels
    sink: Sender<String>,

    // stuff
    id: String,
    callbacks: Arc<Mutex<HashMap<String, Callback>>>,

    // JoinHandles
    sender: JoinHandle<()>,
    receiver: JoinHandle<()>,
    handler: JoinHandle<()>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ddp").field("session", &self.id).finish()
    }
}

/// Establish a WebSocket connection to `url`, and then a DDP connection.
pub async fn connect(url: &str) -> Result<Session, Error> {
    let (ws, _) = tokio_tungstenite::connect_async(url).await?;
    Session::new(ws).await
}

impl Session {
    /// Establish a DDP connection via the given WebSocket connection
    pub async fn new(ws: Websocket) -> Result<Self, Error> {
        // split websocket into read & write parts
        let (ws_sink, ws_stream) = ws.split();

        // allocate communication channels
        let (in_sink, mut stream) = mpsc::channel::<String>(100);
        let (sink, out_stream) = mpsc::channel::<String>(100);

        // spawn worker threads
        // NOTE: wrapping them in a block is required, otherwise the spawned threads will not exit
        // when the function returns.
        let sender = tokio::spawn(async move { worker::sender(ws_sink, out_stream).await });
        let receiver = tokio::spawn(async move { worker::receiver(ws_stream, in_sink).await });

        // establish DDP connection
        let conn = serde_json::to_string(&Message::connect(PROTOCOL_VERSION, None))?;
        sink.send(conn).await?;

        let session = await_connect_response(&mut stream).await?;

        // Set up handling of messages
        let callbacks = Arc::new(Mutex::new(HashMap::new()));
        let handler = {
            let sink = sink.clone();
            let callbacks = callbacks.clone();
            tokio::spawn(async { worker::handler(stream, sink, callbacks).await })
        };

        Ok(Self {
            sink,
            id: session,
            callbacks,
            sender,
            receiver,
            handler,
        })
    }

    /// Execute a remote procedure call, and return the result.
    ///
    /// # Arguments
    ///
    /// * `method` - the RPC method to be called
    /// * `params` - parameters to the method to be called
    ///
    /// # Return values
    ///
    /// This method returns a nested `Result`:
    ///
    /// * The OUTER `Result` describes whether or not the message could be serialized and sent to
    /// the server, and the response could be retreived and deserialized. Its error variants come
    /// from Tungstenite, Tokio and Serde.
    /// * The INNER `Result` describes the **outcome of the call** on the server.
    ///
    /// If the method you're calling does not accept any parameters, just use `Value::Null` as
    /// `params`.
    ///
    /// # Errors
    ///
    /// TODO: document or simplify
    pub async fn call<P: Serialize>(
        &self,
        method: &str,
        params: P,
    ) -> Result<Result<Value, RpcError>, Error> {
        let id = Uuid::new_v4();
        let message = Message::method(method, id.to_string(), params)?;
        let cb = self.register_callback(id);

        self.sink.send(serde_json::to_string(&message)?).await?;

        let result = cb.await?;
        if let Some(err) = result.error {
            Ok(Err(err))
        } else {
            Ok(Ok(result.result.unwrap_or_default()))
        }
    }

    /// Block on the worker threads spawned by the client, until the first one returns.
    pub async fn select(self) {
        tokio::select! {
            _ = self.sender => info!("Sender exited"),
            _ = self.receiver => info!("Receiver exited"),
            _ = self.handler => info!("Handler exited"),
        }
    }

    fn register_callback(&self, id: impl ToString) -> oneshot::Receiver<RpcResult> {
        let (tx, rx) = oneshot::channel();
        self.callbacks
            .lock()
            .expect("get lock on callbacks")
            .insert(id.to_string(), tx);

        rx
    }
}

/// Possible errors that might occur when using this library
#[derive(Debug, Error)]
pub enum Error {
    /// Connection establishmet with the server failed due to a protocol version mismatch
    ///
    /// See [message::Failed].
    #[error("Server version incompatible, we offered {PROTOCOL_VERSION}, it offered {0}")]
    IncompatibleVersion(String),
    /// The server responded with something else than a `connected` or `failed` message during
    /// session establishment
    #[error("Invalid message during connection: {0:?}")]
    InvalidConnectMessage(Message),
    /// Connection establishment failed for some other reason
    #[error("Failed to establish DDP session: {0}")]
    ConnectTimeout(&'static str),

    /// Communication failed on the WebSocket level
    #[error("Error from Websocket: {0}")]
    Tungstenite(#[from] tokio_tungstenite::tungstenite::Error),

    /// A value could not be encoded or decoded to/from JSON
    #[error("Failed to convert JSON: {0}")]
    JSON(#[from] serde_json::Error),

    /// A message could not be written to an [MPSC channel](https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html)
    #[error("Failed writing to MPSC channel: {0}")]
    MpscSend(#[from] tokio::sync::mpsc::error::SendError<String>),
    /// Reading from a [oneshot channel](https://docs.rs/tokio/latest/tokio/sync/oneshot/index.html) failed
    #[error("Failed reading from oneshot channel: {0}")]
    OneshotRecv(#[from] tokio::sync::oneshot::error::RecvError),

    /// The RPC succeeded, but the server responded with an error.
    #[error("Got an error response from an RPC: {0}")]
    Call(String),
}

/// Awaits the first `connected` or `failed` message on the stream
async fn await_connect_response(stream: &mut Receiver<String>) -> Result<String, Error> {
    for _ in 0..3 {
        let msg = stream.recv().await.expect("response");

        if let Ok(res) = serde_json::from_str::<Message>(&msg) {
            return match res {
                Message::Connected(Connected { session }) => Ok(session),
                Message::Failed(Failed { version }) => Err(Error::IncompatibleVersion(version)),
                _ => continue,
            };
        }
    }

    Err(Error::ConnectTimeout(
        "Server did not send a connect or failed message",
    ))
}
