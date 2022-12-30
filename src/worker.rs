use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use log::{error, info, trace, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    oneshot,
};
use tokio_tungstenite::tungstenite;

use super::{InMessage, MessageResult, Websocket};

type Callbacks = Arc<Mutex<HashMap<String, oneshot::Sender<MessageResult>>>>;

/// Worker task that reads messages from a queue and writes them to the websocket
pub async fn sender(
    mut socket_sink: SplitSink<Websocket, tungstenite::Message>,
    mut message_stream: Receiver<String>,
) {
    while let Some(msg) = message_stream.recv().await {
        trace!("MSG OUT {msg}");

        socket_sink
            .send(tungstenite::Message::Text(msg))
            .await
            .unwrap_or_else(|err| error!("Failed sending message: {err}"));
    }
}

/// Worker task that reads incoming messages from the websocket and writes them to a queue
pub async fn receiver(mut socket_stream: SplitStream<Websocket>, message_sink: Sender<String>) {
    while let Some(msg) = socket_stream.next().await {
        let msg = match msg {
            Ok(msg) => msg,
            Err(err) => {
                error!("Error reading message from socket: {err}");
                continue;
            }
        };

        trace!("MSG IN  {msg}");

        match msg {
            tungstenite::Message::Close(_) => {
                info!("Server closed the connection: {msg}");
                return;
            }
            tungstenite::Message::Text(msg) => handle_message(message_sink.clone(), msg).await,
            _ => {
                error!("Unhandled websocket message type: {msg:?}");
            }
        }
    }
}

async fn handle_message(sink: Sender<String>, msg: String) {
    sink.send(msg)
        .await
        .unwrap_or_else(|err| error!("Failed writing message to queueu: {err}"));
}

pub async fn handler(
    mut message_stream: Receiver<String>,
    message_sink: Sender<String>,
    callbacks: Callbacks,
) {
    while let Some(msg) = message_stream.recv().await {
        let Ok(message) = serde_json::from_str::<InMessage>(&msg) else {
            warn!("Unimplemented DDP message: {msg}");
            continue;
        };

        match message {
            InMessage::Ping { id } => handle_ping(&message_sink, id).await,
            InMessage::Pong { .. } => {} // ¯\_(ツ)_/¯
            InMessage::Result(res) => handle_result(callbacks.clone(), res).await,
            _ => warn!("Unhandled DDP message: {message:?}"),
        }
    }
}

async fn handle_ping(sink: &Sender<String>, id: Option<String>) {
    sink.send(serde_json::to_string(&InMessage::Pong { id }).expect("serialize pong"))
        .await
        .unwrap_or_else(|err| error!("Failed sending pong: {err}"));
}

async fn handle_result(callbacks: Callbacks, result: MessageResult) {
    let id = match &result {
        MessageResult::Result { id, .. } => id,
        MessageResult::Error { id, .. } => id,
    };

    let cb = {
        // grab the cb in a block to minimize Mutex lock time on callbacks
        callbacks
            .lock()
            .expect("obtain lock on callbacks")
            .remove(id)
            .expect("find callback")
    };

    cb.send(result).expect("write callback");
}

#[derive(Debug, Serialize, Deserialize)]
struct RecvMessage {
    msg: String,
    id: Option<String>,
}
