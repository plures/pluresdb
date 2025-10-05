//! Network configuration for Rusty Gun

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Network port
    pub port: u16,
    /// Bind address
    pub bind_address: String,
    /// Enable QUIC
    pub enable_quic: bool,
    /// Enable WebRTC
    pub enable_webrtc: bool,
    /// Enable LibP2P
    pub enable_libp2p: bool,
    /// Bootstrap nodes
    pub bootstrap_nodes: Vec<String>,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Maximum connections
    pub max_connections: usize,
    /// Enable encryption
    pub enable_encryption: bool,
    /// Enable NAT traversal
    pub enable_nat_traversal: bool,
    /// Enable DHT
    pub enable_dht: bool,
    /// Discovery configuration
    pub discovery: DiscoveryConfig,
    /// Sync configuration
    pub sync: SyncConfig,
}

/// Discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Enable mDNS discovery
    pub enable_mdns: bool,
    /// Enable DHT discovery
    pub enable_dht: bool,
    /// Discovery interval
    pub discovery_interval: Duration,
    /// Service name for mDNS
    pub service_name: String,
    /// Service type for mDNS
    pub service_type: String,
}

/// Sync configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Enable automatic sync
    pub enable_auto_sync: bool,
    /// Sync interval
    pub sync_interval: Duration,
    /// Maximum sync batch size
    pub max_batch_size: usize,
    /// Enable conflict resolution
    pub enable_conflict_resolution: bool,
    /// Sync timeout
    pub sync_timeout: Duration,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            port: 34569,
            bind_address: "0.0.0.0".to_string(),
            enable_quic: true,
            enable_webrtc: true,
            enable_libp2p: false,
            bootstrap_nodes: Vec::new(),
            connection_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(30),
            max_connections: 100,
            enable_encryption: true,
            enable_nat_traversal: true,
            enable_dht: true,
            discovery: DiscoveryConfig::default(),
            sync: SyncConfig::default(),
        }
    }
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            enable_mdns: true,
            enable_dht: true,
            discovery_interval: Duration::from_secs(60),
            service_name: "rusty-gun".to_string(),
            service_type: "_rusty-gun._tcp".to_string(),
        }
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            enable_auto_sync: true,
            sync_interval: Duration::from_secs(10),
            max_batch_size: 1000,
            enable_conflict_resolution: true,
            sync_timeout: Duration::from_secs(30),
        }
    }
}

/// Network statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    /// Number of connected peers
    pub connected_peers: usize,
    /// Number of discovered peers
    pub discovered_peers: usize,
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Number of messages sent
    pub messages_sent: u64,
    /// Number of messages received
    pub messages_received: u64,
    /// Uptime in seconds
    pub uptime: u64,
    /// Last sync time
    pub last_sync: Option<chrono::DateTime<chrono::Utc>>,
}

/// Peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer ID
    pub id: String,
    /// Peer address
    pub address: String,
    /// Peer public key
    pub public_key: String,
    /// Connection status
    pub status: PeerStatus,
    /// Last seen
    pub last_seen: chrono::DateTime<chrono::Utc>,
    /// Peer capabilities
    pub capabilities: Vec<String>,
    /// Peer metadata
    pub metadata: HashMap<String, String>,
}

/// Peer connection status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerStatus {
    /// Peer is connecting
    Connecting,
    /// Peer is connected
    Connected,
    /// Peer is disconnected
    Disconnected,
    /// Peer connection failed
    Failed,
}

/// Network message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkMessage {
    /// Heartbeat message
    Heartbeat {
        peer_id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Peer discovery message
    PeerDiscovery {
        peer_id: String,
        address: String,
        public_key: String,
        capabilities: Vec<String>,
    },
    /// Data sync message
    DataSync {
        from_peer: String,
        operations: Vec<rusty_gun_core::types::OperationWithMetadata>,
        version_vector: rusty_gun_core::types::VersionVector,
    },
    /// Sync request message
    SyncRequest {
        from_peer: String,
        version_vector: rusty_gun_core::types::VersionVector,
    },
    /// Sync response message
    SyncResponse {
        from_peer: String,
        operations: Vec<rusty_gun_core::types::OperationWithMetadata>,
        version_vector: rusty_gun_core::types::VersionVector,
    },
    /// Error message
    Error {
        from_peer: String,
        error: String,
        code: u32,
    },
}

/// Network event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkEvent {
    /// Peer connected
    PeerConnected {
        peer_id: String,
        address: String,
    },
    /// Peer disconnected
    PeerDisconnected {
        peer_id: String,
        reason: String,
    },
    /// Peer discovered
    PeerDiscovered {
        peer_id: String,
        address: String,
        public_key: String,
    },
    /// Message received
    MessageReceived {
        from_peer: String,
        message: NetworkMessage,
    },
    /// Sync completed
    SyncCompleted {
        peer_id: String,
        operations_count: usize,
    },
    /// Error occurred
    Error {
        error: String,
        code: u32,
    },
}


