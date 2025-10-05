//! Core types and data structures for Rusty Gun

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Unique identifier for a node
pub type NodeId = String;

/// Unique identifier for a peer
pub type PeerId = String;

/// Timestamp for operations
pub type Timestamp = DateTime<Utc>;

/// Version vector for conflict resolution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionVector {
    /// Map of peer ID to logical clock
    pub clocks: HashMap<PeerId, u64>,
}

impl VersionVector {
    /// Create a new empty version vector
    pub fn new() -> Self {
        Self {
            clocks: HashMap::new(),
        }
    }

    /// Increment the clock for a specific peer
    pub fn increment(&mut self, peer_id: &PeerId) {
        let clock = self.clocks.get(peer_id).unwrap_or(&0) + 1;
        self.clocks.insert(peer_id.clone(), clock);
    }

    /// Get the clock value for a peer
    pub fn get(&self, peer_id: &PeerId) -> u64 {
        self.clocks.get(peer_id).unwrap_or(&0).clone()
    }

    /// Check if this version vector happens before another
    pub fn happens_before(&self, other: &VersionVector) -> bool {
        let mut strictly_less = false;
        
        for (peer_id, &clock) in &self.clocks {
            let other_clock = other.get(peer_id);
            if clock > other_clock {
                return false;
            }
            if clock < other_clock {
                strictly_less = true;
            }
        }
        
        // Check if other has clocks we don't have
        for peer_id in other.clocks.keys() {
            if !self.clocks.contains_key(peer_id) {
                strictly_less = true;
            }
        }
        
        strictly_less
    }

    /// Merge two version vectors
    pub fn merge(&mut self, other: &VersionVector) {
        for (peer_id, &clock) in &other.clocks {
            let current_clock = self.get(peer_id);
            self.clocks.insert(peer_id.clone(), current_clock.max(clock));
        }
    }
}

impl Default for VersionVector {
    fn default() -> Self {
        Self::new()
    }
}

/// Node data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeData {
    /// The actual data stored in the node
    pub data: serde_json::Value,
    /// Metadata about the node
    pub metadata: NodeMetadata,
}

/// Node metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeMetadata {
    /// Node ID
    pub id: NodeId,
    /// Version vector for conflict resolution
    pub version: VersionVector,
    /// Timestamp when the node was created
    pub created_at: Timestamp,
    /// Timestamp when the node was last modified
    pub updated_at: Timestamp,
    /// ID of the peer that created this node
    pub created_by: PeerId,
    /// ID of the peer that last modified this node
    pub updated_by: PeerId,
    /// Node type (for schema validation)
    pub node_type: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Whether the node is deleted (tombstone)
    pub deleted: bool,
}

/// Operation types for CRDT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Create a new node
    Create {
        node_id: NodeId,
        data: serde_json::Value,
        node_type: Option<String>,
        tags: Vec<String>,
    },
    /// Update an existing node
    Update {
        node_id: NodeId,
        data: serde_json::Value,
    },
    /// Delete a node (tombstone)
    Delete {
        node_id: NodeId,
    },
    /// Add a relationship between nodes
    Relate {
        from_node: NodeId,
        to_node: NodeId,
        relation_type: String,
        data: Option<serde_json::Value>,
    },
    /// Remove a relationship between nodes
    Unrelate {
        from_node: NodeId,
        to_node: NodeId,
        relation_type: String,
    },
}

/// Operation with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationWithMetadata {
    /// The operation
    pub operation: Operation,
    /// Unique operation ID
    pub operation_id: Uuid,
    /// Timestamp when the operation was created
    pub timestamp: Timestamp,
    /// ID of the peer that created this operation
    pub peer_id: PeerId,
    /// Version vector at the time of operation
    pub version: VersionVector,
    /// Whether this operation has been applied
    pub applied: bool,
}

/// Vector search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    /// Node ID
    pub node_id: NodeId,
    /// Similarity score (0.0 to 1.0)
    pub score: f32,
    /// The actual data
    pub data: serde_json::Value,
    /// Metadata
    pub metadata: NodeMetadata,
}

/// Graph query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphQueryResult {
    /// Nodes matching the query
    pub nodes: Vec<NodeData>,
    /// Relationships between nodes
    pub relationships: Vec<Relationship>,
}

/// Relationship between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    /// Source node ID
    pub from: NodeId,
    /// Target node ID
    pub to: NodeId,
    /// Type of relationship
    pub relation_type: String,
    /// Relationship data
    pub data: Option<serde_json::Value>,
    /// When the relationship was created
    pub created_at: Timestamp,
    /// Who created the relationship
    pub created_by: PeerId,
}

/// Peer information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer ID
    pub id: PeerId,
    /// Peer name
    pub name: String,
    /// Peer public key
    pub public_key: String,
    /// Peer address
    pub address: Option<String>,
    /// Last seen timestamp
    pub last_seen: Timestamp,
    /// Peer capabilities
    pub capabilities: Vec<String>,
    /// Peer status
    pub status: PeerStatus,
}

/// Peer status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PeerStatus {
    /// Peer is online and connected
    Online,
    /// Peer is offline
    Offline,
    /// Peer is connecting
    Connecting,
    /// Peer connection failed
    Failed,
}

/// Sync status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    /// Whether we're currently online
    pub is_online: bool,
    /// Last sync timestamp
    pub last_sync: Option<Timestamp>,
    /// Number of pending operations
    pub pending_operations: usize,
    /// Number of connected peers
    pub connected_peers: usize,
    /// Sync progress (0.0 to 1.0)
    pub sync_progress: f32,
}

/// Configuration for Rusty Gun
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Data directory
    pub data_dir: String,
    /// API server port
    pub api_port: u16,
    /// Web UI port
    pub web_port: u16,
    /// P2P port
    pub p2p_port: u16,
    /// Maximum node size in bytes
    pub max_node_size: usize,
    /// Maximum node depth
    pub max_node_depth: usize,
    /// Vector dimensions
    pub vector_dimensions: usize,
    /// Enable P2P sync
    pub enable_p2p: bool,
    /// Enable encryption
    pub enable_encryption: bool,
    /// Log level
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: "./data".to_string(),
            api_port: 34567,
            web_port: 34568,
            p2p_port: 34569,
            max_node_size: 1024 * 1024, // 1MB
            max_node_depth: 100,
            vector_dimensions: 384,
            enable_p2p: true,
            enable_encryption: true,
            log_level: "info".to_string(),
        }
    }
}


