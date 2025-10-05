//! LibP2P network implementation for Rusty Gun

use crate::{
    error::{Result, NetworkError},
    traits::{NetworkEngine, PeerManager, SyncEngine},
    config::{NetworkConfig, NetworkEvent, NetworkMessage, PeerInfo, NetworkStats, PeerStatus},
};
use rusty_gun_core::types::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// LibP2P network engine
pub struct LibP2PNetworkEngine {
    config: NetworkConfig,
    peer_connections: Arc<RwLock<HashMap<String, LibP2PPeerConnection>>>,
    event_sender: mpsc::UnboundedSender<NetworkEvent>,
    event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<NetworkEvent>>>>,
    peer_manager: Arc<dyn PeerManager>,
    sync_engine: Arc<dyn SyncEngine>,
    stats: Arc<RwLock<NetworkStats>>,
    is_running: Arc<RwLock<bool>>,
}

/// LibP2P peer connection wrapper
#[derive(Debug, Clone)]
pub struct LibP2PPeerConnection {
    /// Peer ID
    pub peer_id: String,
    /// Connection state
    pub state: LibP2PConnectionState,
    /// Multiaddress
    pub multiaddress: String,
}

/// LibP2P connection state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibP2PConnectionState {
    New,
    Connecting,
    Connected,
    Disconnected,
    Failed,
    Closed,
}

impl LibP2PNetworkEngine {
    /// Create a new LibP2P network engine
    pub fn new(
        config: NetworkConfig,
        peer_manager: Arc<dyn PeerManager>,
        sync_engine: Arc<dyn SyncEngine>,
    ) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Self {
            config,
            peer_connections: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            peer_manager,
            sync_engine,
            stats: Arc::new(RwLock::new(NetworkStats {
                connected_peers: 0,
                discovered_peers: 0,
                bytes_sent: 0,
                bytes_received: 0,
                messages_sent: 0,
                messages_received: 0,
                uptime: 0,
                last_sync: None,
            })),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Create a new peer connection
    async fn create_peer_connection(&self, peer_id: String, multiaddress: String) -> Result<LibP2PPeerConnection> {
        // In a real implementation, this would create an actual LibP2P peer connection
        // For now, we'll create a mock connection
        let connection = LibP2PPeerConnection {
            peer_id: peer_id.clone(),
            state: LibP2PConnectionState::New,
            multiaddress,
        };

        info!("Created LibP2P peer connection for: {}", peer_id);
        Ok(connection)
    }

    /// Handle peer connection state change
    async fn handle_connection_state_change(&self, peer_id: &str, state: LibP2PConnectionState) -> Result<()> {
        let mut connections = self.peer_connections.write().await;
        if let Some(connection) = connections.get_mut(peer_id) {
            connection.state = state.clone();
        }

        match state {
            LibP2PConnectionState::Connected => {
                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.connected_peers += 1;
                }

                // Send peer connected event
                let _ = self.event_sender.send(NetworkEvent::PeerConnected {
                    peer_id: peer_id.to_string(),
                    address: "libp2p".to_string(),
                });

                info!("LibP2P peer connected: {}", peer_id);
            }
            LibP2PConnectionState::Disconnected | LibP2PConnectionState::Failed | LibP2PConnectionState::Closed => {
                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    stats.connected_peers = stats.connected_peers.saturating_sub(1);
                }

                // Send peer disconnected event
                let _ = self.event_sender.send(NetworkEvent::PeerDisconnected {
                    peer_id: peer_id.to_string(),
                    reason: format!("Connection state: {:?}", state),
                });

                // Remove connection
                connections.remove(peer_id);

                info!("LibP2P peer disconnected: {}", peer_id);
            }
            _ => {
                debug!("LibP2P peer connection state changed: {} -> {:?}", peer_id, state);
            }
        }

        Ok(())
    }

    /// Handle incoming message
    async fn handle_message(&self, from_peer: &str, message: NetworkMessage) -> Result<()> {
        match message {
            NetworkMessage::Heartbeat { peer_id, timestamp } => {
                debug!("Received LibP2P heartbeat from {} at {}", peer_id, timestamp);
                // Update peer last seen
                if let Some(peer_info) = self.peer_manager.get_peer_info(from_peer).await? {
                    let mut updated_peer = peer_info;
                    updated_peer.last_seen = chrono::Utc::now();
                    self.peer_manager.update_peer(from_peer, updated_peer).await?;
                }
            }
            NetworkMessage::PeerDiscovery { peer_id, address, public_key, capabilities } => {
                debug!("Received LibP2P peer discovery from {}", peer_id);
                let peer_info = PeerInfo {
                    id: peer_id,
                    address,
                    public_key,
                    status: PeerStatus::Disconnected,
                    last_seen: chrono::Utc::now(),
                    capabilities,
                    metadata: HashMap::new(),
                };
                self.peer_manager.add_peer(peer_info).await?;
            }
            NetworkMessage::DataSync { from_peer, operations, version_vector } => {
                debug!("Received LibP2P data sync from {} with {} operations", from_peer, operations.len());
                // Handle sync operations
                self.sync_engine.receive_operations(&from_peer).await?;
            }
            NetworkMessage::SyncRequest { from_peer, version_vector } => {
                debug!("Received LibP2P sync request from {}", from_peer);
                // Handle sync request
                self.sync_engine.sync_with_peer(&from_peer).await?;
            }
            NetworkMessage::SyncResponse { from_peer, operations, version_vector } => {
                debug!("Received LibP2P sync response from {} with {} operations", from_peer, operations.len());
                // Handle sync response
                self.sync_engine.receive_operations(&from_peer).await?;
            }
            NetworkMessage::Error { from_peer, error, code } => {
                warn!("Received LibP2P error from {}: {} (code: {})", from_peer, error, code);
            }
        }

        Ok(())
    }

    /// Parse network message from bytes
    fn parse_message(&self, data: &[u8]) -> Result<NetworkMessage> {
        // Try JSON first
        if let Ok(message) = serde_json::from_slice(data) {
            return Ok(message);
        }

        // Try bincode
        if let Ok(message) = bincode::deserialize(data) {
            return Ok(message);
        }

        Err(NetworkError::InvalidMessage("Failed to parse LibP2P message".to_string()))
    }

    /// Serialize network message to bytes
    fn serialize_message(&self, message: &NetworkMessage) -> Result<Vec<u8>> {
        // Use JSON for now (can be made configurable)
        serde_json::to_vec(message)
            .map_err(|e| NetworkError::Serialization(e))
    }
}

#[async_trait::async_trait]
impl NetworkEngine for LibP2PNetworkEngine {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing LibP2P network engine");
        
        // In a real implementation, this would initialize LibP2P
        // For now, we'll just log that we're initialized
        info!("LibP2P network engine initialized");
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = true;
        }

        info!("LibP2P network engine started");
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // Close all connections
        {
            let mut connections = self.peer_connections.write().await;
            for (peer_id, connection) in connections.drain() {
                debug!("Closed LibP2P connection to peer: {}", peer_id);
            }
        }

        info!("LibP2P network engine stopped");
        Ok(())
    }

    fn get_config(&self) -> &NetworkConfig {
        &self.config
    }

    async fn get_stats(&self) -> Result<NetworkStats> {
        Ok(self.stats.read().await.clone())
    }

    fn get_event_receiver(&self) -> mpsc::UnboundedReceiver<NetworkEvent> {
        self.event_receiver.write().await.take()
            .expect("Event receiver already taken")
    }

    async fn send_message(&self, peer_id: &str, message: NetworkMessage) -> Result<()> {
        let connections = self.peer_connections.read().await;
        let connection = connections.get(peer_id)
            .ok_or_else(|| NetworkError::PeerNotFound { peer_id: peer_id.to_string() })?;

        let message_data = self.serialize_message(&message)?;
        
        if message_data.len() > crate::MAX_MESSAGE_SIZE {
            return Err(NetworkError::MessageTooLarge {
                size: message_data.len(),
                max: crate::MAX_MESSAGE_SIZE,
            });
        }

        // In a real implementation, this would send data through LibP2P
        // For now, we'll just simulate sending
        debug!("Sending LibP2P message to peer {} via {}", peer_id, connection.multiaddress);

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.bytes_sent += message_data.len() as u64;
            stats.messages_sent += 1;
        }

        debug!("Sent LibP2P message to peer: {}", peer_id);
        Ok(())
    }

    async fn broadcast_message(&self, message: NetworkMessage) -> Result<()> {
        let connections = self.peer_connections.read().await;
        let peer_ids: Vec<String> = connections.keys().cloned().collect();
        drop(connections);

        for peer_id in peer_ids {
            if let Err(e) = self.send_message(&peer_id, message.clone()).await {
                error!("Failed to send LibP2P message to peer {}: {}", peer_id, e);
            }
        }

        Ok(())
    }

    async fn connect_peer(&self, address: &str) -> Result<String> {
        // In a real implementation, this would establish a LibP2P connection
        // For now, we'll create a mock connection
        let peer_id = format!("libp2p-{}", uuid::Uuid::new_v4());
        
        let connection = self.create_peer_connection(peer_id.clone(), address.to_string()).await?;
        
        // Add to connections
        {
            let mut connections = self.peer_connections.write().await;
            connections.insert(peer_id.clone(), connection);
        }

        // Simulate connection establishment
        self.handle_connection_state_change(&peer_id, LibP2PConnectionState::Connected).await?;

        info!("Connected to LibP2P peer: {} at {}", peer_id, address);
        Ok(peer_id)
    }

    async fn disconnect_peer(&self, peer_id: &str) -> Result<()> {
        let mut connections = self.peer_connections.write().await;
        if connections.remove(peer_id).is_some() {
            // Update stats
            let mut stats = self.stats.write().await;
            stats.connected_peers = stats.connected_peers.saturating_sub(1);
            
            debug!("Disconnected from LibP2P peer: {}", peer_id);
        }

        Ok(())
    }

    async fn get_connected_peers(&self) -> Result<Vec<PeerInfo>> {
        self.peer_manager.get_peers_by_status(PeerStatus::Connected).await
    }

    async fn get_peer_info(&self, peer_id: &str) -> Result<Option<PeerInfo>> {
        self.peer_manager.get_peer_info(peer_id).await
    }

    async fn is_peer_connected(&self, peer_id: &str) -> bool {
        self.peer_connections.read().await.contains_key(peer_id)
    }
}

impl Clone for LibP2PNetworkEngine {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            peer_connections: self.peer_connections.clone(),
            event_sender: self.event_sender.clone(),
            event_receiver: self.event_receiver.clone(),
            peer_manager: self.peer_manager.clone(),
            sync_engine: self.sync_engine.clone(),
            stats: self.stats.clone(),
            is_running: self.is_running.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_libp2p_network_engine_creation() {
        let config = NetworkConfig::default();
        let peer_manager = Arc::new(MockPeerManager::new());
        let sync_engine = Arc::new(MockSyncEngine::new());
        
        let mut engine = LibP2PNetworkEngine::new(config, peer_manager, sync_engine);
        assert!(engine.initialize().await.is_ok());
    }
}

// Mock implementations for testing (same as in quic.rs and webrtc.rs)
struct MockPeerManager;

impl MockPeerManager {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl PeerManager for MockPeerManager {
    async fn add_peer(&self, _peer_info: PeerInfo) -> Result<()> {
        Ok(())
    }

    async fn remove_peer(&self, _peer_id: &str) -> Result<()> {
        Ok(())
    }

    async fn update_peer(&self, _peer_id: &str, _peer_info: PeerInfo) -> Result<()> {
        Ok(())
    }

    async fn get_all_peers(&self) -> Result<Vec<PeerInfo>> {
        Ok(Vec::new())
    }

    async fn get_peers_by_status(&self, _status: PeerStatus) -> Result<Vec<PeerInfo>> {
        Ok(Vec::new())
    }

    async fn find_peers_by_capability(&self, _capability: &str) -> Result<Vec<PeerInfo>> {
        Ok(Vec::new())
    }

    async fn get_peer_count(&self) -> Result<usize> {
        Ok(0)
    }

    async fn get_peer_info(&self, _peer_id: &str) -> Result<Option<PeerInfo>> {
        Ok(None)
    }
}

struct MockSyncEngine;

impl MockSyncEngine {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl SyncEngine for MockSyncEngine {
    async fn start_sync(&mut self) -> Result<()> {
        Ok(())
    }

    async fn stop_sync(&mut self) -> Result<()> {
        Ok(())
    }

    async fn sync_with_peer(&self, _peer_id: &str) -> Result<()> {
        Ok(())
    }

    async fn sync_with_all_peers(&self) -> Result<()> {
        Ok(())
    }

    async fn send_operations(&self, _peer_id: &str, _operations: Vec<OperationWithMetadata>) -> Result<()> {
        Ok(())
    }

    async fn receive_operations(&self, _peer_id: &str) -> Result<Vec<OperationWithMetadata>> {
        Ok(Vec::new())
    }

    async fn get_sync_status(&self) -> Result<SyncStatus> {
        Ok(SyncStatus {
            is_active: false,
            active_peers: 0,
            last_sync: None,
            pending_operations: 0,
            progress: 0.0,
        })
    }

    async fn get_pending_operations(&self) -> Result<Vec<OperationWithMetadata>> {
        Ok(Vec::new())
    }

    async fn mark_operations_synced(&self, _operation_ids: Vec<uuid::Uuid>) -> Result<()> {
        Ok(())
    }
}


