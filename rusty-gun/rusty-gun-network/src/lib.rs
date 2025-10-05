//! # Rusty Gun Network
//! 
//! P2P networking layer for Rusty Gun with WebRTC, QUIC, and LibP2P support.
//! Provides distributed, local-first networking capabilities.

pub mod quic;
pub mod webrtc;
pub mod libp2p_net;
pub mod discovery;
pub mod sync;
pub mod encryption;
pub mod error;
pub mod traits;
pub mod config;

// Re-export implementations
pub use quic::QuicNetworkEngine;
pub use webrtc::WebRTCNetworkEngine;
pub use libp2p_net::LibP2PNetworkEngine;
pub use discovery::{MdnsDiscovery, DhtDiscovery, CombinedDiscovery};
pub use sync::DataSyncEngine;
pub use encryption::{NetworkEncryption, KeyExchange};

// Re-export main types
pub use traits::{NetworkEngine, PeerManager, SyncEngine};
pub use error::{NetworkError, Result};
pub use config::NetworkConfig;

/// Network protocol version
pub const PROTOCOL_VERSION: &str = "1.0.0";

/// Default network port
pub const DEFAULT_PORT: u16 = 34569;

/// Maximum message size in bytes
pub const MAX_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB

/// Connection timeout in seconds
pub const CONNECTION_TIMEOUT: u64 = 30;

/// Heartbeat interval in seconds
pub const HEARTBEAT_INTERVAL: u64 = 30;