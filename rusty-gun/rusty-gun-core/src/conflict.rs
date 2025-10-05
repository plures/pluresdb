//! Conflict resolution strategies for Rusty Gun
//! 
//! This module provides various conflict resolution strategies for handling
//! concurrent updates in a distributed system.

use crate::{
    error::{Error, Result},
    node::Node,
    types::*,
};
use std::collections::HashMap;

/// Conflict resolution strategy
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictStrategy {
    /// Last writer wins (based on timestamp)
    LastWriterWins,
    /// First writer wins (based on timestamp)
    FirstWriterWins,
    /// Merge data fields
    MergeFields,
    /// Custom conflict resolver
    Custom(Box<dyn ConflictResolver + Send + Sync>),
}

/// Trait for custom conflict resolution
pub trait ConflictResolver {
    /// Resolve a conflict between two nodes
    fn resolve(&self, local: &Node, remote: &Node) -> Result<Node>;
}

/// Default conflict resolver
#[derive(Debug, Default)]
pub struct DefaultConflictResolver {
    strategy: ConflictStrategy,
}

impl DefaultConflictResolver {
    /// Create a new conflict resolver with the specified strategy
    pub fn new(strategy: ConflictStrategy) -> Self {
        Self { strategy }
    }
}

impl ConflictResolver for DefaultConflictResolver {
    fn resolve(&self, local: &Node, remote: &Node) -> Result<Node> {
        match &self.strategy {
            ConflictStrategy::LastWriterWins => self.resolve_last_writer_wins(local, remote),
            ConflictStrategy::FirstWriterWins => self.resolve_first_writer_wins(local, remote),
            ConflictStrategy::MergeFields => self.resolve_merge_fields(local, remote),
            ConflictStrategy::Custom(resolver) => resolver.resolve(local, remote),
        }
    }
}

impl DefaultConflictResolver {
    /// Last writer wins strategy
    fn resolve_last_writer_wins(&self, local: &Node, remote: &Node) -> Result<Node> {
        if remote.data.metadata.updated_at > local.data.metadata.updated_at {
            Ok(remote.clone())
        } else {
            Ok(local.clone())
        }
    }

    /// First writer wins strategy
    fn resolve_first_writer_wins(&self, local: &Node, remote: &Node) -> Result<Node> {
        if remote.data.metadata.created_at < local.data.metadata.created_at {
            Ok(remote.clone())
        } else {
            Ok(local.clone())
        }
    }

    /// Merge fields strategy
    fn resolve_merge_fields(&self, local: &Node, remote: &Node) -> Result<Node> {
        let mut result = local.clone();

        // Merge version vectors
        result.merge_version(&remote.data.metadata.version);

        // Merge data fields
        if let (Some(local_obj), Some(remote_obj)) = (
            local.data.data.as_object(),
            remote.data.data.as_object(),
        ) {
            let mut merged_obj = local_obj.clone();
            
            for (key, remote_value) in remote_obj {
                if let Some(local_value) = merged_obj.get(key) {
                    // Field exists in both, need to resolve conflict
                    let resolved_value = self.resolve_field_conflict(key, local_value, remote_value)?;
                    merged_obj.insert(key.clone(), resolved_value);
                } else {
                    // Field only exists in remote, add it
                    merged_obj.insert(key.clone(), remote_value.clone());
                }
            }

            result.data.data = serde_json::Value::Object(merged_obj);
        }

        // Merge tags
        for tag in &remote.data.metadata.tags {
            if !result.data.metadata.tags.contains(tag) {
                result.data.metadata.tags.push(tag.clone());
            }
        }

        // Update timestamps
        result.data.metadata.updated_at = std::cmp::max(
            local.data.metadata.updated_at,
            remote.data.metadata.updated_at,
        );

        Ok(result)
    }

    /// Resolve conflict for a specific field
    fn resolve_field_conflict(
        &self,
        field: &str,
        local_value: &serde_json::Value,
        remote_value: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Special handling for different field types
        match field {
            "version" | "id" => {
                // For version and ID fields, use the local value
                Ok(local_value.clone())
            }
            "timestamp" | "updated_at" => {
                // For timestamp fields, use the later one
                if let (Some(local_ts), Some(remote_ts)) = (
                    local_value.as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()),
                    remote_value.as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()),
                ) {
                    if remote_ts > local_ts {
                        Ok(remote_value.clone())
                    } else {
                        Ok(local_value.clone())
                    }
                } else {
                    // Fallback to string comparison
                    if remote_value.as_str() > local_value.as_str() {
                        Ok(remote_value.clone())
                    } else {
                        Ok(local_value.clone())
                    }
                }
            }
            "counter" | "count" => {
                // For counter fields, use the maximum value
                if let (Some(local_num), Some(remote_num)) = (local_value.as_u64(), remote_value.as_u64()) {
                    Ok(serde_json::Value::Number(serde_json::Number::from(std::cmp::max(local_num, remote_num))))
                } else {
                    Ok(remote_value.clone())
                }
            }
            "list" | "array" => {
                // For array fields, merge the arrays
                if let (Some(local_arr), Some(remote_arr)) = (local_value.as_array(), remote_value.as_array()) {
                    let mut merged = local_arr.clone();
                    for item in remote_arr {
                        if !merged.contains(item) {
                            merged.push(item.clone());
                        }
                    }
                    Ok(serde_json::Value::Array(merged))
                } else {
                    Ok(remote_value.clone())
                }
            }
            "set" => {
                // For set fields, merge as a set (remove duplicates)
                if let (Some(local_arr), Some(remote_arr)) = (local_value.as_array(), remote_value.as_array()) {
                    let mut merged = local_arr.clone();
                    for item in remote_arr {
                        if !merged.contains(item) {
                            merged.push(item.clone());
                        }
                    }
                    Ok(serde_json::Value::Array(merged))
                } else {
                    Ok(remote_value.clone())
                }
            }
            _ => {
                // For other fields, use last writer wins
                if remote_value.as_str() > local_value.as_str() {
                    Ok(remote_value.clone())
                } else {
                    Ok(local_value.clone())
                }
            }
        }
    }
}

/// Conflict resolution manager
#[derive(Debug)]
pub struct ConflictManager {
    resolver: Box<dyn ConflictResolver + Send + Sync>,
    conflicts: HashMap<NodeId, Vec<Conflict>>,
}

/// A conflict between two nodes
#[derive(Debug, Clone)]
pub struct Conflict {
    pub local_node: Node,
    pub remote_node: Node,
    pub conflict_type: ConflictType,
    pub timestamp: Timestamp,
}

/// Type of conflict
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictType {
    /// Data conflict (different values for same field)
    DataConflict,
    /// Deletion conflict (one node deleted, other updated)
    DeletionConflict,
    /// Version conflict (concurrent updates)
    VersionConflict,
    /// Type conflict (different node types)
    TypeConflict,
}

impl ConflictManager {
    /// Create a new conflict manager
    pub fn new(resolver: Box<dyn ConflictResolver + Send + Sync>) -> Self {
        Self {
            resolver,
            conflicts: HashMap::new(),
        }
    }

    /// Detect conflicts between two nodes
    pub fn detect_conflict(&self, local: &Node, remote: &Node) -> Option<ConflictType> {
        // Check if one is deleted and other is not
        if local.is_deleted() != remote.is_deleted() {
            return Some(ConflictType::DeletionConflict);
        }

        // Check if node types are different
        if local.node_type() != remote.node_type() {
            return Some(ConflictType::TypeConflict);
        }

        // Check if version vectors are concurrent
        if !local.data.metadata.version.happens_before(&remote.data.metadata.version) &&
           !remote.data.metadata.version.happens_before(&local.data.metadata.version) {
            return Some(ConflictType::VersionConflict);
        }

        // Check if data is different
        if local.data.data != remote.data.data {
            return Some(ConflictType::DataConflict);
        }

        None
    }

    /// Resolve a conflict between two nodes
    pub fn resolve_conflict(&self, local: &Node, remote: &Node) -> Result<Node> {
        let conflict_type = self.detect_conflict(local, remote);
        
        if let Some(conflict_type) = conflict_type {
            tracing::warn!(
                "Resolving conflict of type {:?} for node {}",
                conflict_type,
                local.id()
            );
        }

        self.resolver.resolve(local, remote)
    }

    /// Record a conflict for later resolution
    pub fn record_conflict(&mut self, conflict: Conflict) {
        let node_id = conflict.local_node.id().clone();
        self.conflicts.entry(node_id).or_insert_with(Vec::new).push(conflict);
    }

    /// Get all conflicts for a node
    pub fn get_conflicts(&self, node_id: &NodeId) -> Option<&Vec<Conflict>> {
        self.conflicts.get(node_id)
    }

    /// Clear conflicts for a node
    pub fn clear_conflicts(&mut self, node_id: &NodeId) {
        self.conflicts.remove(node_id);
    }

    /// Get all conflicts
    pub fn get_all_conflicts(&self) -> &HashMap<NodeId, Vec<Conflict>> {
        &self.conflicts
    }
}

/// Field-level conflict resolver
#[derive(Debug)]
pub struct FieldConflictResolver {
    field_strategies: HashMap<String, ConflictStrategy>,
    default_strategy: ConflictStrategy,
}

impl FieldConflictResolver {
    /// Create a new field conflict resolver
    pub fn new(default_strategy: ConflictStrategy) -> Self {
        Self {
            field_strategies: HashMap::new(),
            default_strategy,
        }
    }

    /// Set strategy for a specific field
    pub fn set_field_strategy(&mut self, field: String, strategy: ConflictStrategy) {
        self.field_strategies.insert(field, strategy);
    }

    /// Get strategy for a field
    fn get_strategy(&self, field: &str) -> &ConflictStrategy {
        self.field_strategies.get(field).unwrap_or(&self.default_strategy)
    }
}

impl ConflictResolver for FieldConflictResolver {
    fn resolve(&self, local: &Node, remote: &Node) -> Result<Node> {
        let mut result = local.clone();
        result.merge_version(&remote.data.metadata.version);

        if let (Some(local_obj), Some(remote_obj)) = (
            local.data.data.as_object(),
            remote.data.data.as_object(),
        ) {
            let mut merged_obj = local_obj.clone();
            
            for (key, remote_value) in remote_obj {
                if let Some(local_value) = merged_obj.get(key) {
                    let strategy = self.get_strategy(key);
                    let resolved_value = match strategy {
                        ConflictStrategy::LastWriterWins => {
                            if remote.data.metadata.updated_at > local.data.metadata.updated_at {
                                remote_value.clone()
                            } else {
                                local_value.clone()
                            }
                        }
                        ConflictStrategy::FirstWriterWins => {
                            if remote.data.metadata.created_at < local.data.metadata.created_at {
                                remote_value.clone()
                            } else {
                                local_value.clone()
                            }
                        }
                        ConflictStrategy::MergeFields => {
                            // Implement field-specific merge logic
                            self.merge_field_values(key, local_value, remote_value)?
                        }
                        _ => remote_value.clone(),
                    };
                    merged_obj.insert(key.clone(), resolved_value);
                } else {
                    merged_obj.insert(key.clone(), remote_value.clone());
                }
            }

            result.data.data = serde_json::Value::Object(merged_obj);
        }

        Ok(result)
    }
}

impl FieldConflictResolver {
    /// Merge values for a specific field
    fn merge_field_values(
        &self,
        field: &str,
        local_value: &serde_json::Value,
        remote_value: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Implement field-specific merge logic
        match field {
            "tags" | "categories" => {
                // Merge arrays and remove duplicates
                if let (Some(local_arr), Some(remote_arr)) = (local_value.as_array(), remote_value.as_array()) {
                    let mut merged = local_arr.clone();
                    for item in remote_arr {
                        if !merged.contains(item) {
                            merged.push(item.clone());
                        }
                    }
                    Ok(serde_json::Value::Array(merged))
                } else {
                    Ok(remote_value.clone())
                }
            }
            "counters" => {
                // Merge counter objects
                if let (Some(local_obj), Some(remote_obj)) = (local_value.as_object(), remote_value.as_object()) {
                    let mut merged = local_obj.clone();
                    for (key, remote_val) in remote_obj {
                        if let Some(local_val) = merged.get(key) {
                            if let (Some(local_num), Some(remote_num)) = (local_val.as_u64(), remote_val.as_u64()) {
                                merged.insert(key.clone(), serde_json::Value::Number(serde_json::Number::from(local_num + remote_num)));
                            } else {
                                merged.insert(key.clone(), remote_val.clone());
                            }
                        } else {
                            merged.insert(key.clone(), remote_val.clone());
                        }
                    }
                    Ok(serde_json::Value::Object(merged))
                } else {
                    Ok(remote_value.clone())
                }
            }
            _ => Ok(remote_value.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_writer_wins() {
        let resolver = DefaultConflictResolver::new(ConflictStrategy::LastWriterWins);
        
        let mut local = Node::new(
            "node1".to_string(),
            serde_json::json!({"name": "local"}),
            None,
            vec![],
            "peer1".to_string(),
        );
        local.data.metadata.updated_at = chrono::Utc::now() - chrono::Duration::hours(1);

        let mut remote = Node::new(
            "node1".to_string(),
            serde_json::json!({"name": "remote"}),
            None,
            vec![],
            "peer2".to_string(),
        );
        remote.data.metadata.updated_at = chrono::Utc::now();

        let resolved = resolver.resolve(&local, &remote).unwrap();
        assert_eq!(resolved.data.data["name"], "remote");
    }

    #[test]
    fn test_merge_fields() {
        let resolver = DefaultConflictResolver::new(ConflictStrategy::MergeFields);
        
        let local = Node::new(
            "node1".to_string(),
            serde_json::json!({"name": "local", "tags": ["tag1"]}),
            None,
            vec![],
            "peer1".to_string(),
        );

        let remote = Node::new(
            "node1".to_string(),
            serde_json::json!({"name": "remote", "tags": ["tag2"]}),
            None,
            vec![],
            "peer2".to_string(),
        );

        let resolved = resolver.resolve(&local, &remote).unwrap();
        assert_eq!(resolved.data.data["name"], "remote"); // Last writer wins for name
        assert!(resolved.data.data["tags"].as_array().unwrap().contains(&serde_json::json!("tag1")));
        assert!(resolved.data.data["tags"].as_array().unwrap().contains(&serde_json::json!("tag2")));
    }
}


