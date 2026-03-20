//! Object-store restore utilities for the hybrid storage backend.
//!
//! [`ObjectRestorer`] loads a [`Manifest`] from the blob store and
//! reconstructs the full list of [`StoredNode`]s by fetching and
//! deserialising each referenced chunk.

use std::sync::Arc;

use anyhow::Result;
use tracing::{info, instrument};

use crate::StoredNode;
use super::{Manifest, ObjectBridge};

// ---------------------------------------------------------------------------
// ObjectRestorer
// ---------------------------------------------------------------------------

/// Restores database state from a manifest stored in the object store.
///
/// This is the inverse of [`SnapshotManager::snapshot`]: given a manifest
/// hash it retrieves all referenced chunks from the blob store and
/// reconstructs the [`StoredNode`] list.
pub struct ObjectRestorer {
    bridge: Arc<dyn ObjectBridge>,
}

impl std::fmt::Debug for ObjectRestorer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectRestorer").finish_non_exhaustive()
    }
}

impl Clone for ObjectRestorer {
    fn clone(&self) -> Self {
        Self {
            bridge: self.bridge.clone(),
        }
    }
}

impl ObjectRestorer {
    /// Create a new restorer backed by `bridge`.
    pub fn new(bridge: Arc<dyn ObjectBridge>) -> Self {
        Self { bridge }
    }

    /// Restore graph state from the manifest stored at `manifest_hash`.
    ///
    /// The hash must be the value returned by a previous call to
    /// [`ObjectBridge::store_manifest`].
    #[instrument(skip(self), fields(%manifest_hash))]
    pub async fn restore_from_hash(&self, manifest_hash: &str) -> Result<Vec<StoredNode>> {
        let manifest = self
            .bridge
            .load_manifest(manifest_hash)
            .await?
            .ok_or_else(|| anyhow::anyhow!("manifest not found: {}", manifest_hash))?;

        self.restore_from_manifest(&manifest).await
    }

    /// Restore graph state directly from a [`Manifest`].
    #[instrument(skip(self, manifest), fields(manifest_id = %manifest.id, chunk_count = manifest.chunks.len()))]
    pub async fn restore_from_manifest(&self, manifest: &Manifest) -> Result<Vec<StoredNode>> {
        info!(
            manifest_id = %manifest.id,
            node_count = manifest.node_count,
            chunk_count = manifest.chunks.len(),
            "restoring from manifest"
        );

        let nodes = self.bridge.restore(manifest).await?;

        info!(
            manifest_id = %manifest.id,
            restored = nodes.len(),
            "restore complete"
        );

        Ok(nodes)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blob::MemoryBlobStore;
    use crate::bridge::BlobObjectBridge;

    fn make_restorer() -> (ObjectRestorer, Arc<BlobObjectBridge>) {
        let bridge = Arc::new(BlobObjectBridge::new(Arc::new(MemoryBlobStore::default())));
        let restorer = ObjectRestorer::new(bridge.clone());
        (restorer, bridge)
    }

    fn nodes(n: usize) -> Vec<StoredNode> {
        (0..n)
            .map(|i| StoredNode {
                id: format!("n{i}"),
                payload: serde_json::json!({"i": i}),
            })
            .collect()
    }

    #[tokio::test]
    async fn test_restore_from_hash() {
        let (restorer, bridge) = make_restorer();
        let original = nodes(5);
        let manifest = bridge.snapshot(original.clone(), None).await.unwrap();
        let hash = bridge.store_manifest(&manifest).await.unwrap();

        let restored = restorer.restore_from_hash(&hash).await.unwrap();
        assert_eq!(restored.len(), 5);
        for (o, r) in original.iter().zip(restored.iter()) {
            assert_eq!(o.id, r.id);
        }
    }

    #[tokio::test]
    async fn test_restore_from_manifest() {
        let (restorer, bridge) = make_restorer();
        let original = nodes(3);
        let manifest = bridge.snapshot(original.clone(), None).await.unwrap();

        let restored = restorer.restore_from_manifest(&manifest).await.unwrap();
        assert_eq!(restored.len(), 3);
    }

    #[tokio::test]
    async fn test_restore_missing_manifest_returns_error() {
        let (restorer, _bridge) = make_restorer();
        let result = restorer
            .restore_from_hash("0000000000000000000000000000000000000000000000000000000000000000")
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("manifest not found"));
    }

    #[tokio::test]
    async fn test_restore_empty_snapshot() {
        let (restorer, bridge) = make_restorer();
        let manifest = bridge.snapshot(vec![], None).await.unwrap();
        let restored = restorer.restore_from_manifest(&manifest).await.unwrap();
        assert!(restored.is_empty());
    }
}
