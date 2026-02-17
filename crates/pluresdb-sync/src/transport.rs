//! Pluggable transport layer for P2P synchronization.
//!
//! This module defines the core Transport trait that all sync transports
//! must implement. Currently supports:
//! - Hyperswarm: DHT-based P2P with UDP holepunching
//! - Relay: WebSocket relay for corporate networks
//! - Disabled: No sync (local-only mode)

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Raw bytes for message payloads
pub type MessagePayload = Vec<u8>;

/// Unique identifier for a peer in the network
pub type PeerId = String;

/// Topic hash for DHT-based discovery (32 bytes)
pub type TopicHash = [u8; 32];

/// Connection handle for bidirectional communication with a peer
#[async_trait]
pub trait Connection: Send + Sync {
    /// Send a message to the connected peer
    async fn send(&mut self, data: &[u8]) -> Result<()>;

    /// Receive the next message from the peer
    async fn receive(&mut self) -> Result<Option<MessagePayload>>;

    /// Close the connection gracefully
    async fn close(&mut self) -> Result<()>;

    /// Get the peer ID for this connection
    fn peer_id(&self) -> &PeerId;
}

/// Core transport trait for P2P synchronization
#[async_trait]
pub trait Transport: Send + Sync {
    /// Connect to peers on a specific topic
    /// Returns a receiver for incoming connections
    async fn connect(&mut self, topic: TopicHash) -> Result<mpsc::Receiver<Box<dyn Connection>>>;

    /// Announce this peer on a topic (DHT announce)
    async fn announce(&mut self, topic: TopicHash) -> Result<()>;

    /// Lookup peers for a topic (DHT lookup)
    async fn lookup(&self, topic: TopicHash) -> Result<Vec<PeerInfo>>;

    /// Disconnect and cleanup all resources
    async fn disconnect(&mut self) -> Result<()>;

    /// Get transport name (e.g., "hyperswarm", "relay", "disabled")
    fn name(&self) -> &str;
}

/// Information about a discovered peer
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerInfo {
    /// Unique peer identifier
    pub id: PeerId,
    
    /// Optional network address (IP:port or multiaddr)
    pub address: Option<String>,
    
    /// Whether the peer is currently connected
    pub connected: bool,
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Transport mode
    pub mode: TransportMode,
    
    /// Optional relay server URL for relay mode
    pub relay_url: Option<String>,
    
    /// Connection timeout in milliseconds
    pub timeout_ms: u64,
    
    /// Enable/disable encryption (should always be true in production)
    pub encryption: bool,
}

/// Transport mode selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransportMode {
    /// Hyperswarm DHT-based P2P (best for home networks)
    Hyperswarm,
    
    /// WebSocket relay (corporate-friendly, port 443)
    Relay,
    
    /// No sync (local-only mode)
    Disabled,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            mode: TransportMode::Hyperswarm,
            relay_url: None,
            timeout_ms: 30000,
            encryption: true,
        }
    }
}

/// Derive a topic hash from a database ID using BLAKE2b-256
pub fn derive_topic(database_id: &str) -> TopicHash {
    use blake2::{Blake2b, Digest};
    use blake2::digest::consts::U32;
    
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(database_id.as_bytes());
    let result = hasher.finalize();
    
    let mut topic = [0u8; 32];
    topic.copy_from_slice(&result);
    topic
}

/// Create a transport instance based on configuration
pub fn create_transport(config: TransportConfig) -> Box<dyn Transport> {
    use crate::{DisabledTransport, HyperswarmTransport, HyperswarmConfig, RelayTransport};
    
    match config.mode {
        TransportMode::Hyperswarm => {
            let hyperswarm_config = HyperswarmConfig {
                encryption: config.encryption,
                timeout_ms: config.timeout_ms,
                ..Default::default()
            };
            Box::new(HyperswarmTransport::new(hyperswarm_config))
        }
        TransportMode::Relay => {
            let relay_url = config.relay_url.unwrap_or_else(|| {
                "wss://pluresdb-relay.azurewebsites.net".to_string()
            });
            Box::new(RelayTransport::new(relay_url, config.timeout_ms))
        }
        TransportMode::Disabled => {
            Box::new(DisabledTransport::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_topic() {
        let db_id = "my-database-123";
        let topic = derive_topic(db_id);
        
        // Should always produce 32 bytes
        assert_eq!(topic.len(), 32);
        
        // Same input should produce same output
        let topic2 = derive_topic(db_id);
        assert_eq!(topic, topic2);
        
        // Different input should produce different output
        let topic3 = derive_topic("different-database");
        assert_ne!(topic, topic3);
    }

    #[test]
    fn test_transport_config_default() {
        let config = TransportConfig::default();
        assert_eq!(config.mode, TransportMode::Hyperswarm);
        assert!(config.encryption);
        assert_eq!(config.timeout_ms, 30000);
    }

    #[test]
    fn test_transport_mode_serialization() {
        let mode = TransportMode::Hyperswarm;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, r#""hyperswarm""#);
        
        let mode = TransportMode::Relay;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, r#""relay""#);
        
        let mode = TransportMode::Disabled;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, r#""disabled""#);
    }

    #[test]
    fn test_create_transport_hyperswarm() {
        let config = TransportConfig {
            mode: TransportMode::Hyperswarm,
            ..Default::default()
        };
        let transport = create_transport(config);
        assert_eq!(transport.name(), "hyperswarm");
    }

    #[test]
    fn test_create_transport_relay() {
        let config = TransportConfig {
            mode: TransportMode::Relay,
            relay_url: Some("wss://custom-relay.example.com".to_string()),
            ..Default::default()
        };
        let transport = create_transport(config);
        assert_eq!(transport.name(), "relay");
    }

    #[test]
    fn test_create_transport_disabled() {
        let config = TransportConfig {
            mode: TransportMode::Disabled,
            ..Default::default()
        };
        let transport = create_transport(config);
        assert_eq!(transport.name(), "disabled");
    }
}
