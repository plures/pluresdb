//! Graph data structure and operations for Rusty Gun
//! 
//! This module provides the graph data structure and related operations
//! for managing relationships between nodes.

use crate::{
    error::{Error, Result},
    node::Node,
    types::*,
};
use dashmap::DashMap;
use std::collections::{HashMap, HashSet, VecDeque};

/// A graph of nodes and relationships
#[derive(Debug)]
pub struct Graph {
    /// Map of node ID to node
    nodes: Arc<DashMap<NodeId, Node>>,
    /// Map of node ID to outgoing relationships
    outgoing_relationships: Arc<DashMap<NodeId, Vec<Relationship>>>,
    /// Map of node ID to incoming relationships
    incoming_relationships: Arc<DashMap<NodeId, Vec<Relationship>>>,
    /// Map of relationship type to relationships
    relationships_by_type: Arc<DashMap<String, Vec<Relationship>>>,
}

impl Graph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(DashMap::new()),
            outgoing_relationships: Arc::new(DashMap::new()),
            incoming_relationships: Arc::new(DashMap::new()),
            relationships_by_type: Arc::new(DashMap::new()),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&self, node: Node) -> Result<()> {
        let node_id = node.id().clone();
        self.nodes.insert(node_id, node);
        Ok(())
    }

    /// Get a node by ID
    pub fn get_node(&self, node_id: &NodeId) -> Option<Node> {
        self.nodes.get(node_id).map(|entry| entry.clone())
    }

    /// Remove a node from the graph
    pub fn remove_node(&self, node_id: &NodeId) -> Result<()> {
        // Remove the node
        self.nodes.remove(node_id);

        // Remove all relationships involving this node
        self.outgoing_relationships.remove(node_id);
        self.incoming_relationships.remove(node_id);

        // Remove from relationships by type
        for mut relationships in self.relationships_by_type.iter_mut() {
            relationships.retain(|rel| rel.from != *node_id && rel.to != *node_id);
        }

        Ok(())
    }

    /// Add a relationship between two nodes
    pub fn add_relationship(
        &self,
        from_node: &NodeId,
        to_node: &NodeId,
        relation_type: String,
        data: Option<serde_json::Value>,
        created_by: PeerId,
    ) -> Result<Relationship> {
        // Validate that both nodes exist
        if !self.nodes.contains_key(from_node) {
            return Err(Error::NodeNotFound { id: from_node.clone() });
        }
        if !self.nodes.contains_key(to_node) {
            return Err(Error::NodeNotFound { id: to_node.clone() });
        }

        // Create the relationship
        let relationship = Relationship {
            from: from_node.clone(),
            to: to_node.clone(),
            relation_type: relation_type.clone(),
            data,
            created_at: chrono::Utc::now(),
            created_by,
        };

        // Add to outgoing relationships
        self.outgoing_relationships
            .entry(from_node.clone())
            .or_insert_with(Vec::new)
            .push(relationship.clone());

        // Add to incoming relationships
        self.incoming_relationships
            .entry(to_node.clone())
            .or_insert_with(Vec::new)
            .push(relationship.clone());

        // Add to relationships by type
        self.relationships_by_type
            .entry(relation_type)
            .or_insert_with(Vec::new)
            .push(relationship.clone());

        Ok(relationship)
    }

    /// Remove a relationship
    pub fn remove_relationship(
        &self,
        from_node: &NodeId,
        to_node: &NodeId,
        relation_type: &str,
    ) -> Result<()> {
        // Remove from outgoing relationships
        if let Some(mut relationships) = self.outgoing_relationships.get_mut(from_node) {
            relationships.retain(|rel| !(rel.to == *to_node && rel.relation_type == relation_type));
        }

        // Remove from incoming relationships
        if let Some(mut relationships) = self.incoming_relationships.get_mut(to_node) {
            relationships.retain(|rel| !(rel.from == *from_node && rel.relation_type == relation_type));
        }

        // Remove from relationships by type
        if let Some(mut relationships) = self.relationships_by_type.get_mut(relation_type) {
            relationships.retain(|rel| !(rel.from == *from_node && rel.to == *to_node));
        }

        Ok(())
    }

    /// Get all relationships for a node
    pub fn get_relationships(&self, node_id: &NodeId) -> Vec<Relationship> {
        let mut relationships = Vec::new();
        
        // Add outgoing relationships
        if let Some(outgoing) = self.outgoing_relationships.get(node_id) {
            relationships.extend(outgoing.iter().cloned());
        }

        // Add incoming relationships
        if let Some(incoming) = self.incoming_relationships.get(node_id) {
            relationships.extend(incoming.iter().cloned());
        }

        relationships
    }

    /// Get outgoing relationships for a node
    pub fn get_outgoing_relationships(&self, node_id: &NodeId) -> Vec<Relationship> {
        self.outgoing_relationships
            .get(node_id)
            .map(|entry| entry.clone())
            .unwrap_or_default()
    }

    /// Get incoming relationships for a node
    pub fn get_incoming_relationships(&self, node_id: &NodeId) -> Vec<Relationship> {
        self.incoming_relationships
            .get(node_id)
            .map(|entry| entry.clone())
            .unwrap_or_default()
    }

    /// Get relationships by type
    pub fn get_relationships_by_type(&self, relation_type: &str) -> Vec<Relationship> {
        self.relationships_by_type
            .get(relation_type)
            .map(|entry| entry.clone())
            .unwrap_or_default()
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

    /// Find nodes connected to a given node
    pub fn get_connected_nodes(&self, node_id: &NodeId) -> Vec<Node> {
        let mut connected_nodes = HashSet::new();
        
        // Get outgoing relationships
        for relationship in self.get_outgoing_relationships(node_id) {
            if let Some(node) = self.get_node(&relationship.to) {
                connected_nodes.insert(node);
            }
        }

        // Get incoming relationships
        for relationship in self.get_incoming_relationships(node_id) {
            if let Some(node) = self.get_node(&relationship.from) {
                connected_nodes.insert(node);
            }
        }

        connected_nodes.into_iter().collect()
    }

    /// Find shortest path between two nodes
    pub fn find_shortest_path(&self, from: &NodeId, to: &NodeId) -> Option<Vec<NodeId>> {
        if from == to {
            return Some(vec![from.clone()]);
        }

        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut parent = HashMap::new();

        queue.push_back(from.clone());
        visited.insert(from.clone());

        while let Some(current) = queue.pop_front() {
            if current == *to {
                // Reconstruct path
                let mut path = Vec::new();
                let mut node = &current;
                while let Some(parent_node) = parent.get(node) {
                    path.push(node.clone());
                    node = parent_node;
                }
                path.push(from.clone());
                path.reverse();
                return Some(path);
            }

            // Check outgoing relationships
            for relationship in self.get_outgoing_relationships(&current) {
                if !visited.contains(&relationship.to) {
                    visited.insert(relationship.to.clone());
                    parent.insert(relationship.to.clone(), current.clone());
                    queue.push_back(relationship.to.clone());
                }
            }

            // Check incoming relationships
            for relationship in self.get_incoming_relationships(&current) {
                if !visited.contains(&relationship.from) {
                    visited.insert(relationship.from.clone());
                    parent.insert(relationship.from.clone(), current.clone());
                    queue.push_back(relationship.from.clone());
                }
            }
        }

        None
    }

    /// Find all paths between two nodes
    pub fn find_all_paths(&self, from: &NodeId, to: &NodeId, max_depth: usize) -> Vec<Vec<NodeId>> {
        let mut paths = Vec::new();
        let mut current_path = Vec::new();
        self.find_paths_recursive(from, to, &mut current_path, &mut paths, max_depth);
        paths
    }

    /// Recursive helper for finding all paths
    fn find_paths_recursive(
        &self,
        current: &NodeId,
        target: &NodeId,
        current_path: &mut Vec<NodeId>,
        paths: &mut Vec<Vec<NodeId>>,
        max_depth: usize,
    ) {
        if current_path.len() >= max_depth {
            return;
        }

        current_path.push(current.clone());

        if current == target {
            paths.push(current_path.clone());
        } else {
            // Check outgoing relationships
            for relationship in self.get_outgoing_relationships(current) {
                if !current_path.contains(&relationship.to) {
                    self.find_paths_recursive(&relationship.to, target, current_path, paths, max_depth);
                }
            }

            // Check incoming relationships
            for relationship in self.get_incoming_relationships(current) {
                if !current_path.contains(&relationship.from) {
                    self.find_paths_recursive(&relationship.from, target, current_path, paths, max_depth);
                }
            }
        }

        current_path.pop();
    }

    /// Get graph statistics
    pub fn get_stats(&self) -> GraphStats {
        let node_count = self.nodes.len();
        let relationship_count = self.outgoing_relationships
            .iter()
            .map(|entry| entry.len())
            .sum::<usize>();

        let mut node_types = HashMap::new();
        let mut relationship_types = HashMap::new();

        for entry in self.nodes.iter() {
            if let Some(node_type) = &entry.data.metadata.node_type {
                *node_types.entry(node_type.clone()).or_insert(0) += 1;
            }
        }

        for entry in self.relationships_by_type.iter() {
            relationship_types.insert(entry.key().clone(), entry.len());
        }

        GraphStats {
            node_count,
            relationship_count,
            node_types,
            relationship_types,
        }
    }

    /// Clear all data
    pub fn clear(&self) {
        self.nodes.clear();
        self.outgoing_relationships.clear();
        self.incoming_relationships.clear();
        self.relationships_by_type.clear();
    }
}

/// Graph statistics
#[derive(Debug, Clone)]
pub struct GraphStats {
    pub node_count: usize,
    pub relationship_count: usize,
    pub node_types: HashMap<String, usize>,
    pub relationship_types: HashMap<String, usize>,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_creation() {
        let graph = Graph::new();
        assert_eq!(graph.get_all_nodes().len(), 0);
    }

    #[test]
    fn test_add_node() {
        let graph = Graph::new();
        let node = Node::new(
            "node1".to_string(),
            serde_json::json!({"name": "test"}),
            None,
            vec![],
            "peer1".to_string(),
        );

        graph.add_node(node.clone()).unwrap();
        assert_eq!(graph.get_all_nodes().len(), 1);
        assert_eq!(graph.get_node(&"node1".to_string()), Some(node));
    }

    #[test]
    fn test_add_relationship() {
        let graph = Graph::new();
        
        let node1 = Node::new(
            "node1".to_string(),
            serde_json::json!({"name": "node1"}),
            None,
            vec![],
            "peer1".to_string(),
        );
        
        let node2 = Node::new(
            "node2".to_string(),
            serde_json::json!({"name": "node2"}),
            None,
            vec![],
            "peer1".to_string(),
        );

        graph.add_node(node1).unwrap();
        graph.add_node(node2).unwrap();

        let relationship = graph.add_relationship(
            &"node1".to_string(),
            &"node2".to_string(),
            "relates_to".to_string(),
            Some(serde_json::json!({"strength": 0.8})),
            "peer1".to_string(),
        ).unwrap();

        assert_eq!(relationship.from, "node1");
        assert_eq!(relationship.to, "node2");
        assert_eq!(relationship.relation_type, "relates_to");

        let relationships = graph.get_relationships(&"node1".to_string());
        assert_eq!(relationships.len(), 1);
    }

    #[test]
    fn test_shortest_path() {
        let graph = Graph::new();
        
        // Create a simple chain: node1 -> node2 -> node3
        let node1 = Node::new("node1".to_string(), serde_json::json!({}), None, vec![], "peer1".to_string());
        let node2 = Node::new("node2".to_string(), serde_json::json!({}), None, vec![], "peer1".to_string());
        let node3 = Node::new("node3".to_string(), serde_json::json!({}), None, vec![], "peer1".to_string());

        graph.add_node(node1).unwrap();
        graph.add_node(node2).unwrap();
        graph.add_node(node3).unwrap();

        graph.add_relationship(
            &"node1".to_string(),
            &"node2".to_string(),
            "next".to_string(),
            None,
            "peer1".to_string(),
        ).unwrap();

        graph.add_relationship(
            &"node2".to_string(),
            &"node3".to_string(),
            "next".to_string(),
            None,
            "peer1".to_string(),
        ).unwrap();

        let path = graph.find_shortest_path(&"node1".to_string(), &"node3".to_string());
        assert_eq!(path, Some(vec!["node1".to_string(), "node2".to_string(), "node3".to_string()]));
    }
}


