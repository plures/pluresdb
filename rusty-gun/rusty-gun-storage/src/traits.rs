//! Storage engine traits for Rusty Gun

use crate::error::Result;
use rusty_gun_core::{Node, NodeId, types::*};
use serde_json::Value;
use std::collections::HashMap;

/// Main storage engine trait
#[async_trait::async_trait]
pub trait StorageEngine: Send + Sync {
    /// Initialize the storage engine
    async fn initialize(&mut self) -> Result<()>;

    /// Close the storage engine
    async fn close(&mut self) -> Result<()>;

    /// Store a node
    async fn store_node(&self, node: &Node) -> Result<()>;

    /// Load a node by ID
    async fn load_node(&self, node_id: &NodeId) -> Result<Option<Node>>;

    /// Delete a node by ID
    async fn delete_node(&self, node_id: &NodeId) -> Result<()>;

    /// List all node IDs
    async fn list_node_ids(&self) -> Result<Vec<NodeId>>;

    /// List nodes by type
    async fn list_nodes_by_type(&self, node_type: &str) -> Result<Vec<Node>>;

    /// List nodes by tag
    async fn list_nodes_by_tag(&self, tag: &str) -> Result<Vec<Node>>;

    /// Search nodes by query
    async fn search_nodes(&self, query: &str) -> Result<Vec<Node>>;

    /// Store a relationship
    async fn store_relationship(&self, relationship: &Relationship) -> Result<()>;

    /// Load relationships for a node
    async fn load_relationships(&self, node_id: &NodeId) -> Result<Vec<Relationship>>;

    /// Delete a relationship
    async fn delete_relationship(
        &self,
        from: &NodeId,
        to: &NodeId,
        relation_type: &str,
    ) -> Result<()>;

    /// Execute a raw SQL query
    async fn execute_query(&self, query: &str, params: &[Value]) -> Result<QueryResult>;

    /// Begin a transaction
    async fn begin_transaction(&self) -> Result<Box<dyn Transaction>;

    /// Get storage statistics
    async fn get_stats(&self) -> Result<StorageStats>;
}

/// Transaction trait
#[async_trait::async_trait]
pub trait Transaction: Send + Sync {
    /// Commit the transaction
    async fn commit(self: Box<Self>) -> Result<()>;

    /// Rollback the transaction
    async fn rollback(self: Box<Self>) -> Result<()>;

    /// Store a node in this transaction
    async fn store_node(&mut self, node: &Node) -> Result<()>;

    /// Load a node in this transaction
    async fn load_node(&self, node_id: &NodeId) -> Result<Option<Node>>;

    /// Delete a node in this transaction
    async fn delete_node(&mut self, node_id: &NodeId) -> Result<()>;

    /// Execute a query in this transaction
    async fn execute_query(&self, query: &str, params: &[Value]) -> Result<QueryResult>;
}

/// Vector search engine trait
#[async_trait::async_trait]
pub trait VectorSearchEngine: Send + Sync {
    /// Initialize the vector search engine
    async fn initialize(&mut self) -> Result<()>;

    /// Add a vector to the index
    async fn add_vector(
        &self,
        id: &str,
        vector: &[f32],
        metadata: &Value,
    ) -> Result<()>;

    /// Search for similar vectors
    async fn search_vectors(
        &self,
        query_vector: &[f32],
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>>;

    /// Remove a vector from the index
    async fn remove_vector(&self, id: &str) -> Result<()>;

    /// Update a vector in the index
    async fn update_vector(
        &self,
        id: &str,
        vector: &[f32],
        metadata: &Value,
    ) -> Result<()>;

    /// Get vector by ID
    async fn get_vector(&self, id: &str) -> Result<Option<(Vec<f32>, Value)>>;

    /// Get index statistics
    async fn get_stats(&self) -> Result<VectorStats>;
}

/// Query result structure
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub rows: Vec<HashMap<String, Value>>,
    pub columns: Vec<String>,
    pub changes: u64,
    pub last_insert_row_id: i64,
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub node_count: u64,
    pub relationship_count: u64,
    pub storage_size: u64,
    pub index_count: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Vector search result
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub id: String,
    pub score: f32,
    pub metadata: Value,
}

/// Vector search statistics
#[derive(Debug, Clone)]
pub struct VectorStats {
    pub vector_count: u64,
    pub dimensions: usize,
    pub index_size: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Storage factory trait
pub trait StorageFactory {
    type Engine: StorageEngine;
    type VectorEngine: VectorSearchEngine;

    /// Create a new storage engine
    fn create_storage_engine(&self) -> Result<Self::Engine>;

    /// Create a new vector search engine
    fn create_vector_engine(&self) -> Result<Self::VectorEngine>;
}


