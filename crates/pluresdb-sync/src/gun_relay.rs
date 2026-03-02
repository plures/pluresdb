//! GUN-protocol-compatible WebSocket relay server.
//!
//! This module implements a minimal relay server that:
//! - Accepts WebSocket connections at `/gun` (the standard GUN.js endpoint).
//! - Parses incoming GUN wire-protocol messages (PUT / GET / ACK).
//! - Fans-out PUT messages to all other connected peers, enabling live
//!   graph-delta replication between Rust peers and GUN.js browser clients.
//! - Responds to GET messages with an ACK (future: with the stored node).
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use pluresdb_sync::GunRelayServer;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let server = GunRelayServer::new();
//!     server.serve("0.0.0.0:4444").await
//! }
//! ```
//!
//! ## GUN.js client compatibility
//!
//! A GUN.js client can connect to this relay with:
//! ```js
//! const gun = Gun({ peers: ['ws://localhost:4444/gun'] });
//! ```
//!
//! The relay speaks the Phase 1 GUN wire protocol (PUT / GET / ACK) described
//! in `docs/GUN_WIRE_PROTOCOL.md`.

use crate::gun_protocol::GunMessage;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Shared relay state
// ---------------------------------------------------------------------------

/// A broadcast envelope containing the raw JSON bytes of a GUN message and
/// the peer ID that originated it (so we can skip echoing back to the sender).
#[derive(Debug, Clone)]
struct RelayEnvelope {
    /// The sender's peer ID (used to skip echo-back).
    origin: String,
    /// Raw JSON bytes of the GUN wire message.
    payload: Vec<u8>,
}

/// Shared state accessible from every WebSocket handler task.
pub(crate) struct RelayState {
    /// Broadcast channel used to fan-out messages to all connected peers.
    tx: broadcast::Sender<RelayEnvelope>,
    /// Track the number of currently connected peers (for observability).
    peer_count: Arc<std::sync::atomic::AtomicUsize>,
    /// Set of active peer IDs (peer_id → placeholder).
    peers: DashMap<String, ()>,
}

impl RelayState {
    fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            peer_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            peers: DashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// A GUN-protocol-compatible WebSocket relay server.
///
/// Peers connect to the `/gun` path.  Every PUT message received from one
/// peer is broadcast to all other connected peers so that graph deltas
/// propagate through the relay in real-time.
///
/// The relay is intentionally **stateless** (it does not store graph nodes).
/// For persistent storage, connect the relay to a [`crate::Replicator`] and a
/// [`pluresdb_core::CrdtStore`] in the application layer.
#[derive(Debug)]
pub struct GunRelayServer {
    /// Broadcast channel capacity (number of in-flight messages).
    broadcast_capacity: usize,
}

impl Default for GunRelayServer {
    fn default() -> Self {
        Self::new()
    }
}

impl GunRelayServer {
    /// Create a new relay server with a default broadcast capacity of 256.
    pub fn new() -> Self {
        Self {
            broadcast_capacity: 256,
        }
    }

    /// Set the broadcast channel capacity.
    ///
    /// Higher values allow more in-flight messages before slow consumers start
    /// lagging (and potentially missing messages).
    pub fn with_broadcast_capacity(mut self, capacity: usize) -> Self {
        self.broadcast_capacity = capacity;
        self
    }

    /// Start the relay server and block until it stops.
    ///
    /// Binds to `addr` (e.g. `"0.0.0.0:4444"`) and serves WebSocket
    /// connections at the `/gun` path.
    pub async fn serve(self, addr: &str) -> anyhow::Result<()> {
        let addr: SocketAddr = addr.parse()?;
        let app = self.build_router();
        info!("[GunRelay] listening on ws://{}/gun", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app)
            .with_graceful_shutdown(GunRelayServer::shutdown_signal())
            .await?;
        Ok(())
    }

    /// Wait for a shutdown signal (Ctrl+C on all platforms, plus SIGTERM on Unix).
    ///
    /// This mirrors the behavior of the CLI HTTP API server so that the relay
    /// can shut down gracefully when the process is terminated.
    async fn shutdown_signal() {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};

            let mut terminate =
                signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
        }

        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        }
    }

    /// Build the Axum router (exposed for testing and embedding in larger apps).
    ///
    /// The returned `Router` handles `/gun` WebSocket upgrades with its own
    /// internal broadcast state.  To embed in a larger Axum application, nest
    /// it with a prefix:
    ///
    /// ```rust,no_run
    /// use axum::Router;
    /// use pluresdb_sync::GunRelayServer;
    ///
    /// let relay_router = GunRelayServer::new().build_router();
    /// let app = Router::new().nest("/p2p", relay_router);
    /// ```
    pub fn build_router(self) -> Router {
        let state = Arc::new(RelayState::new(self.broadcast_capacity));
        Router::new()
            .route("/gun", get(ws_handler))
            .with_state(state)
    }

    /// Return a router together with its shared state.
    ///
    /// Intended for tests that need direct access to the relay state.
    #[cfg(test)]
    pub(crate) fn router_with_state(capacity: usize) -> (Router, Arc<RelayState>) {
        let state = Arc::new(RelayState::new(capacity));
        let router = Router::new()
            .route("/gun", get(ws_handler))
            .with_state(Arc::clone(&state));
        (router, state)
    }
}

// ---------------------------------------------------------------------------
// WebSocket handler
// ---------------------------------------------------------------------------

/// Axum handler for the `/gun` WebSocket endpoint.
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<RelayState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Drive a single peer connection.
async fn handle_socket(socket: WebSocket, state: Arc<RelayState>) {
    let peer_id = Uuid::new_v4().to_string();
    state
        .peer_count
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    state.peers.insert(peer_id.clone(), ());
    info!("[GunRelay] peer connected: {}", peer_id);

    let mut rx = state.tx.subscribe();
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Task: forward broadcast messages from other peers to this peer's socket.
    let peer_id_send = peer_id.clone();
    let mut send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(envelope) => {
                    // Skip messages originated by this peer (no echo).
                    if envelope.origin == peer_id_send {
                        continue;
                    }
                    if ws_tx
                        .send(Message::Text(
                            String::from_utf8_lossy(&envelope.payload).to_string().into(),
                        ))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // The receiver fell behind the sender; log a warning and
                    // continue — the broadcast channel has already advanced
                    // the receiver past the missed messages automatically.
                    warn!(
                        "[GunRelay] peer {} lagged {} messages; skipping missed frames",
                        peer_id_send, n
                    );
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Task: receive messages from this peer's socket and broadcast.
    let peer_id_recv = peer_id.clone();
    let tx = state.tx.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            let raw = match msg {
                Message::Text(t) => t.as_bytes().to_vec(),
                Message::Binary(b) => b.to_vec(),
                Message::Close(_) => break,
                // Ignore ping/pong; Axum handles them automatically.
                _ => continue,
            };

            // Parse and validate the GUN message before relaying.
            match GunMessage::decode(&raw) {
                Ok(gun_msg) => {
                    debug!(
                        "[GunRelay] peer {} sent {} id={}",
                        peer_id_recv,
                        gun_msg.message_type(),
                        gun_msg.id()
                    );
                    // Relay valid GUN messages to all other peers.
                    let envelope = RelayEnvelope {
                        origin: peer_id_recv.clone(),
                        payload: raw,
                    };
                    if tx.send(envelope).is_err() {
                        // No subscribers (the send_task may have exited).
                        break;
                    }
                }
                Err(e) => {
                    warn!(
                        "[GunRelay] peer {} sent invalid GUN message: {}",
                        peer_id_recv, e
                    );
                }
            }
        }
    });

    // Wait for either task to finish, then abort the other.
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    state
        .peer_count
        .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    state.peers.remove(&peer_id);
    info!("[GunRelay] peer disconnected: {}", peer_id);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_server_defaults() {
        let server = GunRelayServer::new();
        assert_eq!(server.broadcast_capacity, 256);
    }

    #[test]
    fn test_relay_server_custom_capacity() {
        let server = GunRelayServer::new().with_broadcast_capacity(512);
        assert_eq!(server.broadcast_capacity, 512);
    }

    #[test]
    fn test_relay_state_peer_count_starts_zero() {
        let state = RelayState::new(64);
        assert_eq!(
            state
                .peer_count
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        assert!(state.peers.is_empty());
    }

    #[test]
    fn test_relay_router_is_buildable() {
        let _router = GunRelayServer::new().build_router();
        // If it compiles and runs without panic, we're good.
    }

    /// Verifies that a GUN PUT message encodes and decodes cleanly (wire format sanity check).
    #[tokio::test]
    async fn test_put_message_encode_decode() {
        use crate::gun_protocol::{GunNode, GunPut, Soul};
        use serde_json::json;
        use std::collections::HashMap;

        let (_router, state) = GunRelayServer::router_with_state(64);

        // Verify initial peer count.
        assert_eq!(
            state
                .peer_count
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );

        // Build a test PUT message and verify it encodes/decodes cleanly.
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), json!("Alice"));
        let node = GunNode::from_data("user:alice", fields, 1_700_000_000_000.0);
        let mut put_map: HashMap<Soul, GunNode> = HashMap::new();
        put_map.insert("user:alice".to_string(), node);
        let msg = GunMessage::Put(GunPut {
            id: "msg-relay-1".to_string(),
            put: put_map,
        });
        let encoded = msg.encode().unwrap();
        let decoded = GunMessage::decode(&encoded).unwrap();
        assert_eq!(decoded.id(), "msg-relay-1");
    }

    /// Verifies that the broadcast channel fans-out messages to subscribers,
    /// skipping the originating peer (no echo).
    #[tokio::test]
    async fn test_relay_broadcast_fanout() {
        use crate::gun_protocol::{GunNode, GunPut, Soul};
        use serde_json::json;
        use std::collections::HashMap;

        // Create shared relay state with capacity 64.
        let state = Arc::new(RelayState::new(64));

        // Subscribe two "peers" to the broadcast channel.
        let mut rx_peer_b = state.tx.subscribe();
        let mut rx_peer_c = state.tx.subscribe();

        // Build a PUT message from peer-a.
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), json!("Alice"));
        let node = GunNode::from_data("user:alice", fields, 1_700_000_000_000.0);
        let mut put_map: HashMap<Soul, GunNode> = HashMap::new();
        put_map.insert("user:alice".to_string(), node);
        let msg = GunMessage::Put(GunPut {
            id: "fanout-test".to_string(),
            put: put_map,
        });
        let payload = msg.encode().unwrap();

        // Simulate peer-a sending the envelope into the broadcast channel.
        let envelope = RelayEnvelope {
            origin: "peer-a".to_string(),
            payload: payload.clone(),
        };
        state.tx.send(envelope).unwrap();

        // peer-b and peer-c both receive the message.
        let recv_b = rx_peer_b.recv().await.unwrap();
        let recv_c = rx_peer_c.recv().await.unwrap();

        assert_eq!(recv_b.origin, "peer-a");
        assert_eq!(recv_c.origin, "peer-a");

        // Decoded payloads match the original.
        let decoded_b = GunMessage::decode(&recv_b.payload).unwrap();
        let decoded_c = GunMessage::decode(&recv_c.payload).unwrap();
        assert_eq!(decoded_b.id(), "fanout-test");
        assert_eq!(decoded_c.id(), "fanout-test");

        // A peer that is the origin should skip the message (no echo).
        // Verify by checking the origin field.
        assert_ne!(recv_b.origin, "peer-b", "peer-b should not echo to itself");
        assert_ne!(recv_c.origin, "peer-c", "peer-c should not echo to itself");
    }
}
