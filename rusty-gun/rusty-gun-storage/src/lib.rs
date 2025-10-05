//! # Rusty Gun Storage Engine
//! 
//! Storage engine with SQLite compatibility for Rusty Gun.
//! Provides multiple storage backends and vector search capabilities.

pub mod sqlite;
pub mod rocksdb;
pub mod sled;
pub mod vector;
pub mod embeddings;
pub mod vector_service;
pub mod migration;
pub mod error;
pub mod traits;

// Re-export implementations
pub use sqlite::SqliteStorage;
pub use rocksdb::RocksDBStorage;
pub use sled::SledStorage;
pub use vector::{HnswVectorEngine, InMemoryVectorEngine};
pub use embeddings::{EmbeddingGenerator, EmbeddingConfig, EmbeddingModel, EmbeddingResult, TextPreprocessor};
pub use vector_service::{VectorSearchService, SemanticSearchQuery, TextSearchResult, VectorServiceStats, ModelInfo, SearchFilter, FilterOperator};

// Re-export main types
pub use traits::{StorageEngine, VectorSearchEngine};
pub use error::{StorageError, Result};

/// Storage engine configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorageConfig {
    /// Storage backend type
    pub backend: StorageBackend,
    /// Database path
    pub path: String,
    /// Maximum number of connections
    pub max_connections: u32,
    /// Enable WAL mode (SQLite)
    pub enable_wal: bool,
    /// Enable foreign keys (SQLite)
    pub enable_foreign_keys: bool,
    /// Vector search configuration
    pub vector_config: VectorConfig,
}

/// Storage backend types
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StorageBackend {
    /// SQLite database
    Sqlite,
    /// RocksDB key-value store
    RocksDB,
    /// Sled embedded database
    Sled,
}

/// Vector search configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VectorConfig {
    /// Vector dimensions
    pub dimensions: usize,
    /// Maximum number of vectors
    pub max_vectors: usize,
    /// HNSW parameters
    pub hnsw_m: usize,
    pub hnsw_ef_construction: usize,
    pub hnsw_ef: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: StorageBackend::Sqlite,
            path: "./data/rusty-gun.db".to_string(),
            max_connections: 10,
            enable_wal: true,
            enable_foreign_keys: true,
            vector_config: VectorConfig::default(),
        }
    }
}

impl Default for VectorConfig {
    fn default() -> Self {
        Self {
            dimensions: 384,
            max_vectors: 1_000_000,
            hnsw_m: 16,
            hnsw_ef_construction: 200,
            hnsw_ef: 50,
        }
    }
}