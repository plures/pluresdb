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
pub use wal::{DurabilityLevel, WalEntry, WalError, WalOperation, WalValidation, WriteAheadLog};

/// Stable, documented error codes emitted by `pluresdb-storage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageErrorCode {
    OpenFailed,
    OperationFailed,
    SerializationError,
    WalImplausibleEntrySize,
    WalTruncatedEntry,
}

impl StorageErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenFailed => "STORAGE_OPEN_FAILED",
            Self::OperationFailed => "STORAGE_OPERATION_FAILED",
            Self::SerializationError => "STORAGE_SERIALIZATION_ERROR",
            Self::WalImplausibleEntrySize => "STORAGE_WAL_IMPLAUSIBLE_ENTRY_SIZE",
            Self::WalTruncatedEntry => "STORAGE_WAL_TRUNCATED_ENTRY",
        }
    }
}

impl std::fmt::Display for StorageErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

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

    /// Return the total number of stored nodes without loading them into memory.
    fn count(&self) -> Result<usize> {
        Ok(self.list()?.len())
    }

    /// Iterate over nodes one at a time via callback, avoiding full
    /// materialization.  Return `false` from `f` to stop early.
    fn for_each(&self, f: &mut (dyn FnMut(StoredNode) -> bool + Send)) -> Result<()> {
        for node in self.list()? {
            if !f(node) {
                break;
            }
        }
        Ok(())
    }

    /// Iterate over nodes whose ID starts with `prefix`.
    fn for_each_by_prefix(
        &self,
        prefix: &str,
        f: &mut (dyn FnMut(StoredNode) -> bool + Send),
    ) -> Result<()> {
        let prefix = prefix.to_string();
        self.for_each(&mut |node: StoredNode| {
            if node.id.starts_with(&prefix) {
                f(node)
            } else {
                true
            }
        })
    }
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

    /// Return the total number of stored nodes without loading them into memory.
    async fn count(&self) -> Result<usize> {
        Ok(self.list().await?.len())
    }

    /// Iterate over nodes one at a time via callback, avoiding full
    /// materialization.  Return `false` from `f` to stop early.
    async fn for_each(&self, f: &mut (dyn FnMut(StoredNode) -> bool + Send)) -> Result<()> {
        for node in self.list().await? {
            if !f(node) {
                break;
            }
        }
        Ok(())
    }

    /// Iterate over nodes whose ID starts with `prefix`.
    async fn for_each_by_prefix(
        &self,
        prefix: &str,
        f: &mut (dyn FnMut(StoredNode) -> bool + Send),
    ) -> Result<()> {
        let prefix = prefix.to_string();
        self.for_each(&mut |node: StoredNode| {
            if node.id.starts_with(&prefix) {
                f(node)
            } else {
                true
            }
        })
        .await
    }
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
    const DEFAULT_CACHE_CAPACITY_BYTES: u64 = 256 * 1024 * 1024;

    /// Open (or create) a sled database at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        info!(path = %path.as_ref().display(), "opening sled storage");
        let db = sled::Config::default()
            .path(path)
            .cache_capacity(Self::DEFAULT_CACHE_CAPACITY_BYTES)
            .open()?;
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

    fn count(&self) -> Result<usize> {
        Ok(self.db.len())
    }

    fn for_each(&self, f: &mut (dyn FnMut(StoredNode) -> bool + Send)) -> Result<()> {
        for entry in self.db.iter() {
            let (_, value) = entry?;
            let node = Self::deserialize(value)?;
            if !f(node) {
                break;
            }
        }
        Ok(())
    }

    fn for_each_by_prefix(
        &self,
        prefix: &str,
        f: &mut (dyn FnMut(StoredNode) -> bool + Send),
    ) -> Result<()> {
        for entry in self.db.scan_prefix(prefix.as_bytes()) {
            let (_, value) = entry?;
            let node = Self::deserialize(value)?;
            if !f(node) {
                break;
            }
        }
        Ok(())
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

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn node(id: &str) -> StoredNode {
        StoredNode {
            id: id.to_string(),
            payload: serde_json::json!({ "id": id }),
        }
    }

    // -----------------------------------------------------------------------
    // StorageErrorCode::as_str + Display (kills "xyzzy" and Default::default fmt)
    // -----------------------------------------------------------------------

    #[test]
    fn storage_error_code_as_str_exact_mapping() {
        assert_eq!(StorageErrorCode::OpenFailed.as_str(), "STORAGE_OPEN_FAILED");
        assert_eq!(
            StorageErrorCode::OperationFailed.as_str(),
            "STORAGE_OPERATION_FAILED"
        );
        assert_eq!(
            StorageErrorCode::SerializationError.as_str(),
            "STORAGE_SERIALIZATION_ERROR"
        );
        assert_eq!(
            StorageErrorCode::WalImplausibleEntrySize.as_str(),
            "STORAGE_WAL_IMPLAUSIBLE_ENTRY_SIZE"
        );
        assert_eq!(
            StorageErrorCode::WalTruncatedEntry.as_str(),
            "STORAGE_WAL_TRUNCATED_ENTRY"
        );
    }

    #[test]
    fn storage_error_code_display_matches_as_str() {
        // Display must produce the exact non-empty code string (kills the
        // `Ok(Default::default())` fmt mutant, which would write nothing).
        for code in [
            StorageErrorCode::OpenFailed,
            StorageErrorCode::OperationFailed,
            StorageErrorCode::SerializationError,
            StorageErrorCode::WalImplausibleEntrySize,
            StorageErrorCode::WalTruncatedEntry,
        ] {
            let shown = format!("{code}");
            assert_eq!(shown, code.as_str());
            assert!(!shown.is_empty());
        }
        // Pin one concrete value so an all-empty Display can never pass.
        assert_eq!(format!("{}", StorageErrorCode::OpenFailed), "STORAGE_OPEN_FAILED");
    }

    // -----------------------------------------------------------------------
    // SledStorage cache-capacity constant (kills L257 `* -> +` / `* -> /`)
    // -----------------------------------------------------------------------

    #[cfg(feature = "native")]
    #[test]
    fn sled_default_cache_capacity_is_256_mib() {
        // 256 * 1024 * 1024 == 268_435_456. `+` would give 256+1024+1024=2304;
        // `/` would give 256/1024/1024=0. Exact value pins the arithmetic.
        assert_eq!(SledStorage::DEFAULT_CACHE_CAPACITY_BYTES, 268_435_456);
    }

    // -----------------------------------------------------------------------
    // SyncStorageEngine default methods via MemoryStorage
    // (count / for_each / for_each_by_prefix contracts)
    // -----------------------------------------------------------------------

    #[test]
    fn sync_count_reflects_inserted_node_set() {
        let storage = MemoryStorage::default();
        // Distinct count that is neither 0 nor 1, so Ok(0)/Ok(1) both die.
        for i in 0..7 {
            SyncStorageEngine::put(&storage, node(&format!("k{i}"))).unwrap();
        }
        assert_eq!(SyncStorageEngine::count(&storage).unwrap(), 7);
    }

    #[test]
    fn sync_for_each_visits_exactly_the_full_set() {
        let storage = MemoryStorage::default();
        let ids = ["a", "b", "c", "d"];
        for id in ids {
            SyncStorageEngine::put(&storage, node(id)).unwrap();
        }
        let mut visited = std::collections::BTreeSet::new();
        SyncStorageEngine::for_each(&storage, &mut |n: StoredNode| {
            visited.insert(n.id);
            true
        })
        .unwrap();
        let expected: std::collections::BTreeSet<String> =
            ids.iter().map(|s| s.to_string()).collect();
        // Empty-set would pass an `Ok(())` early-return mutant, so assert the
        // exact membership AND non-empty size.
        assert_eq!(visited, expected);
        assert_eq!(visited.len(), 4);
    }

    #[test]
    fn sync_for_each_negation_mutant_dies_on_early_stop() {
        // The body is `if !f(node) { break; }`. Deleting the `!` inverts the
        // stop condition: a callback that returns false on the FIRST node
        // would (mutated) keep going and visit everything. We return false
        // immediately and assert we stopped after exactly one visit.
        let storage = MemoryStorage::default();
        for i in 0..5 {
            SyncStorageEngine::put(&storage, node(&format!("n{i}"))).unwrap();
        }
        let mut count = 0usize;
        SyncStorageEngine::for_each(&storage, &mut |_n: StoredNode| {
            count += 1;
            false // request stop after the first element
        })
        .unwrap();
        // Correct code: stops after 1. Negated `!`: would visit all 5.
        assert_eq!(count, 1);
    }

    #[test]
    fn sync_for_each_by_prefix_filters() {
        let storage = MemoryStorage::default();
        for id in ["user:1", "user:2", "job:1", "job:2", "user:3"] {
            SyncStorageEngine::put(&storage, node(id)).unwrap();
        }
        let mut visited = std::collections::BTreeSet::new();
        SyncStorageEngine::for_each_by_prefix(&storage, "user:", &mut |n: StoredNode| {
            visited.insert(n.id);
            true
        })
        .unwrap();
        let expected: std::collections::BTreeSet<String> =
            ["user:1", "user:2", "user:3"].iter().map(|s| s.to_string()).collect();
        // An `Ok(())` early-return mutant yields empty (fails). A broken
        // filter that visited everything would include the job:* ids (fails).
        assert_eq!(visited, expected);
    }

    // -----------------------------------------------------------------------
    // Async StorageEngine default methods via MemoryStorage
    // -----------------------------------------------------------------------

    #[cfg(feature = "native")]
    #[tokio::test]
    async fn async_count_reflects_inserted_node_set() {
        let storage = MemoryStorage::default();
        for i in 0..6 {
            StorageEngine::put(&storage, node(&format!("k{i}"))).await.unwrap();
        }
        assert_eq!(StorageEngine::count(&storage).await.unwrap(), 6);
    }

    #[cfg(feature = "native")]
    #[tokio::test]
    async fn async_for_each_visits_exactly_the_full_set() {
        let storage = MemoryStorage::default();
        let ids = ["a", "b", "c"];
        for id in ids {
            StorageEngine::put(&storage, node(id)).await.unwrap();
        }
        let mut visited = std::collections::BTreeSet::new();
        StorageEngine::for_each(&storage, &mut |n: StoredNode| {
            visited.insert(n.id);
            true
        })
        .await
        .unwrap();
        let expected: std::collections::BTreeSet<String> =
            ids.iter().map(|s| s.to_string()).collect();
        assert_eq!(visited, expected);
        assert_eq!(visited.len(), 3);
    }

    #[cfg(feature = "native")]
    #[tokio::test]
    async fn async_for_each_negation_mutant_dies_on_early_stop() {
        let storage = MemoryStorage::default();
        for i in 0..5 {
            StorageEngine::put(&storage, node(&format!("n{i}"))).await.unwrap();
        }
        let mut count = 0usize;
        StorageEngine::for_each(&storage, &mut |_n: StoredNode| {
            count += 1;
            false
        })
        .await
        .unwrap();
        assert_eq!(count, 1);
    }

    #[cfg(feature = "native")]
    #[tokio::test]
    async fn async_for_each_by_prefix_filters() {
        let storage = MemoryStorage::default();
        for id in ["user:1", "user:2", "job:1", "user:3"] {
            StorageEngine::put(&storage, node(id)).await.unwrap();
        }
        let mut visited = std::collections::BTreeSet::new();
        StorageEngine::for_each_by_prefix(&storage, "user:", &mut |n: StoredNode| {
            visited.insert(n.id);
            true
        })
        .await
        .unwrap();
        let expected: std::collections::BTreeSet<String> =
            ["user:1", "user:2", "user:3"].iter().map(|s| s.to_string()).collect();
        assert_eq!(visited, expected);
    }

    // -----------------------------------------------------------------------
    // SledStorage (durable) impls — full round-trips + count/for_each
    // -----------------------------------------------------------------------

    #[cfg(feature = "native")]
    fn sled_storage() -> (SledStorage, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let storage = SledStorage::open(dir.path()).unwrap();
        (storage, dir)
    }

    #[cfg(feature = "native")]
    #[test]
    fn sled_sync_put_get_delete_list_round_trip() {
        let (storage, _dir) = sled_storage();
        let n1 = node("alpha");
        let n2 = node("beta");

        // put + get returns the exact stored value (kills put->Ok(()) and
        // get->Ok(None)).
        SyncStorageEngine::put(&storage, n1.clone()).unwrap();
        SyncStorageEngine::put(&storage, n2.clone()).unwrap();
        assert_eq!(
            SyncStorageEngine::get(&storage, "alpha").unwrap(),
            Some(n1.clone())
        );
        assert_eq!(
            SyncStorageEngine::get(&storage, "beta").unwrap(),
            Some(n2.clone())
        );

        // list returns both (kills list->Ok(vec![])).
        let mut listed: Vec<String> = SyncStorageEngine::list(&storage)
            .unwrap()
            .into_iter()
            .map(|n| n.id)
            .collect();
        listed.sort();
        assert_eq!(listed, vec!["alpha".to_string(), "beta".to_string()]);

        // delete then get is None (kills delete->Ok(()) which would leave it).
        SyncStorageEngine::delete(&storage, "alpha").unwrap();
        assert_eq!(SyncStorageEngine::get(&storage, "alpha").unwrap(), None);
        assert_eq!(
            SyncStorageEngine::get(&storage, "beta").unwrap(),
            Some(n2)
        );
    }

    #[cfg(feature = "native")]
    #[test]
    fn sled_sync_count_reflects_node_set() {
        let (storage, _dir) = sled_storage();
        for i in 0..9 {
            SyncStorageEngine::put(&storage, node(&format!("k{i}"))).unwrap();
        }
        // Neither 0 nor 1 — kills both count mutants.
        assert_eq!(SyncStorageEngine::count(&storage).unwrap(), 9);
        // And it tracks deletions.
        SyncStorageEngine::delete(&storage, "k0").unwrap();
        assert_eq!(SyncStorageEngine::count(&storage).unwrap(), 8);
    }

    #[cfg(feature = "native")]
    #[test]
    fn sled_sync_for_each_visits_full_set_and_stops_early() {
        let (storage, _dir) = sled_storage();
        let ids = ["a", "b", "c", "d"];
        for id in ids {
            SyncStorageEngine::put(&storage, node(id)).unwrap();
        }

        // Full visit.
        let mut visited = std::collections::BTreeSet::new();
        SyncStorageEngine::for_each(&storage, &mut |n: StoredNode| {
            visited.insert(n.id);
            true
        })
        .unwrap();
        let expected: std::collections::BTreeSet<String> =
            ids.iter().map(|s| s.to_string()).collect();
        assert_eq!(visited, expected);

        // Early-stop: kills the `delete !` negation mutant in the Sled
        // for_each body (sled iterates in key order, so returning false on the
        // first element must stop after exactly one visit).
        let mut count = 0usize;
        SyncStorageEngine::for_each(&storage, &mut |_n: StoredNode| {
            count += 1;
            false
        })
        .unwrap();
        assert_eq!(count, 1);
    }

    #[cfg(feature = "native")]
    #[test]
    fn sled_sync_for_each_by_prefix_filters_and_stops_early() {
        let (storage, _dir) = sled_storage();
        for id in ["user:1", "user:2", "user:3", "job:1", "job:2"] {
            SyncStorageEngine::put(&storage, node(id)).unwrap();
        }

        // Only the user: prefix is visited (kills for_each_by_prefix->Ok(())).
        let mut visited = std::collections::BTreeSet::new();
        SyncStorageEngine::for_each_by_prefix(&storage, "user:", &mut |n: StoredNode| {
            visited.insert(n.id);
            true
        })
        .unwrap();
        let expected: std::collections::BTreeSet<String> =
            ["user:1", "user:2", "user:3"].iter().map(|s| s.to_string()).collect();
        assert_eq!(visited, expected);

        // Early-stop within the prefix scan kills the `delete !` negation.
        let mut count = 0usize;
        SyncStorageEngine::for_each_by_prefix(&storage, "user:", &mut |_n: StoredNode| {
            count += 1;
            false
        })
        .unwrap();
        assert_eq!(count, 1);
    }

    #[cfg(feature = "native")]
    #[tokio::test]
    async fn sled_async_put_get_delete_list_round_trip() {
        let (storage, _dir) = sled_storage();
        let n1 = node("alpha");
        let n2 = node("beta");
        StorageEngine::put(&storage, n1.clone()).await.unwrap();
        StorageEngine::put(&storage, n2.clone()).await.unwrap();
        assert_eq!(
            StorageEngine::get(&storage, "alpha").await.unwrap(),
            Some(n1)
        );
        let listed = StorageEngine::list(&storage).await.unwrap();
        assert_eq!(listed.len(), 2);
        StorageEngine::delete(&storage, "alpha").await.unwrap();
        assert_eq!(StorageEngine::get(&storage, "alpha").await.unwrap(), None);
        assert_eq!(
            StorageEngine::get(&storage, "beta").await.unwrap(),
            Some(n2)
        );
    }
}
