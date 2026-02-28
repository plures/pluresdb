//! Minimal GUN-protocol replication loop and in-process test transport.
//!
//! # Components
//!
//! - [`Replicator`]: encodes / decodes GUN wire messages and drives a simple
//!   push-pull sync cycle over any [`Connection`].
//! - [`MemConnection`]: paired in-process channels that implement [`Connection`],
//!   intended for unit and integration testing without real network I/O.
//!
//! # Replication flow
//!
//! ```text
//!  Peer A                          Peer B
//!    │  ──── PUT(node1) ──────────► │
//!    │  ──── PUT(node2) ──────────► │
//!    │  ──── PUT(node3) ──────────► │
//!    │  (close / drop sender)       │
//!    │                              │  applies nodes → local store
//! ```
//!
//! Both peers can push simultaneously; each side reads until the connection
//! signals end-of-stream (`receive()` returns `Ok(None)`).

use crate::gun_protocol::{GunAck, GunGet, GunMessage, GunNode, GunPut, Soul};
use crate::transport::{Connection, MessagePayload, PeerId};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// In-process paired connection for testing
// ---------------------------------------------------------------------------

/// In-process [`Connection`] backed by Tokio MPSC channels.
///
/// Create a connected pair with [`MemConnection::pair`] for testing
/// peer-to-peer protocol interactions without network I/O.
pub struct MemConnection {
    peer_id: PeerId,
    tx: mpsc::Sender<MessagePayload>,
    rx: mpsc::Receiver<MessagePayload>,
}

impl MemConnection {
    /// Create two connected [`MemConnection`] instances.
    ///
    /// `id_a` is the identity of the first peer; messages sent by the first
    /// connection are received by the second and vice-versa.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use pluresdb_sync::MemConnection;
    /// let (mut conn_a, mut conn_b) = MemConnection::pair("peer-a", "peer-b");
    /// // conn_a.send(…) → conn_b.receive()
    /// // conn_b.send(…) → conn_a.receive()
    /// ```
    pub fn pair(id_a: &str, id_b: &str) -> (MemConnection, MemConnection) {
        let (tx_a, rx_a) = mpsc::channel(256);
        let (tx_b, rx_b) = mpsc::channel(256);
        let conn_a = MemConnection {
            peer_id: id_b.to_string(),
            tx: tx_a,
            rx: rx_b,
        };
        let conn_b = MemConnection {
            peer_id: id_a.to_string(),
            tx: tx_b,
            rx: rx_a,
        };
        (conn_a, conn_b)
    }
}

#[async_trait]
impl Connection for MemConnection {
    async fn send(&mut self, data: &[u8]) -> Result<()> {
        self.tx
            .send(data.to_vec())
            .await
            .context("MemConnection: send failed — receiver dropped")
    }

    async fn receive(&mut self) -> Result<Option<MessagePayload>> {
        Ok(self.rx.recv().await)
    }

    async fn close(&mut self) -> Result<()> {
        // Dropping the sender signals EOF to the remote receiver.
        // Re-create with a closed channel so subsequent send() calls return Err.
        let (tx, _rx) = mpsc::channel(1);
        drop(std::mem::replace(&mut self.tx, tx));
        Ok(())
    }

    fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }
}

// ---------------------------------------------------------------------------
// Replicator
// ---------------------------------------------------------------------------

/// Drives GUN-protocol replication over a [`Connection`].
///
/// The [`Replicator`] is responsible for:
/// - Encoding local node data as GUN PUT messages.
/// - Sending GET requests for specific souls.
/// - Receiving and decoding incoming GUN messages.
/// - Running a full push-pull sync cycle ([`Replicator::sync`]).
///
/// The replicator is intentionally transport-agnostic: it works with any type
/// that implements [`Connection`], including [`MemConnection`] for tests and
/// the network transports in [`crate::relay`] / the future hyperswarm backend.
#[derive(Debug, Clone)]
pub struct Replicator {
    /// This peer's identifier (included in generated message IDs for
    /// easier debugging of multi-peer traces).
    peer_id: String,
}

impl Replicator {
    /// Create a new [`Replicator`] for the given peer.
    pub fn new(peer_id: impl Into<String>) -> Self {
        Self {
            peer_id: peer_id.into(),
        }
    }

    /// Generate a unique message ID prefixed with this peer's ID.
    fn new_msg_id(&self) -> String {
        format!("{}-{}", self.peer_id, Uuid::new_v4())
    }

    /// Encode a single node as a GUN PUT wire message.
    ///
    /// `data` should be a flat JSON object (`serde_json::Value::Object`).
    /// Non-object values are wrapped under the key `"value"`.
    pub fn encode_put(&self, soul: &str, data: JsonValue) -> Result<Vec<u8>> {
        let fields: HashMap<String, JsonValue> = match data {
            JsonValue::Object(map) => map.into_iter().collect(),
            other => {
                let mut m = HashMap::new();
                m.insert("value".to_string(), other);
                m
            }
        };
        let ts = crate::gun_protocol::now_ms();
        let node = GunNode::from_data(soul, fields, ts);
        let mut put_map = HashMap::new();
        put_map.insert(soul.to_string(), node);
        GunMessage::Put(GunPut {
            id: self.new_msg_id(),
            put: put_map,
        })
        .encode()
    }

    /// Encode a GUN GET request for `soul`.
    pub fn encode_get(&self, soul: &str) -> Result<Vec<u8>> {
        GunMessage::Get(GunGet {
            id: self.new_msg_id(),
            get: crate::gun_protocol::GunGetRequest {
                soul: soul.to_string(),
                field: None,
            },
        })
        .encode()
    }

    /// Encode a GUN GET request for a specific field of `soul`.
    pub fn encode_get_field(&self, soul: &str, field: &str) -> Result<Vec<u8>> {
        GunMessage::Get(GunGet {
            id: self.new_msg_id(),
            get: crate::gun_protocol::GunGetRequest {
                soul: soul.to_string(),
                field: Some(field.to_string()),
            },
        })
        .encode()
    }

    /// Encode a success ACK for `original_msg_id`.
    pub fn encode_ack(&self, original_msg_id: &str) -> Result<Vec<u8>> {
        GunMessage::Ack(GunAck {
            id: self.new_msg_id(),
            reply_to: original_msg_id.to_string(),
            err: None,
            ok: Some(1),
        })
        .encode()
    }

    /// Encode an error ACK for `original_msg_id`.
    pub fn encode_err_ack(&self, original_msg_id: &str, err: &str) -> Result<Vec<u8>> {
        GunMessage::Ack(GunAck {
            id: self.new_msg_id(),
            reply_to: original_msg_id.to_string(),
            err: Some(err.to_string()),
            ok: None,
        })
        .encode()
    }

    /// Push a slice of `(soul, data)` pairs to the remote peer as GUN PUT
    /// messages and close the connection when done.
    ///
    /// Each node is sent as a separate PUT message.  After all nodes have been
    /// sent the connection is closed so the remote side can detect EOF.
    pub async fn push_all<C: Connection>(
        &self,
        conn: &mut C,
        nodes: &[(Soul, JsonValue)],
    ) -> Result<()> {
        for (soul, data) in nodes {
            let payload = self.encode_put(soul, data.clone())?;
            conn.send(&payload).await?;
        }
        conn.close().await
    }

    /// Receive all incoming GUN PUT messages until the connection closes.
    ///
    /// Returns a list of `(soul, fields)` pairs extracted from the received
    /// PUT messages.  GET and ACK messages are silently ignored in this
    /// helper; use [`Replicator::run_loop`] for full bidirectional handling.
    pub async fn receive_all<C: Connection>(
        &self,
        conn: &mut C,
    ) -> Result<Vec<(Soul, HashMap<String, JsonValue>)>> {
        let mut received: Vec<(Soul, HashMap<String, JsonValue>)> = Vec::new();
        loop {
            match conn.receive().await? {
                None => break,
                Some(raw) => {
                    let msg = GunMessage::decode(&raw)?;
                    if let GunMessage::Put(put) = msg {
                        for (soul, node) in put.put {
                            received.push((soul, node.fields));
                        }
                    }
                }
            }
        }
        Ok(received)
    }

    /// Run a full push-pull sync cycle with a remote peer.
    ///
    /// Both peers should call this method concurrently (e.g. via
    /// `tokio::join!`).  Each side:
    /// 1. Pushes `local_nodes` to the remote via GUN PUT messages.
    /// 2. Receives all PUT messages from the remote until EOF.
    ///
    /// Returns the nodes received from the remote peer as `(soul, fields)`
    /// pairs.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use pluresdb_sync::{MemConnection, Replicator};
    /// # use serde_json::json;
    /// # #[tokio::main] async fn main() -> anyhow::Result<()> {
    /// let (mut conn_a, mut conn_b) = MemConnection::pair("a", "b");
    /// let rep_a = Replicator::new("a");
    /// let rep_b = Replicator::new("b");
    ///
    /// let nodes_a = vec![("user:alice".to_string(), json!({"name": "Alice"}))];
    /// let nodes_b = vec![("user:bob".to_string(), json!({"name": "Bob"}))];
    ///
    /// let (from_b, from_a) = tokio::join!(
    ///     rep_a.sync(&mut conn_a, &nodes_a),
    ///     rep_b.sync(&mut conn_b, &nodes_b),
    /// );
    /// # Ok(()) }
    /// ```
    pub async fn sync<C: Connection>(
        &self,
        conn: &mut C,
        local_nodes: &[(Soul, JsonValue)],
    ) -> Result<Vec<(Soul, HashMap<String, JsonValue>)>> {
        // Push local data then close the write side.
        self.push_all(conn, local_nodes).await?;
        // Read whatever the remote side has sent.
        self.receive_all(conn).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_mem_connection_send_receive() {
        let (mut conn_a, mut conn_b) = MemConnection::pair("a", "b");

        conn_a.send(b"hello").await.unwrap();
        let received = conn_b.receive().await.unwrap();
        assert_eq!(received, Some(b"hello".to_vec()));
    }

    #[tokio::test]
    async fn test_mem_connection_close_signals_eof() {
        let (mut conn_a, mut conn_b) = MemConnection::pair("a", "b");

        conn_a.close().await.unwrap();
        let received = conn_b.receive().await.unwrap();
        assert_eq!(received, None);
    }

    #[tokio::test]
    async fn test_mem_connection_peer_ids() {
        let (conn_a, conn_b) = MemConnection::pair("peer-a", "peer-b");
        assert_eq!(conn_a.peer_id(), "peer-b");
        assert_eq!(conn_b.peer_id(), "peer-a");
    }

    #[tokio::test]
    async fn test_replicator_encode_put_decode() {
        let rep = Replicator::new("test-peer");
        let data = json!({"name": "Alice", "role": "admin"});
        let bytes = rep.encode_put("user:alice", data.clone()).unwrap();
        let msg = GunMessage::decode(&bytes).unwrap();
        match msg {
            GunMessage::Put(put) => {
                let node = put.put.get("user:alice").unwrap();
                assert_eq!(node.fields["name"], json!("Alice"));
                assert_eq!(node.fields["role"], json!("admin"));
            }
            other => panic!("unexpected message: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_replicator_encode_get_decode() {
        let rep = Replicator::new("test-peer");
        let bytes = rep.encode_get("user:alice").unwrap();
        let msg = GunMessage::decode(&bytes).unwrap();
        match msg {
            GunMessage::Get(get) => {
                assert_eq!(get.get.soul, "user:alice");
                assert!(get.get.field.is_none());
            }
            other => panic!("unexpected message: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_push_all_and_receive_all() {
        let (mut conn_a, mut conn_b) = MemConnection::pair("peer-a", "peer-b");
        let rep_a = Replicator::new("peer-a");
        let rep_b = Replicator::new("peer-b");

        let nodes = vec![
            ("node:1".to_string(), json!({"value": 1})),
            ("node:2".to_string(), json!({"value": 2})),
        ];

        let push_fut = rep_a.push_all(&mut conn_a, &nodes);
        let recv_fut = rep_b.receive_all(&mut conn_b);

        let (push_result, received) = tokio::join!(push_fut, recv_fut);
        push_result.unwrap();
        let received = received.unwrap();

        assert_eq!(received.len(), 2);
        let souls: Vec<&str> = received.iter().map(|(s, _)| s.as_str()).collect();
        assert!(souls.contains(&"node:1"));
        assert!(souls.contains(&"node:2"));
    }

    #[tokio::test]
    async fn test_bidirectional_sync() {
        let (mut conn_a, mut conn_b) = MemConnection::pair("peer-a", "peer-b");
        let rep_a = Replicator::new("peer-a");
        let rep_b = Replicator::new("peer-b");

        let nodes_a = vec![
            ("user:alice".to_string(), json!({"name": "Alice", "age": 30})),
        ];
        let nodes_b = vec![
            ("user:bob".to_string(), json!({"name": "Bob", "role": "admin"})),
        ];

        let (from_b, from_a) = tokio::join!(
            rep_a.sync(&mut conn_a, &nodes_a),
            rep_b.sync(&mut conn_b, &nodes_b),
        );

        let from_b = from_b.unwrap();
        let from_a = from_a.unwrap();

        // peer-a should have received bob's data
        assert_eq!(from_b.len(), 1);
        assert_eq!(from_b[0].0, "user:bob");
        assert_eq!(from_b[0].1["name"], json!("Bob"));

        // peer-b should have received alice's data
        assert_eq!(from_a.len(), 1);
        assert_eq!(from_a[0].0, "user:alice");
        assert_eq!(from_a[0].1["name"], json!("Alice"));
    }
}
