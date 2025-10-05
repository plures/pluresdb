//! QUIC network implementation for Rusty Gun

use crate::{
    error::{Result, NetworkError},
    traits::{NetworkEngine, PeerManager, SyncEngine},
    config::{NetworkConfig, NetworkEvent, NetworkMessage, PeerInfo, NetworkStats, PeerStatus},
};
use quinn::{Endpoint, ServerConfig, ClientConfig, Connection, RecvStream, SendStream};
use rusty_gun_core::types::*;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// QUIC network engine
pub struct QuicNetworkEngine {
    config: NetworkConfig,
    endpoint: Option<Endpoint>,
    connections: Arc<RwLock<HashMap<String, Connection>>>,
    event_sender: mpsc::UnboundedSender<NetworkEvent>,
    event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<NetworkEvent>>>>,
    peer_manager: Arc<dyn PeerManager>,
    sync_engine: Arc<dyn SyncEngine>,
    stats: Arc<RwLock<NetworkStats>>,
    is_running: Arc<RwLock<bool>>,
}

impl QuicNetworkEngine {
    /// Create a new QUIC network engine
    pub fn new(
        config: NetworkConfig,
        peer_manager: Arc<dyn PeerManager>,
        sync_engine: Arc<dyn SyncEngine>,
    ) -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        
        Self {
            config,
            endpoint: None,
            connections: Arc::new(RwLock::new(HashMap::new())),
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

    /// Create QUIC server configuration
    fn create_server_config(&self) -> Result<ServerConfig> {
        // Generate self-signed certificate for development
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])
            .map_err(|e| NetworkError::Configuration(format!("Failed to generate certificate: {}", e)))?;
        
        let key = rustls::PrivateKey(cert.serialize_private_key_der());
        let cert_chain = vec![rustls::Certificate(cert.serialize_der()
            .map_err(|e| NetworkError::Configuration(format!("Failed to serialize certificate: {}", e)))?)];

        let mut server_crypto = rustls::ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(cert_chain, key)
            .map_err(|e| NetworkError::Configuration(format!("Failed to create server config: {}", e)))?;

        server_crypto.alpn_protocols = vec![b"rusty-gun".to_vec()];

        Ok(ServerConfig::with_crypto(Arc::new(server_crypto)))
    }

    /// Create QUIC client configuration
    fn create_client_config(&self) -> Result<ClientConfig> {
        let mut client_crypto = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(AllowAnyCertVerifier::new()))
            .with_no_client_auth();

        client_crypto.alpn_protocols = vec![b"rusty-gun".to_vec()];

        Ok(ClientConfig::new(Arc::new(client_crypto)))
    }

    /// Handle incoming connection
    async fn handle_connection(&self, connection: Connection) -> Result<()> {
        let peer_id = connection.remote_address().to_string();
        info!("New QUIC connection from: {}", peer_id);

        // Add to connections
        {
            let mut connections = self.connections.write().await;
            connections.insert(peer_id.clone(), connection.clone());
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.connected_peers += 1;
        }

        // Send peer connected event
        let _ = self.event_sender.send(NetworkEvent::PeerConnected {
            peer_id: peer_id.clone(),
            address: connection.remote_address().to_string(),
        });

        // Handle incoming streams
        while let Some(stream) = connection.accept_bi().await {
            match stream {
                Ok((send_stream, recv_stream)) => {
                    if let Err(e) = self.handle_stream(peer_id.clone(), send_stream, recv_stream).await {
                        error!("Error handling stream: {}", e);
                    }
                }
                Err(e) => {
                    error!("Error accepting stream: {}", e);
                    break;
                }
            }
        }

        // Connection closed
        {
            let mut connections = self.connections.write().await;
            connections.remove(&peer_id);
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.connected_peers = stats.connected_peers.saturating_sub(1);
        }

        // Send peer disconnected event
        let _ = self.event_sender.send(NetworkEvent::PeerDisconnected {
            peer_id,
            reason: "Connection closed".to_string(),
        });

        Ok(())
    }

    /// Handle incoming stream
    async fn handle_stream(
        &self,
        peer_id: String,
        mut send_stream: SendStream,
        mut recv_stream: RecvStream,
    ) -> Result<()> {
        let mut buffer = vec![0u8; 4096];
        
        loop {
            match recv_stream.read(&mut buffer).await {
                Ok(Some(bytes_read)) => {
                    // Update stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.bytes_received += bytes_read as u64;
                    }

                    // Parse message
                    let message_data = &buffer[..bytes_read];
                    match self.parse_message(message_data) {
                        Ok(message) => {
                            // Update stats
                            {
                                let mut stats = self.stats.write().await;
                                stats.messages_received += 1;
                            }

                            // Send message received event
                            let _ = self.event_sender.send(NetworkEvent::MessageReceived {
                                from_peer: peer_id.clone(),
                                message: message.clone(),
                            });

                            // Handle message
                            if let Err(e) = self.handle_message(&peer_id, message).await {
                                error!("Error handling message: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse message: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    // Stream ended
                    break;
                }
                Err(e) => {
                    error!("Error reading stream: {}", e);
                    break;
                }
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

        Err(NetworkError::InvalidMessage("Failed to parse message".to_string()))
    }

    /// Serialize network message to bytes
    fn serialize_message(&self, message: &NetworkMessage) -> Result<Vec<u8>> {
        // Use JSON for now (can be made configurable)
        serde_json::to_vec(message)
            .map_err(|e| NetworkError::Serialization(e))
    }

    /// Handle incoming message
    async fn handle_message(&self, from_peer: &str, message: NetworkMessage) -> Result<()> {
        match message {
            NetworkMessage::Heartbeat { peer_id, timestamp } => {
                debug!("Received heartbeat from {} at {}", peer_id, timestamp);
                // Update peer last seen
                if let Some(peer_info) = self.peer_manager.get_peer_info(from_peer).await? {
                    let mut updated_peer = peer_info;
                    updated_peer.last_seen = chrono::Utc::now();
                    self.peer_manager.update_peer(from_peer, updated_peer).await?;
                }
            }
            NetworkMessage::PeerDiscovery { peer_id, address, public_key, capabilities } => {
                debug!("Received peer discovery from {}", peer_id);
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
                debug!("Received data sync from {} with {} operations", from_peer, operations.len());
                // Handle sync operations
                self.sync_engine.receive_operations(&from_peer).await?;
            }
            NetworkMessage::SyncRequest { from_peer, version_vector } => {
                debug!("Received sync request from {}", from_peer);
                // Handle sync request
                self.sync_engine.sync_with_peer(&from_peer).await?;
            }
            NetworkMessage::SyncResponse { from_peer, operations, version_vector } => {
                debug!("Received sync response from {} with {} operations", from_peer, operations.len());
                // Handle sync response
                self.sync_engine.receive_operations(&from_peer).await?;
            }
            NetworkMessage::Error { from_peer, error, code } => {
                warn!("Received error from {}: {} (code: {})", from_peer, error, code);
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl NetworkEngine for QuicNetworkEngine {
    async fn initialize(&mut self) -> Result<()> {
        info!("Initializing QUIC network engine on port {}", self.config.port);
        
        // Create server configuration
        let server_config = self.create_server_config()?;
        
        // Create endpoint
        let bind_addr = format!("{}:{}", self.config.bind_address, self.config.port);
        let endpoint = Endpoint::server(server_config, bind_addr.parse()?)
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to create QUIC endpoint: {}", e)))?;

        self.endpoint = Some(endpoint);
        info!("QUIC network engine initialized");
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let endpoint = self.endpoint.as_ref()
            .ok_or_else(|| NetworkError::Configuration("Network engine not initialized".to_string()))?;

        // Start accepting connections
        let connections = self.connections.clone();
        let event_sender = self.event_sender.clone();
        let stats = self.stats.clone();
        let is_running = self.is_running.clone();

        {
            let mut running = is_running.write().await;
            *running = true;
        }

        let engine = self.clone();
        tokio::spawn(async move {
            while *is_running.read().await {
                if let Some(connection) = endpoint.accept().await {
                    if let Err(e) = engine.handle_connection(connection).await {
                        error!("Error handling connection: {}", e);
                    }
                }
            }
        });

        info!("QUIC network engine started");
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // Close all connections
        {
            let mut connections = self.connections.write().await;
            for (peer_id, connection) in connections.drain() {
                connection.close(0u32.into(), b"Shutting down");
                debug!("Closed connection to peer: {}", peer_id);
            }
        }

        info!("QUIC network engine stopped");
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
        let connections = self.connections.read().await;
        let connection = connections.get(peer_id)
            .ok_or_else(|| NetworkError::PeerNotFound { peer_id: peer_id.to_string() })?;

        let message_data = self.serialize_message(&message)?;
        
        if message_data.len() > crate::MAX_MESSAGE_SIZE {
            return Err(NetworkError::MessageTooLarge {
                size: message_data.len(),
                max: crate::MAX_MESSAGE_SIZE,
            });
        }

        let (mut send_stream, _) = connection.open_bi().await
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to open stream: {}", e)))?;

        send_stream.write_all(&message_data).await
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to send message: {}", e)))?;

        send_stream.finish().await
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to finish stream: {}", e)))?;

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.bytes_sent += message_data.len() as u64;
            stats.messages_sent += 1;
        }

        debug!("Sent message to peer: {}", peer_id);
        Ok(())
    }

    async fn broadcast_message(&self, message: NetworkMessage) -> Result<()> {
        let connections = self.connections.read().await;
        let peer_ids: Vec<String> = connections.keys().cloned().collect();
        drop(connections);

        for peer_id in peer_ids {
            if let Err(e) = self.send_message(&peer_id, message.clone()).await {
                error!("Failed to send message to peer {}: {}", peer_id, e);
            }
        }

        Ok(())
    }

    async fn connect_peer(&self, address: &str) -> Result<String> {
        let endpoint = self.endpoint.as_ref()
            .ok_or_else(|| NetworkError::Configuration("Network engine not initialized".to_string()))?;

        let client_config = self.create_client_config()?;
        let connection = endpoint.connect_with(client_config, address.parse()?, "rusty-gun")
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to connect to {}: {}", address, e)))?
            .await
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to establish connection to {}: {}", address, e)))?;

        let peer_id = connection.remote_address().to_string();
        
        // Add to connections
        {
            let mut connections = self.connections.write().await;
            connections.insert(peer_id.clone(), connection);
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.connected_peers += 1;
        }

        info!("Connected to peer: {} at {}", peer_id, address);
        Ok(peer_id)
    }

    async fn disconnect_peer(&self, peer_id: &str) -> Result<()> {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.remove(peer_id) {
            connection.close(0u32.into(), b"Disconnecting");
            
            // Update stats
            let mut stats = self.stats.write().await;
            stats.connected_peers = stats.connected_peers.saturating_sub(1);
            
            debug!("Disconnected from peer: {}", peer_id);
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
        self.connections.read().await.contains_key(peer_id)
    }
}

impl Clone for QuicNetworkEngine {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            endpoint: None, // Endpoint can't be cloned
            connections: self.connections.clone(),
            event_sender: self.event_sender.clone(),
            event_receiver: self.event_receiver.clone(),
            peer_manager: self.peer_manager.clone(),
            sync_engine: self.sync_engine.clone(),
            stats: self.stats.clone(),
            is_running: self.is_running.clone(),
        }
    }
}

/// Allow any certificate verifier for development
struct AllowAnyCertVerifier;

impl AllowAnyCertVerifier {
    fn new() -> Self {
        Self
    }
}

impl rustls::client::ServerCertVerifier for AllowAnyCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quic_network_engine_creation() {
        let config = NetworkConfig::default();
        let peer_manager = Arc::new(MockPeerManager::new());
        let sync_engine = Arc::new(MockSyncEngine::new());
        
        let mut engine = QuicNetworkEngine::new(config, peer_manager, sync_engine);
        assert!(engine.initialize().await.is_ok());
    }
}

// Mock implementations for testing
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


