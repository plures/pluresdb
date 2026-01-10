//! Storage abstraction for PluresDB.
//!
//! The real system will eventually support multiple pluggable backends. For the
//! initial native bootstrap we provide a simple in-memory store and a sled-based
//! durable implementation that can run entirely within the application process.

pub mod encryption;
pub mod wal;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sled::IVec;
use tokio::sync::RwLock;
use tracing::{info, instrument};

pub use encryption::{EncryptionConfig, EncryptionMetadata};
pub use wal::{DurabilityLevel, WalEntry, WalOperation, WalValidation, WriteAheadLog};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredNode {
    pub id: String,
    pub payload: serde_json::Value,
}

#[async_trait]
pub trait StorageEngine: Send + Sync {
    async fn put(&self, node: StoredNode) -> Result<()>;
    async fn get(&self, id: &str) -> Result<Option<StoredNode>>;
    async fn delete(&self, id: &str) -> Result<()>;
    async fn list(&self) -> Result<Vec<StoredNode>>;
}

/// A non-persistent storage backend useful for tests and in-memory deployments.
#[derive(Debug, Default, Clone)]
pub struct MemoryStorage {
    inner: Arc<RwLock<HashMap<String, StoredNode>>>,
}

#[async_trait]
impl StorageEngine for MemoryStorage {
    #[instrument(skip(self, node))]
    async fn put(&self, node: StoredNode) -> Result<()> {
        self.inner.write().await.insert(node.id.clone(), node);
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<StoredNode>> {
        Ok(self.inner.read().await.get(id).cloned())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        self.inner.write().await.remove(id);
        Ok(())
    }

    async fn list(&self) -> Result<Vec<StoredNode>> {
        Ok(self.inner.read().await.values().cloned().collect())
    }
}

/// Durable storage based on the sled embedded database.
#[derive(Debug, Clone)]
pub struct SledStorage {
    db: sled::Db,
}

impl SledStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        info!(path = %path.as_ref().display(), "opening sled storage");
        let db = sled::open(path)?;
        Ok(Self { db })
    }

    fn serialize(node: &StoredNode) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(node)?)
    }

    fn deserialize(bytes: IVec) -> Result<StoredNode> {
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[async_trait]
impl StorageEngine for SledStorage {
    async fn put(&self, node: StoredNode) -> Result<()> {
        let bytes = Self::serialize(&node)?;
        self.db.insert(node.id.as_bytes(), bytes)?;
        self.db.flush_async().await?;
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
        self.db.flush_async().await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_storage_round_trip() {
        let storage = MemoryStorage::default();
        let node = StoredNode {
            id: "1".to_string(),
            payload: serde_json::json!({"name": "plures"}),
        };
        storage.put(node.clone()).await.unwrap();
        let fetched = storage.get("1").await.unwrap().unwrap();
        assert_eq!(fetched, node);
        storage.delete("1").await.unwrap();
        assert!(storage.get("1").await.unwrap().is_none());
    }
}
