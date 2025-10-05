//! Sled storage engine implementation

use crate::{
    error::{Result, StorageError},
    traits::{StorageEngine, Transaction, QueryResult, StorageStats},
    StorageConfig,
};
use rusty_gun_core::{Node, NodeId, types::*};
use serde_json::Value;
use std::collections::HashMap;

/// Sled storage engine
pub struct SledStorage {
    db: sled::Db,
    config: StorageConfig,
}

impl SledStorage {
    /// Create a new Sled storage engine
    pub async fn new(config: StorageConfig) -> Result<Self> {
        let db = sled::open(&config.path)
            .map_err(|e| StorageError::ConnectionFailed(format!("Failed to open Sled database: {}", e)))?;

        let mut storage = Self { db, config };
        storage.initialize().await?;
        Ok(storage)
    }
}

#[async_trait::async_trait]
impl StorageEngine for SledStorage {
    async fn initialize(&mut self) -> Result<()> {
        // Sled doesn't need table creation, but we can initialize metadata
        tracing::info!("Sled storage engine initialized");
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        self.db.flush_async().await
            .map_err(|e| StorageError::Io(e))?;
        tracing::info!("Sled storage engine closed");
        Ok(())
    }

    async fn store_node(&self, node: &Node) -> Result<()> {
        let key = format!("node:{}", node.id());
        let value = serde_json::to_string(node)
            .map_err(|e| StorageError::Serialization(e))?;
        
        self.db.insert(key.as_bytes(), value.as_bytes())
            .map_err(|e| StorageError::QueryFailed(format!("Failed to store node: {}", e)))?;

        Ok(())
    }

    async fn load_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
        let key = format!("node:{}", node_id);
        
        match self.db.get(key.as_bytes()) {
            Ok(Some(value)) => {
                let node: Node = serde_json::from_slice(&value)
                    .map_err(|e| StorageError::Serialization(e))?;
                Ok(Some(node))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(StorageError::QueryFailed(format!("Failed to load node: {}", e))),
        }
    }

    async fn delete_node(&self, node_id: &NodeId) -> Result<()> {
        let key = format!("node:{}", node_id);
        self.db.remove(key.as_bytes())
            .map_err(|e| StorageError::QueryFailed(format!("Failed to delete node: {}", e)))?;
        Ok(())
    }

    async fn list_node_ids(&self) -> Result<Vec<NodeId>> {
        let mut node_ids = Vec::new();
        let iter = self.db.scan_prefix("node:");
        
        for item in iter {
            let (key, _) = item.map_err(|e| StorageError::QueryFailed(format!("Failed to iterate: {}", e)))?;
            if let Some(node_id) = key.strip_prefix(b"node:") {
                if let Ok(id) = String::from_utf8(node_id.to_vec()) {
                    node_ids.push(id);
                }
            }
        }

        Ok(node_ids)
    }

    async fn list_nodes_by_type(&self, _node_type: &str) -> Result<Vec<Node>> {
        // Sled implementation would need to maintain type indexes
        // For now, return empty vector
        Ok(Vec::new())
    }

    async fn list_nodes_by_tag(&self, _tag: &str) -> Result<Vec<Node>> {
        // Sled implementation would need to maintain tag indexes
        // For now, return empty vector
        Ok(Vec::new())
    }

    async fn search_nodes(&self, _query: &str) -> Result<Vec<Node>> {
        // Sled implementation would need full-text search
        // For now, return empty vector
        Ok(Vec::new())
    }

    async fn store_relationship(&self, relationship: &Relationship) -> Result<()> {
        let key = format!("rel:{}:{}:{}", relationship.from, relationship.to, relationship.relation_type);
        let value = serde_json::to_string(relationship)
            .map_err(|e| StorageError::Serialization(e))?;
        
        self.db.insert(key.as_bytes(), value.as_bytes())
            .map_err(|e| StorageError::QueryFailed(format!("Failed to store relationship: {}", e)))?;

        Ok(())
    }

    async fn load_relationships(&self, _node_id: &NodeId) -> Result<Vec<Relationship>> {
        // Sled implementation would need to maintain relationship indexes
        // For now, return empty vector
        Ok(Vec::new())
    }

    async fn delete_relationship(
        &self,
        from: &NodeId,
        to: &NodeId,
        relation_type: &str,
    ) -> Result<()> {
        let key = format!("rel:{}:{}:{}", from, to, relation_type);
        self.db.remove(key.as_bytes())
            .map_err(|e| StorageError::QueryFailed(format!("Failed to delete relationship: {}", e)))?;
        Ok(())
    }

    async fn execute_query(&self, _query: &str, _params: &[Value]) -> Result<QueryResult> {
        // Sled doesn't support SQL queries
        Err(StorageError::QueryFailed("Sled doesn't support SQL queries".to_string()))
    }

    async fn begin_transaction(&self) -> Result<Box<dyn Transaction>> {
        // Sled transactions would need to be implemented
        Err(StorageError::TransactionFailed("Sled transactions not implemented".to_string()))
    }

    async fn get_stats(&self) -> Result<StorageStats> {
        let node_count = self.list_node_ids().await?.len() as u64;
        
        Ok(StorageStats {
            node_count,
            relationship_count: 0, // Would need to count relationships
            storage_size: self.db.size_on_disk()
                .map_err(|e| StorageError::QueryFailed(format!("Failed to get database size: {}", e)))?,
            index_count: 0,
            last_updated: chrono::Utc::now(),
        })
    }
}

/// Sled transaction wrapper (placeholder)
struct SledTransaction;

#[async_trait::async_trait]
impl Transaction for SledTransaction {
    async fn commit(self: Box<Self>) -> Result<()> {
        // Sled transaction implementation
        Ok(())
    }

    async fn rollback(self: Box<Self>) -> Result<()> {
        // Sled transaction implementation
        Ok(())
    }

    async fn store_node(&mut self, _node: &Node) -> Result<()> {
        // Sled transaction implementation
        Ok(())
    }

    async fn load_node(&self, _node_id: &NodeId) -> Result<Option<Node>> {
        // Sled transaction implementation
        Ok(None)
    }

    async fn delete_node(&mut self, _node_id: &NodeId) -> Result<()> {
        // Sled transaction implementation
        Ok(())
    }

    async fn execute_query(&self, _query: &str, _params: &[Value]) -> Result<QueryResult> {
        // Sled transaction implementation
        Err(StorageError::QueryFailed("Sled doesn't support SQL queries".to_string()))
    }
}


