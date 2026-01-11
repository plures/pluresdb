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
    ActorId, CrdtOperation, CrdtStore, Database, DatabaseOptions, DatabasePath, NodeData,
    NodeId, NodeRecord, QueryResult, SqlValue, VectorClock,
};

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
) -> pluresdb_storage::anyhow::Result<(CrdtStore, SledStorage)> {
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
}

