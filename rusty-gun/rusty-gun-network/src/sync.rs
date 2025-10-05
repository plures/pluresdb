//! Data synchronization implementation for Rusty Gun

use crate::{
    error::{Result, NetworkError},
    traits::SyncEngine,
    config::SyncConfig,
};
use rusty_gun_core::types::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Data synchronization engine
pub struct DataSyncEngine {
    config: SyncConfig,
    pending_operations: Arc<RwLock<Vec<OperationWithMetadata>>>,
    synced_operations: Arc<RwLock<HashMap<uuid::Uuid, bool>>>,
    peer_version_vectors: Arc<RwLock<HashMap<String, VersionVector>>>,
    is_running: Arc<RwLock<bool>>,
    sync_tasks: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl DataSyncEngine {
    /// Create a new data sync engine
    pub fn new(config: SyncConfig) -> Self {
        Self {
            config,
            pending_operations: Arc::new(RwLock::new(Vec::new())),
            synced_operations: Arc::new(RwLock::new(HashMap::new())),
            peer_version_vectors: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
            sync_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add operations to pending queue
    pub async fn add_operations(&self, operations: Vec<OperationWithMetadata>) -> Result<()> {
        let mut pending = self.pending_operations.write().await;
        pending.extend(operations);
        info!("Added {} operations to sync queue", pending.len());
        Ok(())
    }

    /// Get operations that need to be synced with a specific peer
    async fn get_operations_for_peer(&self, peer_id: &str) -> Result<Vec<OperationWithMetadata>> {
        let pending = self.pending_operations.read().await;
        let peer_versions = self.peer_version_vectors.read().await;
        
        if let Some(peer_version) = peer_versions.get(peer_id) {
            // Filter operations that are newer than what the peer has
            let operations: Vec<OperationWithMetadata> = pending
                .iter()
                .filter(|op| {
                    // Check if this operation is newer than what the peer has
                    // This is a simplified version - in reality, you'd compare version vectors
                    true // For now, send all pending operations
                })
                .cloned()
                .collect();
            
            Ok(operations)
        } else {
            // If we don't know the peer's version, send all pending operations
            Ok(pending.clone())
        }
    }

    /// Update peer's version vector
    async fn update_peer_version_vector(&self, peer_id: &str, version_vector: VersionVector) -> Result<()> {
        let mut peer_versions = self.peer_version_vectors.write().await;
        peer_versions.insert(peer_id.to_string(), version_vector);
        Ok(())
    }

    /// Start sync task for a specific peer
    async fn start_peer_sync_task(&self, peer_id: String) -> Result<()> {
        let pending_operations = self.pending_operations.clone();
        let synced_operations = self.synced_operations.clone();
        let config = self.config.clone();
        let is_running = self.is_running.clone();

        let task = tokio::spawn(async move {
            while *is_running.read().await {
                // Get operations to sync
                let operations = {
                    let pending = pending_operations.read().await;
                    pending.clone()
                };

                if !operations.is_empty() {
                    info!("Syncing {} operations with peer {}", operations.len(), peer_id);
                    
                    // Mark operations as synced (in a real implementation, this would be done after successful sync)
                    {
                        let mut synced = synced_operations.write().await;
                        for op in &operations {
                            synced.insert(op.id, true);
                        }
                    }

                    // Remove synced operations from pending
                    {
                        let mut pending = pending_operations.write().await;
                        pending.retain(|op| !synced_operations.read().await.contains_key(&op.id));
                    }
                }

                // Wait for next sync interval
                tokio::time::sleep(config.sync_interval).await;
            }
        });

        // Store the task handle
        {
            let mut tasks = self.sync_tasks.write().await;
            tasks.insert(peer_id, task);
        }

        Ok(())
    }

    /// Stop sync task for a specific peer
    async fn stop_peer_sync_task(&self, peer_id: &str) -> Result<()> {
        let mut tasks = self.sync_tasks.write().await;
        if let Some(task) = tasks.remove(peer_id) {
            task.abort();
            debug!("Stopped sync task for peer: {}", peer_id);
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl SyncEngine for DataSyncEngine {
    async fn start_sync(&mut self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = true;
        }

        info!("Data sync engine started");
        Ok(())
    }

    async fn stop_sync(&mut self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }

        // Stop all sync tasks
        {
            let mut tasks = self.sync_tasks.write().await;
            for (peer_id, task) in tasks.drain() {
                task.abort();
                debug!("Stopped sync task for peer: {}", peer_id);
            }
        }

        info!("Data sync engine stopped");
        Ok(())
    }

    async fn sync_with_peer(&self, peer_id: &str) -> Result<()> {
        // Start sync task for this peer if not already running
        {
            let tasks = self.sync_tasks.read().await;
            if !tasks.contains_key(peer_id) {
                drop(tasks);
                self.start_peer_sync_task(peer_id.to_string()).await?;
            }
        }

        // Get operations to sync
        let operations = self.get_operations_for_peer(peer_id).await?;
        
        if !operations.is_empty() {
            info!("Syncing {} operations with peer {}", operations.len(), peer_id);
            
            // In a real implementation, this would send the operations to the peer
            // For now, we'll just mark them as synced
            {
                let mut synced = self.synced_operations.write().await;
                for op in &operations {
                    synced.insert(op.id, true);
                }
            }

            // Remove synced operations from pending
            {
                let mut pending = self.pending_operations.write().await;
                pending.retain(|op| !self.synced_operations.read().await.contains_key(&op.id));
            }
        }

        Ok(())
    }

    async fn sync_with_all_peers(&self) -> Result<()> {
        // Get all peer IDs from version vectors
        let peer_ids: Vec<String> = {
            let peer_versions = self.peer_version_vectors.read().await;
            peer_versions.keys().cloned().collect()
        };

        // Sync with each peer
        for peer_id in peer_ids {
            if let Err(e) = self.sync_with_peer(&peer_id).await {
                warn!("Failed to sync with peer {}: {}", peer_id, e);
            }
        }

        Ok(())
    }

    async fn send_operations(&self, peer_id: &str, operations: Vec<OperationWithMetadata>) -> Result<()> {
        // In a real implementation, this would send operations over the network
        // For now, we'll just log the operations
        debug!("Sending {} operations to peer {}", operations.len(), peer_id);
        
        // Update peer's version vector (simplified)
        let mut version_vector = VersionVector::new();
        for op in &operations {
            version_vector.increment(&op.author);
        }
        self.update_peer_version_vector(peer_id, version_vector).await?;

        Ok(())
    }

    async fn receive_operations(&self, peer_id: &str) -> Result<Vec<OperationWithMetadata>> {
        // In a real implementation, this would receive operations from the network
        // For now, we'll return empty operations
        debug!("Receiving operations from peer {}", peer_id);
        Ok(Vec::new())
    }

    async fn get_sync_status(&self) -> Result<SyncStatus> {
        let pending_count = self.pending_operations.read().await.len();
        let active_peers = self.sync_tasks.read().await.len();
        let is_active = *self.is_running.read().await;

        Ok(SyncStatus {
            is_active,
            active_peers,
            last_sync: None, // Would be tracked in a real implementation
            pending_operations: pending_count,
            progress: if pending_count > 0 { 0.5 } else { 1.0 }, // Simplified progress calculation
        })
    }

    async fn get_pending_operations(&self) -> Result<Vec<OperationWithMetadata>> {
        let pending = self.pending_operations.read().await;
        Ok(pending.clone())
    }

    async fn mark_operations_synced(&self, operation_ids: Vec<uuid::Uuid>) -> Result<()> {
        let mut synced = self.synced_operations.write().await;
        for id in operation_ids {
            synced.insert(id, true);
        }

        // Remove synced operations from pending
        {
            let mut pending = self.pending_operations.write().await;
            pending.retain(|op| !synced.contains_key(&op.id));
        }

        Ok(())
    }
}

/// Operation with metadata for synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationWithMetadata {
    /// Unique operation ID
    pub id: uuid::Uuid,
    /// Operation type
    pub operation_type: OperationType,
    /// Node ID
    pub node_id: String,
    /// Operation data
    pub data: serde_json::Value,
    /// Author of the operation
    pub author: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Version vector
    pub version_vector: VersionVector,
    /// Dependencies
    pub dependencies: Vec<uuid::Uuid>,
}

/// Operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    /// Create a new node
    CreateNode,
    /// Update an existing node
    UpdateNode,
    /// Delete a node
    DeleteNode,
    /// Create a relationship
    CreateRelationship,
    /// Delete a relationship
    DeleteRelationship,
    /// Update metadata
    UpdateMetadata,
}

impl OperationWithMetadata {
    /// Create a new operation
    pub fn new(
        operation_type: OperationType,
        node_id: String,
        data: serde_json::Value,
        author: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            operation_type,
            node_id,
            data,
            author,
            timestamp: chrono::Utc::now(),
            version_vector: VersionVector::new(),
            dependencies: Vec::new(),
        }
    }

    /// Add a dependency
    pub fn add_dependency(&mut self, operation_id: uuid::Uuid) {
        self.dependencies.push(operation_id);
    }

    /// Check if this operation depends on another
    pub fn depends_on(&self, operation_id: &uuid::Uuid) -> bool {
        self.dependencies.contains(operation_id)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_data_sync_engine() {
        let config = SyncConfig::default();
        let mut engine = DataSyncEngine::new(config);
        
        assert!(engine.start_sync().await.is_ok());
        
        // Add some test operations
        let operations = vec![
            OperationWithMetadata::new(
                OperationType::CreateNode,
                "test-node-1".to_string(),
                serde_json::json!({"name": "Test Node"}),
                "test-author".to_string(),
            ),
        ];
        
        assert!(engine.add_operations(operations).await.is_ok());
        
        let status = engine.get_sync_status().await.unwrap();
        assert!(status.is_active);
        assert_eq!(status.pending_operations, 1);
        
        assert!(engine.stop_sync().await.is_ok());
    }

    #[tokio::test]
    async fn test_operation_metadata() {
        let mut op = OperationWithMetadata::new(
            OperationType::CreateNode,
            "test-node".to_string(),
            serde_json::json!({"name": "Test"}),
            "author".to_string(),
        );
        
        let dep_id = uuid::Uuid::new_v4();
        op.add_dependency(dep_id);
        
        assert!(op.depends_on(&dep_id));
        assert!(!op.depends_on(&uuid::Uuid::new_v4()));
    }
}


