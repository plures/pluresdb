//! Write-Ahead Log (WAL) implementation for durability guarantees.
//!
//! This module provides crash-safe, sequential logging of all CRDT operations
//! to ensure no accepted write is lost. The WAL can be replayed deterministically
//! to reconstruct database state after a crash.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{debug, info, warn, instrument};

/// Durability level for write operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityLevel {
    /// No fsync - fastest, least durable (for testing only)
    None,
    
    /// Fsync WAL only - balanced (default)
    Wal,
    
    /// Fsync WAL and data - slowest, most durable
    Full,
}

impl Default for DurabilityLevel {
    fn default() -> Self {
        Self::Wal
    }
}

/// A single entry in the write-ahead log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalEntry {
    /// Sequence number for ordering
    pub seq: u64,
    
    /// Timestamp when operation was logged
    pub timestamp: i64,
    
    /// Actor that performed the operation
    pub actor: String,
    
    /// The operation payload
    pub operation: WalOperation,
    
    /// CRC32 checksum of the entry
    pub checksum: u32,
}

impl WalEntry {
    /// Creates a new WAL entry with the given sequence number and operation.
    pub fn new(seq: u64, actor: String, operation: WalOperation) -> Self {
        let timestamp = chrono::Utc::now().timestamp();
        let mut entry = Self {
            seq,
            timestamp,
            actor,
            operation,
            checksum: 0,
        };
        entry.checksum = entry.compute_checksum();
        entry
    }
    
    /// Computes the CRC32 checksum of this entry (excluding the checksum field).
    fn compute_checksum(&self) -> u32 {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&self.seq.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.update(self.actor.as_bytes());
        
        // Serialize operation for checksumming
        if let Ok(bytes) = serde_json::to_vec(&self.operation) {
            hasher.update(&bytes);
        }
        
        hasher.finalize()
    }
    
    /// Validates the checksum of this entry.
    pub fn validate_checksum(&self) -> bool {
        let expected = self.compute_checksum();
        self.checksum == expected
    }
}

/// Operations that can be logged in the WAL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WalOperation {
    /// Put/update a node
    Put {
        id: String,
        data: serde_json::Value,
    },
    
    /// Delete a node (creates tombstone)
    Delete {
        id: String,
    },
    
    /// Compact/prune tombstones older than timestamp
    Compact {
        before_timestamp: i64,
    },
    
    /// Checkpoint marker (all operations before this are in base data)
    Checkpoint {
        base_seq: u64,
    },
}

/// Write-Ahead Log for durable operation logging.
#[derive(Debug)]
pub struct WriteAheadLog {
    /// Directory where WAL segments are stored
    dir: PathBuf,
    
    /// Current active segment file
    current_segment: Arc<Mutex<Option<WalSegment>>>,
    
    /// Next sequence number to assign
    next_seq: AtomicU64,
    
    /// Durability level
    durability: DurabilityLevel,
    
    /// Maximum segment size in bytes (default: 64MB)
    max_segment_size: u64,
}

impl WriteAheadLog {
    /// Opens or creates a write-ahead log at the specified directory.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_options(dir, DurabilityLevel::default(), 64 * 1024 * 1024)
    }
    
    /// Opens or creates a WAL with custom options.
    pub fn open_with_options(
        dir: impl AsRef<Path>,
        durability: DurabilityLevel,
        max_segment_size: u64,
    ) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create WAL directory: {}", dir.display()))?;
        
        info!(?dir, ?durability, "opening write-ahead log");
        
        // Find highest sequence number from existing segments
        let next_seq = Self::scan_max_sequence(&dir)?;
        
        Ok(Self {
            dir,
            current_segment: Arc::new(Mutex::new(None)),
            next_seq: AtomicU64::new(next_seq),
            durability,
            max_segment_size,
        })
    }
    
    /// Appends an operation to the WAL.
    #[instrument(skip(self, operation))]
    pub async fn append(&self, actor: String, operation: WalOperation) -> Result<u64> {
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let entry = WalEntry::new(seq, actor, operation);
        
        let mut guard = self.current_segment.lock().await;
        
        // Create new segment if needed
        if guard.is_none() || Self::should_rotate(&guard, self.max_segment_size)? {
            let segment = WalSegment::create(&self.dir, seq)?;
            *guard = Some(segment);
        }
        
        if let Some(segment) = guard.as_mut() {
            segment.append(&entry)?;
            
            // Fsync based on durability level
            if self.durability != DurabilityLevel::None {
                segment.fsync()?;
            }
        }
        
        Ok(seq)
    }
    
    /// Reads all entries from the WAL in sequence order.
    pub async fn read_all(&self) -> Result<Vec<WalEntry>> {
        // First, ensure current segment is flushed
        {
            let mut guard = self.current_segment.lock().await;
            if let Some(segment) = guard.as_mut() {
                segment.fsync()?;
            }
        }
        
        let mut entries = Vec::new();
        
        for segment_path in self.list_segments()? {
            match WalSegment::open_read(&segment_path) {
                Ok(segment) => {
                    match segment.read_all() {
                        Ok(segment_entries) => entries.extend(segment_entries),
                        Err(e) => {
                            warn!(?segment_path, error = ?e, "failed to read WAL segment, skipping");
                        }
                    }
                }
                Err(e) => {
                    warn!(?segment_path, error = ?e, "failed to open WAL segment, skipping");
                }
            }
        }
        
        // Sort by sequence number to ensure proper ordering
        entries.sort_by_key(|e| e.seq);
        
        Ok(entries)
    }
    
    /// Validates all entries and returns statistics about corruption.
    pub async fn validate(&self) -> Result<WalValidation> {
        // First, ensure current segment is flushed
        {
            let mut guard = self.current_segment.lock().await;
            if let Some(segment) = guard.as_mut() {
                segment.fsync()?;
            }
        }
        
        let mut stats = WalValidation::default();
        
        for segment_path in self.list_segments()? {
            match WalSegment::open_read(&segment_path) {
                Ok(segment) => {
                    match segment.read_all() {
                        Ok(entries) => {
                            for entry in entries {
                                stats.total_entries += 1;
                                if entry.validate_checksum() {
                                    stats.valid_entries += 1;
                                } else {
                                    stats.corrupted_entries += 1;
                                    warn!(seq = entry.seq, "corrupted WAL entry detected");
                                }
                            }
                        }
                        Err(e) => {
                            stats.corrupted_segments += 1;
                            warn!(?segment_path, error = ?e, "corrupted WAL segment");
                        }
                    }
                }
                Err(e) => {
                    stats.corrupted_segments += 1;
                    warn!(?segment_path, error = ?e, "failed to open WAL segment");
                }
            }
            stats.total_segments += 1;
        }
        
        Ok(stats)
    }
    
    /// Compacts the WAL by removing entries before the checkpoint.
    pub async fn compact(&self, checkpoint_seq: u64) -> Result<()> {
        info!(checkpoint_seq, "compacting WAL");
        
        for segment_path in self.list_segments()? {
            // Check if this segment only contains entries before checkpoint
            if let Ok(segment) = WalSegment::open_read(&segment_path) {
                if let Ok(entries) = segment.read_all() {
                    if entries.iter().all(|e| e.seq < checkpoint_seq) {
                        // All entries in this segment are before checkpoint, safe to delete
                        debug!(?segment_path, "removing compacted WAL segment");
                        fs::remove_file(&segment_path)?;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Lists all segment files in chronological order.
    fn list_segments(&self) -> Result<Vec<PathBuf>> {
        let mut segments = Vec::new();
        
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("wal") {
                segments.push(path);
            }
        }
        
        segments.sort();
        Ok(segments)
    }
    
    /// Scans existing segments to find the highest sequence number.
    fn scan_max_sequence(dir: &Path) -> Result<u64> {
        let mut max_seq = 0u64;
        
        if dir.exists() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.extension().and_then(|s| s.to_str()) == Some("wal") {
                    if let Ok(segment) = WalSegment::open_read(&path) {
                        if let Ok(entries) = segment.read_all() {
                            for entry in entries {
                                max_seq = max_seq.max(entry.seq);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(max_seq + 1)
    }
    
    /// Checks if the current segment should be rotated.
    fn should_rotate(guard: &Option<WalSegment>, max_size: u64) -> Result<bool> {
        if let Some(segment) = guard {
            Ok(segment.size()? >= max_size)
        } else {
            Ok(false)
        }
    }
}

/// Statistics from WAL validation.
#[derive(Debug, Default, Clone)]
pub struct WalValidation {
    pub total_entries: u64,
    pub valid_entries: u64,
    pub corrupted_entries: u64,
    pub total_segments: u64,
    pub corrupted_segments: u64,
}

impl WalValidation {
    /// Returns true if all entries are valid.
    pub fn is_healthy(&self) -> bool {
        self.corrupted_entries == 0 && self.corrupted_segments == 0
    }
    
    /// Returns the percentage of corrupted entries.
    pub fn corruption_rate(&self) -> f64 {
        if self.total_entries == 0 {
            0.0
        } else {
            (self.corrupted_entries as f64) / (self.total_entries as f64)
        }
    }
}

/// A single WAL segment file.
#[derive(Debug)]
struct WalSegment {
    path: PathBuf,
    file: File,
}

impl WalSegment {
    /// Creates a new WAL segment.
    fn create(dir: &Path, start_seq: u64) -> Result<Self> {
        let filename = format!("{:016x}.wal", start_seq);
        let path = dir.join(filename);
        
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("failed to create WAL segment: {}", path.display()))?;
        
        debug!(?path, "created WAL segment");
        
        Ok(Self { path, file })
    }
    
    /// Opens an existing WAL segment for reading.
    fn open_read(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("failed to open WAL segment: {}", path.display()))?;
        
        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }
    
    /// Appends an entry to this segment.
    fn append(&mut self, entry: &WalEntry) -> Result<()> {
        let bytes = serde_json::to_vec(entry)
            .context("failed to serialize WAL entry")?;
        
        // Write length prefix (u32) followed by entry bytes
        let len = bytes.len() as u32;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(&bytes)?;
        
        Ok(())
    }
    
    /// Fsyncs this segment to disk.
    fn fsync(&mut self) -> Result<()> {
        self.file.sync_data()?;
        Ok(())
    }
    
    /// Returns the current size of this segment in bytes.
    fn size(&self) -> Result<u64> {
        Ok(self.file.metadata()?.len())
    }
    
    /// Reads all entries from this segment.
    fn read_all(&self) -> Result<Vec<WalEntry>> {
        // Open a new file handle for reading (the self.file is in append mode)
        let read_file = File::open(&self.path)
            .with_context(|| format!("failed to open WAL segment for reading: {}", self.path.display()))?;
        
        let mut reader = BufReader::new(read_file);
        let mut entries = Vec::new();
        
        loop {
            // Read length prefix
            let mut len_buf = [0u8; 4];
            match reader.read_exact(&mut len_buf) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            }
            
            let len = u32::from_le_bytes(len_buf) as usize;
            
            // Read entry bytes
            let mut entry_buf = vec![0u8; len];
            reader.read_exact(&mut entry_buf)?;
            
            // Deserialize entry
            match serde_json::from_slice::<WalEntry>(&entry_buf) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    warn!(error = ?e, "failed to deserialize WAL entry, skipping");
                    continue;
                }
            }
        }
        
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_wal_append_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
        
        // Append some operations
        let seq1 = wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"name": "test"}),
            },
        ).await.unwrap();
        
        let seq2 = wal.append(
            "actor-1".to_string(),
            WalOperation::Delete {
                id: "node-2".to_string(),
            },
        ).await.unwrap();
        
        assert_eq!(seq2, seq1 + 1);
        
        // Read back entries
        let entries = wal.read_all().await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].seq, seq1);
        assert_eq!(entries[1].seq, seq2);
        
        // Validate checksums
        assert!(entries[0].validate_checksum());
        assert!(entries[1].validate_checksum());
    }
    
    #[tokio::test]
    async fn test_wal_validation() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
        
        // Append operations
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({}),
            },
        ).await.unwrap();
        
        // Validate
        let validation = wal.validate().await.unwrap();
        assert!(validation.is_healthy());
        assert_eq!(validation.total_entries, 1);
        assert_eq!(validation.valid_entries, 1);
        assert_eq!(validation.corrupted_entries, 0);
    }
    
    #[tokio::test]
    async fn test_wal_compaction() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
        
        // Append operations
        let seq1 = wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({}),
            },
        ).await.unwrap();
        
        wal.append(
            "actor-1".to_string(),
            WalOperation::Checkpoint { base_seq: seq1 },
        ).await.unwrap();
        
        let seq3 = wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-2".to_string(),
                data: serde_json::json!({}),
            },
        ).await.unwrap();
        
        // Compact before seq3
        wal.compact(seq3).await.unwrap();
        
        // Should still have at least one entry
        let entries = wal.read_all().await.unwrap();
        assert!(!entries.is_empty());
    }
    
    #[test]
    fn test_wal_entry_checksum() {
        let entry = WalEntry::new(
            1,
            "actor-1".to_string(),
            WalOperation::Put {
                id: "test".to_string(),
                data: serde_json::json!({"key": "value"}),
            },
        );
        
        assert!(entry.validate_checksum());
        
        // Corrupt the entry
        let mut corrupted = entry.clone();
        corrupted.checksum = 0;
        assert!(!corrupted.validate_checksum());
    }
    
    #[tokio::test]
    async fn test_wal_segment_rotation() {
        let temp_dir = TempDir::new().unwrap();
        // Use very small segment size to force rotation
        let wal = WriteAheadLog::open_with_options(
            temp_dir.path(),
            DurabilityLevel::Wal,
            128, // 128 bytes max segment size
        ).unwrap();
        
        // Append enough operations to trigger rotation
        for i in 0..10 {
            wal.append(
                format!("actor-{}", i),
                WalOperation::Put {
                    id: format!("node-{}", i),
                    data: serde_json::json!({"index": i}),
                },
            ).await.unwrap();
        }
        
        // Should have multiple segments
        let segments = wal.list_segments().unwrap();
        assert!(segments.len() > 1, "expected multiple segments due to rotation");
    }
}
