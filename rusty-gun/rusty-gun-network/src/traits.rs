//! Network engine traits for Rusty Gun

use crate::error::Result;
use crate::config::{NetworkConfig, NetworkEvent, NetworkMessage, PeerInfo, NetworkStats};
use rusty_gun_core::{Node, NodeId, types::*};
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Main network engine trait
#[async_trait::async_trait]
pub trait NetworkEngine: Send + Sync {
    /// Initialize the network engine
    async fn initialize(&mut self) -> Result<()>;

    /// Start the network engine
    async fn start(&mut self) -> Result<()>;

    /// Stop the network engine
    async fn stop(&mut self) -> Result<()>;

    /// Get network configuration
    fn get_config(&self) -> &NetworkConfig;

    /// Get network statistics
    async fn get_stats(&self) -> Result<NetworkStats>;

    /// Get event receiver
    fn get_event_receiver(&self) -> mpsc::UnboundedReceiver<NetworkEvent>;

    /// Send message to a specific peer
    async fn send_message(&self, peer_id: &str, message: NetworkMessage) -> Result<()>;

    /// Broadcast message to all connected peers
    async fn broadcast_message(&self, message: NetworkMessage) -> Result<()>;

    /// Connect to a peer
    async fn connect_peer(&self, address: &str) -> Result<String>;

    /// Disconnect from a peer
    async fn disconnect_peer(&self, peer_id: &str) -> Result<()>;

    /// Get connected peers
    async fn get_connected_peers(&self) -> Result<Vec<PeerInfo>>;

    /// Get peer information
    async fn get_peer_info(&self, peer_id: &str) -> Result<Option<PeerInfo>>;

    /// Check if peer is connected
    async fn is_peer_connected(&self, peer_id: &str) -> bool;
}

/// Peer management trait
#[async_trait::async_trait]
pub trait PeerManager: Send + Sync {
    /// Add a peer
    async fn add_peer(&self, peer_info: PeerInfo) -> Result<()>;

    /// Remove a peer
    async fn remove_peer(&self, peer_id: &str) -> Result<()>;

    /// Update peer information
    async fn update_peer(&self, peer_id: &str, peer_info: PeerInfo) -> Result<()>;

    /// Get all peers
    async fn get_all_peers(&self) -> Result<Vec<PeerInfo>>;

    /// Get peers by status
    async fn get_peers_by_status(&self, status: crate::config::PeerStatus) -> Result<Vec<PeerInfo>>;

    /// Find peers by capability
    async fn find_peers_by_capability(&self, capability: &str) -> Result<Vec<PeerInfo>>;

    /// Get peer count
    async fn get_peer_count(&self) -> Result<usize>;
}

/// Data synchronization trait
#[async_trait::async_trait]
pub trait SyncEngine: Send + Sync {
    /// Start automatic synchronization
    async fn start_sync(&mut self) -> Result<()>;

    /// Stop automatic synchronization
    async fn stop_sync(&mut self) -> Result<()>;

    /// Sync with a specific peer
    async fn sync_with_peer(&self, peer_id: &str) -> Result<()>;

    /// Sync with all connected peers
    async fn sync_with_all_peers(&self) -> Result<()>;

    /// Send operations to a peer
    async fn send_operations(&self, peer_id: &str, operations: Vec<OperationWithMetadata>) -> Result<()>;

    /// Receive operations from a peer
    async fn receive_operations(&self, peer_id: &str) -> Result<Vec<OperationWithMetadata>>;

    /// Get sync status
    async fn get_sync_status(&self) -> Result<SyncStatus>;

    /// Get pending operations
    async fn get_pending_operations(&self) -> Result<Vec<OperationWithMetadata>>;

    /// Mark operations as synced
    async fn mark_operations_synced(&self, operation_ids: Vec<uuid::Uuid>) -> Result<()>;
}

/// Sync status information
#[derive(Debug, Clone)]
pub struct SyncStatus {
    /// Whether sync is active
    pub is_active: bool,
    /// Number of peers being synced with
    pub active_peers: usize,
    /// Last sync time
    pub last_sync: Option<chrono::DateTime<chrono::Utc>>,
    /// Pending operations count
    pub pending_operations: usize,
    /// Sync progress (0.0 to 1.0)
    pub progress: f32,
}

/// Network discovery trait
#[async_trait::async_trait]
pub trait DiscoveryEngine: Send + Sync {
    /// Start peer discovery
    async fn start_discovery(&mut self) -> Result<()>;

    /// Stop peer discovery
    async fn stop_discovery(&mut self) -> Result<()>;

    /// Discover peers on the network
    async fn discover_peers(&self) -> Result<Vec<PeerInfo>>;

    /// Announce this peer to the network
    async fn announce_peer(&self) -> Result<()>;

    /// Get discovered peers
    async fn get_discovered_peers(&self) -> Result<Vec<PeerInfo>>;

    /// Add bootstrap peer
    async fn add_bootstrap_peer(&self, address: &str) -> Result<()>;

    /// Remove bootstrap peer
    async fn remove_bootstrap_peer(&self, address: &str) -> Result<()>;
}

/// Message handler trait
#[async_trait::async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle incoming message
    async fn handle_message(&self, from_peer: &str, message: NetworkMessage) -> Result<()>;

    /// Handle peer connection
    async fn handle_peer_connected(&self, peer_id: &str, peer_info: PeerInfo) -> Result<()>;

    /// Handle peer disconnection
    async fn handle_peer_disconnected(&self, peer_id: &str, reason: &str) -> Result<()>;

    /// Handle peer discovery
    async fn handle_peer_discovered(&self, peer_info: PeerInfo) -> Result<()>;

    /// Handle sync request
    async fn handle_sync_request(&self, from_peer: &str, version_vector: VersionVector) -> Result<()>;

    /// Handle sync response
    async fn handle_sync_response(&self, from_peer: &str, operations: Vec<OperationWithMetadata>) -> Result<()>;
}

/// Network factory trait
pub trait NetworkFactory {
    type Engine: NetworkEngine;
    type PeerManager: PeerManager;
    type SyncEngine: SyncEngine;
    type DiscoveryEngine: DiscoveryEngine;

    /// Create a new network engine
    fn create_network_engine(&self, config: NetworkConfig) -> Result<Self::Engine>;

    /// Create a new peer manager
    fn create_peer_manager(&self) -> Result<Self::PeerManager>;

    /// Create a new sync engine
    fn create_sync_engine(&self) -> Result<Self::SyncEngine>;

    /// Create a new discovery engine
    fn create_discovery_engine(&self, config: NetworkConfig) -> Result<Self::DiscoveryEngine>;
}


