//! Core data structures, CRDT logic, and domain models that power PluresDB.
//!
//! The goal of this crate is to offer a lightweight, dependency-free-on-FFI
//! foundation that can be reused across the native CLI, the Node addon, and
//! any future host integrations.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use hnsw_rs::prelude::*;
use parking_lot::Mutex;
use rusqlite::types::{Value as SqliteValue, ValueRef};
use rusqlite::{params_from_iter, Connection, OpenFlags, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use thiserror::Error;
use tracing::debug;
use uuid::Uuid;

/// Unique identifier for a stored node.
pub type NodeId = String;

/// Logical actor identifier used when merging CRDT updates.
pub type ActorId = String;

// ---------------------------------------------------------------------------
// Auto-embedding trait
// ---------------------------------------------------------------------------

/// Pluggable text-embedding backend.
///
/// Implement this trait to provide custom embedding logic and attach it to a
/// [`CrdtStore`] via [`CrdtStore::with_embedder`].  The store will then call
/// [`embed`][EmbedText::embed] automatically inside [`CrdtStore::put`]
/// whenever text content can be extracted from the node data.
pub trait EmbedText: Send + Sync + std::fmt::Debug {
    /// Generate embeddings for a batch of text strings.
    ///
    /// The returned `Vec` must have exactly the same length as `texts`.
    fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>>;

    /// Dimensionality of the embeddings produced by this backend.
    fn dimension(&self) -> usize;
}

/// A key-value map of logical clocks per actor.
pub type VectorClock = HashMap<ActorId, u64>;

/// Arbitrary JSON payload that callers persist inside PluresDB.
pub type NodeData = JsonValue;

/// Default embedding dimension (bge-small-en-v1.5).
pub const DEFAULT_EMBEDDING_DIM: usize = 768;

/// A search result from vector similarity search.
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub record: NodeRecord,
    /// Cosine similarity score in \[0, 1\] where 1 = identical direction.
    pub score: f32,
}

/// Metadata associated with a persisted node in the CRDT store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeRecord {
    pub id: NodeId,
    pub data: NodeData,
    pub clock: VectorClock,
    pub timestamp: DateTime<Utc>,
    /// Optional embedding vector for vector similarity search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
}

impl NodeRecord {
    /// Creates a new node record with a fresh logical clock entry for the actor.
    pub fn new(id: NodeId, actor: impl Into<ActorId>, data: NodeData) -> Self {
        let actor = actor.into();
        let mut clock = VectorClock::default();
        clock.insert(actor.clone(), 1);
        Self {
            id,
            data,
            clock,
            timestamp: Utc::now(),
            embedding: None,
        }
    }

    /// Increments the logical clock for the given actor and updates the payload.
    pub fn merge_update(&mut self, actor: impl Into<ActorId>, data: NodeData) {
        let actor = actor.into();
        let counter = self.clock.entry(actor).or_insert(0);
        *counter += 1;
        self.timestamp = Utc::now();
        self.data = data;
    }
}

/// HNSW-based vector index for approximate nearest-neighbour search.
///
/// Internally this index maps string node IDs to integer HNSW indices and
/// vice-versa so that the higher-level API can work with node IDs throughout.
/// The HNSW structure uses interior mutability; the index itself is
/// `Send + Sync` so it can be held behind an `Arc`.
pub struct VectorIndex {
    hnsw: Hnsw<'static, f32, DistCosine>,
    /// String node-ID → HNSW numeric index (latest for each ID).
    id_to_idx: DashMap<NodeId, usize>,
    /// HNSW numeric index → string node-ID (set at insert time).
    idx_to_id: DashMap<usize, NodeId>,
    next_idx: Mutex<usize>,
    /// Maximum number of elements the HNSW graph was initialised for.
    max_elements: usize,
}

/// Compile-time assertion that `VectorIndex` is `Send + Sync`.
///
/// If a future version of `hnsw_rs` removes those bounds this will
/// produce a clear compiler error instead of silent unsoundness.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<VectorIndex>();
};

impl std::fmt::Debug for VectorIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorIndex")
            .field("indexed_nodes", &self.id_to_idx.len())
            .finish()
    }
}

impl VectorIndex {
    /// Creates a new index with the given maximum capacity.
    pub fn new(max_elements: usize) -> Self {
        // Standard HNSW hyper-parameters (Malkov & Yashunin 2018).
        Self {
            hnsw: Hnsw::new(16, max_elements, 16, 200, DistCosine),
            id_to_idx: DashMap::new(),
            idx_to_id: DashMap::new(),
            next_idx: Mutex::new(0),
            max_elements,
        }
    }

    /// Inserts or updates the embedding for `id`.
    ///
    /// On repeated calls for the same `id` a fresh HNSW slot is allocated and
    /// the ID→index mapping is updated.  The stale slot is automatically
    /// filtered out in [`search`] because its reverse mapping no longer points
    /// back to the same numeric index.
    ///
    /// # Capacity
    ///
    /// The index was created with a fixed `max_elements` capacity.  Each call
    /// (including updates) consumes one slot.  Inserts beyond the capacity are
    /// silently dropped with a `debug!` log to prevent HNSW panics.  A warning
    /// is emitted when slot usage reaches 90% of capacity.
    pub fn insert(&self, id: &str, embedding: &[f32]) {
        let idx = {
            let mut n = self.next_idx.lock();
            let current = *n;
            if current >= self.max_elements {
                debug!(
                    "VectorIndex at capacity ({} slots); insert for '{}' dropped",
                    self.max_elements, id
                );
                return;
            }
            if current + 1 >= (self.max_elements as f64 * 0.9) as usize {
                debug!(
                    "VectorIndex nearing capacity: {} / {} slots used",
                    current + 1,
                    self.max_elements
                );
            }
            *n += 1;
            current
        };
        self.id_to_idx.insert(id.to_string(), idx);
        self.idx_to_id.insert(idx, id.to_string());
        // HNSW can panic on degenerate vectors; catch and skip.
        let emb_owned: Vec<f32> = embedding.to_vec();
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.hnsw.insert((&emb_owned, idx));
        })).is_err() {
            eprintln!("[VectorIndex] HNSW insert panicked for '{}'; skipping", id);
            self.id_to_idx.remove(id);
            self.idx_to_id.remove(&idx);
        }
    }

    /// Returns up to `limit` node IDs with their cosine similarity scores,
    /// ordered from most to least similar.  Scores are in \[0, 1\] (higher is
    /// more similar; orthogonal or opposite vectors score 0).
    pub fn search(&self, query: &[f32], limit: usize) -> Vec<(NodeId, f32)> {
        if self.id_to_idx.is_empty() {
            return Vec::new();
        }
        // ef_s controls the quality/speed trade-off at query time; 16 is a
        // reasonable default for production workloads.
        let neighbours = self.hnsw.search(query, limit, 16);
        neighbours
            .into_iter()
            .filter_map(|n| {
                // Skip stale HNSW slots whose forward mapping has been
                // superseded by a newer insert for the same node ID.
                let node_id = self.idx_to_id.get(&n.d_id)?.clone();
                let current_idx = self.id_to_idx.get(&*node_id)?;
                if *current_idx != n.d_id {
                    return None;
                }
                // DistCosine computes `1 – cos(θ)`, so its output range is [0, 2]:
                //   distance = 0  →  cos(θ) = 1  (identical vectors)
                //   distance = 1  →  cos(θ) = 0  (orthogonal)
                //   distance = 2  →  cos(θ) = -1 (opposite)
                // We map this to a similarity score in [0, 1] by subtracting
                // from 1 and clamping, so that:
                //   distance 0 → score 1.0 (perfect match)
                //   distance 1 → score 0.0 (no similarity)
                //   distance 2 → score 0.0 (clamped; treated as no similarity)
                let score = (1.0_f32 - n.distance).max(0.0);
                Some((node_id.clone(), score))
            })
            .collect()
    }

    /// Returns `true` if the index contains no vectors.
    pub fn is_empty(&self) -> bool {
        self.id_to_idx.is_empty()
    }
}

impl Default for VectorIndex {
    fn default() -> Self {
        Self::new(1_000_000)
    }
}

/// Actor name stamped on [`NodeRecord`]s that are materialised directly from
/// SQLite without going through the in-memory write path.  Used by
/// [`CrdtStore::get`], [`CrdtStore::list`], and [`CrdtStore::vector_search`]
/// when falling back to the persistence layer.
const SQLITE_ACTOR: &str = "sqlite";

/// Errors that can be produced by the CRDT store.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("node not found: {0}")]
    NotFound(NodeId),
}

/// CRDT operations that clients may apply to the store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrdtOperation {
    Put {
        id: NodeId,
        actor: ActorId,
        data: NodeData,
    },
    Delete {
        id: NodeId,
    },
}

/// A simple conflict-free replicated data store backed by a concurrent map.
pub struct CrdtStore {
    nodes: DashMap<NodeId, NodeRecord>,
    vector_index: Arc<VectorIndex>,
    /// Optional text-embedding backend.  When set, [`put`][CrdtStore::put]
    /// will automatically generate and index an embedding for each node whose
    /// JSON data contains extractable text content.
    embedder: Option<Arc<dyn EmbedText>>,
    /// Optional SQLite persistence layer.  When set, every `put`,
    /// `put_with_embedding`, and `delete` will write-through to SQLite.
    /// Read operations (`get`, `list`) query SQLite directly rather than
    /// keeping all records in the in-memory map.
    persistence: Option<Arc<Database>>,
    /// Tracks whether the HNSW vector index has been populated from SQLite.
    /// Starts as `false` when a persistence layer is attached; the first call
    /// to [`vector_search`][CrdtStore::vector_search] will build the index
    /// lazily and set this to `true`.
    vector_index_ready: AtomicBool,
}

impl std::fmt::Debug for CrdtStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrdtStore")
            .field("nodes", &self.nodes.len())
            .field("vector_index", &self.vector_index)
            .field("embedder", &self.embedder.is_some())
            .finish()
    }
}

impl Default for CrdtStore {
    fn default() -> Self {
        Self {
            nodes: DashMap::new(),
            vector_index: Arc::new(VectorIndex::default()),
            embedder: None,
            persistence: None,
            // No persistence to load from, so the index is trivially ready.
            vector_index_ready: AtomicBool::new(true),
        }
    }
}

impl CrdtStore {
    /// Attach a text-embedding backend to this store.
    ///
    /// After calling this method, every subsequent call to [`put`][Self::put]
    /// will automatically extract text from the node data and generate an
    /// embedding via `embedder`.  The embedding is stored on the
    /// [`NodeRecord`] and indexed in the HNSW graph for vector search.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::sync::Arc;
    /// use pluresdb_core::{CrdtStore, FastEmbedder};
    ///
    /// let embedder = FastEmbedder::new("BAAI/bge-small-en-v1.5").unwrap();
    /// let store = CrdtStore::default().with_embedder(Arc::new(embedder));
    /// ```
    pub fn with_embedder(mut self, embedder: Arc<dyn EmbedText>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Returns a reference to the attached embedder, if any.
    pub fn embedder(&self) -> Option<&dyn EmbedText> {
        self.embedder.as_deref()
    }

    /// Build the HNSW vector index from all nodes that have embeddings.
    ///
    /// When a persistence layer is attached this also loads embeddings from
    /// SQLite so that vector search works after a restart (without hydrating
    /// the full node data).  The index is otherwise populated incrementally by
    /// every [`put_with_embedding`][Self::put_with_embedding] call.
    pub fn build_vector_index(&self) -> usize {
        let expected_dim = self.embedder.as_ref().map(|e| e.dimension());
        let mut indexed = 0usize;
        for entry in self.nodes.iter() {
            if let Some(emb) = &entry.value().embedding {
                // Check dimension match
                if let Some(dim) = expected_dim {
                    if emb.len() != dim { continue; }
                }
                // Validate
                if emb.is_empty() || !emb.iter().all(|v| v.is_finite()) || !emb.iter().any(|v| *v != 0.0) {
                    continue;
                }
                self.vector_index.insert(entry.key(), emb);
                indexed += 1;
            }
        }
        // Also load embeddings from SQLite for historical records.
        if self.persistence.is_some() {
            self.build_vector_index_from_persistence();
        }
        self.vector_index_ready.store(true, Ordering::Release);
        eprintln!("[CrdtStore] Built vector index: {} entries", indexed);
        indexed
    }

    /// Populate the HNSW vector index from embeddings stored in SQLite.
    ///
    /// Only embedding blobs are read — the full node data is *not* loaded into
    /// memory.  This is called lazily by [`vector_search`][Self::vector_search]
    /// on the first search after startup.
    fn build_vector_index_from_persistence(&self) {
        let db = match &self.persistence {
            Some(db) => db,
            None => return,
        };
        let expected_dim = self.embedder.as_ref().map(|e| e.dimension());
        let rows = match db.query(
            "SELECT id, embedding FROM crdt_nodes WHERE embedding IS NOT NULL",
            &[],
        ) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[CrdtStore] build_vector_index_from_persistence failed: {}", e);
                return;
            }
        };
        let mut indexed = 0usize;
        for row in &rows.rows {
            let id = match row.get(0) {
                Some(SqlValue::Text(s)) => s.clone(),
                _ => continue,
            };
            let floats = match row.get(1) {
                Some(SqlValue::Blob(blob)) if blob.len() >= 4 && blob.len() % 4 == 0 => {
                    let floats: Vec<f32> = blob
                        .chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect();
                    if let Some(dim) = expected_dim {
                        if floats.len() != dim {
                            continue;
                        }
                    }
                    if floats.is_empty()
                        || !floats.iter().all(|v| v.is_finite())
                        || !floats.iter().any(|v| *v != 0.0)
                    {
                        continue;
                    }
                    floats
                }
                _ => continue,
            };
            self.vector_index.insert(&id, &floats);
            indexed += 1;
        }
        eprintln!("[CrdtStore] Loaded {} embeddings from SQLite into vector index", indexed);
    }

    /// Attach a SQLite persistence layer.
    ///
    /// The store will:
    /// 1. Create the `crdt_nodes` table if it doesn't exist.
    /// 2. Perform a one-time migration of any legacy `memories` table.
    /// 3. Write-through on every `put`, `put_with_embedding`, and `delete`.
    ///
    /// **No records are loaded into memory at startup.**  Read operations
    /// (`get`, `list`) query SQLite directly, and the HNSW vector index is
    /// built lazily on the first call to [`vector_search`][Self::vector_search].
    pub fn with_persistence(mut self, db: Arc<Database>) -> Result<Self, DatabaseError> {
        // Create table
        db.exec(
            "CREATE TABLE IF NOT EXISTS crdt_nodes (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL,
                embedding BLOB,
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            )"
        )?;
        db.exec("CREATE INDEX IF NOT EXISTS idx_crdt_nodes_updated ON crdt_nodes(updated_at)")?;

        // Migrate legacy `memories` table if it exists and `crdt_nodes` is empty
        let crdt_count: i64 = db.query("SELECT COUNT(*) FROM crdt_nodes", &[])
            .ok()
            .and_then(|r| r.rows.first().and_then(|row| row.first().and_then(|v| v.as_i64())))
            .unwrap_or(0);
        if crdt_count == 0 {
            // Check if legacy memories table exists
            let has_legacy = db.query(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='memories'",
                &[],
            ).ok()
                .and_then(|r| r.rows.first().and_then(|row| row.first().and_then(|v| v.as_i64())))
                .unwrap_or(0) > 0;

            if has_legacy {
                let legacy_count: i64 = db.query("SELECT COUNT(*) FROM memories", &[])
                    .ok()
                    .and_then(|r| r.rows.first().and_then(|row| row.first().and_then(|v| v.as_i64())))
                    .unwrap_or(0);
                if legacy_count > 0 {
                    eprintln!("[CrdtStore] Migrating {} records from legacy `memories` table to `crdt_nodes`...", legacy_count);
                    let migrate_sql = r#"
                        INSERT OR IGNORE INTO crdt_nodes (id, data, embedding, updated_at)
                        SELECT
                            'memory:' || id,
                            json_object(
                                'content', content,
                                'created_at', created_at,
                                'source', COALESCE(source, ''),
                                'tags', tags,
                                'category', COALESCE(category, 'other'),
                                'priority', COALESCE(priority, 'normal'),
                                'last_accessed_at', last_accessed_at,
                                'merge_count', COALESCE(merge_count, 0),
                                'importance_score', COALESCE(importance_score, 1.0)
                            ),
                            embedding,
                            COALESCE(created_at / 1000, strftime('%s','now'))
                        FROM memories
                    "#;
                    match db.exec(migrate_sql) {
                        Ok(result) => eprintln!("[CrdtStore] Migrated {} records from legacy table", result.changes),
                        Err(e) => eprintln!("[CrdtStore] Legacy migration failed: {}", e),
                    }
                }
            }
        }

        self.persistence = Some(db);
        // Mark the HNSW index as needing a lazy build on the next vector_search.
        self.vector_index_ready.store(false, Ordering::Release);
        Ok(self)
    }

    /// Write a node to the SQLite persistence layer (if attached).
    fn persist_node(&self, id: &str, data: &NodeData, embedding: Option<&[f32]>) {
        if let Some(db) = &self.persistence {
            let data_json = serde_json::to_string(data).unwrap_or_default();
            let emb_blob: Option<Vec<u8>> = embedding.map(|e| {
                e.iter().flat_map(|f| f.to_le_bytes()).collect()
            });
            let sql = if emb_blob.is_some() {
                "INSERT OR REPLACE INTO crdt_nodes (id, data, embedding, updated_at) VALUES (?1, ?2, ?3, strftime('%s','now'))"
            } else {
                "INSERT OR REPLACE INTO crdt_nodes (id, data, updated_at) VALUES (?1, ?2, strftime('%s','now'))"
            };
            let params: Vec<SqlValue> = if let Some(blob) = emb_blob {
                vec![
                    SqlValue::Text(id.to_string()),
                    SqlValue::Text(data_json),
                    SqlValue::Blob(blob),
                ]
            } else {
                vec![
                    SqlValue::Text(id.to_string()),
                    SqlValue::Text(data_json),
                ]
            };
            if let Err(e) = db.query(sql, &params) {
                eprintln!("[CrdtStore] persist failed for {}: {}", id, e);
            }
        }
    }

    /// Delete a node from the SQLite persistence layer (if attached).
    ///
    /// Returns `true` if the node was found and deleted from SQLite.
    fn unpersist_node(&self, id: &str) -> bool {
        if let Some(db) = &self.persistence {
            let params = vec![SqlValue::Text(id.to_string())];
            match db.query("DELETE FROM crdt_nodes WHERE id = ?1", &params) {
                Ok(result) => result.changes > 0,
                Err(e) => {
                    eprintln!("[CrdtStore] unpersist failed for {}: {}", id, e);
                    false
                }
            }
        } else {
            false
        }
    }

    /// Fetch a single node's data from SQLite by ID without caching it in memory.
    fn get_from_persistence(&self, id: &str) -> Option<NodeRecord> {
        let db = self.persistence.as_ref()?;
        let params = vec![SqlValue::Text(id.to_string())];
        let rows = db.query("SELECT data FROM crdt_nodes WHERE id = ?1", &params).ok()?;
        let row = rows.rows.into_iter().next()?;
        let data_str = row.into_iter().next().and_then(|v| {
            if let SqlValue::Text(s) = v { Some(s) } else { None }
        })?;
        let data = serde_json::from_str::<NodeData>(&data_str).ok()?;
        Some(NodeRecord::new(id.to_string(), SQLITE_ACTOR, data))
    }

    /// Inserts or updates a node using CRDT semantics.
    ///
    /// When an [`EmbedText`] backend has been attached via
    /// [`with_embedder`][Self::with_embedder], this method will also
    /// auto-generate an embedding for any text content found in `data` and
    /// store it alongside the node (equivalent to calling
    /// [`put_with_embedding`][Self::put_with_embedding] manually).
    ///
    /// Embedding failures are silently ignored so that the put always
    /// succeeds — the node is stored without an embedding in that case.
    pub fn put(&self, id: impl Into<NodeId>, actor: impl Into<ActorId>, data: NodeData) -> NodeId {
        let id = id.into();
        let actor = actor.into();
        // Auto-embed when an embedder is attached and the data contains text.
        if let Some(embedder) = &self.embedder {
            if let Some(text) = extract_text_from_data(&data) {
                if let Ok(mut batch) = embedder.embed(&[text.as_str()]) {
                    if let Some(embedding) = batch.pop() {
                        return self.put_with_embedding(id, actor, data, embedding);
                    }
                }
            }
        }
        self.nodes
            .entry(id.clone())
            .and_modify(|record| record.merge_update(actor.clone(), data.clone()))
            .or_insert_with(|| NodeRecord::new(id.clone(), actor, data.clone()));
        self.persist_node(&id, &data, None);
        id
    }

    /// Inserts or updates a node together with its embedding vector.
    ///
    /// The embedding is stored on the [`NodeRecord`] **and** indexed in the
    /// HNSW graph so that future calls to [`vector_search`] can find it.
    pub fn put_with_embedding(
        &self,
        id: impl Into<NodeId>,
        actor: impl Into<ActorId>,
        data: NodeData,
        embedding: Vec<f32>,
    ) -> NodeId {
        let id = id.into();
        let actor = actor.into();
        // Validate embedding before HNSW insertion
        let emb_valid = !embedding.is_empty()
            && embedding.iter().all(|v| v.is_finite())
            && embedding.iter().any(|v| *v != 0.0);
        let emb_clone = embedding.clone();
        self.nodes
            .entry(id.clone())
            .and_modify(|record| {
                record.merge_update(actor.clone(), data.clone());
                record.embedding = Some(embedding.clone());
            })
            .or_insert_with(|| {
                let mut r = NodeRecord::new(id.clone(), actor, data.clone());
                r.embedding = Some(embedding);
                r
            });
        if emb_valid {
            self.vector_index.insert(&id, &emb_clone);
        }
        self.persist_node(&id, &data, Some(&emb_clone));
        id
    }

    /// Removes a node from the store.
    pub fn delete(&self, id: impl AsRef<str>) -> Result<(), StoreError> {
        let id_ref = id.as_ref();
        let in_sqlite = self.unpersist_node(id_ref);
        let in_memory = self.nodes.remove(id_ref).is_some();
        if in_memory || in_sqlite {
            Ok(())
        } else {
            Err(StoreError::NotFound(id_ref.to_owned()))
        }
    }

    /// Fetches a node by identifier.
    ///
    /// When a persistence layer is attached, nodes that are not in the
    /// in-memory cache are looked up directly from SQLite.
    pub fn get(&self, id: impl AsRef<str>) -> Option<NodeRecord> {
        let id = id.as_ref();
        if let Some(entry) = self.nodes.get(id) {
            return Some(entry.value().clone());
        }
        self.get_from_persistence(id)
    }

    /// Lists all nodes currently stored.
    ///
    /// When a persistence layer is attached, the list is fetched directly from
    /// SQLite rather than from the in-memory map.  In-memory entries (nodes
    /// written in the current session) shadow their SQLite counterparts so
    /// callers always see the most recent data.
    pub fn list(&self) -> Vec<NodeRecord> {
        if let Some(db) = &self.persistence {
            match db.query("SELECT id, data FROM crdt_nodes", &[]) {
                Ok(rows) => {
                    return rows.rows.iter().filter_map(|row| {
                        let id = row.get(0)?.as_str()?.to_string();
                        let data_str = row.get(1)?.as_str()?;
                        let data: NodeData = serde_json::from_str(data_str).ok()?;
                        // Prefer the in-memory version which may have newer data.
                        // DashMap::get is O(1) amortised so this loop is O(n) overall.
                        if let Some(entry) = self.nodes.get(&id) {
                            Some(entry.value().clone())
                        } else {
                            Some(NodeRecord::new(id, SQLITE_ACTOR, data))
                        }
                    }).collect();
                }
                Err(e) => {
                    eprintln!("[CrdtStore] list from SQLite failed: {}", e);
                }
            }
        }
        self.nodes.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Applies a CRDT operation, returning the resulting node identifier when relevant.
    pub fn apply(&self, op: CrdtOperation) -> Result<Option<NodeId>, StoreError> {
        match op {
            CrdtOperation::Put { id, actor, data } => Ok(Some(self.put(id, actor, data))),
            CrdtOperation::Delete { id } => {
                self.delete(&id)?;
                Ok(None)
            }
        }
    }

    /// Generates a CRDT operation representing the provided node data.
    pub fn operation_for(
        &self,
        actor: impl Into<ActorId>,
        data: NodeData,
    ) -> (NodeId, CrdtOperation) {
        let id = Uuid::new_v4().to_string();
        let op = CrdtOperation::Put {
            id: id.clone(),
            actor: actor.into(),
            data,
        };
        (id, op)
    }

    /// Performs approximate nearest-neighbour search over all indexed nodes.
    ///
    /// # Arguments
    /// * `query_embedding` – the query vector (must match the dimension used at
    ///   insert time).
    /// * `limit` – maximum number of results to return.
    /// * `min_score` – minimum cosine similarity (0–1) a node must have to be
    ///   included in the results.
    ///
    /// Returns results ordered from highest to lowest similarity score.
    ///
    /// On the first call after startup the HNSW index is built lazily from the
    /// SQLite `embedding` blobs, so this call may be slightly slower than
    /// subsequent ones.
    pub fn vector_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        mut min_score: f32,
    ) -> Vec<VectorSearchResult> {
        // Basic validation of the query embedding: it must be non-empty and contain
        // only finite values to avoid confusing behavior or downstream errors.
        if query_embedding.is_empty() {
            return Vec::new();
        }
        if query_embedding.iter().any(|v| !v.is_finite()) {
            return Vec::new();
        }

        // Normalize min_score to the documented [0.0, 1.0] range and ensure it is finite.
        if !min_score.is_finite() {
            min_score = 0.0;
        } else if min_score < 0.0 {
            min_score = 0.0;
        } else if min_score > 1.0 {
            min_score = 1.0;
        }

        // Lazily populate the HNSW index from SQLite on the first search call
        // after startup.  This avoids blocking startup while still allowing
        // vector search after a restart.
        //
        // Two concurrent callers can both observe `vector_index_ready = false`
        // and both invoke `build_vector_index_from_persistence`.  This is safe:
        // `VectorIndex::insert` is idempotent for the same (id, vector) pair,
        // and the stale-slot filter in `VectorIndex::search` discards any
        // duplicate HNSW entries.  The redundant work is bounded to the very
        // first batch of concurrent calls on a freshly started store.
        if !self.vector_index_ready.load(Ordering::Acquire) {
            self.build_vector_index_from_persistence();
            self.vector_index_ready.store(true, Ordering::Release);
        }

        let candidates = self.vector_index.search(query_embedding, limit);
        let mut results: Vec<VectorSearchResult> = candidates
            .into_iter()
            .filter_map(|(id, score)| {
                if score < min_score {
                    return None;
                }
                // Resolve node data: prefer in-memory (current session) then SQLite.
                let record = if let Some(entry) = self.nodes.get(&id) {
                    entry.value().clone()
                } else {
                    self.get_from_persistence(&id)?
                };
                Some(VectorSearchResult { record, score })
            })
            .collect();
        // Ensure ordering from highest to lowest similarity.
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results
    }
}

/// Primitive SQLite values returned by the native engine.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SqlValue {
    pub fn as_i64(&self) -> Option<i64> {
        if let Self::Integer(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        if let Self::Real(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Self::Text(value) = self {
            Some(value.as_str())
        } else {
            None
        }
    }

    pub fn as_blob(&self) -> Option<&[u8]> {
        if let Self::Blob(value) = self {
            Some(value.as_slice())
        } else {
            None
        }
    }

    pub fn to_json(&self) -> JsonValue {
        match self {
            SqlValue::Null => JsonValue::Null,
            SqlValue::Integer(value) => json!(value),
            SqlValue::Real(value) => json!(value),
            SqlValue::Text(value) => json!(value),
            SqlValue::Blob(bytes) => json!(bytes),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<SqlValue>>,
    pub changes: u64,
    pub last_insert_rowid: i64,
}

impl QueryResult {
    pub fn rows_as_maps(&self) -> Vec<HashMap<String, SqlValue>> {
        self.rows
            .iter()
            .map(|row| {
                let mut map = HashMap::new();
                for (index, value) in row.iter().cloned().enumerate() {
                    if let Some(column) = self.columns.get(index) {
                        map.insert(column.clone(), value);
                    }
                }
                map
            })
            .collect()
    }

    pub fn rows_as_json(&self) -> Vec<JsonValue> {
        self.rows_as_maps()
            .into_iter()
            .map(|row| {
                let json_object: HashMap<String, JsonValue> = row
                    .into_iter()
                    .map(|(key, value)| (key, value.to_json()))
                    .collect();
                json!(json_object)
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    pub changes: u64,
    pub last_insert_rowid: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DatabasePath {
    InMemory,
    File(PathBuf),
}

#[derive(Debug, Clone)]
pub struct DatabaseOptions {
    pub path: DatabasePath,
    pub read_only: bool,
    pub create_if_missing: bool,
    pub apply_default_pragmas: bool,
    pub custom_pragmas: Vec<(String, String)>,
    pub busy_timeout: Option<Duration>,
    /// HuggingFace model ID to use for automatic text embedding (e.g.
    /// `"BAAI/bge-small-en-v1.5"`).  Requires the `embeddings` feature.
    pub embedding_model: Option<String>,
}

impl Default for DatabaseOptions {
    fn default() -> Self {
        Self {
            path: DatabasePath::InMemory,
            read_only: false,
            create_if_missing: true,
            apply_default_pragmas: true,
            custom_pragmas: Vec::new(),
            busy_timeout: Some(Duration::from_millis(5_000)),
            embedding_model: None,
        }
    }
}

impl DatabaseOptions {
    pub fn in_memory() -> Self {
        Self::default()
    }

    pub fn with_file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: DatabasePath::File(path.into()),
            ..Default::default()
        }
    }

    pub fn read_only(mut self, flag: bool) -> Self {
        self.read_only = flag;
        self
    }

    pub fn create_if_missing(mut self, flag: bool) -> Self {
        self.create_if_missing = flag;
        self
    }

    pub fn apply_default_pragmas(mut self, flag: bool) -> Self {
        self.apply_default_pragmas = flag;
        self
    }

    pub fn add_pragma(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_pragmas.push((name.into(), value.into()));
        self
    }

    pub fn busy_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.busy_timeout = timeout;
        self
    }

    /// Set the HuggingFace model ID to use for automatic text embedding.
    ///
    /// When set, a [`CrdtStore`] constructed from these options will
    /// auto-embed text content on every [`put`][CrdtStore::put].  Requires
    /// the `embeddings` cargo feature to take effect at runtime.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pluresdb_core::DatabaseOptions;
    ///
    /// let opts = DatabaseOptions::default()
    ///     .with_embedding_model("BAAI/bge-small-en-v1.5");
    /// assert_eq!(opts.embedding_model.as_deref(), Some("BAAI/bge-small-en-v1.5"));
    /// ```
    pub fn with_embedding_model(mut self, model_id: impl Into<String>) -> Self {
        self.embedding_model = Some(model_id.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    path: DatabasePath,
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub type DbResult<T> = Result<T, DatabaseError>;

const DEFAULT_PRAGMAS: &[(&str, &str)] = &[
    ("journal_mode", "WAL"),
    ("synchronous", "NORMAL"),
    ("temp_store", "MEMORY"),
    ("mmap_size", "30000000000"),
    ("page_size", "4096"),
    ("cache_size", "-64000"),
];

impl Database {
    pub fn open(options: DatabaseOptions) -> DbResult<Self> {
        let connection = match &options.path {
            DatabasePath::InMemory => Connection::open_in_memory()?,
            DatabasePath::File(path) => {
                Connection::open_with_flags(path, build_open_flags(&options))?
            }
        };

        if let Some(timeout) = options.busy_timeout {
            connection.busy_timeout(timeout)?;
        }

        if options.apply_default_pragmas {
            apply_pragmas(&connection, DEFAULT_PRAGMAS);
        }

        if !options.custom_pragmas.is_empty() {
            let custom: Vec<(&str, &str)> = options
                .custom_pragmas
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect();
            apply_pragmas(&connection, &custom);
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(connection)),
            path: options.path,
        })
    }

    pub fn path(&self) -> &DatabasePath {
        &self.path
    }

    pub fn prepare(&self, sql: impl Into<String>) -> DbResult<Statement> {
        Ok(Statement {
            database: self.clone(),
            sql: sql.into(),
        })
    }

    pub fn exec(&self, sql: &str) -> DbResult<ExecutionResult> {
        self.with_connection(|conn| {
            conn.execute_batch(sql)?;
            Ok(ExecutionResult {
                changes: conn.changes() as u64,
                last_insert_rowid: conn.last_insert_rowid(),
            })
        })
    }

    pub fn query(&self, sql: &str, params: &[SqlValue]) -> DbResult<QueryResult> {
        Statement {
            database: self.clone(),
            sql: sql.to_owned(),
        }
        .query_internal(params)
    }

    pub fn pragma(&self, pragma: &str) -> DbResult<QueryResult> {
        let normalized = if pragma.trim_start().to_lowercase().starts_with("pragma") {
            pragma.trim().to_owned()
        } else {
            format!("PRAGMA {}", pragma)
        };
        self.query(&normalized, &[])
    }

    pub fn transaction<F, T>(&self, f: F) -> DbResult<T>
    where
        F: FnOnce(&Transaction<'_>) -> DbResult<T>,
    {
        self.with_connection(|conn| {
            let tx = conn.transaction()?;
            let result = f(&tx)?;
            tx.commit()?;
            Ok(result)
        })
    }

    fn with_connection<T, F>(&self, f: F) -> DbResult<T>
    where
        F: FnOnce(&mut Connection) -> DbResult<T>,
    {
        let mut guard = self.conn.lock();
        f(&mut guard)
    }
}

#[derive(Debug, Clone)]
pub struct Statement {
    database: Database,
    sql: String,
}

impl Statement {
    pub fn sql(&self) -> &str {
        &self.sql
    }

    pub fn run(&self, params: &[SqlValue]) -> DbResult<ExecutionResult> {
        self.database.with_connection(|conn| {
            let mut stmt = conn.prepare(&self.sql)?;
            let values = params_to_values(params);
            let changes = stmt.execute(params_from_iter(values.iter()))? as u64;
            Ok(ExecutionResult {
                changes,
                last_insert_rowid: conn.last_insert_rowid(),
            })
        })
    }

    pub fn all(&self, params: &[SqlValue]) -> DbResult<QueryResult> {
        self.query_internal(params)
    }

    pub fn get(&self, params: &[SqlValue]) -> DbResult<Option<HashMap<String, SqlValue>>> {
        let result = self.query_internal(params)?;
        Ok(result.rows_as_maps().into_iter().next())
    }

    pub fn columns(&self) -> DbResult<Vec<String>> {
        self.database.with_connection(|conn| {
            let stmt = conn.prepare(&self.sql)?;
            Ok(stmt
                .column_names()
                .iter()
                .map(|name| name.to_string())
                .collect())
        })
    }

    fn query_internal(&self, params: &[SqlValue]) -> DbResult<QueryResult> {
        self.database.with_connection(|conn| {
            let mut stmt = conn.prepare(&self.sql)?;
            let columns = stmt
                .column_names()
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>();
            let values = params_to_values(params);
            let column_count = columns.len();
            let mut rows_iter = stmt.query(params_from_iter(values.iter()))?;
            let mut rows = Vec::new();
            while let Some(row) = rows_iter.next()? {
                rows.push(read_row(&row, column_count)?);
            }
            Ok(QueryResult {
                columns,
                rows,
                changes: conn.changes() as u64,
                last_insert_rowid: conn.last_insert_rowid(),
            })
        })
    }
}

fn build_open_flags(options: &DatabaseOptions) -> OpenFlags {
    let mut flags = OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if options.read_only {
        flags |= OpenFlags::SQLITE_OPEN_READ_ONLY;
    } else {
        flags |= OpenFlags::SQLITE_OPEN_READ_WRITE;
        if options.create_if_missing {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
    }
    flags
}

fn apply_pragmas(connection: &Connection, pragmas: &[(&str, &str)]) {
    for (name, value) in pragmas {
        if let Err(error) = connection.pragma_update(None, name, value) {
            debug!(pragma = %name, "failed to apply pragma: {error}");
        }
    }
}

fn params_to_values(params: &[SqlValue]) -> Vec<SqliteValue> {
    params
        .iter()
        .map(|value| match value {
            SqlValue::Null => SqliteValue::Null,
            SqlValue::Integer(v) => SqliteValue::Integer(*v),
            SqlValue::Real(v) => SqliteValue::Real(*v),
            SqlValue::Text(v) => SqliteValue::Text(v.clone()),
            SqlValue::Blob(v) => SqliteValue::Blob(v.clone()),
        })
        .collect()
}

fn read_row(row: &rusqlite::Row<'_>, column_count: usize) -> Result<Vec<SqlValue>, rusqlite::Error> {
    let mut values = Vec::with_capacity(column_count);
    for index in 0..column_count {
        let value = match row.get_ref(index)? {
            ValueRef::Null => SqlValue::Null,
            ValueRef::Integer(v) => SqlValue::Integer(v),
            ValueRef::Real(v) => SqlValue::Real(v),
            ValueRef::Text(v) => SqlValue::Text(String::from_utf8_lossy(v).into_owned()),
            ValueRef::Blob(v) => SqlValue::Blob(v.to_vec()),
        };
        values.push(value);
    }
    Ok(values)
}

// ---------------------------------------------------------------------------
// Text extraction helper
// ---------------------------------------------------------------------------

/// Extract a plain-text representation from a JSON node payload.
///
/// Priority order for extraction:
/// 1. If the value is itself a string, return it directly.
/// 2. If the value is an object, collect every string-valued leaf at the
///    top level (all keys are considered) and concatenate them with spaces.
///    An empty result is treated the same as `None`.
///
/// Returns `None` when no text could be extracted (e.g. numeric-only
/// payloads or deeply-nested structures without top-level string fields).
fn extract_text_from_data(data: &JsonValue) -> Option<String> {
    match data {
        JsonValue::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        }
        JsonValue::Object(map) => {
            let parts: Vec<&str> = map
                .values()
                .filter_map(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(" "))
            }
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// FastEmbedder — fastembed-backed EmbedText implementation (feature-gated)
// ---------------------------------------------------------------------------

/// A text-embedding backend powered by
/// [fastembed](https://crates.io/crates/fastembed).
///
/// This type is only available when the `embeddings` cargo feature is
/// enabled.  It wraps an ONNX Runtime model and produces `f32` embedding
/// vectors suitable for storage in the [`VectorIndex`].
///
/// # Example
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use pluresdb_core::{CrdtStore, FastEmbedder};
///
/// let embedder = FastEmbedder::new("BAAI/bge-small-en-v1.5")?;
/// let store = CrdtStore::default().with_embedder(Arc::new(embedder));
///
/// // Auto-embeds "user prefers dark mode" on insert:
/// store.put("memory-1", "actor", serde_json::json!({"content": "user prefers dark mode"}));
/// ```
#[cfg(feature = "embeddings")]
pub struct FastEmbedder {
    model: std::sync::Mutex<fastembed::TextEmbedding>,
    dimension: usize,
    model_id: String,
}

#[cfg(feature = "embeddings")]
impl std::fmt::Debug for FastEmbedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastEmbedder")
            .field("dimension", &self.dimension)
            .field("model_id", &self.model_id)
            .finish()
    }
}

#[cfg(feature = "embeddings")]
impl FastEmbedder {
    /// Initialize a new [`FastEmbedder`] from a HuggingFace model ID string.
    ///
    /// The model is downloaded from HuggingFace on first use and cached
    /// locally by the underlying fastembed / ONNX Runtime runtime.
    ///
    /// # Supported models
    ///
    /// | Model ID                          | Dimension |
    /// |-----------------------------------|-----------|
    /// | `BAAI/bge-small-en-v1.5`          | 384       |
    /// | `BAAI/bge-base-en-v1.5`           | 768       |
    /// | `BAAI/bge-large-en-v1.5`          | 1024      |
    /// | `sentence-transformers/all-MiniLM-L6-v2` | 384 |
    ///
    /// # Errors
    ///
    /// Returns an error if `model_id` is not recognised or if the model
    /// fails to initialize (e.g. because it cannot be downloaded).
    pub fn new(model_id: &str) -> anyhow::Result<Self> {
        use fastembed::{InitOptions, TextEmbedding};

        let (embedding_model, dimension) = model_id_to_fastembed(model_id)?;
        let model = TextEmbedding::try_new(InitOptions::new(embedding_model))?;
        Ok(Self {
            model: std::sync::Mutex::new(model),
            dimension,
            model_id: model_id.to_owned(),
        })
    }

    /// Return the model ID this embedder was initialized with.
    pub fn model_id(&self) -> &str {
        &self.model_id
    }
}

#[cfg(feature = "embeddings")]
impl EmbedText for FastEmbedder {
    fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        let owned: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
        let mut model = self.model.lock().map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
        model.embed(owned, None).map_err(Into::into)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

/// Map a HuggingFace model ID string to a fastembed `EmbeddingModel` enum
/// variant and the expected output dimension.
///
/// # Errors
///
/// Returns an error for model IDs that are not (yet) supported.
#[cfg(feature = "embeddings")]
fn model_id_to_fastembed(
    model_id: &str,
) -> anyhow::Result<(fastembed::EmbeddingModel, usize)> {
    use fastembed::EmbeddingModel;
    match model_id {
        "BAAI/bge-small-en-v1.5" => Ok((EmbeddingModel::BGESmallENV15, 384)),
        "BAAI/bge-base-en-v1.5" => Ok((EmbeddingModel::BGEBaseENV15, 768)),
        "BAAI/bge-large-en-v1.5" => Ok((EmbeddingModel::BGELargeENV15, 1024)),
        "sentence-transformers/all-MiniLM-L6-v2" => Ok((EmbeddingModel::AllMiniLML6V2, 384)),
        other => anyhow::bail!(
            "unsupported embedding model '{}'; supported models: \
             BAAI/bge-small-en-v1.5, BAAI/bge-base-en-v1.5, \
             BAAI/bge-large-en-v1.5, sentence-transformers/all-MiniLM-L6-v2",
            other
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::ErrorCode;

    #[test]
    fn put_and_get_round_trip() {
        let store = CrdtStore::default();
        let id = store.put("node-1", "actor-a", serde_json::json!({"hello": "world"}));
        let record = store.get(&id).expect("record should exist");
        assert_eq!(record.data["hello"], "world");
        assert_eq!(record.clock.get("actor-a"), Some(&1));
    }

    #[test]
    fn delete_removes_node() {
        let store = CrdtStore::default();
        let id = store.put("node-2", "actor-a", serde_json::json!({"name": "plures"}));
        store.delete(&id).expect("delete succeeds");
        assert!(store.get(&id).is_none());
    }

    #[test]
    fn apply_operations() {
        let store = CrdtStore::default();
        let op = CrdtOperation::Put {
            id: "node-3".to_string(),
            actor: "actor-a".to_string(),
            data: serde_json::json!({"count": 1}),
        };
        let result = store.apply(op).expect("apply succeeds");
        assert_eq!(result, Some("node-3".to_string()));

        let delete = CrdtOperation::Delete {
            id: "node-3".to_string(),
        };
        let result = store.apply(delete).expect("delete succeeds");
        assert_eq!(result, None);
        assert!(store.get("node-3").is_none());
    }

    #[test]
    fn database_exec_and_query() {
        let db = Database::open(DatabaseOptions::default()).expect("open database");
        db.exec("CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)")
            .expect("create table");

        let insert = db
            .prepare("INSERT INTO users (name) VALUES (?1)")
            .expect("prepare insert");
        insert
            .run(&[SqlValue::Text("Alice".to_string())])
            .expect("insert row");

        let query = db
            .prepare("SELECT id, name FROM users ORDER BY id")
            .expect("prepare select");
        let result = query.all(&[]).expect("query rows");
        assert_eq!(result.columns, vec!["id".to_string(), "name".to_string()]);
        assert_eq!(result.rows.len(), 1);
        match &result.rows[0][1] {
            SqlValue::Text(value) => assert_eq!(value, "Alice"),
            other => panic!("unexpected value: {:?}", other),
        }
    }

    #[test]
    fn database_default_pragmas_applied() {
        let temp = tempfile::NamedTempFile::new().expect("create temp file");
        let db = Database::open(DatabaseOptions::with_file(temp.path()))
            .expect("open database");
        let result = db.pragma("journal_mode").expect("run pragma");
        assert!(!result.rows.is_empty());
        match &result.rows[0][0] {
            SqlValue::Text(mode) => assert_eq!(mode.to_lowercase(), "wal"),
            other => panic!("unexpected pragma value: {:?}", other),
        }
    }

    #[test]
    fn statement_get_returns_none_when_no_rows() {
        let db = Database::open(DatabaseOptions::default()).expect("open database");
        db.exec("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)")
            .expect("create table");

        let select = db
            .prepare("SELECT name FROM items WHERE id = ?1")
            .expect("prepare select");
        let result = select
            .get(&[SqlValue::Integer(42)])
            .expect("query should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn statement_run_propagates_sql_errors() {
        let db = Database::open(DatabaseOptions::default()).expect("open database");
        db.exec("CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT UNIQUE NOT NULL)")
            .expect("create table");

        let insert = db
            .prepare("INSERT INTO users (email) VALUES (?1)")
            .expect("prepare insert");
        insert
            .run(&[SqlValue::Text("alice@example.com".into())])
            .expect("first insert succeeds");

        let err = insert
            .run(&[SqlValue::Text("alice@example.com".into())])
            .expect_err("second insert should fail");
        match err {
            DatabaseError::Sqlite(inner) => {
                assert_eq!(inner.sqlite_error_code(), Some(ErrorCode::ConstraintViolation));
            }
        }
    }

    #[test]
    fn statement_handles_blob_parameters_and_columns() {
        let db = Database::open(DatabaseOptions::default()).expect("open database");
        db.exec("CREATE TABLE files (id INTEGER PRIMARY KEY, data BLOB NOT NULL)")
            .expect("create table");

        let blob = vec![0_u8, 1, 2, 3];
        let insert = db
            .prepare("INSERT INTO files (id, data) VALUES (?1, ?2)")
            .expect("prepare insert");
        insert
            .run(&[SqlValue::Integer(1), SqlValue::Blob(blob.clone())])
            .expect("insert blob row");

        let select = db
            .prepare("SELECT id, data FROM files WHERE id = ?1")
            .expect("prepare select");
        let columns = select.columns().expect("inspect columns");
        assert_eq!(columns, vec!["id".to_string(), "data".to_string()]);

        let result = select
            .all(&[SqlValue::Integer(1)])
            .expect("query single row");
        assert_eq!(result.rows.len(), 1);
        match &result.rows[0][1] {
            SqlValue::Blob(value) => assert_eq!(value, &blob),
            other => panic!("unexpected value: {:?}", other),
        }
    }

    #[test]
    fn put_with_embedding_stores_and_searches() {
        let store = CrdtStore::default();

        // Insert nodes with orthogonal unit-vector embeddings.
        let emb_a: Vec<f32> = vec![1.0, 0.0, 0.0];
        let emb_b: Vec<f32> = vec![0.0, 1.0, 0.0];
        let emb_c: Vec<f32> = vec![0.0, 0.0, 1.0];

        store.put_with_embedding("a", "actor-v", serde_json::json!({"label":"a"}), emb_a.clone());
        store.put_with_embedding("b", "actor-v", serde_json::json!({"label":"b"}), emb_b);
        store.put_with_embedding("c", "actor-v", serde_json::json!({"label":"c"}), emb_c);

        // Embedding should be stored on the NodeRecord.
        let record = store.get("a").expect("node a should exist");
        assert_eq!(record.embedding, Some(emb_a.clone()));

        // Searching with the same vector as "a" should return "a" first.
        let results = store.vector_search(&emb_a, 3, 0.0);
        assert!(!results.is_empty(), "should find at least one result");
        assert_eq!(results[0].record.id, "a");
        assert!(
            results[0].score > 0.99,
            "identical vector should have score near 1.0, got {}",
            results[0].score
        );
        // All results must be ordered highest → lowest.
        for w in results.windows(2) {
            assert!(
                w[0].score >= w[1].score,
                "results should be ordered by descending score: {} < {}",
                w[0].score,
                w[1].score
            );
        }
    }

    #[test]
    fn vector_search_respects_min_score() {
        let store = CrdtStore::default();

        let emb_a: Vec<f32> = vec![1.0, 0.0, 0.0];
        let emb_b: Vec<f32> = vec![0.0, 1.0, 0.0];

        store.put_with_embedding("a", "actor-v", serde_json::json!({}), emb_a.clone());
        store.put_with_embedding("b", "actor-v", serde_json::json!({}), emb_b);

        // High min_score should filter out dissimilar vectors.
        let results = store.vector_search(&emb_a, 10, 0.99);
        assert_eq!(results.len(), 1, "only 'a' should pass the 0.99 threshold");
        assert_eq!(results[0].record.id, "a");
    }

    #[test]
    fn vector_search_empty_index_returns_empty() {
        let store = CrdtStore::default();
        let results = store.vector_search(&[1.0_f32, 0.0, 0.0], 5, 0.0);
        assert!(results.is_empty(), "empty index should return no results");
    }

    #[test]
    fn vector_index_update_keeps_latest_embedding() {
        let store = CrdtStore::default();

        let emb_v1: Vec<f32> = vec![1.0, 0.0, 0.0];
        let emb_v2: Vec<f32> = vec![0.0, 1.0, 0.0];

        store.put_with_embedding("node", "actor", serde_json::json!({"v": 1}), emb_v1);
        store.put_with_embedding("node", "actor", serde_json::json!({"v": 2}), emb_v2.clone());

        let record = store.get("node").expect("node should exist");
        assert_eq!(record.embedding, Some(emb_v2.clone()));

        // Searching with emb_v2 should return "node" as the top hit.
        let results = store.vector_search(&emb_v2, 3, 0.0);
        assert!(!results.is_empty());
        assert_eq!(results[0].record.id, "node");
    }

    // -----------------------------------------------------------------------
    // extract_text_from_data tests
    // -----------------------------------------------------------------------

    #[test]
    fn extract_text_from_string_value() {
        let data = serde_json::json!("hello world");
        assert_eq!(
            extract_text_from_data(&data).as_deref(),
            Some("hello world")
        );
    }

    #[test]
    fn extract_text_from_object_with_string_fields() {
        let data = serde_json::json!({"content": "user prefers dark mode", "type": "memory"});
        let text = extract_text_from_data(&data).expect("should extract text");
        assert!(text.contains("user prefers dark mode"));
        assert!(text.contains("memory"));
    }

    #[test]
    fn extract_text_skips_numeric_only_object() {
        let data = serde_json::json!({"count": 42, "value": 3.14});
        assert!(extract_text_from_data(&data).is_none());
    }

    #[test]
    fn extract_text_returns_none_for_empty_string() {
        let data = serde_json::json!("   ");
        assert!(extract_text_from_data(&data).is_none());
    }

    #[test]
    fn extract_text_returns_none_for_number() {
        let data = serde_json::json!(42);
        assert!(extract_text_from_data(&data).is_none());
    }

    // -----------------------------------------------------------------------
    // Auto-embedding via mock EmbedText
    // -----------------------------------------------------------------------

    /// Minimal test embedder: maps each text to a unit vector in R³ derived
    /// from its length (so two identical strings produce the same vector).
    #[derive(Debug)]
    struct MockEmbedder;

    impl EmbedText for MockEmbedder {
        fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .map(|t| {
                    let n = (t.len() % 3) as f32;
                    let mut v = vec![0.0_f32; 3];
                    v[n as usize] = 1.0;
                    v
                })
                .collect())
        }

        fn dimension(&self) -> usize {
            3
        }
    }

    #[test]
    fn put_auto_embeds_when_embedder_attached() {
        use std::sync::Arc;

        let store = CrdtStore::default().with_embedder(Arc::new(MockEmbedder));

        // Data with a string field — should be auto-embedded.
        store.put("n1", "actor", serde_json::json!({"content": "hello"}));
        let record = store.get("n1").expect("node should exist");
        assert!(
            record.embedding.is_some(),
            "embedding should have been generated automatically"
        );

        // Verify the vector is searchable.
        let emb = record.embedding.as_ref().unwrap();
        let results = store.vector_search(emb, 5, 0.0);
        assert!(!results.is_empty());
        assert_eq!(results[0].record.id, "n1");
    }

    #[test]
    fn put_without_embedder_stores_no_embedding() {
        let store = CrdtStore::default(); // no embedder
        store.put("n2", "actor", serde_json::json!({"content": "hello"}));
        let record = store.get("n2").expect("node should exist");
        assert!(
            record.embedding.is_none(),
            "no embedding should be stored without an embedder"
        );
    }

    #[test]
    fn put_skips_embedding_for_numeric_data() {
        use std::sync::Arc;

        let store = CrdtStore::default().with_embedder(Arc::new(MockEmbedder));
        // Numeric-only payload — no text to embed.
        store.put("n3", "actor", serde_json::json!({"value": 99}));
        let record = store.get("n3").expect("node should exist");
        assert!(
            record.embedding.is_none(),
            "embedding should not be generated for numeric-only payloads"
        );
    }

    // -----------------------------------------------------------------------
    // DatabaseOptions::with_embedding_model
    // -----------------------------------------------------------------------

    #[test]
    fn database_options_with_embedding_model() {
        let opts = DatabaseOptions::default()
            .with_embedding_model("BAAI/bge-small-en-v1.5");
        assert_eq!(
            opts.embedding_model.as_deref(),
            Some("BAAI/bge-small-en-v1.5")
        );
    }

    #[test]
    fn database_options_embedding_model_none_by_default() {
        let opts = DatabaseOptions::default();
        assert!(opts.embedding_model.is_none());
    }

    // -----------------------------------------------------------------------
    // No-hydration / SQLite-direct read tests
    // -----------------------------------------------------------------------

    /// Helper: open an in-memory SQLite database and attach it as persistence.
    fn make_persisted_store() -> CrdtStore {
        let db = Arc::new(Database::open(DatabaseOptions::default()).expect("open db"));
        CrdtStore::default()
            .with_persistence(db)
            .expect("with_persistence")
    }

    #[test]
    fn with_persistence_does_not_hydrate_into_memory() {
        // Pre-populate the database with a row the old-fashioned way.
        let db = Arc::new(Database::open(DatabaseOptions::default()).expect("open db"));
        db.exec(
            "CREATE TABLE IF NOT EXISTS crdt_nodes (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL,
                embedding BLOB,
                updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now'))
            )",
        ).expect("create table");
        db.query(
            "INSERT INTO crdt_nodes (id, data) VALUES (?1, ?2)",
            &[
                SqlValue::Text("node-pre".to_string()),
                SqlValue::Text(r#"{"hello":"from-sqlite"}"#.to_string()),
            ],
        ).expect("insert pre-existing row");

        // Attaching persistence must NOT load the row into self.nodes.
        let store = CrdtStore::default()
            .with_persistence(Arc::clone(&db))
            .expect("with_persistence");

        // In-memory map must be empty — no hydration occurred.
        assert!(
            store.nodes.is_empty(),
            "in-memory map should be empty after with_persistence (no hydration)"
        );

        // But get() must still find the node via SQLite.
        let record = store.get("node-pre").expect("get should fall back to SQLite");
        assert_eq!(record.data["hello"], "from-sqlite");
    }

    #[test]
    fn get_falls_back_to_sqlite_for_persisted_node() {
        let store = make_persisted_store();

        // Write a node — this goes into memory AND SQLite.
        store.put("p1", "actor", serde_json::json!({"v": 1}));

        // Simulate a "restart" by creating a new store on the same SQLite db.
        let db = store.persistence.clone().expect("has persistence");
        let store2 = CrdtStore::default()
            .with_persistence(db)
            .expect("with_persistence");

        // The node should not be in the in-memory map of the new store.
        assert!(store2.nodes.is_empty(), "new store must have empty in-memory map");

        // But get() must retrieve it from SQLite.
        let record = store2.get("p1").expect("should find node via SQLite");
        assert_eq!(record.data["v"], 1);
    }

    #[test]
    fn list_queries_sqlite_directly() {
        let store = make_persisted_store();

        store.put("list-a", "actor", serde_json::json!({"n": "a"}));
        store.put("list-b", "actor", serde_json::json!({"n": "b"}));

        // Simulate a "restart".
        let db = store.persistence.clone().expect("has persistence");
        let store2 = CrdtStore::default()
            .with_persistence(db)
            .expect("with_persistence");

        let records = store2.list();
        assert_eq!(records.len(), 2, "list() should return all SQLite records");
        let ids: Vec<&str> = records.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&"list-a"));
        assert!(ids.contains(&"list-b"));
    }

    #[test]
    fn delete_works_for_sqlite_only_node() {
        let store = make_persisted_store();
        store.put("del-node", "actor", serde_json::json!({"x": 1}));

        // Simulate a "restart".
        let db = store.persistence.clone().expect("has persistence");
        let store2 = CrdtStore::default()
            .with_persistence(db)
            .expect("with_persistence");

        // Node is in SQLite but NOT in the in-memory map of the new store.
        assert!(store2.nodes.is_empty());
        assert!(store2.get("del-node").is_some(), "node should be in SQLite");

        // delete() must succeed even though the node is not in memory.
        store2.delete("del-node").expect("delete should succeed for SQLite-only node");
        assert!(store2.get("del-node").is_none(), "node should be gone after delete");
    }

    #[test]
    fn delete_returns_not_found_for_nonexistent_node_with_persistence() {
        let store = make_persisted_store();
        let err = store.delete("ghost-node").expect_err("should error for missing node");
        assert!(matches!(err, StoreError::NotFound(_)));
    }

    #[test]
    fn vector_search_builds_index_lazily_from_sqlite() {
        let store = make_persisted_store();

        let emb_a: Vec<f32> = vec![1.0, 0.0, 0.0];
        let emb_b: Vec<f32> = vec![0.0, 1.0, 0.0];

        store.put_with_embedding("vs-a", "actor", serde_json::json!({"label":"a"}), emb_a.clone());
        store.put_with_embedding("vs-b", "actor", serde_json::json!({"label":"b"}), emb_b);

        // Simulate a "restart" — new store with no in-memory data.
        let db = store.persistence.clone().expect("has persistence");
        let store2 = CrdtStore::default()
            .with_persistence(db)
            .expect("with_persistence");

        assert!(store2.nodes.is_empty(), "new store must have empty in-memory map");

        // vector_search must build the index lazily and return results from SQLite.
        let results = store2.vector_search(&emb_a, 3, 0.0);
        assert!(!results.is_empty(), "vector_search should return results after lazy build");
        assert_eq!(results[0].record.id, "vs-a");
        assert!(results[0].score > 0.99, "identical vector should score ~1.0");
    }
}

