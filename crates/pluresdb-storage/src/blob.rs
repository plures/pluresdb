//! Content-addressed blob storage for PluresDB.
//!
//! Blobs are stored and retrieved by their **SHA-256 content hash** (hex
//! encoded, lowercase).  This design is intentionally similar to Git's object
//! store and Hypercore's `RandomAccessStorage`, enabling straightforward
//! git-repo replication and large-file sharing in the P2P layer.
//!
//! ## Implementations
//!
//! | Type               | Backing store          | Use case                        |
//! |--------------------|------------------------|---------------------------------|
//! | [`MemoryBlobStore`] | In-memory HashMap      | Tests, ephemeral blobs          |
//! | [`FileBlobStore`]   | Local filesystem (CAS) | Durable node / repo object store|

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use parking_lot::RwLock;

// ---------------------------------------------------------------------------
// BlobStore trait
// ---------------------------------------------------------------------------

/// Content-addressed blob storage.
///
/// All operations are keyed by the SHA-256 hex digest of the stored data.
/// Use [`BlobStore::put`] to store a blob and obtain its hash; use
/// [`BlobStore::get`] to retrieve it by hash.
pub trait BlobStore: Send + Sync {
    /// Store `data` and return its lowercase hex SHA-256 digest.
    ///
    /// Storing the same content twice is idempotent — the same hash is
    /// returned and the data is not duplicated.
    fn put(&self, data: &[u8]) -> Result<String>;

    /// Retrieve the blob identified by its SHA-256 hex digest.
    ///
    /// Returns `Ok(None)` when the hash is not found.
    fn get(&self, hash: &str) -> Result<Option<Vec<u8>>>;

    /// Delete a blob by its SHA-256 hex digest.
    ///
    /// No-op if the blob does not exist.
    fn delete(&self, hash: &str) -> Result<()>;

    /// Check whether a blob exists without fetching its content.
    fn exists(&self, hash: &str) -> Result<bool>;
}

// ---------------------------------------------------------------------------
// SHA-256 helper
// ---------------------------------------------------------------------------

/// Compute the lowercase hex SHA-256 digest of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

// ---------------------------------------------------------------------------
// MemoryBlobStore
// ---------------------------------------------------------------------------

/// In-process content-addressed blob store backed by a `HashMap`.
///
/// Suitable for tests and ephemeral workloads.  All data is lost when the
/// store is dropped.
#[derive(Debug, Default, Clone)]
pub struct MemoryBlobStore {
    inner: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl BlobStore for MemoryBlobStore {
    fn put(&self, data: &[u8]) -> Result<String> {
        let hash = sha256_hex(data);
        self.inner
            .write()
            .entry(hash.clone())
            .or_insert_with(|| data.to_vec());
        Ok(hash)
    }

    fn get(&self, hash: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.inner.read().get(hash).cloned())
    }

    fn delete(&self, hash: &str) -> Result<()> {
        self.inner.write().remove(hash);
        Ok(())
    }

    fn exists(&self, hash: &str) -> Result<bool> {
        Ok(self.inner.read().contains_key(hash))
    }
}

// ---------------------------------------------------------------------------
// FileBlobStore
// ---------------------------------------------------------------------------

/// Filesystem-backed content-addressed blob store.
///
/// Blobs are stored in a two-level fan-out directory layout matching Git's
/// loose object format:
///
/// ```text
/// <base_path>/
///   ab/
///     cdef0123...  (the remaining 62 hex chars)
///   ...
/// ```
///
/// This limits the number of entries per directory to 256 (00–ff), avoiding
/// filesystem performance issues with large flat directories.
#[derive(Debug, Clone)]
pub struct FileBlobStore {
    base_path: PathBuf,
}

impl FileBlobStore {
    /// Open (or create) a [`FileBlobStore`] rooted at `base_path`.
    pub fn open(base_path: impl AsRef<Path>) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        fs::create_dir_all(&base_path)
            .with_context(|| format!("create blob store directory: {}", base_path.display()))?;
        Ok(Self { base_path })
    }

    /// Compute the filesystem path for a given hex hash.
    fn blob_path(&self, hash: &str) -> PathBuf {
        if hash.len() < 4 {
            // Fall back to a single-level path for unusually short hashes
            // (SHA-256 is always 64 chars; this guard is a safety net only).
            return self.base_path.join(hash);
        }
        let (prefix, suffix) = hash.split_at(2);
        self.base_path.join(prefix).join(suffix)
    }
}

impl BlobStore for FileBlobStore {
    fn put(&self, data: &[u8]) -> Result<String> {
        let hash = sha256_hex(data);
        let path = self.blob_path(&hash);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("create blob directory: {}", parent.display())
                })?;
            }
            fs::write(&path, data)
                .with_context(|| format!("write blob {}", path.display()))?;
        }
        Ok(hash)
    }

    fn get(&self, hash: &str) -> Result<Option<Vec<u8>>> {
        let path = self.blob_path(hash);
        if path.exists() {
            let data = fs::read(&path)
                .with_context(|| format!("read blob {}", path.display()))?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    fn delete(&self, hash: &str) -> Result<()> {
        let path = self.blob_path(hash);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("delete blob {}", path.display()))?;
        }
        Ok(())
    }

    fn exists(&self, hash: &str) -> Result<bool> {
        Ok(self.blob_path(hash).exists())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn round_trip_suite(store: &dyn BlobStore) {
        // put → get
        let data = b"hello, blob store!";
        let hash = store.put(data).unwrap();
        assert_eq!(hash.len(), 64, "SHA-256 hex is 64 chars");
        assert!(store.exists(&hash).unwrap());
        let retrieved = store.get(&hash).unwrap().unwrap();
        assert_eq!(retrieved, data);

        // idempotent put
        let hash2 = store.put(data).unwrap();
        assert_eq!(hash, hash2);

        // delete
        store.delete(&hash).unwrap();
        assert!(!store.exists(&hash).unwrap());
        assert!(store.get(&hash).unwrap().is_none());
    }

    #[test]
    fn test_memory_blob_store_round_trip() {
        let store = MemoryBlobStore::default();
        round_trip_suite(&store);
    }

    #[test]
    fn test_file_blob_store_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = FileBlobStore::open(dir.path()).unwrap();
        round_trip_suite(&store);
    }

    #[test]
    fn test_sha256_hex_known_value() {
        // echo -n "" | sha256sum → e3b0c44298fc1c14...
        let empty_hash = sha256_hex(b"");
        assert_eq!(
            empty_hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_different_content_different_hash() {
        let store = MemoryBlobStore::default();
        let h1 = store.put(b"foo").unwrap();
        let h2 = store.put(b"bar").unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_file_blob_store_two_level_layout() {
        let dir = TempDir::new().unwrap();
        let store = FileBlobStore::open(dir.path()).unwrap();
        let data = b"two-level test";
        let hash = store.put(data).unwrap();

        // Verify the file exists at <base>/<hash[0..2]>/<hash[2..]>
        let expected_path = dir.path().join(&hash[..2]).join(&hash[2..]);
        assert!(expected_path.exists());
    }

    #[test]
    fn test_delete_nonexistent_is_noop() {
        let store = MemoryBlobStore::default();
        // Should not error.
        store.delete("0000000000000000000000000000000000000000000000000000000000000000").unwrap();
    }
}
