//! PluresDB - P2P Graph Database with SQLite Compatibility
//!
//! PluresDB is a local-first, offline-first database for modern applications.
//! This crate provides a unified API that re-exports all core PluresDB functionality.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use pluresdb::{CrdtStore, MemoryStorage, StorageEngine};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let storage = MemoryStorage::default();
//! let store = CrdtStore::default();
//!
//! // Use the database...
//! # Ok(())
//! # }
//! ```
//!
//! ## Crate Organization
//!
//! PluresDB is organized into several crates:
//!
//! - **pluresdb-core**: Core CRDTs, data structures, and query primitives
//! - **pluresdb-storage**: Storage abstraction layer with multiple backends
//! - **pluresdb-sync**: Sync orchestration primitives for P2P peers
//!
//! This crate (`pluresdb`) re-exports the most commonly used types and functions
//! from these crates for convenience. You can also depend on individual crates
//! directly if you prefer.
//!
//! ## Features
//!
//! - `default`: Includes tokio for async support
//! - `async`: Enables async/await support (included in default)

// Re-export core types
pub use pluresdb_core::{
    ActorId, CrdtOperation, CrdtStore, Database, DatabaseOptions, DatabasePath, EmbedText,
    NodeData, NodeId, NodeRecord, QueryResult, SqlValue, VectorClock, VectorIndex,
    VectorSearchResult, DEFAULT_EMBEDDING_DIM,
};

#[cfg(feature = "embeddings")]
pub use pluresdb_core::FastEmbedder;

// Re-export storage types
pub use pluresdb_storage::{
    EncryptionConfig, EncryptionMetadata, MemoryStorage, ReplayStats, SledStorage,
    StorageEngine, StoredNode, WalEntry, WalOperation, WriteAheadLog,
};

// Re-export sync types
pub use pluresdb_sync::{SyncBroadcaster, SyncEvent};

// Re-export commonly used error types
pub use pluresdb_core::StoreError as CoreError;
pub use pluresdb_core::DatabaseError;

// Re-export storage replay utilities
pub use pluresdb_storage::{metadata_pruning, rebuild_from_wal, replay_wal};

/// Convenience function to create a new in-memory database
///
/// Returns a tuple of (CrdtStore, MemoryStorage) ready to use.
pub fn new_memory_database() -> (CrdtStore, MemoryStorage) {
    (CrdtStore::default(), MemoryStorage::default())
}

/// Convenience function to create a new persistent database
///
/// Opens a persistent database using SledStorage at the given path.
pub fn new_persistent_database(
    path: impl AsRef<std::path::Path>,
) -> anyhow::Result<(CrdtStore, SledStorage)> {
    let storage = SledStorage::open(path)?;
    Ok((CrdtStore::default(), storage))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_re_exports() {
        // Verify that all re-exported types are accessible
        let _store: CrdtStore = CrdtStore::default();
        let _storage: MemoryStorage = MemoryStorage::default();
        let _broadcaster: SyncBroadcaster = SyncBroadcaster::default();
    }

    #[test]
    fn test_convenience_functions() {
        let (_store, _storage) = new_memory_database();
    }

    // -----------------------------------------------------------------------
    // PluresDB integration: constructor, CRUD, vector search
    // -----------------------------------------------------------------------

    #[test]
    fn integration_crdt_store_crud() {
        let (store, _storage) = new_memory_database();

        // Create
        let id = store.put("doc-1", "user-1", serde_json::json!({"title": "Hello"}));
        assert_eq!(id, "doc-1");

        // Read
        let record = store.get("doc-1").expect("document should exist");
        assert_eq!(record.data["title"], "Hello");
        assert_eq!(record.clock.get("user-1"), Some(&1));

        // Update
        store.put("doc-1", "user-1", serde_json::json!({"title": "Updated"}));
        let updated = store.get("doc-1").expect("document should still exist");
        assert_eq!(updated.data["title"], "Updated");
        assert_eq!(updated.clock.get("user-1"), Some(&2));

        // List
        store.put("doc-2", "user-2", serde_json::json!({"title": "Second"}));
        assert_eq!(store.list().len(), 2);

        // Delete
        store.delete("doc-1").expect("delete should succeed");
        assert!(store.get("doc-1").is_none());
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn integration_vector_search() {
        let (store, _storage) = new_memory_database();

        let emb_a: Vec<f32> = vec![1.0, 0.0, 0.0];
        let emb_b: Vec<f32> = vec![0.0, 1.0, 0.0];

        store.put_with_embedding("v-a", "actor", serde_json::json!({"label": "a"}), emb_a.clone());
        store.put_with_embedding("v-b", "actor", serde_json::json!({"label": "b"}), emb_b);

        let results = store.vector_search(&emb_a, 2, 0.0);
        assert!(!results.is_empty());
        assert_eq!(results[0].record.id, "v-a");
        assert!(results[0].score > 0.99);
    }

    #[test]
    fn integration_sync_broadcaster_default() {
        // Verify SyncBroadcaster can be constructed and subscribed to (no panics)
        let broadcaster = SyncBroadcaster::default();
        let _receiver = broadcaster.subscribe();
    }
}

