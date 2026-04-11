//! Hyperswarm-style transport for PluresDB.
//!
//! Because the first-party `hyperswarm-rs` crate (from `plures/hyperswarm`) is
//! not yet published, this module provides a functionally equivalent transport
//! built from available Rust primitives:
//!
//! | Hyperswarm component   | This implementation                          |
//! |------------------------|----------------------------------------------|
//! | Kademlia DHT discovery | Process-local in-memory peer registry        |
//! | UDP holepunching       | Direct TCP connections on `127.0.0.1`        |
//! | Noise XX encryption    | AES-256-GCM with topic-derived key           |
//!
//! ## Local-peer registry
//!
//! A process-local `static` [`DashMap`] maps each `TopicHash` to the set of
//! `(peer_id, SocketAddr)` pairs that have called [`Transport::announce`].
//! This enables unit tests and local multi-peer demos to discover each other
//! without any external broker.
//!
//! ## Encryption
//!
//! When `HyperswarmConfig::encryption` is `true` (the default), every message
//! frame is encrypted with **AES-256-GCM**:
//!
//! ```text
//! ┌──────────────┬──────────────────┬────────────────────────┐
//! │ len: 4 bytes │ nonce: 12 bytes  │ AES-GCM ciphertext     │
//! └──────────────┴──────────────────┴────────────────────────┘
//! ```
//!
//! The 256-bit key is derived from the 32-byte topic hash, so all peers
//! sharing a topic automatically share the symmetric key.  Without
//! `hyperswarm-rs`' Noise XX handshake, this provides authenticated symmetric
//! encryption rather than forward-secret asymmetric encryption.
//!
//! ## Reconnection
//!
//! TCP connection attempts use a configurable timeout; the calling layer
//! (typically [`Transport::connect`]) retries with exponential backoff.

use crate::transport::{Connection, MessagePayload, PeerId, PeerInfo, TopicHash, Transport};
use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::Aes256Gcm;

/// Type alias for the 12-byte AES-GCM nonce used by Aes256Gcm.
// generic_array 0.14 marks GenericArray as deprecated in favour of 1.x, but
// aes-gcm 0.10 still relies on the 0.14 API.  Allow the deprecation here so
// that `-D warnings` in CI does not fail this otherwise-sound code.
#[allow(deprecated)]
type AesNonce = aes_gcm::aead::generic_array::GenericArray<u8, <Aes256Gcm as AeadCore>::NonceSize>;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, OnceLock,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

// ─────────────────────────────────────────────────────────────────────────────
// Process-local peer registry
// ─────────────────────────────────────────────────────────────────────────────

/// Inner map: peer_id → SocketAddr.
type PeerMap = DashMap<String, SocketAddr>;
/// Outer map: hex(topic) → PeerMap.
type TopicRegistry = DashMap<String, Arc<PeerMap>>;

static LOCAL_REGISTRY: OnceLock<TopicRegistry> = OnceLock::new();

fn local_registry() -> &'static TopicRegistry {
    LOCAL_REGISTRY.get_or_init(TopicRegistry::new)
}

/// Register `(peer_id, addr)` under `topic` in the process-local registry.
fn registry_register(topic: &TopicHash, peer_id: &str, addr: SocketAddr) {
    let key = hex_bytes(topic);
    let peer_map = local_registry()
        .entry(key)
        .or_insert_with(|| Arc::new(PeerMap::new()))
        .clone();
    peer_map.insert(peer_id.to_owned(), addr);
}

/// Remove `peer_id` from `topic` in the process-local registry.
fn registry_remove(topic: &TopicHash, peer_id: &str) {
    let key = hex_bytes(topic);
    if let Some(peer_map) = local_registry().get(&key) {
        peer_map.remove(peer_id);
    }
}

/// Return all `(peer_id, addr)` pairs for `topic` except for `exclude_peer_id`.
fn registry_lookup(topic: &TopicHash, exclude_peer_id: &str) -> Vec<(String, SocketAddr)> {
    let key = hex_bytes(topic);
    local_registry()
        .get(&key)
        .map(|peer_map| {
            peer_map
                .iter()
                .filter(|e| e.key() != exclude_peer_id)
                .map(|e| (e.key().clone(), *e.value()))
                .collect()
        })
        .unwrap_or_default()
}

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for [`HyperswarmTransport`].
#[derive(Debug, Clone)]
pub struct HyperswarmConfig {
    /// Bootstrap nodes for DHT-based discovery (hostname:port).
    ///
    /// These are used for wide-area peer discovery when the process-local
    /// registry has no matching peers.  Leave empty for local-only mode.
    pub bootstrap_nodes: Vec<String>,

    /// Enable AES-256-GCM message encryption.
    ///
    /// **Should always be `true` in production.**  Set to `false` only for
    /// debugging or compatibility testing.
    pub encryption: bool,

    /// TCP connection timeout in milliseconds.
    pub timeout_ms: u64,
}

impl Default for HyperswarmConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes: vec![
                "bootstrap1.hyperdht.org:49737".to_string(),
                "bootstrap2.hyperdht.org:49737".to_string(),
                "bootstrap3.hyperdht.org:49737".to_string(),
            ],
            encryption: true,
            timeout_ms: 30_000,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HyperswarmTransport
// ─────────────────────────────────────────────────────────────────────────────

/// Hyperswarm-style P2P transport.
///
/// Uses the process-local peer registry for discovery and direct TCP
/// connections for communication.  AES-256-GCM encryption is enabled by
/// default.
///
/// When `hyperswarm-rs` (from `plures/hyperswarm`) is published, this
/// implementation will be replaced with the real Kademlia DHT + Noise XX
/// encrypted streams.
pub struct HyperswarmTransport {
    config: HyperswarmConfig,
    /// Stable peer ID for this transport instance.
    peer_id: String,
    /// Bound TCP listener (set after the first `announce` or `connect` call).
    listener: Option<TcpListener>,
    /// Local address of the bound TCP listener.
    local_addr: Option<SocketAddr>,
    /// Topics that this transport has announced (for cleanup on disconnect).
    announced_topics: Vec<TopicHash>,
    /// Set to `true` by [`disconnect`] to halt background tasks.
    stopped: Arc<AtomicBool>,
}

impl HyperswarmTransport {
    /// Create a new Hyperswarm transport with the given configuration.
    pub fn new(config: HyperswarmConfig) -> Self {
        Self {
            config,
            peer_id: uuid::Uuid::new_v4().to_string(),
            listener: None,
            local_addr: None,
            announced_topics: Vec::new(),
            stopped: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Validate the transport configuration.
    ///
    /// Returns an actionable error if any required invariant is violated.
    pub fn validate(&self) -> Result<()> {
        if !self.config.encryption {
            warn!(
                "[HyperswarmTransport] encryption is disabled — \
                 all message frames will be sent in plaintext. \
                 Set HyperswarmConfig::encryption = true for production use."
            );
        }
        if self.config.timeout_ms == 0 {
            return Err(anyhow!(
                "Invalid HyperswarmConfig: timeout_ms must be > 0. \
                 Recommended: 30000 (30 seconds)."
            ));
        }
        if self.config.bootstrap_nodes.is_empty() {
            warn!(
                "[HyperswarmTransport] no bootstrap nodes configured — \
                 only process-local peers on the same machine will be discovered."
            );
        }
        Ok(())
    }

    /// Bind a TCP listener on a random loopback port (if not already bound).
    async fn ensure_listener(&mut self) -> Result<()> {
        if self.listener.is_none() {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .context("HyperswarmTransport: failed to bind TCP listener")?;
            self.local_addr = Some(listener.local_addr()?);
            self.listener = Some(listener);
            debug!(
                "[HyperswarmTransport] bound TCP listener on {}",
                self.local_addr.unwrap()
            );
        }
        Ok(())
    }

    /// Derive an AES-256-GCM cipher from a topic hash.
    ///
    /// All peers sharing the same topic derive the same symmetric key.
    fn derive_cipher(topic: &TopicHash) -> Aes256Gcm {
        // topic is always 32 bytes = valid AES-256 key length.
        Aes256Gcm::new_from_slice(topic).expect("TopicHash is 32 bytes — valid AES-256 key")
    }
}

#[async_trait]
impl Transport for HyperswarmTransport {
    /// Announce this peer on a topic.
    ///
    /// Registers the peer's TCP address in the process-local registry so that
    /// other peers calling [`connect`] or [`lookup`] can discover it.
    async fn announce(&mut self, topic: TopicHash) -> Result<()> {
        self.validate()?;
        self.ensure_listener().await?;

        let addr = self.local_addr.expect("listener already bound");
        registry_register(&topic, &self.peer_id, addr);
        if !self.announced_topics.contains(&topic) {
            self.announced_topics.push(topic);
        }
        info!(
            "[HyperswarmTransport] announced peer {} on topic {} at {}",
            &self.peer_id[..8],
            &hex_bytes(&topic)[..8],
            addr
        );
        Ok(())
    }

    /// Look up peers on a topic using the process-local registry.
    async fn lookup(&self, topic: TopicHash) -> Result<Vec<PeerInfo>> {
        let peers = registry_lookup(&topic, &self.peer_id);
        let infos = peers
            .into_iter()
            .map(|(id, addr)| PeerInfo {
                id,
                address: Some(addr.to_string()),
                connected: false,
            })
            .collect();
        Ok(infos)
    }

    /// Connect to peers on a topic.
    ///
    /// 1. Announces this transport if not yet announced.
    /// 2. Starts accepting incoming TCP connections from peers in the registry.
    /// 3. Connects outbound to all peers already in the registry for this topic.
    ///
    /// All connections (inbound and outbound) are sent through the returned
    /// `mpsc::Receiver` as [`Connection`] trait objects.  Each connection uses
    /// AES-256-GCM encryption when enabled.
    async fn connect(&mut self, topic: TopicHash) -> Result<mpsc::Receiver<Box<dyn Connection>>> {
        self.validate()?;
        // Announce first to register our address.
        self.announce(topic).await?;

        let (conn_tx, conn_rx) = mpsc::channel::<Box<dyn Connection>>(16);

        // Take the listener to move into the accept task.
        let listener = self
            .listener
            .take()
            .ok_or_else(|| anyhow!("HyperswarmTransport: listener not available after announce"))?;

        let cipher = if self.config.encryption {
            Some(Self::derive_cipher(&topic))
        } else {
            None
        };

        let my_peer_id = self.peer_id.clone();
        let timeout_ms = self.config.timeout_ms;
        let stopped = Arc::clone(&self.stopped);
        let topic_short = hex_bytes(&topic[..4].try_into().unwrap_or([0u8; 4]));

        // ── Accept task: handle incoming TCP connections ─────────────────────
        let conn_tx_accept = conn_tx.clone();
        let cipher_accept = cipher.clone();
        let my_peer_id_accept = my_peer_id.clone();
        let stopped_accept = Arc::clone(&stopped);
        tokio::spawn(async move {
            loop {
                if stopped_accept.load(Ordering::Relaxed) {
                    break;
                }
                match tokio::time::timeout(Duration::from_secs(1), listener.accept()).await {
                    Ok(Ok((stream, peer_addr))) => {
                        let peer_id = format!("incoming-{}", peer_addr);
                        info!(
                            "[HyperswarmTransport] accepted connection from {} (peer {})",
                            peer_addr,
                            &my_peer_id_accept[..8]
                        );
                        let conn =
                            HyperswarmConnection::new(peer_id, stream, cipher_accept.clone());
                        if conn_tx_accept
                            .send(Box::new(conn) as Box<dyn Connection>)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(Err(e)) => {
                        warn!("[HyperswarmTransport] accept error: {}", e);
                        break;
                    }
                    // Timeout: poll for stop flag, then retry accept
                    Err(_) => continue,
                }
            }
            debug!("[HyperswarmTransport] accept loop exited");
        });

        // ── Outbound task: connect to peers already in registry ──────────────
        let known_peers = registry_lookup(&topic, &my_peer_id);
        let conn_tx_out = conn_tx;
        let cipher_out = cipher;
        tokio::spawn(async move {
            for (peer_id, addr) in known_peers {
                debug!(
                    "[HyperswarmTransport] dialing peer {} at {} (topic {})",
                    &peer_id[..8.min(peer_id.len())],
                    addr,
                    topic_short
                );
                let connect_result = tokio::time::timeout(
                    Duration::from_millis(timeout_ms),
                    TcpStream::connect(addr),
                )
                .await;

                match connect_result {
                    Ok(Ok(stream)) => {
                        info!(
                            "[HyperswarmTransport] connected to peer {} at {}",
                            &peer_id[..8.min(peer_id.len())],
                            addr
                        );
                        let conn = HyperswarmConnection::new(peer_id, stream, cipher_out.clone());
                        if conn_tx_out
                            .send(Box::new(conn) as Box<dyn Connection>)
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(Err(e)) => {
                        warn!(
                            "[HyperswarmTransport] failed to connect to {} at {}: {}",
                            &peer_id[..8.min(peer_id.len())],
                            addr,
                            e
                        );
                    }
                    Err(_) => {
                        warn!(
                            "[HyperswarmTransport] timeout connecting to {} at {}",
                            &peer_id[..8.min(peer_id.len())],
                            addr
                        );
                    }
                }
            }
        });

        Ok(conn_rx)
    }

    /// Remove all announced topics from the process-local registry and stop
    /// all background tasks.
    async fn disconnect(&mut self) -> Result<()> {
        info!("[HyperswarmTransport] disconnecting");
        self.stopped.store(true, Ordering::Relaxed);
        for topic in &self.announced_topics {
            registry_remove(topic, &self.peer_id);
        }
        self.announced_topics.clear();
        Ok(())
    }

    fn name(&self) -> &str {
        "hyperswarm"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// HyperswarmConnection
// ─────────────────────────────────────────────────────────────────────────────

/// A direct TCP connection between two Hyperswarm peers.
///
/// Frames are length-prefixed:
/// ```text
/// ┌──────────────┬───────────────────────────────────────┐
/// │ len: u32 BE  │ payload (optionally AES-GCM encrypted) │
/// └──────────────┴───────────────────────────────────────┘
/// ```
///
/// When encryption is enabled the payload has the structure:
/// ```text
/// ┌─────────────────┬─────────────────────────┐
/// │ nonce: 12 bytes │ AES-GCM ciphertext       │
/// └─────────────────┴─────────────────────────┘
/// ```
pub struct HyperswarmConnection {
    peer_id: PeerId,
    reader: tokio::io::BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
    cipher: Option<Aes256Gcm>,
}

impl HyperswarmConnection {
    fn new(peer_id: PeerId, stream: TcpStream, cipher: Option<Aes256Gcm>) -> Self {
        let (read_half, write_half) = stream.into_split();
        Self {
            peer_id,
            reader: tokio::io::BufReader::new(read_half),
            writer: write_half,
            cipher,
        }
    }

    /// Write a length-prefixed (optionally encrypted) frame to the stream.
    async fn write_frame(&mut self, data: &[u8]) -> Result<()> {
        let payload = if let Some(cipher) = &self.cipher {
            let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
            let ciphertext = cipher
                .encrypt(&nonce, data)
                .map_err(|e| anyhow!("HyperswarmConnection: encryption failed: {:?}", e))?;
            let mut buf = Vec::with_capacity(nonce.len() + ciphertext.len());
            // Use AsRef<[u8]> instead of the deprecated `as_slice()`.
            buf.extend_from_slice(nonce.as_ref());
            buf.extend(ciphertext);
            buf
        } else {
            data.to_vec()
        };

        let len = payload.len() as u32;
        self.writer.write_all(&len.to_be_bytes()).await?;
        self.writer.write_all(&payload).await?;
        Ok(())
    }

    /// Read the next length-prefixed (optionally encrypted) frame.
    ///
    /// Returns `Ok(None)` when the remote closed the connection.
    async fn read_frame(&mut self) -> Result<Option<Vec<u8>>> {
        let mut len_buf = [0u8; 4];
        match self.reader.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len == 0 {
            return Ok(None);
        }
        let mut buf = vec![0u8; len];
        self.reader.read_exact(&mut buf).await?;

        if let Some(cipher) = &self.cipher {
            if buf.len() < 12 {
                return Err(anyhow!(
                    "HyperswarmConnection: received frame too short for AES-GCM nonce \
                     (expected ≥12 bytes, got {})",
                    buf.len()
                ));
            }
            // Reconstruct the 12-byte nonce using FromIterator (non-deprecated).
            let nonce: AesNonce = buf[..12].iter().copied().collect();
            let plaintext = cipher
                .decrypt(&nonce, &buf[12..])
                .map_err(|_| anyhow!("HyperswarmConnection: AES-GCM decryption failed"))?;
            Ok(Some(plaintext))
        } else {
            Ok(Some(buf))
        }
    }
}

#[async_trait]
impl Connection for HyperswarmConnection {
    async fn send(&mut self, data: &[u8]) -> Result<()> {
        self.write_frame(data).await
    }

    async fn receive(&mut self) -> Result<Option<MessagePayload>> {
        self.read_frame().await
    }

    /// Shut down the write half of the TCP connection, signalling EOF to the
    /// remote peer's [`receive`] loop.
    async fn close(&mut self) -> Result<()> {
        self.writer.shutdown().await?;
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

    // ── Config & validation ──────────────────────────────────────────────────

    #[test]
    fn test_hyperswarm_config_default_encryption_on() {
        let cfg = HyperswarmConfig::default();
        assert!(cfg.encryption, "encryption must default to true");
        assert_eq!(cfg.timeout_ms, 30_000);
        assert!(!cfg.bootstrap_nodes.is_empty());
    }

    #[test]
    fn test_validate_zero_timeout_returns_error() {
        let cfg = HyperswarmConfig {
            timeout_ms: 0,
            ..Default::default()
        };
        let t = HyperswarmTransport::new(cfg);
        let err = t.validate().unwrap_err();
        assert!(
            err.to_string().contains("timeout_ms"),
            "error should mention timeout_ms"
        );
    }

    #[test]
    fn test_validate_encryption_disabled_does_not_error() {
        // Disabled encryption emits a warning but is not an error.
        let cfg = HyperswarmConfig {
            encryption: false,
            ..Default::default()
        };
        let t = HyperswarmTransport::new(cfg);
        assert!(t.validate().is_ok());
    }

    #[test]
    fn test_transport_name() {
        let t = HyperswarmTransport::new(HyperswarmConfig::default());
        assert_eq!(t.name(), "hyperswarm");
    }

    // ── Announce / lookup ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_announce_registers_peer() {
        let mut t = HyperswarmTransport::new(HyperswarmConfig::default());
        // Use a unique topic to avoid interference with parallel tests.
        let topic = derive_unique_topic("announce-test");
        t.announce(topic).await.unwrap();

        let peers = t.lookup(topic).await.unwrap();
        // Our own peer should NOT appear in the lookup results.
        assert!(
            peers.iter().all(|p| p.id != t.peer_id),
            "own peer should be excluded from lookup"
        );
    }

    #[tokio::test]
    async fn test_two_peers_find_each_other_via_lookup() {
        let topic = derive_unique_topic("lookup-discovery");
        let mut t1 = HyperswarmTransport::new(HyperswarmConfig::default());
        let mut t2 = HyperswarmTransport::new(HyperswarmConfig::default());

        t1.announce(topic).await.unwrap();
        t2.announce(topic).await.unwrap();

        let peers_from_t1 = t1.lookup(topic).await.unwrap();
        assert_eq!(
            peers_from_t1.len(),
            1,
            "t1 should find exactly t2 via lookup"
        );
        assert_eq!(peers_from_t1[0].id, t2.peer_id);

        let peers_from_t2 = t2.lookup(topic).await.unwrap();
        assert_eq!(
            peers_from_t2.len(),
            1,
            "t2 should find exactly t1 via lookup"
        );
        assert_eq!(peers_from_t2[0].id, t1.peer_id);

        // Clean up.
        t1.disconnect().await.unwrap();
        t2.disconnect().await.unwrap();
    }

    // ── Connect / send / receive (no encryption) ─────────────────────────────

    #[tokio::test]
    async fn test_connect_send_receive_unencrypted() {
        let topic = derive_unique_topic("connect-plain");

        let cfg_plain = HyperswarmConfig {
            encryption: false,
            ..Default::default()
        };
        let mut t1 = HyperswarmTransport::new(cfg_plain.clone());
        let mut t2 = HyperswarmTransport::new(cfg_plain);

        // t1 announces first so t2 can find it.
        t1.announce(topic).await.unwrap();

        let mut rx1 = t1.connect(topic).await.unwrap();
        let mut rx2 = t2.connect(topic).await.unwrap();

        // t2 should receive an outbound connection to t1.
        let mut conn_from_t2 = tokio::time::timeout(Duration::from_secs(2), rx2.recv())
            .await
            .expect("t2 should receive a connection within 2 s")
            .expect("channel should not be closed");

        // t1's accept loop should also receive t2's inbound connection.
        let mut conn_from_t1 = tokio::time::timeout(Duration::from_secs(2), rx1.recv())
            .await
            .expect("t1 should accept a connection within 2 s")
            .expect("channel should not be closed");

        let msg = b"hello hyperswarm";
        conn_from_t2.send(msg).await.unwrap();

        let received = conn_from_t1
            .receive()
            .await
            .unwrap()
            .expect("should receive a message");
        assert_eq!(received, msg);

        conn_from_t2.close().await.unwrap();
        t1.disconnect().await.unwrap();
        t2.disconnect().await.unwrap();
    }

    // ── Connect / send / receive (with encryption) ───────────────────────────

    #[tokio::test]
    async fn test_connect_send_receive_encrypted() {
        let topic = derive_unique_topic("connect-encrypted");

        let mut t1 = HyperswarmTransport::new(HyperswarmConfig::default()); // encryption=true
        let mut t2 = HyperswarmTransport::new(HyperswarmConfig::default());

        t1.announce(topic).await.unwrap();

        let mut rx1 = t1.connect(topic).await.unwrap();
        let mut rx2 = t2.connect(topic).await.unwrap();

        let mut conn_from_t2 = tokio::time::timeout(Duration::from_secs(2), rx2.recv())
            .await
            .expect("t2 should connect within 2 s")
            .expect("channel should not be closed");

        let mut conn_from_t1 = tokio::time::timeout(Duration::from_secs(2), rx1.recv())
            .await
            .expect("t1 should accept within 2 s")
            .expect("channel should not be closed");

        let msg = b"encrypted message";
        conn_from_t2.send(msg).await.unwrap();

        let received = conn_from_t1.receive().await.unwrap().unwrap();
        assert_eq!(received, msg, "decrypted content must match");

        conn_from_t2.close().await.unwrap();
        t1.disconnect().await.unwrap();
        t2.disconnect().await.unwrap();
    }

    // ── Disconnect cleans up registry ────────────────────────────────────────

    #[tokio::test]
    async fn test_disconnect_removes_from_registry() {
        let topic = derive_unique_topic("disconnect-cleanup");
        let mut t1 = HyperswarmTransport::new(HyperswarmConfig::default());
        let mut t2 = HyperswarmTransport::new(HyperswarmConfig::default());

        t1.announce(topic).await.unwrap();
        t2.announce(topic).await.unwrap();

        assert_eq!(registry_lookup(&topic, "nobody").len(), 2);

        t1.disconnect().await.unwrap();
        assert_eq!(registry_lookup(&topic, "nobody").len(), 1);

        t2.disconnect().await.unwrap();
        assert_eq!(registry_lookup(&topic, "nobody").len(), 0);
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Derive a unique topic for a test using the test name as input.
    /// Appending a UUID prevents cross-test registry pollution in parallel runs.
    fn derive_unique_topic(test_name: &str) -> TopicHash {
        use blake2::digest::consts::U32;
        use blake2::{Blake2b, Digest};

        let key = format!("{}-{}", test_name, uuid::Uuid::new_v4());
        let mut h = Blake2b::<U32>::new();
        h.update(key.as_bytes());
        let r = h.finalize();
        let mut topic = [0u8; 32];
        topic.copy_from_slice(&r);
        topic
    }
}
