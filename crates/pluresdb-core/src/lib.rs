//! Core data structures, CRDT logic, and domain models that power PluresDB.
//!
//! The goal of this crate is to offer a lightweight, dependency-free-on-FFI
//! foundation that can be reused across the native CLI, the Node addon, and
//! any future host integrations.

pub mod plugin;
pub use plugin::{NoOpPlugin, PluresLmPlugin};

pub mod procedures;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures::executor::block_on;
use hnsw_rs::prelude::*;
use parking_lot::Mutex;
use pluresdb_storage::{StorageEngine, StoredNode};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
#[cfg(feature = "sqlite-compat")]
use serde_json::json;
use thiserror::Error;
use tracing::debug;
use uuid::Uuid;

#[cfg(feature = "sqlite-compat")]
use std::path::PathBuf;
#[cfg(feature = "sqlite-compat")]
use std::time::Duration;
#[cfg(feature = "sqlite-compat")]
use rusqlite::types::{Value as SqliteValue, ValueRef};
#[cfg(feature = "sqlite-compat")]
use rusqlite::{params_from_iter, Connection, OpenFlags, Transaction};

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

    /// Optional model identifier, used for task labelling and observability.
    ///
    /// Returns `None` by default.  Override for embedders that track model IDs.
    fn model_id(&self) -> Option<&str> {
        None
    }
}

/// A key-value map of logical clocks per actor.
pub type VectorClock = HashMap<ActorId, u64>;

/// Arbitrary JSON payload that callers persist inside PluresDB.
pub type NodeData = JsonValue;

/// Default embedding dimension (bge-small-en-v1.5).
pub const DEFAULT_EMBEDDING_DIM: usize = 768;

/// A pending embedding computation task, enqueued by [`CrdtStore::put`] and
/// processed by the background worker started via
/// [`CrdtStore::spawn_embedding_worker`].
#[derive(Debug, Clone)]
pub struct EmbeddingTask {
    /// The node identifier for which an embedding should be computed.
    pub node_id: NodeId,
    /// Plain-text extracted from the node data at enqueue time.
    pub extracted_text: String,
    /// Model identifier reported by the embedder at enqueue time, if available.
    pub model_id: Option<String>,
    /// Wall-clock time when the task was enqueued.
    pub timestamp: DateTime<Utc>,
}

/// Observability snapshot for the embedding background worker.
///
/// Returned by [`CrdtStore::embedding_worker_stats`].
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingWorkerStats {
    /// Number of tasks currently waiting in the queue.
    pub queue_depth: usize,
    /// Wall-clock time of the most recently completed embedding task, if any.
    pub last_processed: Option<DateTime<Utc>>,
    /// Number of tasks dropped because the queue was at capacity.
    pub dropped_tasks: usize,
}

/// A search result from vector similarity search.
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    /// The full node record that matched the query.
    pub record: NodeRecord,
    /// Cosine similarity score in \[0, 1\] where 1 = identical direction.
    pub score: f32,
}

/// Metadata associated with a persisted node in the CRDT store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeRecord {
    /// Unique node identifier.
    pub id: NodeId,
    /// Arbitrary JSON payload stored with this node.
    pub data: NodeData,
    /// Per-actor logical write counters used for CRDT merges.
    pub clock: VectorClock,
    /// Wall-clock time of the last write that touched this node.
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

/// Errors that can be produced by the CRDT store.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("node not found: {0}")]
    NotFound(NodeId),
}

/// CRDT operations that clients may apply to the store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrdtOperation {
    /// Insert or update a node identified by `id`.
    Put {
        id: NodeId,
        actor: ActorId,
        data: NodeData,
    },
    /// Remove the node identified by `id`.
    Delete {
        id: NodeId,
    },
}

/// A simple conflict-free replicated data store backed by a concurrent map.
pub struct CrdtStore {
    nodes: DashMap<NodeId, NodeRecord>,
    vector_index: Arc<VectorIndex>,
    /// Optional text-embedding backend.  When set, [`put`][CrdtStore::put]
    /// enqueues an async embedding task for each node whose JSON data contains
    /// extractable text content.
    embedder: Option<Arc<dyn EmbedText>>,
    /// Optional plugin that separates higher-level layers (e.g. pluresLM)
    /// from the core engine.  When set, [`put`][CrdtStore::put] calls
    /// [`PluresLmPlugin::on_node_written`] after every successful write, and
    /// [`delete`][CrdtStore::delete] calls
    /// [`PluresLmPlugin::on_node_deleted`] after every successful removal.
    lm_plugin: Option<Arc<dyn PluresLmPlugin>>,
    /// Optional storage engine persistence layer.  When set, every `put`,
    /// `put_with_embedding`, and `delete` will write-through to the engine.
    /// Read operations (`get`, `list`) query the engine directly rather than
    /// keeping all records in the in-memory map.
    persistence: Option<Arc<dyn StorageEngine>>,
    /// Tracks whether the HNSW vector index has been populated from storage.
    /// Starts as `false` when a persistence layer is attached; the first call
    /// to [`vector_search`][CrdtStore::vector_search] will build the index
    /// lazily and set this to `true`.
    vector_index_ready: AtomicBool,
    /// Sender half of the async embedding task queue.
    /// When `Some`, [`put`][CrdtStore::put] enqueues tasks here instead of
    /// blocking on embedding inference.
    embedding_tx: Option<std::sync::mpsc::SyncSender<EmbeddingTask>>,
    /// Receiver half — claimed exactly once by
    /// [`spawn_embedding_worker`][CrdtStore::spawn_embedding_worker].
    embedding_rx: parking_lot::Mutex<Option<std::sync::mpsc::Receiver<EmbeddingTask>>>,
    /// Approximate number of tasks currently waiting in the queue.
    embedding_queue_depth: AtomicUsize,
    /// Wall-clock timestamp of the last task processed by the background worker.
    embedding_last_processed: parking_lot::Mutex<Option<DateTime<Utc>>>,
    /// Number of tasks dropped because the queue was at capacity.
    embedding_dropped: AtomicUsize,
}

impl std::fmt::Debug for CrdtStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrdtStore")
            .field("nodes", &self.nodes.len())
            .field("vector_index", &self.vector_index)
            .field("embedder", &self.embedder.is_some())
            .field("lm_plugin", &self.lm_plugin.as_ref().map(|p| p.plugin_id()))
            .finish()
    }
}

impl Default for CrdtStore {
    fn default() -> Self {
        Self {
            nodes: DashMap::new(),
            vector_index: Arc::new(VectorIndex::default()),
            embedder: None,
            lm_plugin: None,
            persistence: None,
            // No persistence to load from, so the index is trivially ready.
            vector_index_ready: AtomicBool::new(true),
            embedding_tx: None,
            embedding_rx: parking_lot::Mutex::new(None),
            embedding_queue_depth: AtomicUsize::new(0),
            embedding_last_processed: parking_lot::Mutex::new(None),
            embedding_dropped: AtomicUsize::new(0),
        }
    }
}

impl CrdtStore {
    /// Attach a text-embedding backend to this store.
    ///
    /// After calling this method, every subsequent call to [`put`][Self::put]
    /// will extract text from the node data and enqueue an
    /// [`EmbeddingTask`] for processing by the background worker (started via
    /// [`spawn_embedding_worker`][Self::spawn_embedding_worker]).  The default
    /// queue capacity is **1 024** tasks; use
    /// [`with_embedder_capacity`][Self::with_embedder_capacity] to customise.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::sync::Arc;
    /// use pluresdb_core::{CrdtStore, FastEmbedder};
    ///
    /// let embedder = FastEmbedder::new("BAAI/bge-small-en-v1.5").unwrap();
    /// let store = Arc::new(CrdtStore::default().with_embedder(Arc::new(embedder)));
    /// CrdtStore::spawn_embedding_worker(Arc::clone(&store));
    /// ```
    pub fn with_embedder(self, embedder: Arc<dyn EmbedText>) -> Self {
        self.with_embedder_capacity(embedder, 1024)
    }

    /// Like [`with_embedder`][Self::with_embedder] but with a custom task-queue
    /// capacity.
    ///
    /// `capacity` is the maximum number of pending embedding tasks that can
    /// sit in the queue at any time.  When the queue is full, new tasks are
    /// dropped and counted in [`EmbeddingWorkerStats::dropped_tasks`].
    pub fn with_embedder_capacity(mut self, embedder: Arc<dyn EmbedText>, capacity: usize) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(capacity);
        self.embedder = Some(embedder);
        self.embedding_tx = Some(tx);
        *self.embedding_rx.lock() = Some(rx);
        self
    }

    /// Returns a reference to the attached embedder, if any.
    pub fn embedder(&self) -> Option<&dyn EmbedText> {
        self.embedder.as_deref()
    }

    /// Attach a pluresLM plugin to this store.
    ///
    /// The plugin is called after every successful [`put`][Self::put] and
    /// [`delete`][Self::delete] operation, providing a clean extension point
    /// for pluresLM (and other higher-level layers) without coupling the core
    /// engine to any particular schema or model.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::sync::Arc;
    /// use pluresdb_core::{CrdtStore, PluresLmPlugin, NoOpPlugin};
    ///
    /// let store = CrdtStore::default().with_lm_plugin(Arc::new(NoOpPlugin));
    /// ```
    pub fn with_lm_plugin(mut self, plugin: Arc<dyn PluresLmPlugin>) -> Self {
        self.lm_plugin = Some(plugin);
        self
    }

    /// Returns the identifier of the attached pluresLM plugin, if any.
    pub fn lm_plugin_id(&self) -> Option<&str> {
        self.lm_plugin.as_ref().map(|p| p.plugin_id())
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

    /// Populate the HNSW vector index from embeddings stored in the persistence layer.
    ///
    /// Node records are fully loaded and deserialized from storage, but only their
    /// embedding field is used for indexing. This is called lazily by
    /// [`vector_search`][Self::vector_search] on the first search after startup.
    fn build_vector_index_from_persistence(&self) {
        let storage = match &self.persistence {
            Some(s) => s,
            None => return,
        };
        let expected_dim = self.embedder.as_ref().map(|e| e.dimension());
        let nodes = match block_on(storage.list()) {
            Ok(nodes) => nodes,
            Err(e) => {
                tracing::error!("[CrdtStore] build_vector_index_from_persistence failed: {}", e);
                return;
            }
        };
        let mut indexed = 0usize;
        for stored in nodes {
            let record = match serde_json::from_value::<NodeRecord>(stored.payload) {
                Ok(r) => r,
                Err(_) => continue,
            };
            if let Some(emb) = &record.embedding {
                if let Some(dim) = expected_dim {
                    if emb.len() != dim {
                        continue;
                    }
                }
                if emb.is_empty()
                    || !emb.iter().all(|v| v.is_finite())
                    || !emb.iter().any(|v| *v != 0.0)
                {
                    continue;
                }
                self.vector_index.insert(&record.id, emb);
                indexed += 1;
            }
        }
        tracing::debug!("[CrdtStore] Loaded {} embeddings from storage into vector index", indexed);
    }

    /// Attach a storage engine persistence layer.
    ///
    /// The store will write-through on every `put`, `put_with_embedding`,
    /// and `delete`.
    ///
    /// **No records are loaded into memory at startup.**  Read operations
    /// (`get`, `list`) query the engine directly, and the HNSW vector index
    /// is built lazily on the first call to
    /// [`vector_search`][Self::vector_search].
    pub fn with_persistence(mut self, storage: Arc<dyn StorageEngine>) -> Self {
        self.persistence = Some(storage);
        // Mark the HNSW index as needing a lazy build on the next vector_search.
        self.vector_index_ready.store(false, Ordering::Release);
        self
    }

    /// Write a node to the persistence layer (if attached).
    fn persist_node(&self, record: &NodeRecord) {
        if let Some(storage) = &self.persistence {
            let payload = match serde_json::to_value(record) {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!(
                        "[CrdtStore] persist skipped for '{}': serialization failed: {}",
                        record.id,
                        e
                    );
                    return;
                }
            };
            let stored = StoredNode {
                id: record.id.clone(),
                payload,
            };
            if let Err(e) = block_on(storage.put(stored)) {
                tracing::error!("[CrdtStore] persist failed for {}: {}", record.id, e);
            }
        }
    }

    /// Delete a node from the persistence layer (if attached).
    ///
    /// Returns `true` if the node was found and deleted from storage.
    fn unpersist_node(&self, id: &str) -> bool {
        if let Some(storage) = &self.persistence {
            let exists = block_on(storage.get(id))
                .ok()
                .flatten()
                .is_some();
            if exists {
                if let Err(e) = block_on(storage.delete(id)) {
                    tracing::error!("[CrdtStore] unpersist failed for {}: {}", id, e);
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Fetch a single node's data from the persistence layer by ID.
    fn get_from_persistence(&self, id: &str) -> Option<NodeRecord> {
        let storage = self.persistence.as_ref()?;
        let stored = block_on(storage.get(id)).ok()??;
        serde_json::from_value::<NodeRecord>(stored.payload).ok()
    }

    /// Inserts or updates a node using CRDT semantics.
    ///
    /// The node is stored **immediately** — this method never blocks on
    /// embedding inference or index building.  When an [`EmbedText`] backend
    /// has been attached via [`with_embedder`][Self::with_embedder], an
    /// [`EmbeddingTask`] is enqueued for the background worker (started via
    /// [`spawn_embedding_worker`][Self::spawn_embedding_worker]) which will
    /// compute the embedding and update the vector index out-of-band.
    ///
    /// If the task queue is full the embedding task is dropped (the node is
    /// still stored) and the dropped-task counter incremented.
    pub fn put(&self, id: impl Into<NodeId>, actor: impl Into<ActorId>, data: NodeData) -> NodeId {
        let id = id.into();
        let actor = actor.into();
        // Store the node immediately using CRDT semantics.
        self.nodes
            .entry(id.clone())
            .and_modify(|record| record.merge_update(actor.clone(), data.clone()))
            .or_insert_with(|| NodeRecord::new(id.clone(), actor, data.clone()));
        if let Some(entry) = self.nodes.get(&id) {
            self.persist_node(entry.value());
        }
        // Enqueue an embedding task if the data contains text (non-blocking).
        if let Some(tx) = &self.embedding_tx {
            if let Some(text) = extract_text_from_data(&data) {
                let model_id = self
                    .embedder
                    .as_ref()
                    .and_then(|e| e.model_id())
                    .map(str::to_owned);
                let task = EmbeddingTask {
                    node_id: id.clone(),
                    extracted_text: text,
                    model_id,
                    timestamp: Utc::now(),
                };
                match tx.try_send(task) {
                    Ok(()) => {
                        self.embedding_queue_depth.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(std::sync::mpsc::TrySendError::Full(_)) => {
                        self.embedding_dropped.fetch_add(1, Ordering::Relaxed);
                        tracing::warn!(
                            "[CrdtStore] embedding queue full; task for '{}' dropped",
                            id
                        );
                    }
                    Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                        tracing::warn!(
                            "[CrdtStore] embedding worker disconnected; task for '{}' dropped",
                            id
                        );
                    }
                }
            }
        }
        // Notify the plugin (non-blocking).
        if let Some(plugin) = &self.lm_plugin {
            plugin.on_node_written(&id, &data);
        }
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
        if let Some(entry) = self.nodes.get(&id) {
            self.persist_node(entry.value());
        }
        // Notify the plugin (non-blocking).
        if let Some(plugin) = &self.lm_plugin {
            plugin.on_node_written(&id, &data);
        }
        id
    }

    /// Apply a computed embedding to an existing node and update the vector index.
    ///
    /// Called by the background embedding worker.  Unlike
    /// [`put_with_embedding`][Self::put_with_embedding], this method does
    /// **not** touch the CRDT vector clock so CRDT merge semantics remain
    /// deterministic.
    fn set_embedding_for_node(&self, node_id: &str, embedding: Vec<f32>) {
        let emb_valid = !embedding.is_empty()
            && embedding.iter().all(|v| v.is_finite())
            && embedding.iter().any(|v| *v != 0.0);
        self.nodes
            .entry(node_id.to_string())
            .and_modify(|record| record.embedding = Some(embedding.clone()));
        if emb_valid {
            self.vector_index.insert(node_id, &embedding);
        }
        if let Some(entry) = self.nodes.get(node_id) {
            self.persist_node(entry.value());
        }
    }

    /// Removes a node from the store.
    pub fn delete(&self, id: impl AsRef<str>) -> Result<(), StoreError> {
        let id_ref = id.as_ref();
        let in_sqlite = self.unpersist_node(id_ref);
        let in_memory = self.nodes.remove(id_ref).is_some();
        if in_memory || in_sqlite {
            // Notify the plugin (non-blocking).
            if let Some(plugin) = &self.lm_plugin {
                plugin.on_node_deleted(&id_ref.to_owned());
            }
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
    /// the storage engine rather than from the in-memory map.  In-memory
    /// entries (nodes written in the current session) shadow their stored
    /// counterparts so callers always see the most recent data.
    pub fn list(&self) -> Vec<NodeRecord> {
        if let Some(storage) = &self.persistence {
            match block_on(storage.list()) {
                Ok(nodes) => {
                    return nodes.into_iter().filter_map(|stored| {
                        let record = serde_json::from_value::<NodeRecord>(stored.payload).ok()?;
                        // Prefer the in-memory version which may have newer data.
                        if let Some(entry) = self.nodes.get(&record.id) {
                            Some(entry.value().clone())
                        } else {
                            Some(record)
                        }
                    }).collect();
                }
                Err(e) => {
                    tracing::error!("[CrdtStore] list from storage failed: {}", e);
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

    /// Spawn a background OS thread that processes pending embedding tasks.
    ///
    /// **Must** be called after wrapping the store in an [`Arc`].  The worker
    /// takes ownership of the receiver end of the task queue and runs until all
    /// senders are dropped (i.e. the `Arc<CrdtStore>` is fully released).
    ///
    /// Each task calls the attached embedder, then updates the node record and
    /// vector index via [`set_embedding_for_node`][Self::set_embedding_for_node]
    /// without touching the CRDT vector clock.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - no embedder has been attached via [`with_embedder`][Self::with_embedder], or
    /// - this method has already been called (the receiver is only claimable once).
    pub fn spawn_embedding_worker(store: Arc<Self>) -> std::thread::JoinHandle<()> {
        let rx = store
            .embedding_rx
            .lock()
            .take()
            .expect("spawn_embedding_worker: no embedder attached or worker already started");
        let store_w = Arc::clone(&store);
        std::thread::Builder::new()
            .name("pluresdb-embedding-worker".into())
            .spawn(move || {
                while let Ok(task) = rx.recv() {
                    // Decrement the in-flight counter before doing the (slow)
                    // inference work so stats reflect the queue accurately.
                    store_w
                        .embedding_queue_depth
                        .fetch_sub(1, Ordering::Relaxed);
                    if let Some(embedder) = &store_w.embedder {
                        if let Ok(mut batch) =
                            embedder.embed(&[task.extracted_text.as_str()])
                        {
                            if let Some(embedding) = batch.pop() {
                                store_w
                                    .set_embedding_for_node(&task.node_id, embedding);
                            }
                        }
                    }
                    *store_w.embedding_last_processed.lock() = Some(Utc::now());
                }
            })
            .expect("failed to spawn embedding worker thread")
    }

    /// Returns an observability snapshot for the embedding background worker.
    ///
    /// - [`queue_depth`][EmbeddingWorkerStats::queue_depth]: tasks waiting to
    ///   be processed.
    /// - [`last_processed`][EmbeddingWorkerStats::last_processed]: wall-clock
    ///   time of the most recently processed task.
    /// - [`dropped_tasks`][EmbeddingWorkerStats::dropped_tasks]: tasks
    ///   discarded because the queue was at capacity.
    pub fn embedding_worker_stats(&self) -> EmbeddingWorkerStats {
        EmbeddingWorkerStats {
            queue_depth: self.embedding_queue_depth.load(Ordering::Relaxed),
            last_processed: *self.embedding_last_processed.lock(),
            dropped_tasks: self.embedding_dropped.load(Ordering::Relaxed),
        }
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

// ---------------------------------------------------------------------------
// SQLite compatibility layer (feature-gated)
// ---------------------------------------------------------------------------

/// Primitive SQLite values returned by the native engine.
#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone, PartialEq)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

#[cfg(feature = "sqlite-compat")]
impl SqlValue {
    /// Return the integer value, or `None` if this is not `SqlValue::Integer`.
    pub fn as_i64(&self) -> Option<i64> {
        if let Self::Integer(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Return the floating-point value, or `None` if this is not `SqlValue::Real`.
    pub fn as_f64(&self) -> Option<f64> {
        if let Self::Real(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Return a string slice, or `None` if this is not `SqlValue::Text`.
    pub fn as_str(&self) -> Option<&str> {
        if let Self::Text(value) = self {
            Some(value.as_str())
        } else {
            None
        }
    }

    /// Return the raw byte slice, or `None` if this is not `SqlValue::Blob`.
    pub fn as_blob(&self) -> Option<&[u8]> {
        if let Self::Blob(value) = self {
            Some(value.as_slice())
        } else {
            None
        }
    }

    /// Convert to a `serde_json::Value` for serialization.
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

/// The result of executing a `SELECT` query.
#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    /// Ordered list of column names from the query.
    pub columns: Vec<String>,
    /// All matching rows; each row is a vec of column values in column order.
    pub rows: Vec<Vec<SqlValue>>,
    /// Number of rows affected by the last DML statement.
    pub changes: u64,
    /// Row-id of the last INSERT, or 0 if not applicable.
    pub last_insert_rowid: i64,
}

#[cfg(feature = "sqlite-compat")]
impl QueryResult {
    /// Convert each row to a `HashMap<column_name, value>`.
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

    /// Convert each row to a JSON object (`serde_json::Value`).
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

/// Result of executing a DML statement (INSERT, UPDATE, DELETE).
#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    /// Number of rows modified by the statement.
    pub changes: u64,
    /// Row-id of the last INSERT, or 0 if not applicable.
    pub last_insert_rowid: i64,
}

/// Location of a SQLite database file.
#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone, PartialEq)]
pub enum DatabasePath {
    /// Pure in-memory database; data is not persisted across connections.
    InMemory,
    /// File-backed database at the given path.
    File(PathBuf),
}

/// Configuration for opening a [`Database`].
///
/// Use the builder methods to construct the desired configuration.
/// The default configuration creates an in-memory SQLite database.
#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone)]
pub struct DatabaseOptions {
    /// Where to store data.
    pub path: DatabasePath,
    /// Open the database in read-only mode.
    pub read_only: bool,
    /// Create the database file if it does not exist.
    pub create_if_missing: bool,
    /// Apply the default performance pragmas on open.
    pub apply_default_pragmas: bool,
    /// Additional SQLite PRAGMA statements applied after the defaults.
    pub custom_pragmas: Vec<(String, String)>,
    /// How long SQLite should wait on a locked database before returning `BUSY`.
    pub busy_timeout: Option<Duration>,
    /// HuggingFace model ID to use for automatic text embedding (e.g.
    /// `"BAAI/bge-small-en-v1.5"`).  Requires the `embeddings` feature.
    pub embedding_model: Option<String>,
}

#[cfg(feature = "sqlite-compat")]
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

#[cfg(feature = "sqlite-compat")]
impl DatabaseOptions {
    /// Create options for an in-memory database (the default).
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Create options for a file-backed database.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the SQLite database file.
    pub fn with_file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: DatabasePath::File(path.into()),
            ..Default::default()
        }
    }

    /// Set whether to open the database in read-only mode.
    pub fn read_only(mut self, flag: bool) -> Self {
        self.read_only = flag;
        self
    }

    /// Set whether the database file should be created if it does not exist.
    pub fn create_if_missing(mut self, flag: bool) -> Self {
        self.create_if_missing = flag;
        self
    }

    /// Set whether to apply the default performance PRAGMAs on open.
    pub fn apply_default_pragmas(mut self, flag: bool) -> Self {
        self.apply_default_pragmas = flag;
        self
    }

    /// Append a custom SQLite PRAGMA to apply after opening the database.
    pub fn add_pragma(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_pragmas.push((name.into(), value.into()));
        self
    }

    /// Set the busy-timeout for locked database access.
    ///
    /// Pass `None` to disable the busy timeout.
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
    /// # #[cfg(feature = "sqlite-compat")]
    /// use pluresdb_core::DatabaseOptions;
    ///
    /// # #[cfg(feature = "sqlite-compat")] {
    /// let opts = DatabaseOptions::default()
    ///     .with_embedding_model("BAAI/bge-small-en-v1.5");
    /// assert_eq!(opts.embedding_model.as_deref(), Some("BAAI/bge-small-en-v1.5"));
    /// # }
    /// ```
    pub fn with_embedding_model(mut self, model_id: impl Into<String>) -> Self {
        self.embedding_model = Some(model_id.into());
        self
    }
}

/// A SQLite-compatible database connection backed by PluresDB.
///
/// Wraps a `rusqlite::Connection` behind an `Arc<Mutex<_>>` so the handle can
/// be cloned and shared across threads.  All public methods are synchronous
/// and safe to call from multiple threads.
///
/// # Example
///
/// ```rust,no_run
/// # #[cfg(feature = "sqlite-compat")]
/// # {
/// use pluresdb_core::{Database, DatabaseOptions};
///
/// let db = Database::open(DatabaseOptions::in_memory()).unwrap();
/// db.exec("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)").unwrap();
/// db.query("INSERT INTO users VALUES (1, 'Alice')", &[]).unwrap();
/// let result = db.query("SELECT * FROM users", &[]).unwrap();
/// println!("{:?}", result.rows_as_maps());
/// # }
/// ```
#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    path: DatabasePath,
}

/// Errors produced by the SQLite-compatibility layer.
#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Error)]
pub enum DatabaseError {
    /// A rusqlite error occurred.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

#[cfg(feature = "sqlite-compat")]
pub type DbResult<T> = Result<T, DatabaseError>;

#[cfg(feature = "sqlite-compat")]
const DEFAULT_PRAGMAS: &[(&str, &str)] = &[
    ("journal_mode", "WAL"),
    ("synchronous", "NORMAL"),
    ("temp_store", "MEMORY"),
    ("mmap_size", "30000000000"),
    ("page_size", "4096"),
    ("cache_size", "-64000"),
];

#[cfg(feature = "sqlite-compat")]
impl Database {
    /// Open a SQLite database with the given options.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::Sqlite`] if the underlying connection cannot
    /// be established (e.g. the file does not exist and `create_if_missing` is
    /// `false`).
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

    /// Return a reference to the database path used to open this connection.
    pub fn path(&self) -> &DatabasePath {
        &self.path
    }

    /// Create a prepared [`Statement`] that can be executed multiple times.
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL template, optionally containing `?` parameter placeholders.
    pub fn prepare(&self, sql: impl Into<String>) -> DbResult<Statement> {
        Ok(Statement {
            database: self.clone(),
            sql: sql.into(),
        })
    }

    /// Execute one or more semicolon-delimited SQL statements that do not return rows.
    ///
    /// Typically used for DDL (CREATE TABLE, DROP TABLE) or multi-statement scripts.
    ///
    /// # Errors
    ///
    /// Returns [`DatabaseError::Sqlite`] if any statement fails.
    pub fn exec(&self, sql: &str) -> DbResult<ExecutionResult> {
        self.with_connection(|conn| {
            conn.execute_batch(sql)?;
            Ok(ExecutionResult {
                changes: conn.changes() as u64,
                last_insert_rowid: conn.last_insert_rowid(),
            })
        })
    }

    /// Execute a SQL query and return all matching rows.
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL statement with optional `?` placeholders.
    /// * `params` - Bound parameter values in placeholder order.
    pub fn query(&self, sql: &str, params: &[SqlValue]) -> DbResult<QueryResult> {
        Statement {
            database: self.clone(),
            sql: sql.to_owned(),
        }
        .query_internal(params)
    }

    /// Execute a SQLite PRAGMA statement and return the result.
    ///
    /// The `pragma` argument may or may not include the `PRAGMA` keyword; both
    /// forms are accepted (e.g. `"journal_mode"` or `"PRAGMA journal_mode"`).
    pub fn pragma(&self, pragma: &str) -> DbResult<QueryResult> {
        let normalized = if pragma.trim_start().to_lowercase().starts_with("pragma") {
            pragma.trim().to_owned()
        } else {
            format!("PRAGMA {}", pragma)
        };
        self.query(&normalized, &[])
    }

    /// Execute `f` inside a SQLite transaction.
    ///
    /// Commits automatically on success.  If `f` returns an error or panics,
    /// the transaction is rolled back.
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

/// A compiled SQL statement associated with a [`Database`] connection.
///
/// Obtain a `Statement` via [`Database::prepare`].  The same statement can be
/// executed multiple times with different parameter bindings.
#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone)]
pub struct Statement {
    database: Database,
    sql: String,
}

#[cfg(feature = "sqlite-compat")]
impl Statement {
    /// Return the SQL template string that was used to create this statement.
    pub fn sql(&self) -> &str {
        &self.sql
    }

    /// Execute the statement as a DML command (INSERT, UPDATE, DELETE).
    ///
    /// # Arguments
    ///
    /// * `params` - Bound parameter values in placeholder order.
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

    /// Execute the statement as a SELECT query and return all matching rows.
    ///
    /// # Arguments
    ///
    /// * `params` - Bound parameter values in placeholder order.
    pub fn all(&self, params: &[SqlValue]) -> DbResult<QueryResult> {
        self.query_internal(params)
    }

    /// Execute the statement as a SELECT query and return the first row, if any.
    ///
    /// # Arguments
    ///
    /// * `params` - Bound parameter values in placeholder order.
    pub fn get(&self, params: &[SqlValue]) -> DbResult<Option<HashMap<String, SqlValue>>> {
        let result = self.query_internal(params)?;
        Ok(result.rows_as_maps().into_iter().next())
    }

    /// Return the column names produced by this statement (without executing it).
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

#[cfg(feature = "sqlite-compat")]
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

#[cfg(feature = "sqlite-compat")]
fn apply_pragmas(connection: &Connection, pragmas: &[(&str, &str)]) {
    for (name, value) in pragmas {
        if let Err(error) = connection.pragma_update(None, name, value) {
            debug!(pragma = %name, "failed to apply pragma: {error}");
        }
    }
}

#[cfg(feature = "sqlite-compat")]
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

#[cfg(feature = "sqlite-compat")]
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
/// let store = Arc::new(CrdtStore::default().with_embedder(Arc::new(embedder)));
/// CrdtStore::spawn_embedding_worker(Arc::clone(&store));
///
/// // Enqueues an async embedding task for "user prefers dark mode":
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

    fn model_id(&self) -> Option<&str> {
        Some(&self.model_id)
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
    use pluresdb_storage::MemoryStorage;

    #[test]
    fn put_and_get_round_trip() {
        let store = CrdtStore::default();
        let id = store.put("node-1", "actor-a", serde_json::json!({"hello": "world"}));
        let record = store.get(&id).expect("record should exist");
        assert_eq!(record.data["hello"], "world");
        assert_eq!(record.clock.get("actor-a"), Some(&1));
    }

    #[test]
    fn lm_plugin_lifecycle_hooks() {
        use crate::plugin::NoOpPlugin;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        #[derive(Debug)]
        struct CountingPlugin {
            written: Arc<AtomicUsize>,
            deleted: Arc<AtomicUsize>,
        }
        impl crate::plugin::PluresLmPlugin for CountingPlugin {
            fn plugin_id(&self) -> &str {
                "counting"
            }
            fn on_node_written(&self, _id: &NodeId, _data: &NodeData) {
                self.written.fetch_add(1, Ordering::Relaxed);
            }
            fn on_node_deleted(&self, _id: &NodeId) {
                self.deleted.fetch_add(1, Ordering::Relaxed);
            }
        }

        let written = Arc::new(AtomicUsize::new(0));
        let deleted = Arc::new(AtomicUsize::new(0));
        let plugin = Arc::new(CountingPlugin {
            written: Arc::clone(&written),
            deleted: Arc::clone(&deleted),
        });

        let store = CrdtStore::default().with_lm_plugin(plugin);
        assert_eq!(store.lm_plugin_id(), Some("counting"));

        // Two writes via put → on_node_written called twice.
        store.put("a", "actor", serde_json::json!({}));
        store.put("b", "actor", serde_json::json!({}));
        assert_eq!(written.load(Ordering::Relaxed), 2);

        // put_with_embedding also triggers on_node_written.
        store.put_with_embedding("c", "actor", serde_json::json!({"label": "c"}), vec![1.0, 0.0, 0.0]);
        assert_eq!(written.load(Ordering::Relaxed), 3, "put_with_embedding must also fire on_node_written");

        // One delete → on_node_deleted called once.
        store.delete("a").unwrap();
        assert_eq!(deleted.load(Ordering::Relaxed), 1);

        // No-op plugin compiles and can be attached.
        let _store2 = CrdtStore::default().with_lm_plugin(Arc::new(NoOpPlugin));
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

    /// Poll `store` until `node_id` has an embedding, or panic after `attempts × 10 ms`.
    fn wait_for_embedding(store: &CrdtStore, node_id: &str, attempts: usize) -> NodeRecord {
        (0..attempts)
            .find_map(|_| {
                std::thread::sleep(std::time::Duration::from_millis(10));
                let r = store.get(node_id).expect("node should still exist");
                r.embedding.is_some().then_some(r)
            })
            .unwrap_or_else(|| panic!("embedding for '{}' not computed within ~{} ms", node_id, attempts * 10))
    }

    #[test]
    fn put_auto_embeds_when_embedder_attached() {
        use std::sync::Arc;

        let store = Arc::new(CrdtStore::default().with_embedder(Arc::new(MockEmbedder)));
        CrdtStore::spawn_embedding_worker(Arc::clone(&store));

        // Node is stored immediately — before the worker has had a chance to embed.
        store.put("n1", "actor", serde_json::json!({"content": "hello"}));
        let immediate = store.get("n1").expect("node should exist right away");
        assert_eq!(immediate.id, "n1", "node is stored immediately");

        // Wait up to ~1 s for the background worker to compute the embedding.
        let record = wait_for_embedding(&store, "n1", 100);

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

        let store = Arc::new(CrdtStore::default().with_embedder(Arc::new(MockEmbedder)));
        CrdtStore::spawn_embedding_worker(Arc::clone(&store));
        // Numeric-only payload — no text to embed, no task should be enqueued.
        store.put("n3", "actor", serde_json::json!({"value": 99}));
        let record = store.get("n3").expect("node should exist");
        assert!(
            record.embedding.is_none(),
            "embedding should not be generated for numeric-only payloads"
        );
        // No task was queued because there was no text.
        assert_eq!(store.embedding_worker_stats().queue_depth, 0);
    }

    #[test]
    fn put_node_stored_immediately_before_worker_runs() {
        use std::sync::Arc;

        // Without spawning the worker, put() should return immediately and the
        // node should be present — but without an embedding yet.
        let store = Arc::new(CrdtStore::default().with_embedder(Arc::new(MockEmbedder)));
        store.put("n-pre", "actor", serde_json::json!({"content": "test content"}));

        let record = store.get("n-pre").expect("node should exist immediately");
        assert!(
            record.embedding.is_none(),
            "embedding should not be set before worker processes the task"
        );
        assert_eq!(
            store.embedding_worker_stats().queue_depth,
            1,
            "one task should be queued"
        );
    }

    #[test]
    fn embedding_worker_stats_dropped_tasks() {
        use std::sync::Arc;

        // Capacity-0 channel: every try_send returns TrySendError::Full because
        // no receiver is blocking on recv().
        let store =
            Arc::new(CrdtStore::default().with_embedder_capacity(Arc::new(MockEmbedder), 0));

        store.put("d1", "actor", serde_json::json!({"content": "drop me"}));

        let stats = store.embedding_worker_stats();
        assert_eq!(stats.dropped_tasks, 1, "task should have been dropped");
        assert_eq!(stats.queue_depth, 0, "dropped task must not inflate queue_depth");
    }

    #[test]
    fn embedding_worker_stats_last_processed_updated() {
        use std::sync::Arc;

        let store = Arc::new(CrdtStore::default().with_embedder(Arc::new(MockEmbedder)));
        CrdtStore::spawn_embedding_worker(Arc::clone(&store));

        // Before any puts, last_processed should be None.
        assert!(store.embedding_worker_stats().last_processed.is_none());

        store.put("ts1", "actor", serde_json::json!({"content": "hello"}));

        // Wait for the worker to process the task (reuse the embedding-ready helper).
        wait_for_embedding(&store, "ts1", 100);

        let processed = store
            .embedding_worker_stats()
            .last_processed
            .expect("last_processed should be set after worker runs");
        assert!(
            processed <= Utc::now(),
            "last_processed should not be in the future"
        );
    }

    // -----------------------------------------------------------------------
    // Storage engine persistence tests (no SQLite required)
    // -----------------------------------------------------------------------

    /// Helper: create a CrdtStore backed by an in-memory storage engine.
    fn make_storage_store() -> (CrdtStore, Arc<MemoryStorage>) {
        let storage = Arc::new(MemoryStorage::default());
        let store = CrdtStore::default()
            .with_persistence(storage.clone() as Arc<dyn StorageEngine>);
        (store, storage)
    }

    #[test]
    fn with_storage_does_not_hydrate_into_memory() {
        let storage = Arc::new(MemoryStorage::default());

        // Pre-populate storage with a record.
        let pre_record = NodeRecord::new(
            "node-pre".to_string(),
            "actor",
            serde_json::json!({"hello": "from-storage"}),
        );
        block_on(storage.put(StoredNode {
            id: "node-pre".to_string(),
            payload: serde_json::to_value(&pre_record).unwrap(),
        })).expect("pre-populate storage");

        // Attaching persistence must NOT load the row into self.nodes.
        let store = CrdtStore::default()
            .with_persistence(storage.clone() as Arc<dyn StorageEngine>);

        assert!(
            store.nodes.is_empty(),
            "in-memory map should be empty after with_persistence (no hydration)"
        );

        // But get() must still find the node via storage.
        let record = store.get("node-pre").expect("get should fall back to storage");
        assert_eq!(record.data["hello"], "from-storage");
    }

    #[test]
    fn get_falls_back_to_storage_for_persisted_node() {
        let (store, storage) = make_storage_store();

        // Write a node — this goes into memory AND storage.
        store.put("p1", "actor", serde_json::json!({"v": 1}));

        // Simulate a "restart" by creating a new store on the same storage.
        let store2 = CrdtStore::default()
            .with_persistence(storage as Arc<dyn StorageEngine>);

        assert!(store2.nodes.is_empty(), "new store must have empty in-memory map");

        let record = store2.get("p1").expect("should find node via storage");
        assert_eq!(record.data["v"], 1);
    }

    #[test]
    fn list_queries_storage_directly() {
        let (store, storage) = make_storage_store();

        store.put("list-a", "actor", serde_json::json!({"n": "a"}));
        store.put("list-b", "actor", serde_json::json!({"n": "b"}));

        // Simulate a "restart".
        let store2 = CrdtStore::default()
            .with_persistence(storage as Arc<dyn StorageEngine>);

        let records = store2.list();
        assert_eq!(records.len(), 2, "list() should return all storage records");
        let ids: Vec<&str> = records.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&"list-a"));
        assert!(ids.contains(&"list-b"));
    }

    #[test]
    fn delete_works_for_storage_only_node() {
        let (store, storage) = make_storage_store();
        store.put("del-node", "actor", serde_json::json!({"x": 1}));

        // Simulate a "restart".
        let store2 = CrdtStore::default()
            .with_persistence(storage as Arc<dyn StorageEngine>);

        assert!(store2.nodes.is_empty());
        assert!(store2.get("del-node").is_some(), "node should be in storage");

        store2.delete("del-node").expect("delete should succeed for storage-only node");
        assert!(store2.get("del-node").is_none(), "node should be gone after delete");
    }

    #[test]
    fn delete_returns_not_found_for_nonexistent_with_storage() {
        let (store, _storage) = make_storage_store();
        let err = store.delete("ghost-node").expect_err("should error for missing node");
        assert!(matches!(err, StoreError::NotFound(_)));
    }

    #[test]
    fn vector_search_builds_index_lazily_from_storage() {
        let (store, storage) = make_storage_store();

        let emb_a: Vec<f32> = vec![1.0, 0.0, 0.0];
        let emb_b: Vec<f32> = vec![0.0, 1.0, 0.0];

        store.put_with_embedding("vs-a", "actor", serde_json::json!({"label":"a"}), emb_a.clone());
        store.put_with_embedding("vs-b", "actor", serde_json::json!({"label":"b"}), emb_b);

        // Simulate a "restart" — new store with no in-memory data.
        let store2 = CrdtStore::default()
            .with_persistence(storage as Arc<dyn StorageEngine>);

        assert!(store2.nodes.is_empty(), "new store must have empty in-memory map");

        // vector_search must build the index lazily and return results from storage.
        let results = store2.vector_search(&emb_a, 3, 0.0);
        assert!(!results.is_empty(), "vector_search should return results after lazy build");
        assert_eq!(results[0].record.id, "vs-a");
        assert!(results[0].score > 0.99, "identical vector should score ~1.0");
    }

    // -----------------------------------------------------------------------
    // SQLite compatibility tests (require sqlite-compat feature)
    // -----------------------------------------------------------------------

    #[cfg(feature = "sqlite-compat")]
    mod sqlite_compat_tests {
        use super::*;

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
            use rusqlite::ErrorCode;
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
    }
}

