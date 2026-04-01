//! Periodic snapshot manager for the hybrid storage backend.
//!
//! [`SnapshotManager`] serialises the current graph state into content-
//! addressed chunks via an [`ObjectBridge`] and keeps a bounded history of
//! manifests for time-travel queries and rollback.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use parking_lot::RwLock;
use tracing::{info, instrument, warn};

use super::{Manifest, ObjectBridge};
use crate::StoredNode;

// ---------------------------------------------------------------------------
// SnapshotConfig
// ---------------------------------------------------------------------------

/// Configuration for the [`SnapshotManager`].
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// How often to take automatic snapshots.  `None` disables periodic
    /// snapshotting (manual snapshots via [`SnapshotManager::snapshot`] still
    /// work).
    pub interval: Option<Duration>,

    /// Maximum number of manifests to retain in the history ring-buffer.
    /// Older manifests are evicted when the history is full, but their chunks
    /// remain in the blob store for as long as the store retains them.
    pub max_history: usize,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            interval: Some(Duration::from_secs(3600)), // 1 hour
            max_history: 24,
        }
    }
}

// ---------------------------------------------------------------------------
// SnapshotManager
// ---------------------------------------------------------------------------

/// Manages periodic and on-demand snapshots of the graph database.
///
/// Each snapshot serialises the complete node list into content-addressed
/// chunks via the [`ObjectBridge`], then stores the resulting [`Manifest`].
/// Up to `config.max_history` manifests are kept in a bounded ring-buffer so
/// callers can roll back or query historical states.
pub struct SnapshotManager {
    bridge: Arc<dyn ObjectBridge>,
    config: SnapshotConfig,
    /// Ring-buffer of manifest records, oldest first.
    history: RwLock<Vec<ManifestRecord>>,
}

impl std::fmt::Debug for SnapshotManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SnapshotManager")
            .field("config", &self.config)
            .field("history_len", &self.history.read().len())
            .finish_non_exhaustive()
    }
}

/// A single record in the snapshot history.
#[derive(Debug, Clone)]
pub struct ManifestRecord {
    /// The manifest itself.
    pub manifest: Manifest,
    /// SHA-256 hash under which the manifest is stored in the blob store.
    pub hash: String,
}

impl SnapshotManager {
    /// Create a new manager using `bridge` for storage and `config` for
    /// policy (interval, retention).
    pub fn new(bridge: Arc<dyn ObjectBridge>, config: SnapshotConfig) -> Self {
        Self {
            bridge,
            config,
            history: RwLock::new(Vec::new()),
        }
    }

    /// Take an immediate snapshot of `nodes`.
    ///
    /// The resulting manifest is persisted to the blob store and appended to
    /// the history ring-buffer.  If the history exceeds `max_history`, the
    /// oldest entry is evicted.
    ///
    /// Returns the [`ManifestRecord`] for the new snapshot.
    #[instrument(skip(self, nodes), fields(node_count = nodes.len()))]
    pub async fn snapshot(
        &self,
        nodes: Vec<StoredNode>,
        label: Option<String>,
    ) -> Result<ManifestRecord> {
        let node_count = nodes.len();
        info!(node_count, label = ?label, "taking snapshot");

        let manifest = self.bridge.snapshot(nodes, label).await?;
        let hash = self.bridge.store_manifest(&manifest).await?;

        let record = ManifestRecord { manifest, hash };

        // Append to history, evicting oldest if full.
        {
            let mut history = self.history.write();
            history.push(record.clone());
            if history.len() > self.config.max_history {
                let evicted = history.remove(0);
                warn!(
                    evicted_id = %evicted.manifest.id,
                    "snapshot history full — evicted oldest manifest"
                );
            }
        }

        info!(
            manifest_id = %record.manifest.id,
            hash = %record.hash,
            node_count,
            "snapshot complete"
        );

        Ok(record)
    }

    /// Returns a copy of the current history, oldest first.
    pub fn history(&self) -> Vec<ManifestRecord> {
        self.history.read().clone()
    }

    /// Returns the most recent [`ManifestRecord`], or `None` if no snapshot
    /// has been taken yet.
    pub fn latest(&self) -> Option<ManifestRecord> {
        self.history.read().last().cloned()
    }

    /// Restore graph state from the most recent snapshot.
    ///
    /// Returns `Ok(None)` if no snapshot has been taken yet.
    pub async fn restore_latest(&self) -> Result<Option<Vec<StoredNode>>> {
        let record = match self.latest() {
            Some(r) => r,
            None => return Ok(None),
        };
        let nodes = self.bridge.restore(&record.manifest).await?;
        Ok(Some(nodes))
    }

    /// Restore graph state from the snapshot identified by `manifest_hash`.
    pub async fn restore_from_hash(&self, manifest_hash: &str) -> Result<Vec<StoredNode>> {
        let manifest = self
            .bridge
            .load_manifest(manifest_hash)
            .await?
            .ok_or_else(|| anyhow::anyhow!("manifest not found: {}", manifest_hash))?;
        self.bridge.restore(&manifest).await
    }

    /// Configuration used by this manager.
    pub fn config(&self) -> &SnapshotConfig {
        &self.config
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

    fn make_manager(max_history: usize) -> SnapshotManager {
        let bridge = Arc::new(BlobObjectBridge::new(Arc::new(MemoryBlobStore::default())));
        SnapshotManager::new(
            bridge,
            SnapshotConfig {
                interval: None,
                max_history,
            },
        )
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
    async fn test_snapshot_and_restore_latest() {
        let mgr = make_manager(10);
        assert!(mgr.latest().is_none());

        mgr.snapshot(nodes(4), Some("first".to_string()))
            .await
            .unwrap();
        let record = mgr.latest().unwrap();
        assert_eq!(record.manifest.node_count, 4);
        assert_eq!(record.manifest.label.as_deref(), Some("first"));

        let restored = mgr.restore_latest().await.unwrap().unwrap();
        assert_eq!(restored.len(), 4);
    }

    #[tokio::test]
    async fn test_history_retention() {
        let mgr = make_manager(3);

        for i in 0..5 {
            mgr.snapshot(nodes(i), None).await.unwrap();
        }

        // History must never exceed max_history.
        let h = mgr.history();
        assert_eq!(h.len(), 3);
        // Latest snapshot had 4 nodes (i=4).
        assert_eq!(mgr.latest().unwrap().manifest.node_count, 4);
    }

    #[tokio::test]
    async fn test_restore_from_hash() {
        let mgr = make_manager(5);
        let record = mgr.snapshot(nodes(3), None).await.unwrap();
        let restored = mgr.restore_from_hash(&record.hash).await.unwrap();
        assert_eq!(restored.len(), 3);
    }

    #[tokio::test]
    async fn test_restore_latest_no_snapshot() {
        let mgr = make_manager(5);
        assert!(mgr.restore_latest().await.unwrap().is_none());
    }
}
