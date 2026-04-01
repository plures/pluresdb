//! Hybrid storage bridge connecting PluresDB's Sled engine to a durable
//! content-addressed object store.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │  PluresDB Graph Engine              │
//! │  (Sled — fast reads/writes/traversals) │
//! ├─────────────────────────────────────┤
//! │  ObjectBridge (this module)         │
//! │  • Snapshot serializer              │
//! │  • WAL → object store flusher       │
//! │  • Restore from object store        │
//! ├─────────────────────────────────────┤
//! │  BlobStore (content-addressed CAS)  │
//! │  • SHA-256 content dedup            │
//! │  • MemoryBlobStore / FileBlobStore  │
//! └─────────────────────────────────────┘
//! ```
//!
//! The [`ObjectBridge`] trait is the central abstraction.  Callers hold an
//! [`Arc<dyn ObjectBridge>`] and interact with three operations:
//!
//! - [`ObjectBridge::snapshot`] – serialize the current graph state into
//!   content-addressed chunks and return a [`Manifest`].
//! - [`ObjectBridge::restore`] – given a [`Manifest`], fetch chunks and
//!   reconstruct the full list of [`StoredNode`]s.
//! - [`ObjectBridge::flush_wal`] – write a batch of [`WalEntry`] records to
//!   the object store as an append-only log chunk.

pub mod restorer;
pub mod snapshot;
pub mod wal_flusher;

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::wal::WalEntry;
use crate::{BlobStore, StoredNode};

pub use restorer::ObjectRestorer;
pub use snapshot::SnapshotManager;
pub use wal_flusher::WalFlusher;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A reference to a single content-addressed chunk stored in a [`BlobStore`].
///
/// The `hash` field is the lowercase hex SHA-256 digest produced by the blob
/// store.  The `size` field is the uncompressed byte length of the chunk
/// payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkRef {
    /// Lowercase hex SHA-256 digest of the chunk payload.
    pub hash: String,
    /// Byte length of the serialized chunk payload.
    pub size: usize,
}

/// A snapshot manifest that records the ordered list of chunks needed to
/// reconstruct a complete database state.
///
/// Each snapshot covers all [`StoredNode`]s at a single point in time.
/// Old manifests can be retained for time-travel queries or rollback.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    /// Unique identifier for this snapshot.
    pub id: String,
    /// Unix timestamp (seconds) when this snapshot was created.
    pub created_at: i64,
    /// Ordered chunks whose concatenated payloads reconstruct all nodes.
    pub chunks: Vec<ChunkRef>,
    /// Number of nodes stored across all chunks.
    pub node_count: usize,
    /// Optional human-readable label (e.g. `"hourly"`, `"pre-migration"`).
    pub label: Option<String>,
}

impl Manifest {
    /// Creates a new manifest with a random UUID and the current timestamp.
    pub fn new(chunks: Vec<ChunkRef>, node_count: usize, label: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: Utc::now().timestamp(),
            chunks,
            node_count,
            label,
        }
    }

    /// Serialises this manifest to JSON bytes for storage.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialises a manifest from JSON bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice(data)?)
    }
}

// ---------------------------------------------------------------------------
// ObjectBridge trait
// ---------------------------------------------------------------------------

/// Core abstraction connecting PluresDB's graph engine to a durable
/// content-addressed object store.
///
/// Implementations are responsible for chunking, serialising, and storing
/// graph state; the trait itself is agnostic to the underlying blob backend.
///
/// The built-in implementation ([`BlobObjectBridge`]) uses the existing
/// [`BlobStore`] trait (SHA-256 CAS) so no extra dependencies are required.
#[async_trait]
pub trait ObjectBridge: Send + Sync {
    /// Serialise `nodes` into content-addressed chunks, store them via the
    /// underlying blob store, and return a [`Manifest`] pointing to the chunks.
    ///
    /// Identical node data across snapshots will produce identical chunk
    /// hashes, enabling automatic deduplication.
    async fn snapshot(&self, nodes: Vec<StoredNode>, label: Option<String>) -> Result<Manifest>;

    /// Download all chunks referenced by `manifest`, reassemble them, and
    /// return the reconstructed [`StoredNode`] list.
    async fn restore(&self, manifest: &Manifest) -> Result<Vec<StoredNode>>;

    /// Serialise `entries` as an append-only log chunk and persist it to the
    /// blob store.  Returns a [`ChunkRef`] identifying the stored chunk.
    async fn flush_wal(&self, entries: Vec<WalEntry>) -> Result<ChunkRef>;

    /// Persist a manifest to the blob store so it can be retrieved later via
    /// [`ObjectBridge::load_manifest`].  Returns the SHA-256 hash of the
    /// stored manifest bytes.
    async fn store_manifest(&self, manifest: &Manifest) -> Result<String>;

    /// Retrieve a previously stored manifest by the hash returned from
    /// [`ObjectBridge::store_manifest`].
    async fn load_manifest(&self, hash: &str) -> Result<Option<Manifest>>;
}

// ---------------------------------------------------------------------------
// BlobObjectBridge — default implementation
// ---------------------------------------------------------------------------

/// Default [`ObjectBridge`] implementation built on top of the existing
/// [`BlobStore`] trait.
///
/// Graph state is chunked into fixed-size JSON batches, each batch stored as
/// a single blob.  The manifest records the ordered list of chunk hashes so
/// that restore can reassemble the original node list.
pub struct BlobObjectBridge {
    store: Arc<dyn BlobStore>,
    /// Maximum number of nodes serialised into a single chunk.
    chunk_size: usize,
}

impl std::fmt::Debug for BlobObjectBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlobObjectBridge")
            .field("chunk_size", &self.chunk_size)
            .finish_non_exhaustive()
    }
}

impl Clone for BlobObjectBridge {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            chunk_size: self.chunk_size,
        }
    }
}

impl BlobObjectBridge {
    /// Maximum nodes per chunk (default: 1_000).
    pub const DEFAULT_CHUNK_SIZE: usize = 1_000;

    /// Create a new bridge backed by `store` with the default chunk size.
    pub fn new(store: Arc<dyn BlobStore>) -> Self {
        Self {
            store,
            chunk_size: Self::DEFAULT_CHUNK_SIZE,
        }
    }

    /// Create a new bridge with a custom chunk size.
    pub fn with_chunk_size(store: Arc<dyn BlobStore>, chunk_size: usize) -> Self {
        assert!(chunk_size > 0, "chunk_size must be greater than zero");
        Self { store, chunk_size }
    }
}

#[async_trait]
impl ObjectBridge for BlobObjectBridge {
    async fn snapshot(&self, nodes: Vec<StoredNode>, label: Option<String>) -> Result<Manifest> {
        let node_count = nodes.len();
        let mut chunks = Vec::new();

        for batch in nodes.chunks(self.chunk_size) {
            let bytes = serde_json::to_vec(batch)?;
            let size = bytes.len();
            let hash = self.store.put(&bytes)?;
            chunks.push(ChunkRef { hash, size });
        }

        // An empty snapshot still produces a valid (zero-chunk) manifest.
        Ok(Manifest::new(chunks, node_count, label))
    }

    async fn restore(&self, manifest: &Manifest) -> Result<Vec<StoredNode>> {
        let mut nodes = Vec::with_capacity(manifest.node_count);

        for chunk_ref in &manifest.chunks {
            match self.store.get(&chunk_ref.hash)? {
                Some(bytes) => {
                    let batch: Vec<StoredNode> = serde_json::from_slice(&bytes)?;
                    nodes.extend(batch);
                }
                None => {
                    anyhow::bail!(
                        "chunk {} referenced by manifest {} is missing from the blob store",
                        chunk_ref.hash,
                        manifest.id
                    );
                }
            }
        }

        Ok(nodes)
    }

    async fn flush_wal(&self, entries: Vec<WalEntry>) -> Result<ChunkRef> {
        let bytes = serde_json::to_vec(&entries)?;
        let size = bytes.len();
        let hash = self.store.put(&bytes)?;
        Ok(ChunkRef { hash, size })
    }

    async fn store_manifest(&self, manifest: &Manifest) -> Result<String> {
        let bytes = manifest.to_bytes()?;
        let hash = self.store.put(&bytes)?;
        Ok(hash)
    }

    async fn load_manifest(&self, hash: &str) -> Result<Option<Manifest>> {
        match self.store.get(hash)? {
            Some(bytes) => Ok(Some(Manifest::from_bytes(&bytes)?)),
            None => Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blob::MemoryBlobStore;

    fn make_bridge() -> BlobObjectBridge {
        BlobObjectBridge::new(Arc::new(MemoryBlobStore::default()))
    }

    fn sample_nodes(n: usize) -> Vec<StoredNode> {
        (0..n)
            .map(|i| StoredNode {
                id: format!("node-{i}"),
                payload: serde_json::json!({"index": i, "name": format!("node-{i}")}),
            })
            .collect()
    }

    #[tokio::test]
    async fn test_snapshot_restore_round_trip() {
        let bridge = make_bridge();
        let nodes = sample_nodes(5);

        let manifest = bridge.snapshot(nodes.clone(), None).await.unwrap();
        assert_eq!(manifest.node_count, 5);
        assert!(!manifest.chunks.is_empty());

        let restored = bridge.restore(&manifest).await.unwrap();
        assert_eq!(restored.len(), nodes.len());
        for (original, restored_node) in nodes.iter().zip(restored.iter()) {
            assert_eq!(original.id, restored_node.id);
            assert_eq!(original.payload, restored_node.payload);
        }
    }

    #[tokio::test]
    async fn test_snapshot_empty_nodes() {
        let bridge = make_bridge();
        let manifest = bridge.snapshot(vec![], None).await.unwrap();
        assert_eq!(manifest.node_count, 0);
        assert!(manifest.chunks.is_empty());

        let restored = bridge.restore(&manifest).await.unwrap();
        assert!(restored.is_empty());
    }

    #[tokio::test]
    async fn test_snapshot_deduplication() {
        let bridge = make_bridge();
        let nodes = sample_nodes(3);

        let manifest1 = bridge
            .snapshot(nodes.clone(), Some("snap-1".to_string()))
            .await
            .unwrap();
        let manifest2 = bridge
            .snapshot(nodes.clone(), Some("snap-2".to_string()))
            .await
            .unwrap();

        // Same content produces the same chunk hashes (dedup).
        assert_eq!(manifest1.chunks, manifest2.chunks);
        // But each snapshot gets a unique manifest ID.
        assert_ne!(manifest1.id, manifest2.id);
    }

    #[tokio::test]
    async fn test_snapshot_multi_chunk() {
        let store = Arc::new(MemoryBlobStore::default());
        // chunk_size=2 → 5 nodes produce 3 chunks (2+2+1)
        let bridge = BlobObjectBridge::with_chunk_size(store, 2);
        let nodes = sample_nodes(5);

        let manifest = bridge.snapshot(nodes.clone(), None).await.unwrap();
        assert_eq!(manifest.chunks.len(), 3);
        assert_eq!(manifest.node_count, 5);

        let restored = bridge.restore(&manifest).await.unwrap();
        assert_eq!(restored.len(), 5);
    }

    #[tokio::test]
    async fn test_flush_wal_round_trip() {
        use crate::wal::{WalEntry, WalOperation};

        let bridge = make_bridge();
        let entries = vec![
            WalEntry::new(
                1,
                "actor-1".to_string(),
                WalOperation::Put {
                    id: "n1".to_string(),
                    data: serde_json::json!({"k": "v"}),
                },
            ),
            WalEntry::new(
                2,
                "actor-1".to_string(),
                WalOperation::Delete {
                    id: "n2".to_string(),
                },
            ),
        ];

        let chunk_ref = bridge.flush_wal(entries.clone()).await.unwrap();
        assert_eq!(chunk_ref.hash.len(), 64);
        assert!(chunk_ref.size > 0);

        // Fetch and deserialise the chunk to verify WAL entries are intact.
        let raw = bridge.store.get(&chunk_ref.hash).unwrap().unwrap();
        let decoded: Vec<WalEntry> = serde_json::from_slice(&raw).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].seq, 1);
        assert_eq!(decoded[1].seq, 2);
    }

    #[tokio::test]
    async fn test_store_and_load_manifest() {
        let bridge = make_bridge();
        let nodes = sample_nodes(3);
        let manifest = bridge.snapshot(nodes, None).await.unwrap();

        let hash = bridge.store_manifest(&manifest).await.unwrap();
        let loaded = bridge.load_manifest(&hash).await.unwrap().unwrap();

        assert_eq!(manifest.id, loaded.id);
        assert_eq!(manifest.chunks, loaded.chunks);
        assert_eq!(manifest.node_count, loaded.node_count);
    }

    #[tokio::test]
    async fn test_load_manifest_missing() {
        let bridge = make_bridge();
        let result = bridge
            .load_manifest("0000000000000000000000000000000000000000000000000000000000000000")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_manifest_round_trip_bytes() {
        let manifest = Manifest::new(
            vec![ChunkRef {
                hash: "a".repeat(64),
                size: 128,
            }],
            10,
            Some("test".to_string()),
        );
        let bytes = manifest.to_bytes().unwrap();
        let restored = Manifest::from_bytes(&bytes).unwrap();
        assert_eq!(manifest.id, restored.id);
        assert_eq!(manifest.node_count, restored.node_count);
        assert_eq!(manifest.label, restored.label);
    }
}
