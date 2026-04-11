//! Relay transport for corporate-friendly WebSocket sync.
//!
//! This transport uses WebSocket connections on port 443 (HTTPS) to bypass
//! corporate firewall restrictions. It connects to a [`crate::GunRelayServer`]
//! (or any compatible GUN relay) and receives a per-topic isolated broadcast
//! channel.
//!
//! ## Topic routing
//!
//! When the relay URL ends with `/gun`, the transport appends the hex-encoded
//! 32-byte topic hash as an additional path segment:
//! ```text
//! ws://relay.example.com/gun/<topic_hex>
//! ```
//! The `GunRelayServer` routes each topic to its own isolated broadcast
//! channel, so peers on different databases do not receive each other's
//! messages.
//!
//! ## Reconnection
//!
//! The transport reconnects automatically with truncated exponential backoff
//! (100 ms → 200 ms → … → 30 s) up to a configurable retry limit.  Each new
//! underlying WebSocket connection is delivered to the caller as a fresh
//! [`RelayConnection`] via the returned `mpsc::Receiver`.
//!
//! ## Encryption
//!
//! When the relay URL uses `wss://`, TLS provides transport-layer encryption
//! for all traffic between the client and the relay server.  The relay itself
//! cannot read message contents because they are GUN wire-protocol bytes.  For
//! true end-to-end encryption between peers, encrypt the GUN node payloads at
//! the application layer before calling [`Connection::send`].

use crate::transport::{Connection, MessagePayload, PeerId, PeerInfo, TopicHash, Transport};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, info, warn};

/// Initial reconnect backoff delay.
const INITIAL_BACKOFF_MS: u64 = 100;
/// Maximum reconnect backoff delay.
const MAX_BACKOFF_MS: u64 = 30_000;
/// Default maximum number of reconnect attempts (0 = unlimited).
const DEFAULT_MAX_RETRIES: u32 = 10;

// ─────────────────────────────────────────────────────────────────────────────
// RelayTransport
// ─────────────────────────────────────────────────────────────────────────────

/// WebSocket relay transport for peer-to-peer sync.
///
/// Connects to a [`crate::GunRelayServer`] (or any compatible GUN relay) and
/// delivers incoming connection handles to the caller via the channel returned
/// by [`Transport::connect`].  The transport reconnects automatically when the
/// WebSocket closes.
///
/// # Configuration
///
/// ```rust
/// use pluresdb_sync::RelayTransport;
///
/// // Default: 10 reconnect attempts, 30 s timeout
/// let transport = RelayTransport::new("wss://relay.example.com/gun".to_string(), 30_000);
///
/// // Custom retry limit
/// let transport = RelayTransport::with_max_retries(
///     "ws://localhost:4444/gun".to_string(), 5_000, 5,
/// );
/// ```
pub struct RelayTransport {
    /// Base WebSocket URL of the relay server (e.g. `wss://relay.example.com/gun`).
    relay_url: String,
    /// Connection timeout in milliseconds.
    timeout_ms: u64,
    /// Maximum reconnect attempts (0 = unlimited).
    max_retries: u32,
    /// Stable peer identifier for this transport instance.
    peer_id: String,
    /// Set to `true` by [`disconnect`] to halt the reconnect loop.
    stopped: Arc<AtomicBool>,
}

impl RelayTransport {
    /// Create a new relay transport with the default retry limit (10).
    pub fn new(relay_url: String, timeout_ms: u64) -> Self {
        Self::with_max_retries(relay_url, timeout_ms, DEFAULT_MAX_RETRIES)
    }

    /// Create a relay transport with a custom maximum reconnect attempt count.
    ///
    /// Set `max_retries` to `0` for unlimited reconnects.
    pub fn with_max_retries(relay_url: String, timeout_ms: u64, max_retries: u32) -> Self {
        Self {
            relay_url,
            timeout_ms,
            max_retries,
            peer_id: uuid::Uuid::new_v4().to_string(),
            stopped: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Validate the relay transport configuration.
    ///
    /// Returns an actionable error if the URL scheme is not `ws://` or `wss://`.
    /// Emits a warning (but does not fail) when an unencrypted `ws://` URL is used.
    pub fn validate(&self) -> Result<()> {
        if !self.relay_url.starts_with("ws://") && !self.relay_url.starts_with("wss://") {
            return Err(anyhow!(
                "Invalid relay URL '{}': must start with ws:// (unencrypted) or \
                 wss:// (TLS-encrypted).  Example: wss://relay.example.com/gun",
                self.relay_url
            ));
        }
        if self.timeout_ms == 0 {
            return Err(anyhow!(
                "Invalid relay timeout: timeout_ms must be greater than 0. \
                 Recommended: 30000 (30 seconds)."
            ));
        }
        if self.relay_url.starts_with("ws://") {
            warn!(
                "[RelayTransport] relay URL uses unencrypted ws:// — \
                 prefer wss:// for production. URL: {}",
                self.relay_url
            );
        }
        Ok(())
    }

    /// Build the WebSocket URL for the given topic.
    ///
    /// Appends the lowercase hex-encoded topic hash as a path segment so that
    /// the relay can route to the per-topic broadcast channel.
    fn topic_url(&self, topic: &TopicHash) -> String {
        let topic_hex = hex_bytes(topic);
        format!("{}/{}", self.relay_url.trim_end_matches('/'), topic_hex)
    }
}

#[async_trait]
impl Transport for RelayTransport {
    /// Connect to the relay server on the given topic.
    ///
    /// Spawns a background task that:
    /// 1. Connects to `{relay_url}/{topic_hex}` with exponential backoff.
    /// 2. Wraps the WebSocket stream in a [`RelayConnection`] and sends it
    ///    through the returned `mpsc::Receiver`.
    /// 3. Waits for the connection to close, then reconnects.
    ///
    /// Reconnection stops when `max_retries` is exceeded or [`disconnect`] is
    /// called.
    async fn connect(&mut self, topic: TopicHash) -> Result<mpsc::Receiver<Box<dyn Connection>>> {
        self.validate()?;

        let url = self.topic_url(&topic);
        let topic_short = &hex_bytes(&topic)[..8];
        info!(
            "[RelayTransport] connecting to {} for topic {}…",
            url, topic_short
        );

        let (conn_tx, conn_rx) = mpsc::channel::<Box<dyn Connection>>(16);
        let max_retries = self.max_retries;
        let timeout_ms = self.timeout_ms;
        let stopped = Arc::clone(&self.stopped);
        let peer_id_base = self.peer_id.clone();
        let topic_short = topic_short.to_owned();

        tokio::spawn(async move {
            let mut attempt = 0u32;
            let mut backoff = Duration::from_millis(INITIAL_BACKOFF_MS);
            let max_backoff = Duration::from_millis(MAX_BACKOFF_MS);
            let connect_timeout = Duration::from_millis(timeout_ms);

            loop {
                if stopped.load(Ordering::Relaxed) {
                    debug!("[RelayTransport] stopped — exiting reconnect loop");
                    break;
                }
                if max_retries > 0 && attempt >= max_retries {
                    warn!(
                        "[RelayTransport] max retries ({}) reached for topic {}",
                        max_retries, topic_short
                    );
                    break;
                }

                attempt += 1;
                debug!(
                    "[RelayTransport] connect attempt {} for topic {}",
                    attempt, topic_short
                );

                let connect_result =
                    tokio::time::timeout(connect_timeout, connect_async(url.as_str())).await;

                match connect_result {
                    Ok(Ok((ws_stream, _))) => {
                        info!(
                            "[RelayTransport] connected (attempt {}) for topic {}",
                            attempt, topic_short
                        );
                        // Reset attempt counter on success so we always get
                        // `max_retries` attempts after the next disconnect.
                        attempt = 0;
                        backoff = Duration::from_millis(INITIAL_BACKOFF_MS);

                        let (mut ws_tx, mut ws_rx) = ws_stream.split();

                        // Internal channels bridging the WebSocket to RelayConnection.
                        let (send_tx, mut send_rx) = mpsc::channel::<Vec<u8>>(64);
                        let (recv_tx, recv_rx) = mpsc::channel::<Vec<u8>>(64);

                        // Write task: drain send_rx → WebSocket.
                        let write_task = tokio::spawn(async move {
                            while let Some(data) = send_rx.recv().await {
                                if ws_tx.send(Message::Binary(data.into())).await.is_err() {
                                    break;
                                }
                            }
                            let _ = ws_tx.close().await;
                        });

                        // Read task: WebSocket → recv_tx.
                        let recv_tx_inner = recv_tx;
                        let read_task = tokio::spawn(async move {
                            while let Some(result) = ws_rx.next().await {
                                let data = match result {
                                    Ok(Message::Binary(b)) => b.to_vec(),
                                    Ok(Message::Text(t)) => t.as_bytes().to_vec(),
                                    Ok(Message::Close(_)) | Err(_) => break,
                                    _ => continue,
                                };
                                if recv_tx_inner.send(data).await.is_err() {
                                    break;
                                }
                            }
                        });

                        let conn = RelayConnection {
                            peer_id: peer_id_base.clone(),
                            tx: send_tx,
                            rx: recv_rx,
                        };

                        if conn_tx
                            .send(Box::new(conn) as Box<dyn Connection>)
                            .await
                            .is_err()
                        {
                            // Caller dropped its receiver — stop reconnecting.
                            write_task.abort();
                            read_task.abort();
                            break;
                        }

                        // Block until both I/O tasks finish (connection closed).
                        tokio::select! {
                            _ = write_task => {}
                            _ = read_task => {}
                        }
                        info!(
                            "[RelayTransport] relay connection closed for topic {}",
                            topic_short
                        );
                    }
                    Ok(Err(e)) => {
                        warn!("[RelayTransport] connect attempt {} failed: {}", attempt, e);
                    }
                    Err(_) => {
                        warn!(
                            "[RelayTransport] connect attempt {} timed out after {}ms",
                            attempt, timeout_ms
                        );
                    }
                }

                if stopped.load(Ordering::Relaxed) {
                    break;
                }
                debug!("[RelayTransport] backing off {:?} before retry", backoff);
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        });

        Ok(conn_rx)
    }

    /// Announce this peer on a topic.
    ///
    /// For relay transport the announcement is implicit: when a peer calls
    /// [`connect`] it joins the relay topic channel and all other connected
    /// peers receive its messages.  This method validates configuration and
    /// logs the intent.
    async fn announce(&mut self, topic: TopicHash) -> Result<()> {
        self.validate()?;
        info!(
            "[RelayTransport] announce on topic {}",
            &hex_bytes(&topic)[..8]
        );
        Ok(())
    }

    /// Look up peers on a topic.
    ///
    /// The GUN relay protocol does not expose a peer list — peer discovery
    /// happens implicitly when peers exchange messages through the relay.
    /// Returns an empty list.
    async fn lookup(&self, _topic: TopicHash) -> Result<Vec<PeerInfo>> {
        Ok(vec![])
    }

    /// Stop the background reconnect loop and close the transport.
    async fn disconnect(&mut self) -> Result<()> {
        info!("[RelayTransport] disconnecting");
        self.stopped.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn name(&self) -> &str {
        "relay"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RelayConnection
// ─────────────────────────────────────────────────────────────────────────────

/// A connection tunnelled through a GUN relay server.
///
/// Wraps MPSC channels that bridge to a background WebSocket I/O task.
/// Messages sent via [`send`] are forwarded to the WebSocket as binary
/// frames; messages received from the relay arrive via [`receive`].
///
/// [`close`] drops the send-side channel, which signals EOF to the write task
/// and causes the underlying WebSocket connection to close gracefully.
pub struct RelayConnection {
    peer_id: PeerId,
    tx: mpsc::Sender<Vec<u8>>,
    rx: mpsc::Receiver<Vec<u8>>,
}

#[async_trait]
impl Connection for RelayConnection {
    async fn send(&mut self, data: &[u8]) -> Result<()> {
        self.tx
            .send(data.to_vec())
            .await
            .map_err(|_| anyhow!("RelayConnection: send failed — relay disconnected"))
    }

    async fn receive(&mut self) -> Result<Option<MessagePayload>> {
        Ok(self.rx.recv().await)
    }

    async fn close(&mut self) -> Result<()> {
        // Replace the sender with a dead channel to trigger EOF on the write task.
        let (tx, _) = mpsc::channel(1);
        let _ = std::mem::replace(&mut self.tx, tx);
        Ok(())
    }

    fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Encode a byte slice as a lowercase hex string.
fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
        s
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Construction & validation ────────────────────────────────────────────

    #[test]
    fn test_relay_transport_name() {
        let t = RelayTransport::new("wss://relay.example.com/gun".to_string(), 30_000);
        assert_eq!(t.name(), "relay");
    }

    #[test]
    fn test_validate_valid_wss_url() {
        let t = RelayTransport::new("wss://relay.example.com/gun".to_string(), 30_000);
        assert!(t.validate().is_ok());
    }

    #[test]
    fn test_validate_valid_ws_url() {
        let t = RelayTransport::new("ws://localhost:4444/gun".to_string(), 5_000);
        // ws:// is allowed (just emits a warning)
        assert!(t.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_scheme_returns_actionable_error() {
        let t = RelayTransport::new("http://relay.example.com/gun".to_string(), 30_000);
        let err = t.validate().unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("ws://") && msg.contains("wss://"),
            "error should mention both ws:// and wss://, got: {msg}"
        );
    }

    #[test]
    fn test_validate_zero_timeout_returns_error() {
        let t = RelayTransport::new("wss://relay.example.com/gun".to_string(), 0);
        let err = t.validate().unwrap_err();
        assert!(
            err.to_string().contains("timeout_ms"),
            "error should mention timeout_ms"
        );
    }

    #[test]
    fn test_topic_url_appends_hex_topic() {
        let t = RelayTransport::new("ws://localhost:4444/gun".to_string(), 5_000);
        let topic = [0xABu8; 32];
        let url = t.topic_url(&topic);
        assert!(url.starts_with("ws://localhost:4444/gun/"));
        assert!(url.ends_with(&"ab".repeat(32)));
    }

    // ── announce / lookup return without error ───────────────────────────────

    #[tokio::test]
    async fn test_announce_ok() {
        let mut t = RelayTransport::new("wss://relay.example.com/gun".to_string(), 30_000);
        let topic = [1u8; 32];
        assert!(t.announce(topic).await.is_ok());
    }

    #[tokio::test]
    async fn test_lookup_returns_empty() {
        let t = RelayTransport::new("wss://relay.example.com/gun".to_string(), 30_000);
        let topic = [1u8; 32];
        let peers = t.lookup(topic).await.unwrap();
        assert!(peers.is_empty());
    }

    // ── connect with invalid URL returns error immediately ───────────────────

    #[tokio::test]
    async fn test_connect_invalid_url_returns_error() {
        let mut t = RelayTransport::new("ftp://invalid".to_string(), 5_000);
        let topic = [0u8; 32];
        assert!(t.connect(topic).await.is_err());
    }

    // ── disconnect stops reconnect loop ──────────────────────────────────────

    #[tokio::test]
    async fn test_disconnect_sets_stopped_flag() {
        let mut t = RelayTransport::new("wss://relay.example.com/gun".to_string(), 30_000);
        assert!(!t.stopped.load(Ordering::Relaxed));
        t.disconnect().await.unwrap();
        assert!(t.stopped.load(Ordering::Relaxed));
    }
}
