//! # Rusty Gun Core
//! 
//! Core CRDT and data structures for Rusty Gun - a P2P graph database.
//! 
//! This library provides the fundamental building blocks for a distributed,
//! conflict-free replicated data type (CRDT) system that enables local-first
//! applications with automatic synchronization.

pub mod crdt;
pub mod node;
pub mod graph;
pub mod conflict;
pub mod crypto;
pub mod error;
pub mod types;

// Re-export main types for convenience
pub use crdt::Crdt;
pub use node::Node;
pub use graph::Graph;
pub use conflict::ConflictResolver;
pub use error::{Error, Result};
pub use types::*;

/// Version of the Rusty Gun protocol
pub const PROTOCOL_VERSION: &str = "1.0.0";

/// Maximum size of a single node in bytes
pub const MAX_NODE_SIZE: usize = 1024 * 1024; // 1MB

/// Maximum depth of nested nodes
pub const MAX_NODE_DEPTH: usize = 100;

/// Default vector dimensions for embeddings
pub const DEFAULT_VECTOR_DIM: usize = 384;