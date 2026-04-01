//! Storage abstraction for PluresDB.
//!
//! The real system will eventually support multiple pluggable backends. For the
//! initial native bootstrap we provide a simple in-memory store and a sled-based
//! durable implementation that can run entirely within the application process.

#[cfg(feature = "native")]
pub mod blob;
#[cfg(feature = "native")]
pub mod bridge;
#[cfg(feature = "native")]
pub mod encryption;
#[cfg(feature = "native")]
pub mod rad;
#[cfg(feature = "native")]
pub mod replay;
#[cfg(feature = "native")]
pub mod wal;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[cfg(feature = "native")]
use async_trait::async_trait;
#[cfg(feature = "native")]
use sled::IVec;
#[cfg(feature = "native")]
use std::path::Path;
#[cfg(feature = "native")]
use tracing::info;

#[cfg(feature = "native")]
pub use blob::{sha256_hex, validate_hash, BlobStore, FileBlobStore, MemoryBlobStore};
#[cfg(feature = "native")]
pub use bridge::{
    BlobObjectBridge, ChunkRef, Manifest, ObjectBridge, ObjectRestorer, SnapshotManager, WalFlusher,
};
#[cfg(feature = "native")]
pub use encryption::{EncryptionConfig, EncryptionMetadata};
#[cfg(feature = "native")]
pub use rad::{RadAdapter, SledRadAdapter};
#[cfg(feature = "native")]
pub use replay::{metadata_pruning, rebuild_from_wal, replay_wal, ReplayStats};
#[cfg(feature = "native")]
pub use wal::{DurabilityLevel, WalEntry, WalOperation, WalValidation, WriteAheadLog};

/// A node persisted by a storage engine.
///
/// Wraps an arbitrary JSON `payload` under a stable string `id`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredNode {
    /// Stable, unique identifier for this node.
    pub id: String,
    /// Arbitrary JSON payload associated with the node.
    pub payload: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Synchronous storage trait (always available, WASM-safe)
// ---------------------------------------------------------------------------

/// Synchronous CRUD interface for pluggable storage backends.
///
/// This trait is always available (including on WASM targets) and does not
/// require async runtimes.
pub trait SyncStorageEngine: Send + Sync {
    /// Persist or overwrite `node`, keyed by [`StoredNode::id`].
    fn put(&self, node: StoredNode) -> Result<()>;
    /// Return the node with the given `id`, or `None` if it does not exist.
    fn get(&self, id: &str) -> Result<Option<StoredNode>>;
    /// Remove the node with the given `id`.  Silently succeeds if absent.
    fn delete(&self, id: &str) -> Result<()>;
    /// Return all nodes currently held by this storage engine.
    fn list(&self) -> Result<Vec<StoredNode>>;
}

// ---------------------------------------------------------------------------
// Async storage trait (native only)
// ---------------------------------------------------------------------------

/// Async CRUD interface for pluggable storage backends.
///
/// Implement this trait to provide a custom persistence layer for PluresDB.
/// The two built-in implementations are [`MemoryStorage`] (non-durable, for
/// tests) and [`SledStorage`] (durable, embedded).
///
/// All methods are `async` so implementations may perform I/O without
/// blocking the Tokio runtime.
#[cfg(feature = "native")]
#[async_trait]
pub trait StorageEngine: Send + Sync {
    /// Persist or overwrite `node`, keyed by [`StoredNode::id`].
    async fn put(&self, node: StoredNode) -> Result<()>;
    /// Return the node with the given `id`, or `None` if it does not exist.
    async fn get(&self, id: &str) -> Result<Option<StoredNode>>;
    /// Remove the node with the given `id`.  Silently succeeds if absent.
    async fn delete(&self, id: &str) -> Result<()>;
    /// Return all nodes currently held by this storage engine.
    async fn list(&self) -> Result<Vec<StoredNode>>;
}

/// A non-persistent storage backend useful for tests and in-memory deployments.
#[derive(Debug, Default, Clone)]
pub struct MemoryStorage {
    inner: Arc<RwLock<HashMap<String, StoredNode>>>,
}

impl SyncStorageEngine for MemoryStorage {
    #[instrument(skip(self, node))]
    fn put(&self, node: StoredNode) -> Result<()> {
        self.inner.write().insert(node.id.clone(), node);
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<StoredNode>> {
        Ok(self.inner.read().get(id).cloned())
    }

    fn delete(&self, id: &str) -> Result<()> {
        self.inner.write().remove(id);
        Ok(())
    }

    fn list(&self) -> Result<Vec<StoredNode>> {
        Ok(self.inner.read().values().cloned().collect())
    }
}

#[cfg(feature = "native")]
#[async_trait]
impl StorageEngine for MemoryStorage {
    #[instrument(skip(self, node))]
    async fn put(&self, node: StoredNode) -> Result<()> {
        SyncStorageEngine::put(self, node)
    }

    async fn get(&self, id: &str) -> Result<Option<StoredNode>> {
        SyncStorageEngine::get(self, id)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        SyncStorageEngine::delete(self, id)
    }

    async fn list(&self) -> Result<Vec<StoredNode>> {
        SyncStorageEngine::list(self)
    }
}

/// Durable storage based on the sled embedded database.
#[cfg(feature = "native")]
#[derive(Debug, Clone)]
pub struct SledStorage {
    db: sled::Db,
}

#[cfg(feature = "native")]
impl SledStorage {
    /// Open (or create) a sled database at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        info!(path = %path.as_ref().display(), "opening sled storage");
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    /// Access the underlying sled database for advanced operations.
    pub fn db(&self) -> &sled::Db {
        &self.db
    }

    fn serialize(node: &StoredNode) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(node)?)
    }

    fn deserialize(bytes: IVec) -> Result<StoredNode> {
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[cfg(feature = "native")]
#[async_trait]
impl StorageEngine for SledStorage {
    async fn put(&self, node: StoredNode) -> Result<()> {
        let bytes = Self::serialize(&node)?;
        self.db.insert(node.id.as_bytes(), bytes)?;
        self.db.flush()?;
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<StoredNode>> {
        match self.db.get(id.as_bytes())? {
            Some(bytes) => Ok(Some(Self::deserialize(bytes)?)),
            None => Ok(None),
        }
    }

    async fn delete(&self, id: &str) -> Result<()> {
        self.db.remove(id.as_bytes())?;
        self.db.flush()?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<StoredNode>> {
        let mut out = Vec::new();
        for entry in self.db.iter() {
            let (_, value) = entry?;
            out.push(Self::deserialize(value)?);
        }
        Ok(out)
    }
}

#[cfg(feature = "native")]
impl SyncStorageEngine for SledStorage {
    fn put(&self, node: StoredNode) -> Result<()> {
        let bytes = Self::serialize(&node)?;
        self.db.insert(node.id.as_bytes(), bytes)?;
        self.db.flush()?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<StoredNode>> {
        match self.db.get(id.as_bytes())? {
            Some(bytes) => Ok(Some(Self::deserialize(bytes)?)),
            None => Ok(None),
        }
    }

    fn delete(&self, id: &str) -> Result<()> {
        self.db.remove(id.as_bytes())?;
        self.db.flush()?;
        Ok(())
    }

    fn list(&self) -> Result<Vec<StoredNode>> {
        let mut out = Vec::new();
        for entry in self.db.iter() {
            let (_, value) = entry?;
            out.push(Self::deserialize(value)?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_storage_sync_round_trip() {
        let storage = MemoryStorage::default();
        let node = StoredNode {
            id: "1".to_string(),
            payload: serde_json::json!({"name": "plures"}),
        };
        SyncStorageEngine::put(&storage, node.clone()).unwrap();
        let fetched = SyncStorageEngine::get(&storage, "1").unwrap().unwrap();
        assert_eq!(fetched, node);
        SyncStorageEngine::delete(&storage, "1").unwrap();
        assert!(SyncStorageEngine::get(&storage, "1").unwrap().is_none());
    }

    #[cfg(feature = "native")]
    #[tokio::test]
    async fn memory_storage_round_trip() {
        let storage = MemoryStorage::default();
        let node = StoredNode {
            id: "1".to_string(),
            payload: serde_json::json!({"name": "plures"}),
        };
        StorageEngine::put(&storage, node.clone()).await.unwrap();
        let fetched = StorageEngine::get(&storage, "1").await.unwrap().unwrap();
        assert_eq!(fetched, node);
        StorageEngine::delete(&storage, "1").await.unwrap();
        assert!(StorageEngine::get(&storage, "1").await.unwrap().is_none());
    }
}
