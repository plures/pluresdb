//! Node implementation for Rusty Gun
//! 
//! This module provides the Node data structure and related functionality.

use crate::{
    error::{Error, Result},
    types::*,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A node in the Rusty Gun graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// The node data
    pub data: NodeData,
}

impl Node {
    /// Create a new node
    pub fn new(
        id: NodeId,
        data: serde_json::Value,
        node_type: Option<String>,
        tags: Vec<String>,
        created_by: PeerId,
    ) -> Self {
        let now = chrono::Utc::now();
        let mut version = VersionVector::new();
        version.increment(&created_by);

        Self {
            data: NodeData {
                data,
                metadata: NodeMetadata {
                    id,
                    version,
                    created_at: now,
                    updated_at: now,
                    created_by: created_by.clone(),
                    updated_by: created_by,
                    node_type,
                    tags,
                    deleted: false,
                },
            },
        }
    }

    /// Get the node ID
    pub fn id(&self) -> &NodeId {
        &self.data.metadata.id
    }

    /// Get the node data
    pub fn data(&self) -> &serde_json::Value {
        &self.data.data
    }

    /// Get mutable reference to node data
    pub fn data_mut(&mut self) -> &mut serde_json::Value {
        &mut self.data.data
    }

    /// Get the node metadata
    pub fn metadata(&self) -> &NodeMetadata {
        &self.data.metadata
    }

    /// Get mutable reference to node metadata
    pub fn metadata_mut(&mut self) -> &mut NodeMetadata {
        &mut self.data.metadata
    }

    /// Check if the node is deleted
    pub fn is_deleted(&self) -> bool {
        self.data.metadata.deleted
    }

    /// Mark the node as deleted
    pub fn mark_deleted(&mut self) {
        self.data.metadata.deleted = true;
        self.data.metadata.updated_at = chrono::Utc::now();
    }

    /// Update the node data
    pub fn update_data(&mut self, data: serde_json::Value, updated_by: PeerId) {
        self.data.data = data;
        self.data.metadata.updated_at = chrono::Utc::now();
        self.data.metadata.updated_by = updated_by;
    }

    /// Update the version vector
    pub fn update_version(&mut self, peer_id: &PeerId) {
        self.data.metadata.version.increment(peer_id);
    }

    /// Merge version vectors
    pub fn merge_version(&mut self, other: &VersionVector) {
        self.data.metadata.version.merge(other);
    }

    /// Check if this node should be updated based on version vectors
    pub fn should_update(&self, other: &Node) -> bool {
        // If other node is deleted and we're not, don't update
        if other.is_deleted() && !self.is_deleted() {
            return false;
        }

        // If we're deleted and other is not, update
        if self.is_deleted() && !other.is_deleted() {
            return true;
        }

        // Compare version vectors
        if other.data.metadata.version.happens_before(&self.data.metadata.version) {
            return false;
        }

        if self.data.metadata.version.happens_before(&other.data.metadata.version) {
            return true;
        }

        // If concurrent, use timestamp as tiebreaker
        other.data.metadata.updated_at > self.data.metadata.updated_at
    }

    /// Merge with another node (for conflict resolution)
    pub fn merge(&mut self, other: &Node) -> Result<()> {
        // Merge version vectors
        self.merge_version(&other.data.metadata.version);

        // If other node is deleted, mark as deleted
        if other.is_deleted() {
            self.mark_deleted();
            return Ok(());
        }

        // If we're deleted and other is not, restore
        if self.is_deleted() && !other.is_deleted() {
            self.data.metadata.deleted = false;
        }

        // Merge data (simple strategy: use the newer data)
        if other.data.metadata.updated_at > self.data.metadata.updated_at {
            self.data.data = other.data.data.clone();
            self.data.metadata.updated_at = other.data.metadata.updated_at;
            self.data.metadata.updated_by = other.data.metadata.updated_by.clone();
        }

        // Merge tags
        for tag in &other.data.metadata.tags {
            if !self.data.metadata.tags.contains(tag) {
                self.data.metadata.tags.push(tag.clone());
            }
        }

        Ok(())
    }

    /// Get a field from the node data
    pub fn get_field(&self, field: &str) -> Option<&serde_json::Value> {
        self.data.data.get(field)
    }

    /// Set a field in the node data
    pub fn set_field(&mut self, field: &str, value: serde_json::Value) {
        if let Some(obj) = self.data.data.as_object_mut() {
            obj.insert(field.to_string(), value);
        }
    }

    /// Remove a field from the node data
    pub fn remove_field(&mut self, field: &str) -> Option<serde_json::Value> {
        if let Some(obj) = self.data.data.as_object_mut() {
            obj.remove(field)
        } else {
            None
        }
    }

    /// Get all field names
    pub fn get_field_names(&self) -> Vec<String> {
        if let Some(obj) = self.data.data.as_object() {
            obj.keys().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Check if the node has a specific field
    pub fn has_field(&self, field: &str) -> bool {
        self.data.data.get(field).is_some()
    }

    /// Get the node type
    pub fn node_type(&self) -> Option<&String> {
        self.data.metadata.node_type.as_ref()
    }

    /// Get the tags
    pub fn tags(&self) -> &Vec<String> {
        &self.data.metadata.tags
    }

    /// Add a tag
    pub fn add_tag(&mut self, tag: String) {
        if !self.data.metadata.tags.contains(&tag) {
            self.data.metadata.tags.push(tag);
        }
    }

    /// Remove a tag
    pub fn remove_tag(&mut self, tag: &str) {
        self.data.metadata.tags.retain(|t| t != tag);
    }

    /// Check if the node has a specific tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.data.metadata.tags.contains(&tag.to_string())
    }

    /// Get the size of the node in bytes
    pub fn size(&self) -> usize {
        // Approximate size calculation
        let data_size = serde_json::to_string(&self.data.data)
            .map(|s| s.len())
            .unwrap_or(0);
        let metadata_size = serde_json::to_string(&self.data.metadata)
            .map(|s| s.len())
            .unwrap_or(0);
        data_size + metadata_size
    }

    /// Validate the node
    pub fn validate(&self, max_size: usize, max_depth: usize) -> Result<()> {
        // Check size
        let size = self.size();
        if size > max_size {
            return Err(Error::NodeTooLarge { size, max: max_size });
        }

        // Check depth (simple implementation)
        let depth = self.calculate_depth();
        if depth > max_depth {
            return Err(Error::NodeDepthExceeded { depth, max: max_depth });
        }

        // Validate node ID
        if self.data.metadata.id.is_empty() {
            return Err(Error::InvalidNodeId { id: self.data.metadata.id.clone() });
        }

        Ok(())
    }

    /// Calculate the depth of nested objects
    fn calculate_depth(&self) -> usize {
        self.calculate_value_depth(&self.data.data, 0)
    }

    /// Recursively calculate depth of a JSON value
    fn calculate_value_depth(&self, value: &serde_json::Value, current_depth: usize) -> usize {
        match value {
            serde_json::Value::Object(obj) => {
                let mut max_depth = current_depth + 1;
                for (_, v) in obj {
                    max_depth = max_depth.max(self.calculate_value_depth(v, current_depth + 1));
                }
                max_depth
            }
            serde_json::Value::Array(arr) => {
                let mut max_depth = current_depth + 1;
                for v in arr {
                    max_depth = max_depth.max(self.calculate_value_depth(v, current_depth + 1));
                }
                max_depth
            }
            _ => current_depth,
        }
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.data.metadata.id == other.data.metadata.id
    }
}

impl Eq for Node {}

impl std::hash::Hash for Node {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.data.metadata.id.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let node = Node::new(
            "node1".to_string(),
            serde_json::json!({"name": "test"}),
            Some("user".to_string()),
            vec!["test".to_string()],
            "peer1".to_string(),
        );

        assert_eq!(node.id(), "node1");
        assert_eq!(node.data()["name"], "test");
        assert_eq!(node.node_type(), Some(&"user".to_string()));
        assert!(node.has_tag("test"));
        assert!(!node.is_deleted());
    }

    #[test]
    fn test_node_field_operations() {
        let mut node = Node::new(
            "node1".to_string(),
            serde_json::json!({"name": "test"}),
            None,
            vec![],
            "peer1".to_string(),
        );

        // Test field operations
        assert_eq!(node.get_field("name"), Some(&serde_json::json!("test")));
        assert!(node.has_field("name"));

        node.set_field("age", serde_json::json!(25));
        assert_eq!(node.get_field("age"), Some(&serde_json::json!(25)));

        let removed = node.remove_field("age");
        assert_eq!(removed, Some(serde_json::json!(25)));
        assert!(!node.has_field("age"));
    }

    #[test]
    fn test_node_tag_operations() {
        let mut node = Node::new(
            "node1".to_string(),
            serde_json::json!({}),
            None,
            vec!["tag1".to_string()],
            "peer1".to_string(),
        );

        assert!(node.has_tag("tag1"));
        assert!(!node.has_tag("tag2"));

        node.add_tag("tag2".to_string());
        assert!(node.has_tag("tag2"));

        node.remove_tag("tag1");
        assert!(!node.has_tag("tag1"));
        assert!(node.has_tag("tag2"));
    }

    #[test]
    fn test_node_validation() {
        let node = Node::new(
            "node1".to_string(),
            serde_json::json!({"name": "test"}),
            None,
            vec![],
            "peer1".to_string(),
        );

        // Should pass validation
        assert!(node.validate(1024, 10).is_ok());

        // Should fail on size limit
        let large_data = serde_json::json!({"data": "x".repeat(2000)});
        let large_node = Node::new(
            "node2".to_string(),
            large_data,
            None,
            vec![],
            "peer1".to_string(),
        );
        assert!(large_node.validate(1000, 10).is_err());
    }
}


