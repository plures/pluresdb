//! Conflict-free Replicated Data Type (CRDT) implementation
//! 
//! This module provides the core CRDT functionality for Rusty Gun,
//! enabling automatic conflict resolution in a distributed system.

use crate::{
    error::{Error, Result},
    node::Node,
    types::*,
};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Main CRDT implementation
#[derive(Debug)]
pub struct Crdt {
    /// Map of node ID to node data
    nodes: Arc<DashMap<NodeId, Node>>,
    /// Map of node ID to relationships
    relationships: Arc<DashMap<NodeId, Vec<Relationship>>>,
    /// Version vector for this peer
    version_vector: Arc<RwLock<VersionVector>>,
    /// Peer ID
    peer_id: PeerId,
}

impl Crdt {
    /// Create a new CRDT instance
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            nodes: Arc::new(DashMap::new()),
            relationships: Arc::new(DashMap::new()),
            version_vector: Arc::new(RwLock::new(VersionVector::new())),
            peer_id,
        }
    }

    /// Get a node by ID
    pub fn get_node(&self, node_id: &NodeId) -> Result<Option<Node>> {
        Ok(self.nodes.get(node_id).map(|entry| entry.clone()))
    }

    /// Create a new node
    pub fn create_node(
        &self,
        node_id: NodeId,
        data: serde_json::Value,
        node_type: Option<String>,
        tags: Vec<String>,
    ) -> Result<Node> {
        // Validate node ID
        if node_id.is_empty() {
            return Err(Error::InvalidNodeId { id: node_id });
        }

        // Check if node already exists
        if self.nodes.contains_key(&node_id) {
            return Err(Error::Validation(format!("Node '{}' already exists", node_id)));
        }

        // Create node metadata
        let now = chrono::Utc::now();
        let mut version = self.version_vector.read().clone();
        version.increment(&self.peer_id);

        let metadata = NodeMetadata {
            id: node_id.clone(),
            version: version.clone(),
            created_at: now,
            updated_at: now,
            created_by: self.peer_id.clone(),
            updated_by: self.peer_id.clone(),
            node_type,
            tags,
            deleted: false,
        };

        let node = Node {
            data: NodeData {
                data,
                metadata,
            },
        };

        // Update version vector
        {
            let mut vv = self.version_vector.write();
            vv.merge(&version);
        }

        // Store the node
        self.nodes.insert(node_id, node.clone());

        info!("Created node: {}", node_id);
        Ok(node)
    }

    /// Update an existing node
    pub fn update_node(&self, node_id: &NodeId, data: serde_json::Value) -> Result<Node> {
        // Get existing node
        let mut node = self.nodes.get(node_id)
            .ok_or_else(|| Error::NodeNotFound { id: node_id.clone() })?
            .clone();

        // Check if node is deleted
        if node.data.metadata.deleted {
            return Err(Error::Validation(format!("Cannot update deleted node '{}'", node_id)));
        }

        // Update node data
        node.data.data = data;
        node.data.metadata.updated_at = chrono::Utc::now();
        node.data.metadata.updated_by = self.peer_id.clone();

        // Update version vector
        let mut version = self.version_vector.read().clone();
        version.increment(&self.peer_id);
        node.data.metadata.version = version.clone();

        // Update version vector
        {
            let mut vv = self.version_vector.write();
            vv.merge(&version);
        }

        // Store updated node
        self.nodes.insert(node_id.clone(), node.clone());

        debug!("Updated node: {}", node_id);
        Ok(node)
    }

    /// Delete a node (create tombstone)
    pub fn delete_node(&self, node_id: &NodeId) -> Result<()> {
        // Get existing node
        let mut node = self.nodes.get(node_id)
            .ok_or_else(|| Error::NodeNotFound { id: node_id.clone() })?
            .clone();

        // Mark as deleted
        node.data.metadata.deleted = true;
        node.data.metadata.updated_at = chrono::Utc::now();
        node.data.metadata.updated_by = self.peer_id.clone();

        // Update version vector
        let mut version = self.version_vector.read().clone();
        version.increment(&self.peer_id);
        node.data.metadata.version = version.clone();

        // Update version vector
        {
            let mut vv = self.version_vector.write();
            vv.merge(&version);
        }

        // Store updated node
        self.nodes.insert(node_id.clone(), node);

        info!("Deleted node: {}", node_id);
        Ok(())
    }

    /// Add a relationship between nodes
    pub fn add_relationship(
        &self,
        from_node: &NodeId,
        to_node: &NodeId,
        relation_type: String,
        data: Option<serde_json::Value>,
    ) -> Result<()> {
        // Validate nodes exist
        if !self.nodes.contains_key(from_node) {
            return Err(Error::NodeNotFound { id: from_node.clone() });
        }
        if !self.nodes.contains_key(to_node) {
            return Err(Error::NodeNotFound { id: to_node.clone() });
        }

        // Create relationship
        let relationship = Relationship {
            from: from_node.clone(),
            to: to_node.clone(),
            relation_type,
            data,
            created_at: chrono::Utc::now(),
            created_by: self.peer_id.clone(),
        };

        // Store relationship
        self.relationships
            .entry(from_node.clone())
            .or_insert_with(Vec::new)
            .push(relationship);

        debug!("Added relationship: {} -> {}", from_node, to_node);
        Ok(())
    }

    /// Remove a relationship between nodes
    pub fn remove_relationship(
        &self,
        from_node: &NodeId,
        to_node: &NodeId,
        relation_type: &str,
    ) -> Result<()> {
        if let Some(mut relationships) = self.relationships.get_mut(from_node) {
            relationships.retain(|rel| {
                !(rel.to == *to_node && rel.relation_type == relation_type)
            });
        }

        debug!("Removed relationship: {} -> {} ({})", from_node, to_node, relation_type);
        Ok(())
    }

    /// Get all relationships for a node
    pub fn get_relationships(&self, node_id: &NodeId) -> Vec<Relationship> {
        self.relationships
            .get(node_id)
            .map(|entry| entry.clone())
            .unwrap_or_default()
    }

    /// Apply an operation from another peer
    pub fn apply_operation(&self, operation: &OperationWithMetadata) -> Result<()> {
        debug!("Applying operation: {:?}", operation.operation);

        match &operation.operation {
            Operation::Create {
                node_id,
                data,
                node_type,
                tags,
            } => {
                // Check if node already exists
                if let Some(existing_node) = self.nodes.get(node_id) {
                    // Check if we should update based on version vector
                    if self.should_update_node(&existing_node.data.metadata, &operation.version) {
                        self.update_node(node_id, data.clone())?;
                    }
                } else {
                    // Create new node
                    self.create_node(
                        node_id.clone(),
                        data.clone(),
                        node_type.clone(),
                        tags.clone(),
                    )?;
                }
            }
            Operation::Update { node_id, data } => {
                if let Some(existing_node) = self.nodes.get(node_id) {
                    if self.should_update_node(&existing_node.data.metadata, &operation.version) {
                        self.update_node(node_id, data.clone())?;
                    }
                }
            }
            Operation::Delete { node_id } => {
                if let Some(existing_node) = self.nodes.get(node_id) {
                    if self.should_update_node(&existing_node.data.metadata, &operation.version) {
                        self.delete_node(node_id)?;
                    }
                }
            }
            Operation::Relate {
                from_node,
                to_node,
                relation_type,
                data,
            } => {
                self.add_relationship(from_node, to_node, relation_type.clone(), data.clone())?;
            }
            Operation::Unrelate {
                from_node,
                to_node,
                relation_type,
            } => {
                self.remove_relationship(from_node, to_node, relation_type)?;
            }
        }

        // Update version vector
        {
            let mut vv = self.version_vector.write();
            vv.merge(&operation.version);
        }

        Ok(())
    }

    /// Check if we should update a node based on version vectors
    fn should_update_node(&self, existing_metadata: &NodeMetadata, incoming_version: &VersionVector) -> bool {
        // If incoming version happens before existing, don't update
        if incoming_version.happens_before(&existing_metadata.version) {
            return false;
        }

        // If existing version happens before incoming, update
        if existing_metadata.version.happens_before(incoming_version) {
            return true;
        }

        // If they're concurrent, use timestamp as tiebreaker
        incoming_version.get(&existing_metadata.updated_by) > existing_metadata.version.get(&existing_metadata.updated_by)
    }

    /// Get all nodes
    pub fn get_all_nodes(&self) -> Vec<Node> {
        self.nodes.iter().map(|entry| entry.clone()).collect()
    }

    /// Get nodes by type
    pub fn get_nodes_by_type(&self, node_type: &str) -> Vec<Node> {
        self.nodes
            .iter()
            .filter(|entry| {
                entry.data.metadata.node_type.as_ref() == Some(&node_type.to_string())
            })
            .map(|entry| entry.clone())
            .collect()
    }

    /// Get nodes by tag
    pub fn get_nodes_by_tag(&self, tag: &str) -> Vec<Node> {
        self.nodes
            .iter()
            .filter(|entry| {
                entry.data.metadata.tags.contains(&tag.to_string())
            })
            .map(|entry| entry.clone())
            .collect()
    }

    /// Get version vector
    pub fn get_version_vector(&self) -> VersionVector {
        self.version_vector.read().clone()
    }

    /// Get peer ID
    pub fn get_peer_id(&self) -> &PeerId {
        &self.peer_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crdt_creation() {
        let crdt = Crdt::new("peer1".to_string());
        assert_eq!(crdt.get_peer_id(), "peer1");
    }

    #[test]
    fn test_node_creation() {
        let crdt = Crdt::new("peer1".to_string());
        let data = serde_json::json!({"name": "test"});
        
        let node = crdt.create_node(
            "node1".to_string(),
            data,
            Some("user".to_string()),
            vec!["test".to_string()],
        ).unwrap();

        assert_eq!(node.data.metadata.id, "node1");
        assert_eq!(node.data.metadata.created_by, "peer1");
        assert!(!node.data.metadata.deleted);
    }

    #[test]
    fn test_node_update() {
        let crdt = Crdt::new("peer1".to_string());
        let data = serde_json::json!({"name": "test"});
        
        crdt.create_node(
            "node1".to_string(),
            data,
            Some("user".to_string()),
            vec!["test".to_string()],
        ).unwrap();

        let new_data = serde_json::json!({"name": "updated"});
        let updated_node = crdt.update_node(&"node1".to_string(), new_data).unwrap();

        assert_eq!(updated_node.data.data["name"], "updated");
    }

    #[test]
    fn test_node_deletion() {
        let crdt = Crdt::new("peer1".to_string());
        let data = serde_json::json!({"name": "test"});
        
        crdt.create_node(
            "node1".to_string(),
            data,
            Some("user".to_string()),
            vec!["test".to_string()],
        ).unwrap();

        crdt.delete_node(&"node1".to_string()).unwrap();

        let node = crdt.get_node(&"node1".to_string()).unwrap().unwrap();
        assert!(node.data.metadata.deleted);
    }
}


