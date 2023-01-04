//! This module represents all the different messages that can be passed between a DDP client and
//! server.
//!
//! In general, all DDP messages look like this:
//!
//! ```text
//! {
//!   "msg": "<type of message>",
//!   <fields specific to this type>
//! }
//! ```
//!
//! To implement this, all messages are wrapped by the [`Message`] enum.
//!
//! [Upstream documentation](https://github.com/meteor/meteor/blob/master/packages/ddp/DDP.md)

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The DDP message type
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "msg")]
pub enum Message {
    // Establishing a DDP connection
    /// See [`Connect`]
    Connect(Connect),
    /// See [`Connected`]
    Connected(Connected),
    /// See [`Failed`]
    Failed(Failed),

    // Heartbeats
    /// See [`Ping`]
    Ping(Ping),
    /// See [`Pong`]
    Pong(Pong),

    // Data management
    // TODO: Sub,
    // TODO: Unsub,
    // TODO: Nosub,
    // TODO: Added(Added),
    // TODO: Changed,
    // TODO: Removed,
    // TODO: Ready,
    // TODO: AddedBefore,
    // TODO: MovedBefore,

    // RPC
    /// See [`Method`]
    Method(Method),
    /// See [`RpcResult`]
    Result(RpcResult),
    // TODO: Updated(Updated),
}

impl Message {
    /// Return a `connect` message with the given session ID
    pub fn connect(version: &str, session: Option<String>) -> Self {
        let version = version.to_string();
        Self::Connect(Connect {
            session,
            support: vec![version.clone()],
            version,
        })
    }
    /// Returns a `ping` message
    pub fn ping(id: Option<String>) -> Self {
        Self::Ping(Ping { id })
    }

    /// Returns a `pong` message
    pub fn pong(id: Option<String>) -> Self {
        Self::Pong(Pong { id })
    }

    /// Return a `method` message, ready for sending. `params` can be anything that can be
    /// serialized by `serde_json`.
    ///
    /// Hint: `serde_json::Value` implements `Serialize`, so you can use the `json!()` macro to
    /// create ad-hoc data structures.
    ///
    /// # Example
    ///
    /// ```
    /// use ddp::message::Message;
    /// use serde_json::json;
    /// use uuid::Uuid;
    ///
    /// let id = "random-id".to_string();
    /// let params = json!({ "param": "foo" });
    ///
    /// let method = Message::method("some-method", id, Some(params))?;
    ///
    /// assert_eq!(
    ///     r#"{"msg":"method","method":"some-method","id":"random-id","params":{"param":"foo"}}"#,
    ///     serde_json::to_string(&method)?,
    /// );
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if `params` fails to serialize into a `serde_json::Value`.
    pub fn method<P: Serialize>(
        method: &str,
        id: String,
        params: P,
    ) -> Result<Self, serde_json::Error> {
        let params = serde_json::to_value(params)?;

        Ok(Self::Method(Method {
            method: method.to_string(),
            id,
            params,
            random_seed: None,
        }))
    }
}

/// The client would like to establish a connection
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Connect {
    /// Existing DDP session (for reconnects)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Proposed protocol version
    pub version: String,
    /// Supported protocol versions
    pub support: Vec<String>,
}

impl Connect {
    /// Construct a new `connect` message without a session set
    pub fn new(version: &str) -> Self {
        let version = version.to_string();
        Self {
            session: None,
            support: vec![version.clone()],
            version,
        }
    }
}

/// A DDP connection has been established
#[derive(Debug, Serialize, Deserialize)]
pub struct Connected {
    /// Session ID as returned by the server
    ///
    /// Can be used to reestablish a DDP connection after a transport conneciton loss.
    pub session: String,
}

/// Establishing a DDP connection failed (because of version mismatch)
#[derive(Debug, Serialize, Deserialize)]
pub struct Failed {
    /// Protocol version the server is proposing instead of what we offered
    pub version: String,
}

/// Incoming heartbeat
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Ping {
    /// Optional ID of the ping
    ///
    /// Must be included in the response if set
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl Ping {
    /// Returns an appropriate [`Pong`] message
    pub fn pong(self) -> Pong {
        Pong { id: self.id }
    }
}

/// Outgoing heartbeat
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Pong {
    /// Optional ID of the ping
    ///
    /// See [Ping::id]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// A remote method call
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Method {
    /// Method to be called
    method: String,
    /// Arbitrary client-determined identifier for this call
    ///
    /// Used to correlate returned data messages.
    id: String,
    /// Parameters to the method
    #[serde(skip_serializing_if = "Value::is_null")]
    params: Value,
    /// Optional client-determined seed for pseudo-random generators
    #[serde(skip_serializing_if = "Option::is_none")]
    random_seed: Option<Value>,
}

// TODO: combine those two structs... make `call` return `Result<Value, Error>`

/// The successful result of a method call
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcResult {
    /// ID of the corresponding method call
    pub id: String,
    /// Return value of the method called
    pub result: Option<Value>,
    /// The error thrown by the method (or `method-not-found`)
    pub error: Option<RpcError>,
}

/// An error that represents errors raised by an RPC call or a subscription
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcError {
    /// Error identifier
    pub error: String,
    /// Pre-defined by the DDP protocol, always `Meteor.Error`
    pub error_type: Option<String>,
    /// Optional description of the error
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn ping_serialize() {
        assert_eq!(
            r#"{"msg":"ping"}"#,
            serde_json::to_string(&Message::ping(None)).unwrap(),
        );

        let id = Uuid::new_v4().to_string();
        assert_eq!(
            format!(r#"{{"msg":"ping","id":"{id}"}}"#),
            serde_json::to_string(&Message::ping(Some(id.clone()))).unwrap(),
        );
    }

    #[test]
    fn ping_deserialize() {
        let json = r#"{"msg":"ping"}"#;
        let msg = serde_json::from_str::<Message>(json).unwrap();
        if let Message::Ping(ping) = msg {
            assert!(ping.id.is_none());
        } else {
            panic!("Expected Ping, got {msg:?}");
        }

        let id = Uuid::new_v4().to_string();
        let json = format!(r#"{{"msg":"ping","id":"{id}"}}"#);
        let msg = serde_json::from_str::<Message>(&json).unwrap();
        if let Message::Ping(ping) = msg {
            assert_eq!(ping.id, Some(id));
        } else {
            panic!("Expected Ping, got {msg:?}");
        }
    }

    #[test]
    fn message_method() {
        let method = random_string();
        let id = random_string();

        let msg = Message::method(&method, id.clone(), Value::Null).unwrap();
        let Message::Method(m) = msg else {
            panic!("Expected Method, got {msg:?}");
        };

        assert_eq!(m.method, method);
        assert_eq!(m.id, id);

        let method = random_string();
        let id = random_string();
        let param = random_string();

        let msg = Message::method(&method, id.clone(), Some(json!({ "param": param }))).unwrap();
        let Message::Method(m) = msg else {
            panic!("Expected Method, got {msg:?}");
        };

        assert_eq!(m.method, method);
        assert_eq!(m.id, id);
        assert_eq!(m.params, json!({ "param": param }));
    }

    fn random_string() -> String {
        Uuid::new_v4().to_string()
    }
}
