use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Response to a connection attempt by a client
#[derive(Debug, Deserialize)]
#[serde(tag = "msg")]
pub enum ConnectResponse {
    #[serde(rename = "connected")]
    Connected { session: String },
    #[serde(rename = "failed")]
    Failed { version: String },
}

/// A message that is sent from the server to the client
///
/// Heartbeat messages can go either way so they show up here and in [OutMessage].
///
/// [Upstream documentation](https://github.com/meteor/meteor/blob/master/packages/ddp/DDP.md)
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "msg")]
pub enum InMessage {
    /// A DDP connection has been established
    #[serde(rename = "connected")]
    Connected { session: String },
    /// Establishing a DDP connection failed (because of version mismatch)
    #[serde(rename = "failed")]
    Failed { version: String },

    // Heartbeats
    #[serde(rename = "ping")]
    Ping {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    #[serde(rename = "pong")]
    Pong {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },

    // TODO: Managing Data:
    #[serde(rename = "added")]
    Added {
        collection: String,
        id: String,
        fields: Option<Value>,
    },

    // RPC
    #[serde(rename = "result")]
    Result(MessageResult),
    #[serde(rename = "updated")]
    Updated { methods: Vec<String> },
}

/// RPC result message variants
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageResult {
    Error { id: String, error: String },
    Result { id: String, result: Option<Value> },
}

/// A message passed from the client to the server
///
/// Heartbeat messages can go either way so they show up here and in [InMessage].
///
/// [Upstream documentation](https://github.com/meteor/meteor/blob/master/packages/ddp/DDP.md)
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "msg")]
pub enum OutMessage {
    // Establishing a DDP Connection
    #[serde(rename = "connect")]
    Connect {
        #[serde(skip_serializing_if = "Option::is_none")]
        session: Option<String>,
        version: String,
        support: Vec<String>,
    },

    // Heartbeats
    #[serde(rename = "ping")]
    Ping {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    #[serde(rename = "pong")]
    Pong {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },

    // TODO: Managing Data:

    // RPC
    #[serde(rename = "method")]
    Method {
        method: String,
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<Value>,
    },
}

impl OutMessage {
    /// Construct a "connect" message with the given parameters
    pub fn connect(version: impl ToString, session: Option<String>) -> Self {
        let version = version.to_string();

        Self::Connect {
            support: vec![version.clone()],
            version,
            session,
        }
    }
    /// Construct a "method" message with the given parameters
    pub fn method(method: impl ToString, id: impl ToString, params: Option<Value>) -> Self {
        Self::Method {
            method: method.to_string(),
            id: id.to_string(),
            params,
        }
    }
}
