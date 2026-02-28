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

/// In-process [`Connection`] backed by Tokio unbounded MPSC channels.
///
/// Uses **unbounded** channels so that [`Replicator::sync`] never deadlocks
/// when both peers push concurrently (no buffer limit to fill).
///
/// Create a connected pair with [`MemConnection::pair`] for testing
/// peer-to-peer protocol interactions without network I/O.
pub struct MemConnection {
    peer_id: PeerId,
    tx: mpsc::UnboundedSender<MessagePayload>,
    rx: mpsc::UnboundedReceiver<MessagePayload>,
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
        let (tx_a, rx_a) = mpsc::unbounded_channel();
        let (tx_b, rx_b) = mpsc::unbounded_channel();
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
            .context("MemConnection: send failed — receiver dropped")
    }

    async fn receive(&mut self) -> Result<Option<MessagePayload>> {
        Ok(self.rx.recv().await)
    }

    async fn close(&mut self) -> Result<()> {
        // Replacing the sender with a new disconnected one signals EOF to the
        // remote receiver without closing the receive side (which would prevent
        // the local peer from reading remaining in-flight messages).
        let (tx, _rx) = mpsc::unbounded_channel();
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
    ///
    /// # Phase 1 note
    ///
    /// This method stamps **all fields** with the current time (`now_ms()`) as
    /// their HAM state timestamp.  This is correct for a first-time write but
    /// will "refresh" all field timestamps on every subsequent replication,
    /// effectively resetting per-field conflict history.  When preserving the
    /// prior HAM state matters (e.g., incremental updates), use
    /// [`encode_gun_node`][Self::encode_gun_node] and build the [`GunNode`]
    /// with per-field timestamps from the local store.
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
        self.encode_gun_node(soul, node)
    }

    /// Encode a pre-built [`GunNode`] as a GUN PUT wire message.
    ///
    /// Use this when you need to preserve per-field HAM state timestamps
    /// from the local store (e.g., for incremental / delta replication).
    pub fn encode_gun_node(&self, soul: &str, node: GunNode) -> Result<Vec<u8>> {
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

    /// Send a slice of `(soul, data)` pairs to the remote peer as GUN PUT
    /// messages **without** closing the connection.
    ///
    /// Use this when the connection should remain open for further reads or
    /// writes after sending (e.g., bidirectional streaming).  Call
    /// [`push_all`][Self::push_all] to send and then close in one step.
    ///
    /// # Error handling
    ///
    /// If `send_all` returns an error partway through the slice (e.g., the
    /// remote dropped the connection), the connection is in an indeterminate
    /// state and callers should treat it as unusable and drop it.
    pub async fn send_all<C: Connection>(
        &self,
        conn: &mut C,
        nodes: &[(Soul, JsonValue)],
    ) -> Result<()> {
        for (soul, data) in nodes {
            let payload = self.encode_put(soul, data.clone())?;
            conn.send(&payload).await?;
        }
        Ok(())
    }

    /// Send a slice of `(soul, data)` pairs to the remote peer as GUN PUT
    /// messages and **close** the connection when done.
    ///
    /// Closing signals EOF to the remote receiver so it knows the stream is
    /// complete.  Use [`send_all`][Self::send_all] if the connection should
    /// remain open.
    pub async fn push_all<C: Connection>(
        &self,
        conn: &mut C,
        nodes: &[(Soul, JsonValue)],
    ) -> Result<()> {
        self.send_all(conn, nodes).await?;
        conn.close().await
    }

    /// Receive all incoming GUN PUT messages until the connection closes.
    ///
    /// Returns a list of [`GunNode`] values (including their HAM per-field
    /// state) extracted from received PUT messages, keyed by soul.  GET and
    /// ACK messages are silently ignored.
    ///
    /// Preserving the full [`GunNode`] (rather than just `fields`) lets
    /// callers perform correct HAM merges and maintain conflict-resolution
    /// state downstream.
    pub async fn receive_all<C: Connection>(
        &self,
        conn: &mut C,
    ) -> Result<Vec<(Soul, GunNode)>> {
        let mut received: Vec<(Soul, GunNode)> = Vec::new();
        loop {
            match conn.receive().await? {
                None => break,
                Some(raw) => {
                    let msg = GunMessage::decode(&raw)?;
                    if let GunMessage::Put(put) = msg {
                        for (soul, node) in put.put {
                            received.push((soul, node));
                        }
                    }
                }
            }
        }
        Ok(received)
    }

    /// Run a full push-pull sync cycle with a remote peer.
    ///
    /// Each side sends its local nodes to the remote via GUN PUT messages,
    /// then closes the write direction and reads until the remote does the
    /// same.  Both peers MUST call this method concurrently (e.g. via
    /// `tokio::join!`) so that their respective sends and receives can make
    /// progress simultaneously.
    ///
    /// On bounded transports the caller is responsible for ensuring both
    /// sides run concurrently; otherwise a deadlock can occur when both
    /// sides' send buffers fill before either starts reading.
    /// [`MemConnection`] (the in-process test transport) uses an **unbounded**
    /// channel specifically to avoid this; production transports should
    /// implement appropriate backpressure or half-close mechanisms.
    ///
    /// Returns the nodes received from the remote peer as `(soul, GunNode)`
    /// pairs, preserving the full HAM per-field state for downstream merges.
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
    ) -> Result<Vec<(Soul, GunNode)>> {
        // Push local data and close the write side so the remote peer detects
        // EOF.  This must run concurrently with the remote's push (tokio::join!
        // at the call site) so that neither side blocks waiting for the other
        // to start reading.
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
    async fn test_receive_all_preserves_ham_state() {
        let (mut conn_a, mut conn_b) = MemConnection::pair("peer-a", "peer-b");
        let rep_a = Replicator::new("peer-a");
        let rep_b = Replicator::new("peer-b");

        let nodes = vec![("user:alice".to_string(), json!({"name": "Alice"}))];

        let (_, received) = tokio::join!(
            rep_a.push_all(&mut conn_a, &nodes),
            rep_b.receive_all(&mut conn_b),
        );
        let received = received.unwrap();

        let (_, node) = &received[0];
        // HAM state must be preserved alongside field data.
        assert!(node.meta.state.contains_key("name"));
        assert!(node.meta.state["name"] > 0.0);
    }

    #[tokio::test]
    async fn test_send_all_does_not_close_connection() {
        let (mut conn_a, mut conn_b) = MemConnection::pair("peer-a", "peer-b");
        let rep_a = Replicator::new("peer-a");

        let nodes = vec![("node:1".to_string(), json!({"x": 1}))];

        // send_all should NOT close the connection.
        rep_a.send_all(&mut conn_a, &nodes).await.unwrap();

        // Receive the sent message.
        let msg = conn_b.receive().await.unwrap();
        assert!(msg.is_some());

        // Connection is still open — we can send another message.
        conn_a.send(b"extra").await.unwrap();
        let extra = conn_b.receive().await.unwrap();
        assert_eq!(extra, Some(b"extra".to_vec()));
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
        assert_eq!(from_b[0].1.fields["name"], json!("Bob"));

        // peer-b should have received alice's data
        assert_eq!(from_a.len(), 1);
        assert_eq!(from_a[0].0, "user:alice");
        assert_eq!(from_a[0].1.fields["name"], json!("Alice"));
    }
}

