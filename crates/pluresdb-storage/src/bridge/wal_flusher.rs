//! WAL-to-object-store flusher for the hybrid storage backend.
//!
//! [`WalFlusher`] batches [`WalEntry`] records, writes each batch to the
//! blob store as an append-only log chunk, and keeps a compact index of all
//! flushed chunk references so that consumers can replay flushed WAL data.

use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use tracing::{debug, info, instrument, warn};

use super::{ChunkRef, ObjectBridge};
use crate::wal::WalEntry;

// ---------------------------------------------------------------------------
// WalFlusherConfig
// ---------------------------------------------------------------------------

/// Configuration for the [`WalFlusher`].
#[derive(Debug, Clone)]
pub struct WalFlusherConfig {
    /// Maximum number of entries to accumulate before an automatic flush.
    /// A manual call to [`WalFlusher::flush`] always flushes regardless of
    /// batch size.
    pub batch_size: usize,
}

impl Default for WalFlusherConfig {
    fn default() -> Self {
        Self { batch_size: 256 }
    }
}

// ---------------------------------------------------------------------------
// WalFlusher
// ---------------------------------------------------------------------------

/// Batches [`WalEntry`] records and flushes them to the object store.
///
/// Each flush writes a single chunk containing a JSON-serialised
/// `Vec<WalEntry>`.  The returned [`ChunkRef`] is appended to an internal
/// log so callers can later reconstruct the full WAL stream from the object
/// store.
pub struct WalFlusher {
    bridge: Arc<dyn ObjectBridge>,
    config: WalFlusherConfig,
    pending: Mutex<Vec<WalEntry>>,
    /// Ordered list of chunk refs produced by each flush.
    flushed: Mutex<Vec<ChunkRef>>,
}

impl std::fmt::Debug for WalFlusher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalFlusher")
            .field("config", &self.config)
            .field("pending_count", &self.pending.lock().len())
            .field("flushed_count", &self.flushed.lock().len())
            .finish_non_exhaustive()
    }
}

impl WalFlusher {
    /// Create a new flusher backed by `bridge`.
    pub fn new(bridge: Arc<dyn ObjectBridge>, config: WalFlusherConfig) -> Self {
        Self {
            bridge,
            config,
            pending: Mutex::new(Vec::new()),
            flushed: Mutex::new(Vec::new()),
        }
    }

    /// Add a [`WalEntry`] to the pending buffer.
    ///
    /// If the buffer reaches `batch_size` entries, an automatic flush is
    /// triggered.
    #[instrument(skip(self, entry), fields(seq = entry.seq))]
    pub async fn push(&self, entry: WalEntry) -> Result<Option<ChunkRef>> {
        let should_flush = {
            let mut pending = self.pending.lock();
            pending.push(entry);
            pending.len() >= self.config.batch_size
        };

        if should_flush {
            debug!("auto-flushing WAL batch (batch_size reached)");
            let chunk = self.flush().await?;
            return Ok(Some(chunk));
        }

        Ok(None)
    }

    /// Add multiple [`WalEntry`] records to the pending buffer.
    ///
    /// Entries are appended in order; an automatic flush is triggered if the
    /// buffer reaches `batch_size`.
    pub async fn push_batch(&self, entries: Vec<WalEntry>) -> Result<Vec<ChunkRef>> {
        let mut chunks = Vec::new();
        for entry in entries {
            if let Some(chunk) = self.push(entry).await? {
                chunks.push(chunk);
            }
        }
        Ok(chunks)
    }

    /// Flush all pending entries to the object store immediately, even if the
    /// batch is not yet full.
    ///
    /// Returns a [`ChunkRef`] for the written chunk.  If the pending buffer is
    /// empty, a `ChunkRef` with an empty hash and zero size is returned without
    /// writing any chunk.
    #[instrument(skip(self))]
    pub async fn flush(&self) -> Result<ChunkRef> {
        let entries = {
            let mut pending = self.pending.lock();
            std::mem::take(&mut *pending)
        };

        if entries.is_empty() {
            debug!("flush called with empty pending buffer — nothing to write");
            return Ok(ChunkRef {
                hash: String::new(),
                size: 0,
            });
        }

        let entry_count = entries.len();
        let chunk = self.bridge.flush_wal(entries).await?;

        {
            let mut flushed = self.flushed.lock();
            flushed.push(chunk.clone());
        }

        info!(entry_count, chunk_hash = %chunk.hash, "WAL batch flushed to object store");
        Ok(chunk)
    }

    /// Returns a copy of all chunk refs produced so far, oldest first.
    pub fn flushed_chunks(&self) -> Vec<ChunkRef> {
        self.flushed.lock().clone()
    }

    /// Returns the number of entries currently waiting in the pending buffer.
    pub fn pending_count(&self) -> usize {
        self.pending.lock().len()
    }

    /// Warn if there are pending entries that have not been flushed.
    pub fn warn_if_unflushed(&self) {
        let n = self.pending_count();
        if n > 0 {
            warn!(pending = n, "WalFlusher dropped with unflushed entries");
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
    use crate::bridge::BlobObjectBridge;
    use crate::wal::{WalEntry, WalOperation};

    fn make_flusher(batch_size: usize) -> WalFlusher {
        let bridge = Arc::new(BlobObjectBridge::new(Arc::new(MemoryBlobStore::default())));
        WalFlusher::new(bridge, WalFlusherConfig { batch_size })
    }

    fn put_entry(seq: u64) -> WalEntry {
        WalEntry::new(
            seq,
            "actor".to_string(),
            WalOperation::Put {
                id: format!("n{seq}"),
                data: serde_json::json!({"seq": seq}),
            },
        )
    }

    #[tokio::test]
    async fn test_manual_flush() {
        let flusher = make_flusher(10);
        flusher.push(put_entry(1)).await.unwrap();
        flusher.push(put_entry(2)).await.unwrap();
        assert_eq!(flusher.pending_count(), 2);

        let chunk = flusher.flush().await.unwrap();
        assert_eq!(chunk.hash.len(), 64);
        assert_eq!(flusher.pending_count(), 0);
        assert_eq!(flusher.flushed_chunks().len(), 1);
    }

    #[tokio::test]
    async fn test_auto_flush_on_batch_size() {
        let flusher = make_flusher(3);
        for seq in 1..=3 {
            flusher.push(put_entry(seq)).await.unwrap();
        }
        // Third push should have triggered auto-flush.
        assert_eq!(flusher.pending_count(), 0);
        assert_eq!(flusher.flushed_chunks().len(), 1);
    }

    #[tokio::test]
    async fn test_flush_empty_returns_empty_chunk() {
        let flusher = make_flusher(10);
        let chunk = flusher.flush().await.unwrap();
        assert!(chunk.hash.is_empty());
        assert_eq!(chunk.size, 0);
        assert!(flusher.flushed_chunks().is_empty());
    }

    #[tokio::test]
    async fn test_push_batch() {
        let flusher = make_flusher(2);
        let entries: Vec<_> = (1..=5).map(put_entry).collect();
        let chunks = flusher.push_batch(entries).await.unwrap();
        // 5 entries with batch_size=2 → auto-flush at 2 and 4, leaving 1 pending
        assert_eq!(chunks.len(), 2);
        assert_eq!(flusher.pending_count(), 1);
    }

    #[tokio::test]
    async fn test_flushed_chunks_accumulate() {
        let flusher = make_flusher(10);
        for i in 0..3 {
            flusher.push(put_entry(i)).await.unwrap();
            flusher.flush().await.unwrap();
        }
        assert_eq!(flusher.flushed_chunks().len(), 3);
    }
}
