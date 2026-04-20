//! Core data structures, CRDT logic, and domain models that power PluresDB.
//!
//! The goal of this crate is to offer a lightweight, dependency-free-on-FFI
//! foundation that can be reused across the native CLI, the Node addon, and
//! any future host integrations.

pub mod plugin;
pub use plugin::{NoOpPlugin, PluresLmPlugin};

/// Higher-level document, training, and AI-agent procedures built on top of
/// the core CRDT store.  See [`procedures::document`], [`procedures::training`],
/// and [`procedures::ai_procedures`] for the individual sub-modules.
pub mod procedures;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
#[cfg(feature = "native")]
use parking_lot::Mutex;
#[cfg(feature = "native")]
use pluresdb_storage::StorageEngine;
use pluresdb_storage::StoredNode;
#[cfg(not(feature = "native"))]
use pluresdb_storage::SyncStorageEngine;
use serde::{Deserialize, Serialize};
#[cfg(feature = "sqlite-compat")]
use serde_json::json;
use serde_json::Value as JsonValue;
use thiserror::Error;
#[cfg(feature = "native")]
use tracing::debug;
use uuid::Uuid;

#[cfg(feature = "native")]
use futures::executor::block_on;
#[cfg(feature = "native")]
use hnsw_rs::prelude::*;

#[cfg(feature = "sqlite-compat")]
use rusqlite::types::{Value as SqliteValue, ValueRef};
#[cfg(feature = "sqlite-compat")]
use rusqlite::{params_from_iter, Connection, OpenFlags, Transaction};
#[cfg(feature = "sqlite-compat")]
use std::path::PathBuf;
#[cfg(feature = "sqlite-compat")]
use std::time::Duration;

/// Unique identifier for a stored node.
pub type NodeId = String;

/// Logical actor identifier used when merging CRDT updates.
pub type ActorId = String;

// ---------------------------------------------------------------------------
// Auto-embedding trait
// ---------------------------------------------------------------------------

/// Pluggable text-embedding backend.
pub trait EmbedText: Send + Sync + std::fmt::Debug {
    /// Generate embeddings for a batch of text strings.
    fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>>;

    /// Dimensionality of the embeddings produced by this backend.
    fn dimension(&self) -> usize;

    /// Optional model identifier.
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

/// A pending embedding computation task.
#[derive(Debug, Clone)]
pub struct EmbeddingTask {
    pub node_id: NodeId,
    pub extracted_text: String,
    pub model_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Observability snapshot for the embedding background worker.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingWorkerStats {
    pub queue_depth: usize,
    pub last_processed: Option<DateTime<Utc>>,
    pub dropped_tasks: usize,
}

/// A search result from vector similarity search.
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub record: NodeRecord,
    pub score: f32,
}

/// Metadata associated with a persisted node in the CRDT store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeRecord {
    pub id: NodeId,
    pub data: NodeData,
    pub clock: VectorClock,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_score: Option<f32>,
}

impl NodeRecord {
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
            quality_score: None,
        }
    }

    pub fn merge_update(&mut self, actor: impl Into<ActorId>, data: NodeData) {
        let actor = actor.into();
        let counter = self.clock.entry(actor).or_insert(0);
        *counter += 1;
        self.timestamp = Utc::now();
        self.data = data;
    }
}

// ---------------------------------------------------------------------------
// Vector Index — HNSW (native) or BruteForce (WASM)
// ---------------------------------------------------------------------------

#[cfg(feature = "native")]
pub struct VectorIndex {
    hnsw: Hnsw<'static, f32, DistCosine>,
    id_to_idx: DashMap<NodeId, usize>,
    idx_to_id: DashMap<usize, NodeId>,
    next_idx: Mutex<usize>,
    max_elements: usize,
}

#[cfg(feature = "native")]
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<VectorIndex>();
};

#[cfg(feature = "native")]
impl std::fmt::Debug for VectorIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorIndex")
            .field("indexed_nodes", &self.id_to_idx.len())
            .finish()
    }
}

#[cfg(feature = "native")]
impl VectorIndex {
    pub fn new(max_elements: usize) -> Self {
        Self {
            hnsw: Hnsw::new(16, max_elements, 16, 200, DistCosine),
            id_to_idx: DashMap::new(),
            idx_to_id: DashMap::new(),
            next_idx: Mutex::new(0),
            max_elements,
        }
    }

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
        let emb_owned: Vec<f32> = embedding.to_vec();
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.hnsw.insert((&emb_owned, idx));
        }))
        .is_err()
        {
            eprintln!("[VectorIndex] HNSW insert panicked for '{}'; skipping", id);
            self.id_to_idx.remove(id);
            self.idx_to_id.remove(&idx);
        }
    }

    pub fn search(&self, query: &[f32], limit: usize) -> Vec<(NodeId, f32)> {
        if self.id_to_idx.is_empty() {
            return Vec::new();
        }
        let neighbours = self.hnsw.search(query, limit, 16);
        neighbours
            .into_iter()
            .filter_map(|n| {
                let node_id = self.idx_to_id.get(&n.d_id)?.clone();
                let current_idx = self.id_to_idx.get(&*node_id)?;
                if *current_idx != n.d_id {
                    return None;
                }
                let score = (1.0_f32 - n.distance).max(0.0);
                Some((node_id.clone(), score))
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.id_to_idx.is_empty()
    }
}

#[cfg(feature = "native")]
impl Default for VectorIndex {
    fn default() -> Self {
        Self::new(1_024)
    }
}

// ---------------------------------------------------------------------------
// BruteForceVectorIndex — WASM-safe fallback
// ---------------------------------------------------------------------------

#[cfg(not(feature = "native"))]
#[derive(Debug)]
pub struct BruteForceVectorIndex {
    embeddings: DashMap<NodeId, Vec<f32>>,
}

#[cfg(not(feature = "native"))]
impl BruteForceVectorIndex {
    pub fn new(_max_elements: usize) -> Self {
        Self {
            embeddings: DashMap::new(),
        }
    }

    pub fn insert(&self, id: &str, embedding: &[f32]) {
        self.embeddings.insert(id.to_string(), embedding.to_vec());
    }

    pub fn search(&self, query: &[f32], limit: usize) -> Vec<(NodeId, f32)> {
        let query_norm = vec_norm(query);
        if query_norm == 0.0 {
            return Vec::new();
        }
        let mut results: Vec<(NodeId, f32)> = self
            .embeddings
            .iter()
            .filter_map(|entry| {
                let emb = entry.value();
                let emb_norm = vec_norm(emb);
                if emb_norm == 0.0 {
                    return None;
                }
                let dot: f32 = query.iter().zip(emb.iter()).map(|(a, b)| a * b).sum();
                let score = (dot / (query_norm * emb_norm)).max(0.0);
                Some((entry.key().clone(), score))
            })
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        results
    }

    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }
}

#[cfg(not(feature = "native"))]
impl Default for BruteForceVectorIndex {
    fn default() -> Self {
        Self::new(1_024)
    }
}

#[cfg(not(feature = "native"))]
fn vec_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

// ---------------------------------------------------------------------------
// Unified type alias for the vector index
// ---------------------------------------------------------------------------

#[cfg(feature = "native")]
type ActiveVectorIndex = VectorIndex;

#[cfg(not(feature = "native"))]
type ActiveVectorIndex = BruteForceVectorIndex;

/// Errors that can be produced by the CRDT store.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("node not found: {0}")]
    NotFound(NodeId),
}

/// Stable, documented error codes emitted by `pluresdb-core`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreErrorCode {
    NodeNotFound,
    SqliteError,
    InvalidInput,
    SerializationError,
    FeatureDisabled,
}

impl CoreErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NodeNotFound => "CORE_NODE_NOT_FOUND",
            Self::SqliteError => "CORE_SQLITE_ERROR",
            Self::InvalidInput => "CORE_INVALID_INPUT",
            Self::SerializationError => "CORE_SERIALIZATION_ERROR",
            Self::FeatureDisabled => "CORE_FEATURE_DISABLED",
        }
    }
}

impl std::fmt::Display for CoreErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl StoreError {
    pub const fn code(&self) -> CoreErrorCode {
        match self {
            Self::NotFound(_) => CoreErrorCode::NodeNotFound,
        }
    }
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
    vector_index: parking_lot::RwLock<Arc<ActiveVectorIndex>>,
    embedder: Option<Arc<dyn EmbedText>>,
    lm_plugin: Option<Arc<dyn PluresLmPlugin>>,
    #[cfg(feature = "native")]
    persistence: Option<Arc<dyn StorageEngine>>,
    #[cfg(not(feature = "native"))]
    persistence: Option<Arc<dyn SyncStorageEngine>>,
    vector_index_ready: AtomicBool,
    #[cfg(feature = "native")]
    embedding_tx: Option<std::sync::mpsc::SyncSender<EmbeddingTask>>,
    #[cfg(feature = "native")]
    embedding_rx: parking_lot::Mutex<Option<std::sync::mpsc::Receiver<EmbeddingTask>>>,
    embedding_queue_depth: AtomicUsize,
    embedding_last_processed: parking_lot::Mutex<Option<DateTime<Utc>>>,
    embedding_dropped: AtomicUsize,
}

impl std::fmt::Debug for CrdtStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CrdtStore")
            .field("nodes", &self.nodes.len())
            .field("vector_index", &*self.vector_index.read())
            .field("embedder", &self.embedder.is_some())
            .field("lm_plugin", &self.lm_plugin.as_ref().map(|p| p.plugin_id()))
            .finish()
    }
}

impl Default for CrdtStore {
    fn default() -> Self {
        Self {
            nodes: DashMap::new(),
            vector_index: parking_lot::RwLock::new(Arc::new(ActiveVectorIndex::default())),
            embedder: None,
            lm_plugin: None,
            persistence: None,
            vector_index_ready: AtomicBool::new(true),
            #[cfg(feature = "native")]
            embedding_tx: None,
            #[cfg(feature = "native")]
            embedding_rx: parking_lot::Mutex::new(None),
            embedding_queue_depth: AtomicUsize::new(0),
            embedding_last_processed: parking_lot::Mutex::new(None),
            embedding_dropped: AtomicUsize::new(0),
        }
    }
}

impl CrdtStore {
    fn is_recent(timestamp: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        timestamp >= now - chrono::Duration::days(7)
    }

    fn compute_quality_score(record: &NodeRecord, now: DateTime<Utc>) -> f32 {
        let mut score = 0.0_f32;
        let content_len = Self::memory_content_text(record.data.as_object())
            .map(|text| text.chars().count())
            .unwrap_or(0);
        if content_len >= 50 {
            score += 0.2;
        }

        let has_category = record
            .data
            .as_object()
            .and_then(|obj| obj.get("category"))
            .and_then(JsonValue::as_str)
            .map(|category| !category.eq_ignore_ascii_case("conversation"))
            .unwrap_or(false);
        if has_category {
            score += 0.2;
        }

        let has_tags = record
            .data
            .as_object()
            .and_then(|obj| obj.get("tags"))
            .and_then(JsonValue::as_array)
            .map(|tags| !tags.is_empty())
            .unwrap_or(false);
        if has_tags {
            score += 0.2;
        }

        let has_explicit_source = record
            .data
            .as_object()
            .and_then(|obj| obj.get("source"))
            .map(|source| match source {
                JsonValue::Null => false,
                JsonValue::String(text) => !text.trim().is_empty(),
                JsonValue::Array(values) => !values.is_empty(),
                JsonValue::Object(map) => !map.is_empty(),
                _ => true,
            })
            .unwrap_or(false);
        if has_explicit_source {
            score += 0.2;
        }

        if Self::is_recent(record.timestamp, now) {
            score += 0.2;
        }

        score.clamp(0.0, 1.0)
    }

    fn memory_content_text(data: Option<&serde_json::Map<String, JsonValue>>) -> Option<&str> {
        data.and_then(|obj| {
            obj.get("content")
                .and_then(JsonValue::as_str)
                .or_else(|| obj.get("text").and_then(JsonValue::as_str))
        })
    }

    fn ensure_quality_score(&self, mut record: NodeRecord) -> NodeRecord {
        let now = Utc::now();
        let quality = record
            .quality_score
            .filter(|score| score.is_finite())
            .map(|score| score.clamp(0.0, 1.0))
            .unwrap_or_else(|| Self::compute_quality_score(&record, now));
        let needs_update = match record.quality_score {
            Some(existing) if existing.is_finite() => (existing - quality).abs() > f32::EPSILON,
            _ => true,
        };
        if needs_update {
            record.quality_score = Some(quality);
            self.nodes.entry(record.id.clone()).and_modify(|stored| {
                stored.quality_score = Some(quality);
            });
            self.persist_node(&record);
        }
        record
    }

    fn blended_search_score(vector_similarity: f32, quality: f32, is_recent: bool) -> f32 {
        let recency = if is_recent { 1.0 } else { 0.0 };
        (0.7 * vector_similarity.clamp(0.0, 1.0) + 0.2 * quality.clamp(0.0, 1.0) + 0.1 * recency)
            .clamp(0.0, 1.0)
    }

    /// Attach a text-embedding backend to this store.
    #[cfg(feature = "native")]
    pub fn with_embedder(self, embedder: Arc<dyn EmbedText>) -> Self {
        self.with_embedder_capacity(embedder, 1024)
    }

    #[cfg(feature = "native")]
    pub fn with_embedder_capacity(mut self, embedder: Arc<dyn EmbedText>, capacity: usize) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(capacity);
        self.embedder = Some(embedder);
        self.embedding_tx = Some(tx);
        *self.embedding_rx.lock() = Some(rx);
        self
    }

    pub fn embedder(&self) -> Option<&dyn EmbedText> {
        self.embedder.as_deref()
    }

    pub fn with_lm_plugin(mut self, plugin: Arc<dyn PluresLmPlugin>) -> Self {
        self.lm_plugin = Some(plugin);
        self
    }

    pub fn lm_plugin_id(&self) -> Option<&str> {
        self.lm_plugin.as_ref().map(|p| p.plugin_id())
    }

    pub fn build_vector_index(&self) -> usize {
        let expected_dim = self.embedder.as_ref().map(|e| e.dimension());
        let mut indexed = 0usize;
        for entry in self.nodes.iter() {
            if let Some(emb) = &entry.value().embedding {
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
                self.vector_index.read().insert(entry.key(), emb);
                indexed += 1;
            }
        }
        if self.persistence.is_some() {
            self.build_vector_index_from_persistence();
        }
        self.vector_index_ready.store(true, Ordering::Release);
        eprintln!("[CrdtStore] Built vector index: {} entries", indexed);
        indexed
    }

    fn build_vector_index_from_persistence(&self) {
        let storage = match &self.persistence {
            Some(s) => s,
            None => return,
        };
        let expected_dim = self.embedder.as_ref().map(|e| e.dimension());

        // First pass: count valid embeddings to right-size the HNSW index.
        let mut embedding_count = 0usize;
        let count_result = Self::storage_for_each(storage.as_ref(), &mut |stored: StoredNode| {
            let record = match serde_json::from_value::<NodeRecord>(stored.payload) {
                Ok(r) => r,
                Err(_) => return true,
            };
            if let Some(emb) = &record.embedding {
                if let Some(dim) = expected_dim {
                    if emb.len() != dim { return true; }
                }
                if !emb.is_empty() && emb.iter().all(|v| v.is_finite()) && emb.iter().any(|v| *v != 0.0) {
                    embedding_count += 1;
                }
            }
            true
        });
        if let Err(e) = count_result {
            tracing::error!("[CrdtStore] counting embeddings from persistence failed: {}", e);
            return;
        }
        if embedding_count == 0 {
            tracing::debug!("[CrdtStore] No embeddings found in persistence; skipping index build");
            return;
        }

        // Right-size: 2x actual count, minimum 1024.
        let capacity = (embedding_count * 2).max(1024);
        let new_index = Arc::new(ActiveVectorIndex::new(capacity));
        tracing::info!("[CrdtStore] Building vector index: {} embeddings, capacity {}", embedding_count, capacity);

        // Second pass: stream and insert embeddings.
        let idx = new_index.clone();
        let dim = expected_dim;
        let insert_result = Self::storage_for_each(storage.as_ref(), &mut |stored: StoredNode| {
            let record = match serde_json::from_value::<NodeRecord>(stored.payload) {
                Ok(r) => r,
                Err(_) => return true,
            };
            if let Some(emb) = &record.embedding {
                if let Some(d) = dim {
                    if emb.len() != d { return true; }
                }
                if !emb.is_empty() && emb.iter().all(|v| v.is_finite()) && emb.iter().any(|v| *v != 0.0) {
                    idx.insert(&record.id, emb);
                }
            }
            true
        });
        if let Err(e) = insert_result {
            tracing::error!("[CrdtStore] build_vector_index_from_persistence failed: {}", e);
            return;
        }

        // Swap in the right-sized index.
        *self.vector_index.write() = new_index;
        tracing::info!("[CrdtStore] Loaded {} embeddings into right-sized vector index (capacity {})", embedding_count, capacity);
    }

    #[cfg(feature = "native")]
    pub fn with_persistence(mut self, storage: Arc<dyn StorageEngine>) -> Self {
        self.persistence = Some(storage);
        self.vector_index_ready.store(false, Ordering::Release);
        self
    }

    #[cfg(not(feature = "native"))]
    pub fn with_persistence(mut self, storage: Arc<dyn SyncStorageEngine>) -> Self {
        self.persistence = Some(storage);
        self.vector_index_ready.store(false, Ordering::Release);
        self
    }

    // Remove the with_persistence_async method entirely since we don't need it.

    // -- Storage abstraction helpers (native uses block_on, WASM uses sync) --

    #[cfg(feature = "native")]
    fn storage_put(storage: &dyn StorageEngine, node: StoredNode) -> anyhow::Result<()> {
        block_on(storage.put(node))
    }

    #[cfg(not(feature = "native"))]
    fn storage_put(storage: &dyn SyncStorageEngine, node: StoredNode) -> anyhow::Result<()> {
        storage.put(node)
    }

    #[cfg(feature = "native")]
    fn storage_get(storage: &dyn StorageEngine, id: &str) -> anyhow::Result<Option<StoredNode>> {
        block_on(storage.get(id))
    }

    #[cfg(not(feature = "native"))]
    fn storage_get(
        storage: &dyn SyncStorageEngine,
        id: &str,
    ) -> anyhow::Result<Option<StoredNode>> {
        storage.get(id)
    }

    #[cfg(feature = "native")]
    fn storage_delete(storage: &dyn StorageEngine, id: &str) -> anyhow::Result<()> {
        block_on(storage.delete(id))
    }

    #[cfg(not(feature = "native"))]
    fn storage_delete(storage: &dyn SyncStorageEngine, id: &str) -> anyhow::Result<()> {
        storage.delete(id)
    }

    #[cfg(feature = "native")]
    fn storage_list(storage: &dyn StorageEngine) -> anyhow::Result<Vec<StoredNode>> {
        block_on(storage.list())
    }

    #[cfg(not(feature = "native"))]
    fn storage_list(storage: &dyn SyncStorageEngine) -> anyhow::Result<Vec<StoredNode>> {
        storage.list()
    }

    #[cfg(feature = "native")]
    fn storage_for_each(storage: &dyn StorageEngine, f: &mut (dyn FnMut(StoredNode) -> bool + Send)) -> anyhow::Result<()> {
        block_on(storage.for_each(f))
    }

    #[cfg(not(feature = "native"))]
    fn storage_for_each(storage: &dyn SyncStorageEngine, f: &mut (dyn FnMut(StoredNode) -> bool + Send)) -> anyhow::Result<()> {
        storage.for_each(f)
    }

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
            if let Err(e) = Self::storage_put(storage.as_ref(), stored) {
                tracing::error!("[CrdtStore] persist failed for {}: {}", record.id, e);
            }
        }
    }

    fn unpersist_node(&self, id: &str) -> bool {
        if let Some(storage) = &self.persistence {
            let exists = Self::storage_get(storage.as_ref(), id)
                .ok()
                .flatten()
                .is_some();
            if exists {
                if let Err(e) = Self::storage_delete(storage.as_ref(), id) {
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

    fn get_from_persistence(&self, id: &str) -> Option<NodeRecord> {
        let storage = self.persistence.as_ref()?;
        let stored = Self::storage_get(storage.as_ref(), id).ok()??;
        serde_json::from_value::<NodeRecord>(stored.payload).ok()
    }

    pub fn put(&self, id: impl Into<NodeId>, actor: impl Into<ActorId>, data: NodeData) -> NodeId {
        let id = id.into();
        let actor = actor.into();
        self.nodes
            .entry(id.clone())
            .and_modify(|record| record.merge_update(actor.clone(), data.clone()))
            .or_insert_with(|| NodeRecord::new(id.clone(), actor, data.clone()));
        if let Some(entry) = self.nodes.get(&id) {
            self.persist_node(entry.value());
        }
        // Enqueue embedding task (native only).
        #[cfg(feature = "native")]
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
        if let Some(plugin) = &self.lm_plugin {
            plugin.on_node_written(&id, &data);
        }
        id
    }

    pub fn put_with_embedding(
        &self,
        id: impl Into<NodeId>,
        actor: impl Into<ActorId>,
        data: NodeData,
        embedding: Vec<f32>,
    ) -> NodeId {
        let id = id.into();
        let actor = actor.into();
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
            self.vector_index.read().insert(&id, &emb_clone);
        }
        if let Some(entry) = self.nodes.get(&id) {
            self.persist_node(entry.value());
        }
        if let Some(plugin) = &self.lm_plugin {
            plugin.on_node_written(&id, &data);
        }
        id
    }

    #[cfg(feature = "native")]
    fn set_embedding_for_node(&self, node_id: &str, embedding: Vec<f32>) {
        let emb_valid = !embedding.is_empty()
            && embedding.iter().all(|v| v.is_finite())
            && embedding.iter().any(|v| *v != 0.0);
        self.nodes
            .entry(node_id.to_string())
            .and_modify(|record| record.embedding = Some(embedding.clone()));
        if emb_valid {
            self.vector_index.read().insert(node_id, &embedding);
        }
        if let Some(entry) = self.nodes.get(node_id) {
            self.persist_node(entry.value());
        }
    }

    pub fn delete(&self, id: impl AsRef<str>) -> Result<(), StoreError> {
        let id_ref = id.as_ref();
        let in_sqlite = self.unpersist_node(id_ref);
        let in_memory = self.nodes.remove(id_ref).is_some();
        if in_memory || in_sqlite {
            if let Some(plugin) = &self.lm_plugin {
                plugin.on_node_deleted(&id_ref.to_owned());
            }
            Ok(())
        } else {
            Err(StoreError::NotFound(id_ref.to_owned()))
        }
    }

    pub fn get(&self, id: impl AsRef<str>) -> Option<NodeRecord> {
        let id = id.as_ref();
        if let Some(entry) = self.nodes.get(id) {
            let record = entry.value().clone();
            drop(entry);
            return Some(self.ensure_quality_score(record));
        }
        self.get_from_persistence(id)
            .map(|record| self.ensure_quality_score(record))
    }

    pub fn list(&self) -> Vec<NodeRecord> {
        if let Some(storage) = &self.persistence {
            match Self::storage_list(storage.as_ref()) {
                Ok(nodes) => {
                    return nodes
                        .into_iter()
                        .filter_map(|stored| {
                            let record =
                                serde_json::from_value::<NodeRecord>(stored.payload).ok()?;
                            if let Some(entry) = self.nodes.get(&record.id) {
                                Some(entry.value().clone())
                            } else {
                                Some(record)
                            }
                        })
                        .collect();
                }
                Err(e) => {
                    tracing::error!("[CrdtStore] list from storage failed: {}", e);
                }
            }
        }
        self.nodes
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Iterate over all nodes via a callback without collecting into a Vec.
    ///
    /// In-memory entries shadow stored counterparts.  Return `false` to stop.
    pub fn for_each_sync(&self, f: &mut (dyn FnMut(&NodeRecord) -> bool + Send)) {
        if let Some(storage) = &self.persistence {
            let mut seen = std::collections::HashSet::new();
            for entry in self.nodes.iter() {
                seen.insert(entry.key().clone());
                if !f(entry.value()) { return; }
            }
            let _ = Self::storage_for_each(storage.as_ref(), &mut |stored: StoredNode| {
                let record = match serde_json::from_value::<NodeRecord>(stored.payload) {
                    Ok(r) => r,
                    Err(_) => return true,
                };
                if seen.contains(&record.id) { return true; }
                f(&record)
            });
            return;
        }
        for entry in self.nodes.iter() {
            if !f(entry.value()) { break; }
        }
    }

    pub fn apply(&self, op: CrdtOperation) -> Result<Option<NodeId>, StoreError> {
        match op {
            CrdtOperation::Put { id, actor, data } => Ok(Some(self.put(id, actor, data))),
            CrdtOperation::Delete { id } => {
                self.delete(&id)?;
                Ok(None)
            }
        }
    }

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

    #[cfg(feature = "native")]
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
                    store_w
                        .embedding_queue_depth
                        .fetch_sub(1, Ordering::Relaxed);
                    if let Some(embedder) = &store_w.embedder {
                        if let Ok(mut batch) = embedder.embed(&[task.extracted_text.as_str()]) {
                            if let Some(embedding) = batch.pop() {
                                store_w.set_embedding_for_node(&task.node_id, embedding);
                            }
                        }
                    }
                    *store_w.embedding_last_processed.lock() = Some(Utc::now());
                }
            })
            .expect("failed to spawn embedding worker thread")
    }

    pub fn embedding_worker_stats(&self) -> EmbeddingWorkerStats {
        EmbeddingWorkerStats {
            queue_depth: self.embedding_queue_depth.load(Ordering::Relaxed),
            last_processed: *self.embedding_last_processed.lock(),
            dropped_tasks: self.embedding_dropped.load(Ordering::Relaxed),
        }
    }

    pub fn vector_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        mut min_score: f32,
    ) -> Vec<VectorSearchResult> {
        if query_embedding.is_empty() {
            return Vec::new();
        }
        if query_embedding.iter().any(|v| !v.is_finite()) {
            return Vec::new();
        }

        if !min_score.is_finite() {
            min_score = 0.0;
        } else {
            min_score = min_score.clamp(0.0, 1.0);
        }

        if !self.vector_index_ready.load(Ordering::Acquire) {
            self.build_vector_index_from_persistence();
            self.vector_index_ready.store(true, Ordering::Release);
        }

        let candidates = self.vector_index.read().search(query_embedding, limit);
        let now = Utc::now();
        let mut results: Vec<VectorSearchResult> = candidates
            .into_iter()
            .filter_map(|(id, vector_similarity)| {
                let record = if let Some(entry) = self.nodes.get(&id) {
                    let record = entry.value().clone();
                    drop(entry);
                    record
                } else {
                    self.get_from_persistence(&id)?
                };
                let record = self.ensure_quality_score(record);
                let quality = record.quality_score.unwrap_or(0.0);
                let blended_score = Self::blended_search_score(
                    vector_similarity,
                    quality,
                    Self::is_recent(record.timestamp, now),
                );
                if blended_score < min_score {
                    return None;
                }
                Some(VectorSearchResult {
                    record,
                    score: blended_score,
                })
            })
            .collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }
}

// ---------------------------------------------------------------------------
// SQLite compatibility layer (feature-gated)
// ---------------------------------------------------------------------------

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

#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone, PartialEq)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<SqlValue>>,
    pub changes: u64,
    pub last_insert_rowid: i64,
}

#[cfg(feature = "sqlite-compat")]
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

#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    pub changes: u64,
    pub last_insert_rowid: i64,
}

#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone, PartialEq)]
pub enum DatabasePath {
    InMemory,
    File(PathBuf),
}

#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone)]
pub struct DatabaseOptions {
    pub path: DatabasePath,
    pub read_only: bool,
    pub create_if_missing: bool,
    pub apply_default_pragmas: bool,
    pub custom_pragmas: Vec<(String, String)>,
    pub busy_timeout: Option<Duration>,
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

    pub fn with_embedding_model(mut self, model_id: impl Into<String>) -> Self {
        self.embedding_model = Some(model_id.into());
        self
    }
}

#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    path: DatabasePath,
}

#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

#[cfg(feature = "sqlite-compat")]
impl DatabaseError {
    pub const fn code(&self) -> CoreErrorCode {
        match self {
            Self::Sqlite(_) => CoreErrorCode::SqliteError,
        }
    }
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

#[cfg(feature = "sqlite-compat")]
#[derive(Debug, Clone)]
pub struct Statement {
    database: Database,
    sql: String,
}

#[cfg(feature = "sqlite-compat")]
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
fn read_row(
    row: &rusqlite::Row<'_>,
    column_count: usize,
) -> Result<Vec<SqlValue>, rusqlite::Error> {
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

#[cfg(feature = "native")]
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
// FastEmbedder (feature-gated)
// ---------------------------------------------------------------------------

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

    pub fn model_id(&self) -> &str {
        &self.model_id
    }
}

#[cfg(feature = "embeddings")]
impl EmbedText for FastEmbedder {
    fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        let owned: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
        let mut model = self
            .model
            .lock()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {}", e))?;
        model.embed(owned, None)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_id(&self) -> Option<&str> {
        Some(&self.model_id)
    }
}

#[cfg(feature = "embeddings")]
fn model_id_to_fastembed(model_id: &str) -> anyhow::Result<(fastembed::EmbeddingModel, usize)> {
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
    #[cfg(feature = "native")]
    use pluresdb_storage::StorageEngine;
    #[cfg(not(feature = "native"))]
    use pluresdb_storage::SyncStorageEngine;

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

        store.put("a", "actor", serde_json::json!({}));
        store.put("b", "actor", serde_json::json!({}));
        assert_eq!(written.load(Ordering::Relaxed), 2);

        store.put_with_embedding(
            "c",
            "actor",
            serde_json::json!({"label": "c"}),
            vec![1.0, 0.0, 0.0],
        );
        assert_eq!(
            written.load(Ordering::Relaxed),
            3,
            "put_with_embedding must also fire on_node_written"
        );

        store.delete("a").unwrap();
        assert_eq!(deleted.load(Ordering::Relaxed), 1);

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

        let emb_a: Vec<f32> = vec![1.0, 0.0, 0.0];
        let emb_b: Vec<f32> = vec![0.0, 1.0, 0.0];
        let emb_c: Vec<f32> = vec![0.0, 0.0, 1.0];

        store.put_with_embedding(
            "a",
            "actor-v",
            serde_json::json!({"label":"a"}),
            emb_a.clone(),
        );
        store.put_with_embedding("b", "actor-v", serde_json::json!({"label":"b"}), emb_b);
        store.put_with_embedding("c", "actor-v", serde_json::json!({"label":"c"}), emb_c);

        let record = store.get("a").expect("node a should exist");
        assert_eq!(record.embedding, Some(emb_a.clone()));

        let results = store.vector_search(&emb_a, 3, 0.0);
        assert!(!results.is_empty(), "should find at least one result");
        assert_eq!(results[0].record.id, "a");
        assert!(
            results[0].score > 0.83,
            "identical vector with blend should have score above 0.83, got {}",
            results[0].score
        );
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

        let results = store.vector_search(&emb_a, 10, 0.8);
        assert_eq!(results.len(), 1, "only 'a' should pass the 0.8 threshold");
        assert_eq!(results[0].record.id, "a");
    }

    #[test]
    fn vector_search_blends_quality_into_ranking() {
        let store = CrdtStore::default();
        let query: Vec<f32> = vec![1.0, 0.0, 0.0];

        store.put_with_embedding(
            "low-quality-high-sim",
            "actor-v",
            serde_json::json!({
                "content": "short",
                "category": "conversation",
                "tags": [],
            }),
            query.clone(),
        );
        store.put_with_embedding(
            "high-quality-lower-sim",
            "actor-v",
            serde_json::json!({
                "content": "This memory captures a specific architecture decision, rationale, and implementation notes for a production migration.",
                "category": "decision",
                "tags": ["architecture", "migration"],
                "source": "design-doc-42"
            }),
            vec![0.8, 0.6, 0.0],
        );

        let results = store.vector_search(&query, 2, 0.0);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].record.id, "high-quality-lower-sim");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn get_computes_quality_score_for_existing_persisted_node_on_first_access() {
        let (store, storage) = make_storage_store();
        store.put_with_embedding(
            "quality-backfill",
            "actor",
            serde_json::json!({
                "content": "This memory contains enough detail and metadata to receive a quality score.",
                "category": "decision",
                "tags": ["test"],
                "source": "manual"
            }),
            vec![1.0, 0.0, 0.0],
        );

        let store2 = CrdtStore::default().with_persistence(wrap_mem_storage(storage.clone()));
        let record = store2
            .get("quality-backfill")
            .expect("persisted node should be accessible");
        assert!(
            record.quality_score.is_some(),
            "first access should backfill quality_score"
        );

        let store3 = CrdtStore::default().with_persistence(wrap_mem_storage(storage));
        let persisted_again = store3
            .get("quality-backfill")
            .expect("backfilled node should still be accessible");
        assert!(persisted_again.quality_score.is_some());
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

        let results = store.vector_search(&emb_v2, 3, 0.0);
        assert!(!results.is_empty());
        assert_eq!(results[0].record.id, "node");
    }

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
        let data = serde_json::json!({"count": 42, "value": 3.15});
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

    // Auto-embedding tests (native only — require threading)
    #[cfg(feature = "native")]
    mod embedding_tests {
        use super::*;

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

        fn wait_for_embedding(store: &CrdtStore, node_id: &str, attempts: usize) -> NodeRecord {
            (0..attempts)
                .find_map(|_| {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    let r = store.get(node_id).expect("node should still exist");
                    r.embedding.is_some().then_some(r)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "embedding for '{}' not computed within ~{} ms",
                        node_id,
                        attempts * 10
                    )
                })
        }

        #[test]
        fn put_auto_embeds_when_embedder_attached() {
            let store = Arc::new(CrdtStore::default().with_embedder(Arc::new(MockEmbedder)));
            CrdtStore::spawn_embedding_worker(Arc::clone(&store));

            store.put("n1", "actor", serde_json::json!({"content": "hello"}));
            let immediate = store.get("n1").expect("node should exist right away");
            assert_eq!(immediate.id, "n1");

            let record = wait_for_embedding(&store, "n1", 100);
            let emb = record.embedding.as_ref().unwrap();
            let results = store.vector_search(emb, 5, 0.0);
            assert!(!results.is_empty());
            assert_eq!(results[0].record.id, "n1");
        }

        #[test]
        fn put_without_embedder_stores_no_embedding() {
            let store = CrdtStore::default();
            store.put("n2", "actor", serde_json::json!({"content": "hello"}));
            let record = store.get("n2").expect("node should exist");
            assert!(record.embedding.is_none());
        }

        #[test]
        fn put_skips_embedding_for_numeric_data() {
            let store = Arc::new(CrdtStore::default().with_embedder(Arc::new(MockEmbedder)));
            CrdtStore::spawn_embedding_worker(Arc::clone(&store));
            store.put("n3", "actor", serde_json::json!({"value": 99}));
            let record = store.get("n3").expect("node should exist");
            assert!(record.embedding.is_none());
            assert_eq!(store.embedding_worker_stats().queue_depth, 0);
        }

        #[test]
        fn put_node_stored_immediately_before_worker_runs() {
            let store = Arc::new(CrdtStore::default().with_embedder(Arc::new(MockEmbedder)));
            store.put(
                "n-pre",
                "actor",
                serde_json::json!({"content": "test content"}),
            );

            let record = store.get("n-pre").expect("node should exist immediately");
            assert!(record.embedding.is_none());
            assert_eq!(store.embedding_worker_stats().queue_depth, 1);
        }

        #[test]
        fn embedding_worker_stats_dropped_tasks() {
            let store =
                Arc::new(CrdtStore::default().with_embedder_capacity(Arc::new(MockEmbedder), 0));

            store.put("d1", "actor", serde_json::json!({"content": "drop me"}));

            let stats = store.embedding_worker_stats();
            assert_eq!(stats.dropped_tasks, 1);
            assert_eq!(stats.queue_depth, 0);
        }

        #[test]
        fn embedding_worker_stats_last_processed_updated() {
            let store = Arc::new(CrdtStore::default().with_embedder(Arc::new(MockEmbedder)));
            CrdtStore::spawn_embedding_worker(Arc::clone(&store));

            assert!(store.embedding_worker_stats().last_processed.is_none());

            store.put("ts1", "actor", serde_json::json!({"content": "hello"}));
            wait_for_embedding(&store, "ts1", 100);

            let processed = store
                .embedding_worker_stats()
                .last_processed
                .expect("last_processed should be set");
            assert!(processed <= Utc::now());
        }
    }

    // Storage engine persistence tests
    fn make_storage_store() -> (CrdtStore, Arc<MemoryStorage>) {
        let storage = Arc::new(MemoryStorage::default());
        let store = CrdtStore::default().with_persistence(wrap_mem_storage(storage.clone()));
        (store, storage)
    }

    #[cfg(feature = "native")]
    fn wrap_mem_storage(s: Arc<MemoryStorage>) -> Arc<dyn StorageEngine> {
        s as Arc<dyn StorageEngine>
    }

    #[cfg(not(feature = "native"))]
    fn wrap_mem_storage(s: Arc<MemoryStorage>) -> Arc<dyn SyncStorageEngine> {
        s as Arc<dyn SyncStorageEngine>
    }

    #[test]
    fn with_storage_does_not_hydrate_into_memory() {
        let storage = Arc::new(MemoryStorage::default());

        let pre_record = NodeRecord::new(
            "node-pre".to_string(),
            "actor",
            serde_json::json!({"hello": "from-storage"}),
        );
        pluresdb_storage::SyncStorageEngine::put(
            storage.as_ref(),
            StoredNode {
                id: "node-pre".to_string(),
                payload: serde_json::to_value(&pre_record).unwrap(),
            },
        )
        .expect("pre-populate storage");

        let store = CrdtStore::default().with_persistence(wrap_mem_storage(storage.clone()));

        assert!(store.nodes.is_empty());

        let record = store
            .get("node-pre")
            .expect("get should fall back to storage");
        assert_eq!(record.data["hello"], "from-storage");
    }

    #[test]
    fn get_falls_back_to_storage_for_persisted_node() {
        let (store, storage) = make_storage_store();
        store.put("p1", "actor", serde_json::json!({"v": 1}));

        let store2 = CrdtStore::default().with_persistence(wrap_mem_storage(storage));
        assert!(store2.nodes.is_empty());

        let record = store2.get("p1").expect("should find node via storage");
        assert_eq!(record.data["v"], 1);
    }

    #[test]
    fn list_queries_storage_directly() {
        let (store, storage) = make_storage_store();
        store.put("list-a", "actor", serde_json::json!({"n": "a"}));
        store.put("list-b", "actor", serde_json::json!({"n": "b"}));

        let store2 = CrdtStore::default().with_persistence(wrap_mem_storage(storage));
        let records = store2.list();
        assert_eq!(records.len(), 2);
        let ids: Vec<&str> = records.iter().map(|r| r.id.as_str()).collect();
        assert!(ids.contains(&"list-a"));
        assert!(ids.contains(&"list-b"));
    }

    #[test]
    fn delete_works_for_storage_only_node() {
        let (store, storage) = make_storage_store();
        store.put("del-node", "actor", serde_json::json!({"x": 1}));

        let store2 = CrdtStore::default().with_persistence(wrap_mem_storage(storage));
        assert!(store2.nodes.is_empty());
        assert!(store2.get("del-node").is_some());

        store2
            .delete("del-node")
            .expect("delete should succeed for storage-only node");
        assert!(store2.get("del-node").is_none());
    }

    #[test]
    fn delete_returns_not_found_for_nonexistent_with_storage() {
        let (store, _storage) = make_storage_store();
        let err = store
            .delete("ghost-node")
            .expect_err("should error for missing node");
        assert!(matches!(err, StoreError::NotFound(_)));
        assert_eq!(err.code(), CoreErrorCode::NodeNotFound);
    }

    #[test]
    fn vector_search_builds_index_lazily_from_storage() {
        let (store, storage) = make_storage_store();

        let emb_a: Vec<f32> = vec![1.0, 0.0, 0.0];
        let emb_b: Vec<f32> = vec![0.0, 1.0, 0.0];

        store.put_with_embedding(
            "vs-a",
            "actor",
            serde_json::json!({"label":"a"}),
            emb_a.clone(),
        );
        store.put_with_embedding("vs-b", "actor", serde_json::json!({"label":"b"}), emb_b);

        let store2 = CrdtStore::default().with_persistence(wrap_mem_storage(storage));
        assert!(store2.nodes.is_empty());

        let results = store2.vector_search(&emb_a, 3, 0.0);
        assert!(!results.is_empty());
        assert_eq!(results[0].record.id, "vs-a");
        assert!(results[0].score > 0.83);
    }

    #[cfg(feature = "sqlite-compat")]
    mod sqlite_compat_tests {
        use super::*;

        #[test]
        fn database_exec_and_query() {
            let db = Database::open(DatabaseOptions::default()).expect("open database");
            db.exec(
                "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL)",
            )
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
            let db =
                Database::open(DatabaseOptions::with_file(temp.path())).expect("open database");
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
                    assert_eq!(
                        inner.sqlite_error_code(),
                        Some(ErrorCode::ConstraintViolation)
                    );
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
            let opts = DatabaseOptions::default().with_embedding_model("BAAI/bge-small-en-v1.5");
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

        #[test]
        fn database_error_exposes_stable_code() {
            let err = Database::open(DatabaseOptions::with_file(
                "/definitely/missing/dir/db.sqlite",
            ))
            .expect_err("opening file in missing parent directory should fail");
            assert_eq!(err.code(), CoreErrorCode::SqliteError);
        }
    }
}
