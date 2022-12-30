//! This module implements a client to talk to a remote via the [DDP protocol](https://github.com/meteor/meteor/blob/master/packages/ddp/DDP.md)

// TODO: missing_docs
#![deny(
    missing_debug_implementations,
    missing_copy_implementations,
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
use serde_json::Value;
use thiserror::Error;
use tokio::{
    net::TcpStream,
    sync::{
        mpsc::{self, Sender},
        oneshot,
    },
    task::JoinHandle,
};

use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub use types::*;
use uuid::Uuid;

mod types;
mod worker;

type Websocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type Callback = oneshot::Sender<MessageResult>;

pub const PROTOCOL_VERSION: &str = "1";

/// DDP Client
pub struct Connection {
    // communication channels
    sink: Sender<String>,

    // stuff
    session: String,
    callbacks: Arc<Mutex<HashMap<String, Callback>>>,

    // JoinHandles
    sender: JoinHandle<()>,
    receiver: JoinHandle<()>,
    handler: JoinHandle<()>,
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ddp")
            .field("session", &self.session)
            .finish()
    }
}

impl Connection {
    /// Establish a DDP connection via the given WebSocket connection
    /// TODO: move this to `ddp::connect`
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
        let conn = serde_json::to_string(&OutMessage::connect(PROTOCOL_VERSION, None))?;
        sink.send(conn).await?;

        let msg = stream.recv().await.expect("response");
        let res = serde_json::from_str::<InMessage>(&msg)?;
        let session = match res {
            InMessage::Connected { session } => Ok(session),
            InMessage::Failed { version } => Err(Error::IncompatibleVersion(version)),
            _ => Err(Error::InvalidConnectMessage(res)),
        }?;

        // Set up handling of messages
        let callbacks = Arc::new(Mutex::new(HashMap::new()));
        let handler = {
            let sink = sink.clone();
            let callbacks = callbacks.clone();
            tokio::spawn(async { worker::handler(stream, sink, callbacks).await })
        };

        Ok(Self {
            sink,
            session,
            callbacks,
            sender,
            receiver,
            handler,
        })
    }

    /// Execute a remote procedure call, and return the result.
    pub async fn call(
        &self,
        method: impl ToString,
        params: Option<Value>,
    ) -> Result<Option<Value>, Error> {
        let id = Uuid::new_v4();
        let message = OutMessage::method(method, id, params);
        let cb = self.register_callback(id);

        self.sink.send(serde_json::to_string(&message)?).await?;

        match cb.await? {
            MessageResult::Result { result, .. } => Ok(result),
            MessageResult::Error { error, .. } => Err(Error::Call(error)),
        }
    }

    /// Block on the worker threads spawned by the client
    pub async fn run(self) {
        tokio::select! {
            _ = self.sender => info!("Sender exited"),
            _ = self.receiver => info!("Receiver exited"),
            _ = self.handler => info!("Handler exited"),
        }
    }

    fn register_callback(&self, id: impl ToString) -> oneshot::Receiver<MessageResult> {
        let (tx, rx) = oneshot::channel();
        self.callbacks
            .lock()
            .expect("get lock on callbacks")
            .insert(id.to_string(), tx);

        rx
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Server version incompatible, we offered {PROTOCOL_VERSION}, it offered {0}")]
    IncompatibleVersion(String),
    #[error("Invalid message during connection: {0:?}")]
    InvalidConnectMessage(InMessage),

    #[error("Error from Websocket: {0}")]
    Tungstenite(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("Failed to convert JSON: {0}")]
    JSON(#[from] serde_json::Error),

    #[error("Failed writing to MPSC channel: {0}")]
    MpscSend(#[from] tokio::sync::mpsc::error::SendError<String>),
    #[error("Failed reading from oneshot channel: {0}")]
    OneshotRecv(#[from] tokio::sync::oneshot::error::RecvError),

    #[error("Got an error response from an RPC: {0}")]
    Call(String),
}