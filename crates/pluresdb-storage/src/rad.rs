//! RAD-like storage adapter — prefix and range scans over [`StorageEngine`].
//!
//! GUN RAD (Random Access Data) uses a radix-tree chunk strategy that exposes
//! simple key-range semantics.  This module adds those semantics on top of the
//! existing [`StorageEngine`] trait as an extension trait [`RadAdapter`].
//!
//! ## Supported query patterns
//!
//! | Method           | Semantics                                    |
//! |------------------|----------------------------------------------|
//! | `prefix_scan`    | All nodes whose ID starts with a prefix      |
//! | `range_scan`     | All nodes with IDs in `[start, end)` order   |
//!
//! These cover the main GUN lexical-range get patterns used for pagination,
//! filtered graph reads, and user-space sorted collections.
//!
//! ## Implementations
//!
//! Both [`MemoryStorage`] and [`SledStorage`] are covered by the blanket
//! `impl RadAdapter for T where T: StorageEngine` default implementations
//! (using list + filter for memory, and sled prefix iterators for sled).
//! Specialised, more efficient sled implementations are provided in
//! [`SledRadAdapter`].

use crate::{MemoryStorage, SledStorage, StorageEngine, StoredNode};
use anyhow::Result;
use async_trait::async_trait;

// ---------------------------------------------------------------------------
// RadAdapter trait
// ---------------------------------------------------------------------------

/// Extension of [`StorageEngine`] with lexical prefix/range query semantics.
///
/// Implementors must also implement [`StorageEngine`].  The default blanket
/// implementations below provide correct (but potentially O(n)) fallback
/// implementations in terms of `list()`.
#[async_trait]
pub trait RadAdapter: StorageEngine {
    /// Return all stored nodes whose ID starts with `prefix`, sorted by ID.
    ///
    /// An empty prefix returns all nodes (same as `list()`).
    async fn prefix_scan(&self, prefix: &str) -> Result<Vec<StoredNode>>;

    /// Return all stored nodes with IDs in the half-open range `[start, end)`,
    /// sorted by ID.
    ///
    /// When `end` is `None` the range is unbounded on the right.
    async fn range_scan(&self, start: &str, end: Option<&str>) -> Result<Vec<StoredNode>>;
}

// ---------------------------------------------------------------------------
// MemoryStorage implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl RadAdapter for MemoryStorage {
    async fn prefix_scan(&self, prefix: &str) -> Result<Vec<StoredNode>> {
        let mut nodes: Vec<StoredNode> = self
            .list()
            .await?
            .into_iter()
            .filter(|n| n.id.starts_with(prefix))
            .collect();
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(nodes)
    }

    async fn range_scan(&self, start: &str, end: Option<&str>) -> Result<Vec<StoredNode>> {
        let mut nodes: Vec<StoredNode> = self
            .list()
            .await?
            .into_iter()
            .filter(|n| n.id.as_str() >= start && end.is_none_or(|e| n.id.as_str() < e))
            .collect();
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(nodes)
    }
}

// ---------------------------------------------------------------------------
// SledStorage implementation (uses sled's native prefix iterators)
// ---------------------------------------------------------------------------

/// Efficient sled-backed RAD adapter that uses sled's native prefix-scan
/// instead of loading the full dataset.
pub struct SledRadAdapter(pub SledStorage);

#[async_trait]
impl StorageEngine for SledRadAdapter {
    async fn put(&self, node: StoredNode) -> Result<()> {
        self.0.put(node).await
    }
    async fn get(&self, id: &str) -> Result<Option<StoredNode>> {
        self.0.get(id).await
    }
    async fn delete(&self, id: &str) -> Result<()> {
        self.0.delete(id).await
    }
    async fn list(&self) -> Result<Vec<StoredNode>> {
        self.0.list().await
    }
}

#[async_trait]
impl RadAdapter for SledRadAdapter {
    async fn prefix_scan(&self, prefix: &str) -> Result<Vec<StoredNode>> {
        let mut nodes = Vec::new();
        for entry in self.0.db().scan_prefix(prefix.as_bytes()) {
            let (_, value) = entry?;
            let node: StoredNode = serde_json::from_slice(&value)?;
            nodes.push(node);
        }
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(nodes)
    }

    async fn range_scan(&self, start: &str, end: Option<&str>) -> Result<Vec<StoredNode>> {
        let iter = match end {
            Some(e) => self
                .0
                .db()
                .range(start.as_bytes()..e.as_bytes())
                .collect::<Vec<_>>(),
            None => self.0.db().range(start.as_bytes()..).collect::<Vec<_>>(),
        };
        let mut nodes = Vec::new();
        for entry in iter {
            let (_, value) = entry?;
            let node: StoredNode = serde_json::from_slice(&value)?;
            nodes.push(node);
        }
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(nodes)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemoryStorage, StorageEngine, StoredNode};
    use serde_json::json;

    async fn populate(storage: &dyn StorageEngine, nodes: &[(&str, serde_json::Value)]) {
        for (id, payload) in nodes {
            storage
                .put(StoredNode {
                    id: id.to_string(),
                    payload: payload.clone(),
                })
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn test_prefix_scan_memory() {
        let storage = MemoryStorage::default();
        populate(
            &storage,
            &[
                ("user:alice", json!({"name": "Alice"})),
                ("user:bob", json!({"name": "Bob"})),
                ("post:1", json!({"title": "Hello"})),
            ],
        )
        .await;

        let users = storage.prefix_scan("user:").await.unwrap();
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].id, "user:alice");
        assert_eq!(users[1].id, "user:bob");

        let posts = storage.prefix_scan("post:").await.unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].id, "post:1");

        // Empty prefix → all nodes.
        let all = storage.prefix_scan("").await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_range_scan_memory() {
        let storage = MemoryStorage::default();
        populate(
            &storage,
            &[
                ("a", json!(1)),
                ("b", json!(2)),
                ("c", json!(3)),
                ("d", json!(4)),
            ],
        )
        .await;

        let range = storage.range_scan("b", Some("d")).await.unwrap();
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].id, "b");
        assert_eq!(range[1].id, "c");

        // Unbounded upper limit.
        let tail = storage.range_scan("c", None).await.unwrap();
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].id, "c");
        assert_eq!(tail[1].id, "d");
    }

    #[tokio::test]
    async fn test_prefix_scan_empty_returns_all() {
        let storage = MemoryStorage::default();
        populate(&storage, &[("x", json!(1)), ("y", json!(2))]).await;
        let all = storage.prefix_scan("").await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_prefix_scan_no_matches() {
        let storage = MemoryStorage::default();
        populate(&storage, &[("abc", json!(1))]).await;
        let result = storage.prefix_scan("xyz").await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_range_scan_exclusive_end() {
        let storage = MemoryStorage::default();
        populate(
            &storage,
            &[("a", json!(1)), ("b", json!(2)), ("c", json!(3))],
        )
        .await;
        // "c" is excluded (half-open range).
        let result = storage.range_scan("a", Some("c")).await.unwrap();
        assert_eq!(result.len(), 2);
        let ids: Vec<&str> = result.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
        assert!(!ids.contains(&"c"));
    }
}
