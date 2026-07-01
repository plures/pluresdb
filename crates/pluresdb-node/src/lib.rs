//! Node.js bindings for PluresDB.
//!
//! This crate provides Node.js bindings using N-API to expose
//! PluresDB functionality to Node.js applications.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use parking_lot::Mutex;
use pluresdb_core::{CoreErrorCode, CrdtStore, NodeRecord, StoreError};
use pluresdb_procedures::agens::{AgensEvent, AgensRuntime};
use pluresdb_procedures::engine::ProcedureEngine;
use pluresdb_px::db::procedures as px_procedures;
use pluresdb_px::db::schema::{
    AgentContext as PxAgentContext, Condition as PxCondition, Constraint as PxConstraint,
    Severity as PxSeverity,
};
use pluresdb_px::db::seed::default_store as px_default_store;
use pluresdb_px::db::store::PraxisStore;
use pluresdb_px::px::parse as px_parse;
use pluresdb_px::px::px_ast::{ConstraintDecl as PxAstConstraintDecl, Severity as PxAstSeverity};
use pluresdb_px::px::{expr_to_string as px_expr_to_string, Statement as PxStatement};
use pluresdb_storage::{SledStorage, StorageEngine, StorageErrorCode};
use pluresdb_sync::{SyncBroadcaster, SyncErrorCode, SyncEvent};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "sqlite-compat")]
use pluresdb_core::{Database, DatabaseOptions, SqlValue};

fn node_error(code: &str, message: impl Into<String>) -> Error {
    Error::from_reason(format!("[{}] {}", code, message.into()))
}

fn map_node_error<E: std::fmt::Display>(code: &str, error: E) -> Error {
    node_error(code, error.to_string())
}

fn map_store_error(error: StoreError) -> Error {
    node_error(error.code().as_str(), error.to_string())
}

// ---------------------------------------------------------------------------
// Praxis constraints on the CrdtStore (single source of truth)
//
// TASK-PX-CANON Stage 2 / ADR-0017 Stage B6: there is exactly ONE system of
// record for praxis constraints — the `CrdtStore`. Constraints are persisted as
// ordinary PluresDB nodes shaped `{ "type": PRAXIS_CONSTRAINT_TYPE, "constraint":
// <Constraint JSON> }`, keyed by the constraint id. No constraint state lives in
// a side in-memory `PraxisStore`; that struct is used only as an *ephemeral*
// read-model projected fresh from the CrdtStore for each evaluate/on_action.
// ---------------------------------------------------------------------------

/// `data["type"]` discriminator marking a CrdtStore node as a praxis constraint.
const PRAXIS_CONSTRAINT_TYPE: &str = "praxis_constraint";

/// Build the `NodeData` JSON for persisting `constraint` as a CrdtStore node.
fn constraint_node_data(constraint: &PxConstraint) -> Result<serde_json::Value> {
    let c = serde_json::to_value(constraint)
        .map_err(|e| map_node_error(CoreErrorCode::SerializationError.as_str(), e))?;
    Ok(serde_json::json!({ "type": PRAXIS_CONSTRAINT_TYPE, "constraint": c }))
}

/// Decode a CrdtStore node's `data` back into a [`PxConstraint`], if it is a
/// praxis-constraint node. Returns `None` for any other node type / shape.
fn constraint_from_node_data(data: &serde_json::Value) -> Option<PxConstraint> {
    if data.get("type").and_then(|v| v.as_str()) != Some(PRAXIS_CONSTRAINT_TYPE) {
        return None;
    }
    serde_json::from_value::<PxConstraint>(data.get("constraint")?.clone()).ok()
}

/// Project an ephemeral [`PraxisStore`] read-model from the praxis-constraint
/// nodes currently persisted in the `CrdtStore`. This is the ONLY way the
/// evaluate/on_action procedures see constraints — the CrdtStore is the source
/// of truth, the `PraxisStore` is a throwaway view rebuilt on each call.
fn project_praxis_store(store: &Arc<Mutex<CrdtStore>>) -> PraxisStore {
    let records = {
        let store = store.lock();
        store.list()
    };
    let mut pstore = PraxisStore::new();
    for record in records {
        if let Some(constraint) = constraint_from_node_data(&record.data) {
            pstore.upsert_constraint(constraint);
        }
    }
    pstore
}

/// Persist (insert-or-update) a single [`PxConstraint`] as a CrdtStore node and
/// broadcast the upsert. The constraint id is the node id, so re-persisting the
/// same id updates the same node (CRDT merge), keeping one record per id.
fn persist_constraint(
    store: &Arc<Mutex<CrdtStore>>,
    broadcaster: &Arc<SyncBroadcaster>,
    actor_id: &str,
    constraint: &PxConstraint,
) -> Result<()> {
    let data = constraint_node_data(constraint)?;
    let node_id = {
        let store = store.lock();
        store.put(constraint.id.clone(), actor_id.to_string(), data)
    };
    broadcaster
        .publish(SyncEvent::NodeUpsert { id: node_id })
        .map_err(|e| map_node_error(SyncErrorCode::BroadcastPublishFailed.as_str(), e))?;
    Ok(())
}

/// Map a parsed `.px` `constraint` block (string `when:`/`require:` exprs +
/// declared `severity:`) into an enforcing schema [`Constraint`].
///
/// The `require:` expression is compiled through the canonical Stage-1
/// `compile_nl` path (the single source-of-truth expression grammar), yielding
/// a real [`Condition`] AST — so `require: amount <= 100` actually blocks
/// `amount = 500`. The block's declared `severity:` overrides the keyword-
/// inferred severity. The block `name` becomes the constraint `id`.
fn px_constraint_to_schema(pc: &PxAstConstraintDecl) -> PxConstraint {
    // px-ast carries `require`/`when` as typed `Expr`. Render each back to its
    // canonical source string via the crate's single expr renderer (the same
    // one the executor's condition parser round-trips), then compile through the
    // real grammar-backed `compile_nl` path so `require: amount <= 100` actually
    // blocks `amount = 500`.
    let require_src = pc.require.as_ref().map(px_expr_to_string).unwrap_or_default();
    let mut constraint = px_procedures::compile_nl(&require_src, pc.name.name.clone());

    // Honor the explicitly declared severity from the .px source (typed enum).
    constraint.severity = match pc.severity {
        PxAstSeverity::Error => PxSeverity::Error,
        // `Warning`/`Info` are non-blocking; schema has no `info` variant.
        _ => PxSeverity::Warning,
    };

    // If the block declared a `when:` predicate, compile it too and use it as
    // the precondition (the `require`-derived `when` from compile_nl, typically
    // `Always`, is replaced). A non-parseable `when:` falls back to `Always`.
    if let Some(when_expr) = pc.when.as_ref() {
        let when_src = px_expr_to_string(when_expr);
        let when_constraint = px_procedures::compile_nl(&when_src, pc.name.name.clone());
        // A `when:` clause is semantically "only check when this predicate
        // holds"; reuse its compiled invariant as the precondition.
        if !matches!(when_constraint.require, PxCondition::Always) {
            constraint.when = when_constraint.require;
        }
    }

    if let Some(msg) = &pc.message {
        constraint.description = msg.value.clone();
    }
    constraint
}

/// Seed the built-in praxis constraints into the `CrdtStore` exactly once.
///
/// Reads the canonical default constraint set from [`px_default_store`] and
/// writes any not-yet-present constraint as a CrdtStore node. Idempotent: a
/// constraint id that already exists as a node is left untouched (so a
/// persisted DB that already carries the seed — or runtime edits to a seeded id
/// — is not clobbered on reopen).
fn seed_praxis_into_crdt(store: &Arc<Mutex<CrdtStore>>, actor_id: &str) {
    let defaults = px_default_store();
    let store = store.lock();
    for constraint in defaults.constraints() {
        if store.get(&constraint.id).is_none() {
            if let Ok(data) = constraint_node_data(constraint) {
                store.put(constraint.id.clone(), actor_id.to_string(), data);
            }
        }
    }
}

/// PluresDB database instance for Node.js
#[napi]
pub struct PluresDatabase {
    store: Arc<Mutex<CrdtStore>>,
    /// Storage engine for persistence (SledStorage when `db_path` is provided,
    /// or `None` when no persistence is attached).
    #[allow(dead_code)]
    storage: Option<Arc<dyn StorageEngine>>,
    #[cfg(feature = "sqlite-compat")]
    db: Option<Arc<Database>>,
    broadcaster: Arc<SyncBroadcaster>,
    actor_id: String,
}

#[napi]
impl PluresDatabase {
    /// Create a new PluresDB instance
    #[napi(constructor)]
    pub fn new(actor_id: Option<String>, db_path: Option<String>) -> Result<Self> {
        let actor_id = actor_id.unwrap_or_else(|| "node-actor".to_string());

        let (store, storage) = if let Some(path) = &db_path {
            let sled_storage = Arc::new(
                SledStorage::open(path)
                    .map_err(|e| map_node_error(StorageErrorCode::OpenFailed.as_str(), e))?,
            );
            let store = CrdtStore::default()
                .with_persistence(sled_storage.clone() as Arc<dyn StorageEngine>);
            (store, Some(sled_storage as Arc<dyn StorageEngine>))
        } else {
            (CrdtStore::default(), None)
        };

        #[cfg(feature = "sqlite-compat")]
        let db = if let Some(path) = db_path {
            let options = DatabaseOptions::with_file(path).create_if_missing(true);
            Some(Arc::new(Database::open(options).map_err(|e| {
                map_node_error(CoreErrorCode::SqliteError.as_str(), e)
            })?))
        } else {
            None
        };

        let core_store = Arc::new(Mutex::new(store));
        // Seed the built-in praxis constraints into the CrdtStore (the single
        // system of record) once, so seeded enforcement is preserved AND every
        // constraint is a real, queryable PluresDB node (TASK-PX-CANON Stage 2;
        // ADR-0017 Stage B6). No separate in-memory PraxisStore is retained.
        seed_praxis_into_crdt(&core_store, &actor_id);

        Ok(Self {
            store: core_store,
            storage,
            #[cfg(feature = "sqlite-compat")]
            db,
            broadcaster: Arc::new(SyncBroadcaster::default()),
            actor_id,
        })
    }

    /// Create a PluresDB instance with automatic text embedding.
    ///
    /// `model` is a HuggingFace model ID such as `"BAAI/bge-small-en-v1.5"`.
    /// Every subsequent call to [`put`][PluresDatabase::put] will
    /// automatically embed any text content found in the node data.
    ///
    /// Requires the crate to be compiled with the `embeddings` cargo feature.
    /// If the feature is not enabled the method returns an error at runtime.
    ///
    /// ## JavaScript example
    ///
    /// ```js
    /// const { PluresDatabase } = require('@plures/pluresdb');
    ///
    /// const db = PluresDatabase.newWithEmbeddings('BAAI/bge-small-en-v1.5');
    /// db.put('memory-1', { content: 'user prefers dark mode' });
    /// const results = db.vectorSearch([...queryEmbedding], 5, 0.3);
    /// ```
    #[napi(factory)]
    pub fn new_with_embeddings(
        model: String,
        actor_id: Option<String>,
        db_path: Option<String>,
    ) -> Result<Self> {
        let actor_id = actor_id.unwrap_or_else(|| "node-actor".to_string());

        #[cfg(feature = "embeddings")]
        {
            use pluresdb_core::FastEmbedder;
            let embedder = FastEmbedder::new(&model).map_err(|e| {
                node_error(
                    CoreErrorCode::InvalidInput.as_str(),
                    format!("Failed to initialize embedding model '{}': {}", model, e),
                )
            })?;
            let mut store = CrdtStore::default().with_embedder(Arc::new(embedder));

            // Open persistent storage if db_path provided.
            let storage: Option<Arc<dyn StorageEngine>> = if let Some(ref path) = db_path {
                let sled_storage = Arc::new(
                    SledStorage::open(path)
                        .map_err(|e| map_node_error(StorageErrorCode::OpenFailed.as_str(), e))?,
                );
                store = store.with_persistence(sled_storage.clone() as Arc<dyn StorageEngine>);
                Some(sled_storage as Arc<dyn StorageEngine>)
            } else {
                None
            };

            #[cfg(feature = "sqlite-compat")]
            let db = if let Some(ref path) = db_path {
                let options = DatabaseOptions::with_file(path).create_if_missing(true);
                Some(Arc::new(Database::open(options).map_err(|e| {
                    map_node_error(CoreErrorCode::SqliteError.as_str(), e)
                })?))
            } else {
                None
            };

            let core_store = Arc::new(Mutex::new(store));
            seed_praxis_into_crdt(&core_store, &actor_id);

            Ok(Self {
                store: core_store,
                storage,
                #[cfg(feature = "sqlite-compat")]
                db,
                broadcaster: Arc::new(SyncBroadcaster::default()),
                actor_id,
            })
        }

        #[cfg(not(feature = "embeddings"))]
        {
            let _ = (model, db_path, actor_id);
            Err(node_error(
                CoreErrorCode::FeatureDisabled.as_str(),
                "auto-embedding is not available: pluresdb-node was compiled without \
                 the 'embeddings' cargo feature"
                    .to_string(),
            ))
        }
    }

    /// Insert or update a node
    #[napi]
    pub fn put(&self, id: String, data: serde_json::Value) -> Result<String> {
        let store = self.store.clone();
        let broadcaster = self.broadcaster.clone();
        let actor_id = self.actor_id.clone();

        let node_id = {
            let store = store.lock();
            store.put(id.clone(), actor_id, data)
        };

        // Publish sync event
        broadcaster
            .publish(SyncEvent::NodeUpsert {
                id: node_id.clone(),
            })
            .map_err(|e| map_node_error(SyncErrorCode::BroadcastPublishFailed.as_str(), e))?;

        Ok(node_id)
    }

    /// Retrieve a node by ID
    #[napi]
    pub fn get(&self, id: String) -> Result<Option<serde_json::Value>> {
        let store = self.store.clone();

        let record = {
            let store = store.lock();
            store.get(id)
        };

        match record {
            Some(record) => {
                // Return the data portion as JSON
                Ok(Some(record.data))
            }
            None => Ok(None),
        }
    }

    /// Get a node with full metadata (including vector clock and timestamp)
    #[napi]
    pub fn get_with_metadata(&self, id: String) -> Result<Option<serde_json::Value>> {
        let store = self.store.clone();

        let record = {
            let store = store.lock();
            store.get(id)
        };

        match record {
            Some(record) => Ok(Some(serde_json::json!({
                "id": record.id,
                "data": record.data,
                "clock": record.clock,
                "timestamp": record.timestamp.to_rfc3339(),
            }))),
            None => Ok(None),
        }
    }

    /// Delete a node by ID
    #[napi]
    pub fn delete(&self, id: String) -> Result<()> {
        let store = self.store.clone();
        let broadcaster = self.broadcaster.clone();

        {
            let store = store.lock();
            store.delete(&id).map_err(map_store_error)?;
        }

        // Publish sync event
        broadcaster
            .publish(SyncEvent::NodeDelete { id: id.clone() })
            .map_err(|e| map_node_error(SyncErrorCode::BroadcastPublishFailed.as_str(), e))?;

        Ok(())
    }

    /// List all nodes
    #[napi]
    pub fn list(&self) -> Result<Vec<serde_json::Value>> {
        let store = self.store.clone();

        let records = {
            let store = store.lock();
            store.list()
        };

        // Convert records to JSON objects with id and data
        let result: Vec<serde_json::Value> = records
            .into_iter()
            .map(|record| {
                serde_json::json!({
                    "id": record.id,
                    "data": record.data,
                    "timestamp": record.timestamp.to_rfc3339(),
                })
            })
            .collect();

        Ok(result)
    }

    /// List nodes filtered by type
    #[napi]
    pub fn list_by_type(&self, node_type: String) -> Result<Vec<serde_json::Value>> {
        let store = self.store.clone();

        let records = {
            let store = store.lock();
            store.list()
        };

        let result: Vec<serde_json::Value> = records
            .into_iter()
            .filter(|record| {
                record
                    .data
                    .get("type")
                    .and_then(|v| v.as_str())
                    .map(|t| t == node_type)
                    .unwrap_or(false)
            })
            .map(|record| {
                serde_json::json!({
                    "id": record.id,
                    "data": record.data,
                    "timestamp": record.timestamp.to_rfc3339(),
                })
            })
            .collect();

        Ok(result)
    }

    /// Execute SQL query
    ///
    /// Requires the `sqlite-compat` cargo feature to be enabled.
    #[napi]
    pub fn query(
        &self,
        sql: String,
        params: Option<Vec<serde_json::Value>>,
    ) -> Result<serde_json::Value> {
        #[cfg(feature = "sqlite-compat")]
        {
            let db = self.db.as_ref().ok_or_else(|| {
                node_error(
                    CoreErrorCode::InvalidInput.as_str(),
                    "SQL queries require a database (provide db_path in constructor)".to_string(),
                )
            })?;

            let sql_params: Vec<SqlValue> = if let Some(p) = params {
                p.into_iter()
                    .map(|v| -> Result<SqlValue> {
                        Ok(match v {
                            serde_json::Value::Null => SqlValue::Null,
                            serde_json::Value::Number(n) => {
                                if n.is_i64() {
                                    SqlValue::Integer(n.as_i64().unwrap())
                                } else {
                                    SqlValue::Real(n.as_f64().unwrap())
                                }
                            }
                            serde_json::Value::String(s) => SqlValue::Text(s),
                            serde_json::Value::Bool(b) => SqlValue::Integer(if b { 1 } else { 0 }),
                            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                                SqlValue::Text(serde_json::to_string(&v).map_err(|e| {
                                    map_node_error(CoreErrorCode::SerializationError.as_str(), e)
                                })?)
                            }
                        })
                    })
                    .collect::<Result<Vec<_>>>()?
            } else {
                vec![]
            };

            let result = db
                .query(&sql, &sql_params)
                .map_err(|e| map_node_error(CoreErrorCode::SqliteError.as_str(), e))?;

            return Ok(serde_json::json!({
                "columns": result.columns,
                "rows": result.rows_as_json(),
                "changes": result.changes,
                "lastInsertRowid": result.last_insert_rowid
            }));
        }

        #[cfg(not(feature = "sqlite-compat"))]
        {
            let _ = (sql, params);
            Err(node_error(
                CoreErrorCode::FeatureDisabled.as_str(),
                "SQL queries require the 'sqlite-compat' cargo feature to be enabled".to_string(),
            ))
        }
    }

    /// Execute SQL statement (INSERT, UPDATE, DELETE)
    ///
    /// Requires the `sqlite-compat` cargo feature to be enabled.
    #[napi]
    pub fn exec(&self, sql: String) -> Result<serde_json::Value> {
        #[cfg(feature = "sqlite-compat")]
        {
            let db = self.db.as_ref().ok_or_else(|| {
                node_error(
                    CoreErrorCode::InvalidInput.as_str(),
                    "SQL execution requires a database (provide db_path in constructor)"
                        .to_string(),
                )
            })?;

            let result = db
                .exec(&sql)
                .map_err(|e| map_node_error(CoreErrorCode::SqliteError.as_str(), e))?;

            return Ok(serde_json::json!({
                "changes": result.changes,
                "lastInsertRowid": result.last_insert_rowid
            }));
        }

        #[cfg(not(feature = "sqlite-compat"))]
        {
            let _ = sql;
            Err(node_error(
                CoreErrorCode::FeatureDisabled.as_str(),
                "SQL execution requires the 'sqlite-compat' cargo feature to be enabled"
                    .to_string(),
            ))
        }
    }

    /// Search nodes by text content
    #[napi]
    pub fn search(&self, query: String, limit: Option<u32>) -> Result<Vec<serde_json::Value>> {
        let store = self.store.clone();
        let limit = limit.unwrap_or(10) as usize;

        let records = {
            let store = store.lock();
            store.list()
        };

        let query_lower = query.to_lowercase();
        let mut matches: Vec<(NodeRecord, usize)> = records
            .into_iter()
            .filter_map(|record| {
                let json_str = serde_json::to_string(&record.data).ok()?;
                let count = json_str.to_lowercase().matches(&query_lower).count();
                if count > 0 {
                    Some((record, count))
                } else {
                    None
                }
            })
            .collect();

        matches.sort_by_key(|b| std::cmp::Reverse(b.1));
        matches.truncate(limit);

        let result: Vec<serde_json::Value> = matches
            .into_iter()
            .map(|(record, score)| {
                serde_json::json!({
                    "id": record.id,
                    "data": record.data,
                    "score": score,
                    "timestamp": record.timestamp.to_rfc3339(),
                })
            })
            .collect();

        Ok(result)
    }

    /// Vector similarity search using a pre-computed embedding.
    ///
    /// `embedding` must be a flat array of finite floats.  Results are ordered
    /// by cosine similarity (highest first) and filtered by `threshold` (0–1).
    #[napi]
    pub fn vector_search(
        &self,
        embedding: Vec<f64>,
        limit: Option<u32>,
        threshold: Option<f64>,
    ) -> Result<Vec<serde_json::Value>> {
        if embedding.is_empty() {
            return Err(node_error(
                CoreErrorCode::InvalidInput.as_str(),
                "embedding must not be empty",
            ));
        }
        if embedding.iter().any(|v| !v.is_finite()) {
            return Err(node_error(
                CoreErrorCode::InvalidInput.as_str(),
                "embedding contains non-finite values (NaN or Inf)",
            ));
        }
        let threshold_val = threshold.unwrap_or(0.0);
        if !threshold_val.is_finite() || !(0.0..=1.0).contains(&threshold_val) {
            return Err(node_error(
                CoreErrorCode::InvalidInput.as_str(),
                "threshold must be a finite number in [0.0, 1.0]",
            ));
        }

        let store = self.store.clone();
        let limit = limit.unwrap_or(10) as usize;
        let min_score = threshold_val as f32;

        // Convert f64 → f32 for the HNSW index.
        let query: Vec<f32> = embedding.iter().map(|&v| v as f32).collect();

        let results = {
            let store = store.lock();
            store.vector_search(&query, limit, min_score)
        };

        let output: Vec<serde_json::Value> = results
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.record.id,
                    "data": r.record.data,
                    "score": r.score,
                    "timestamp": r.record.timestamp.to_rfc3339(),
                })
            })
            .collect();

        Ok(output)
    }

    /// Insert or update a node together with its embedding vector.
    ///
    /// The embedding is indexed immediately so that it is available for
    /// subsequent [`vector_search`] calls.
    #[napi]
    pub fn put_with_embedding(
        &self,
        id: String,
        data: serde_json::Value,
        embedding: Vec<f64>,
    ) -> Result<String> {
        if embedding.is_empty() {
            return Err(node_error(
                CoreErrorCode::InvalidInput.as_str(),
                "embedding must not be empty",
            ));
        }
        if embedding.iter().any(|v| !v.is_finite()) {
            return Err(node_error(
                CoreErrorCode::InvalidInput.as_str(),
                "embedding contains non-finite values (NaN or Inf)",
            ));
        }

        let store = self.store.clone();
        let broadcaster = self.broadcaster.clone();
        let actor_id = self.actor_id.clone();

        let emb_f32: Vec<f32> = embedding.iter().map(|&v| v as f32).collect();

        let node_id = {
            let store = store.lock();
            store.put_with_embedding(id, actor_id, data, emb_f32)
        };

        broadcaster
            .publish(SyncEvent::NodeUpsert {
                id: node_id.clone(),
            })
            .map_err(|e| map_node_error(SyncErrorCode::BroadcastPublishFailed.as_str(), e))?;

        Ok(node_id)
    }

    /// Subscribe to node changes (returns a subscription ID)
    /// Note: Full async subscription support requires additional async infrastructure
    #[napi]
    pub fn subscribe(&self) -> Result<String> {
        // Subscribe and return subscription ID
        // In a full implementation, this would return a subscription handle
        // that can be used to receive events via async callbacks
        let _receiver = self.broadcaster.subscribe();
        Ok("subscription-1".to_string())
    }

    /// Embed text using the configured embedding model.
    ///
    /// Only available when the database was created via `newWithEmbeddings()`.
    /// Returns a flat `Vec<f64>` suitable for passing to `vectorSearch()`.
    #[napi]
    pub fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f64>>> {
        let store = self.store.lock();
        let embedder = store.embedder().ok_or_else(|| {
            node_error(
                CoreErrorCode::InvalidInput.as_str(),
                "embed() requires a database created with newWithEmbeddings()",
            )
        })?;
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let vecs = embedder
            .embed(&refs)
            .map_err(|e| map_node_error(CoreErrorCode::InvalidInput.as_str(), e))?;
        // Convert f32 → f64 for JS compatibility
        Ok(vecs
            .into_iter()
            .map(|v| v.into_iter().map(|x| x as f64).collect())
            .collect())
    }

    /// Get the embedding dimension, or null if no embedder is configured.
    #[napi]
    pub fn embedding_dimension(&self) -> Option<u32> {
        let store = self.store.lock();
        store.embedder().map(|e| e.dimension() as u32)
    }

    /// Get the actor ID for this database instance
    #[napi]
    pub fn get_actor_id(&self) -> String {
        self.actor_id.clone()
    }

    /// Execute a DSL query string against the CRDT store.
    ///
    /// Returns the procedure result as a JSON object with `nodes`, and
    /// optionally `aggregate` or `mutated` fields.
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// const result = db.execDsl('filter(category == "decision") |> sort(by: "score", dir: "desc") |> limit(10)');
    /// console.log(result.nodes);
    /// ```
    #[napi]
    pub fn exec_dsl(&self, query: String) -> Result<serde_json::Value> {
        let store = self.store.lock();
        let engine = ProcedureEngine::new(&store, self.actor_id.as_str());
        let result = engine
            .exec_dsl(&query)
            .map_err(|e| map_node_error(CoreErrorCode::InvalidInput.as_str(), e))?;
        serde_json::to_value(&result)
            .map_err(|e| map_node_error(CoreErrorCode::SerializationError.as_str(), e))
    }

    /// Execute a JSON IR query against the CRDT store.
    ///
    /// `steps` must be a JSON array of step objects as produced by the
    /// pluresdb-procedures builder or parser.
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// const result = db.execIr([
    ///   { op: "filter", predicate: { field: "category", cmp: "==", value: "decision" } },
    ///   { op: "limit", n: 5 }
    /// ]);
    /// ```
    #[napi]
    pub fn exec_ir(&self, steps: serde_json::Value) -> Result<serde_json::Value> {
        let store = self.store.lock();
        let engine = ProcedureEngine::new(&store, self.actor_id.as_str());
        let result = engine
            .exec_ir(&steps)
            .map_err(|e| map_node_error(CoreErrorCode::InvalidInput.as_str(), e))?;
        serde_json::to_value(&result)
            .map_err(|e| map_node_error(CoreErrorCode::SerializationError.as_str(), e))
    }

    // -----------------------------------------------------------------------
    // Praxis constraint engine (TASK-PX-CANON Stage 2 / ADR-0017 Stage B6)
    //
    // These methods expose the `pluresdb-px` constraint engine through NAPI,
    // mirroring the `exec_ir` marshaling pattern (serde_json::Value in / out,
    // `map_node_error` on failure). Constraints are persisted as nodes in the
    // `CrdtStore` — the SINGLE system of record. Read paths (evaluate/on_action/
    // query_gaps) project an ephemeral `PraxisStore` view from those nodes;
    // write paths (compile_nl/apply_correction/undo_correction/load/insert)
    // persist constraints back as CrdtStore nodes. There is no separate
    // in-memory constraint store. JS names are camelCased by napi
    // (`pxEvaluate`, `pxOnAction`, ...).
    // -----------------------------------------------------------------------

    /// Evaluate an [`AgentContext`] against the praxis constraints persisted in
    /// the CrdtStore and return every violated constraint.
    ///
    /// `ctx` is a JSON object `{ action_type, target, session_type, metadata }`
    /// that deserializes into the Rust `AgentContext`. Returns a JSON array of
    /// `Violation` objects (each with `constraint` and `message`).
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// const violations = db.pxEvaluate({
    ///   action_type: 'write_file',
    ///   target: 'config.toml',
    ///   session_type: 'main',
    ///   metadata: { resource_owner: '' },
    /// });
    /// ```
    #[napi]
    pub fn px_evaluate(&self, ctx: serde_json::Value) -> Result<serde_json::Value> {
        let ctx: PxAgentContext = serde_json::from_value(ctx)
            .map_err(|e| map_node_error(CoreErrorCode::InvalidInput.as_str(), e))?;
        let pstore = project_praxis_store(&self.store);
        let violations = px_procedures::evaluate(&pstore, &ctx);
        serde_json::to_value(&violations)
            .map_err(|e| map_node_error(CoreErrorCode::SerializationError.as_str(), e))
    }

    /// Pre-action hook: evaluate `ctx` against the CrdtStore-persisted
    /// constraints and **block** when any error-severity constraint fires.
    ///
    /// On a permitted action returns `{ "violations": [...] }` (the warning-only
    /// violations, which may be empty). On a blocked action **throws** a JS
    /// error carrying the `ActionBlocked` detail, so the Node caller sees a real
    /// exception for the block path.
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// try {
    ///   const { violations } = db.pxOnAction({ action_type: 'read_file', target: 'README.md', session_type: 'main', metadata: {} });
    /// } catch (err) {
    ///   // action was blocked
    /// }
    /// ```
    #[napi]
    pub fn px_on_action(&self, ctx: serde_json::Value) -> Result<serde_json::Value> {
        let ctx: PxAgentContext = serde_json::from_value(ctx)
            .map_err(|e| map_node_error(CoreErrorCode::InvalidInput.as_str(), e))?;
        let pstore = project_praxis_store(&self.store);
        match px_procedures::on_action(&pstore, &ctx) {
            Ok(warnings) => {
                let value = serde_json::to_value(&warnings)
                    .map_err(|e| map_node_error(CoreErrorCode::SerializationError.as_str(), e))?;
                Ok(serde_json::json!({ "violations": value }))
            }
            Err(blocked) => Err(map_node_error(CoreErrorCode::InvalidInput.as_str(), blocked)),
        }
    }

    /// Compile a natural-language rule into a [`Constraint`] and **persist** it
    /// to the CrdtStore so it is immediately evaluable and survives restart.
    ///
    /// `text` is the rule description; `id` is the stable constraint ID. Returns
    /// the compiled `Constraint` as JSON.
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// const c = db.pxCompileNl('write_ actions must declare a resource_owner', 'C-OWNER');
    /// ```
    #[napi]
    pub fn px_compile_nl(&self, text: String, id: String) -> Result<serde_json::Value> {
        let constraint = px_procedures::compile_nl(&text, id);
        // Persist to the CrdtStore (single source of truth) so the freshly
        // compiled constraint is evaluable now and durable across restarts.
        persist_constraint(&self.store, &self.broadcaster, &self.actor_id, &constraint)?;
        serde_json::to_value(&constraint)
            .map_err(|e| map_node_error(CoreErrorCode::SerializationError.as_str(), e))
    }

    /// Apply a user correction: compile `correction_text` into a constraint
    /// (prefixed `[correction]`) and **persist** it to the CrdtStore.
    ///
    /// Returns a `CorrectionApplied` record (`constraint`, `is_new`,
    /// `confirmation`) as JSON. `is_new` reflects whether a constraint with that
    /// id already existed as a CrdtStore node.
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// const applied = db.pxApplyCorrection('risk_score must stay below threshold', 'C-CORR-1');
    /// ```
    #[napi]
    pub fn px_apply_correction(
        &self,
        correction_text: String,
        id: String,
    ) -> Result<serde_json::Value> {
        // `is_new` is determined against the CrdtStore (the real record).
        let is_new = {
            let store = self.store.lock();
            store.get(&id).is_none()
        };
        // Reuse the real compile path, then apply the `[correction]` prefix
        // exactly as `procedures::apply_correction` does, but persist to CRDT.
        let mut constraint = px_procedures::compile_nl(&correction_text, &id);
        constraint.description = format!("[correction] {}", constraint.description);
        persist_constraint(&self.store, &self.broadcaster, &self.actor_id, &constraint)?;
        let confirmation = format!(
            "Got it, I'll remember to {} going forward.",
            constraint.description.trim_start_matches("[correction] ")
        );
        let applied = serde_json::json!({
            "constraint": serde_json::to_value(&constraint)
                .map_err(|e| map_node_error(CoreErrorCode::SerializationError.as_str(), e))?,
            "is_new": is_new,
            "confirmation": confirmation,
        });
        Ok(applied)
    }

    /// Undo a previously applied correction by deleting its constraint node from
    /// the CrdtStore.
    ///
    /// Returns the removed `Constraint` as JSON, or `null` if no constraint with
    /// `constraint_id` exists.
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// const removed = db.pxUndoCorrection('C-CORR-1'); // Constraint | null
    /// ```
    #[napi]
    pub fn px_undo_correction(&self, constraint_id: String) -> Result<serde_json::Value> {
        // Read-then-delete against the CrdtStore (single source of truth).
        let removed = {
            let store = self.store.lock();
            let existing = store
                .get(&constraint_id)
                .and_then(|r| constraint_from_node_data(&r.data));
            if existing.is_some() {
                store.delete(&constraint_id).map_err(map_store_error)?;
            }
            existing
        };
        if removed.is_some() {
            self.broadcaster
                .publish(SyncEvent::NodeDelete {
                    id: constraint_id.clone(),
                })
                .map_err(|e| {
                    map_node_error(SyncErrorCode::BroadcastPublishFailed.as_str(), e)
                })?;
        }
        serde_json::to_value(&removed)
            .map_err(|e| map_node_error(CoreErrorCode::SerializationError.as_str(), e))
    }

    /// Query evidence gaps: all `Evidence` records whose result is `Unknown`.
    ///
    /// Evidence records are not yet persisted as CrdtStore nodes (constraints
    /// are the Stage-2 system-of-record scope); the canonical seeded evidence
    /// set is read from the default store for gap reporting. Returns a JSON
    /// array of `Evidence` objects.
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// const gaps = db.pxQueryGaps();
    /// ```
    #[napi]
    pub fn px_query_gaps(&self) -> Result<serde_json::Value> {
        let pstore = px_default_store();
        // `query_gaps` returns borrowed references; clone to owned before the
        // store is dropped so the values can be serialized.
        let gaps: Vec<_> = px_procedures::query_gaps(&pstore)
            .into_iter()
            .cloned()
            .collect();
        serde_json::to_value(&gaps)
            .map_err(|e| map_node_error(CoreErrorCode::SerializationError.as_str(), e))
    }

    /// Load `.px` source through the **canonical** `pluresdb_px::px::parse`
    /// grammar and persist every declared `constraint` block as a CrdtStore
    /// node (the single system of record).
    ///
    /// Each parsed `PxConstraint`'s `require:` expression is compiled to a real
    /// enforcing [`Condition`] via the Stage-1 `compile_nl` path (the canonical
    /// expression grammar), so a block like `require: amount <= 100` actually
    /// blocks `amount = 500`. The declared `severity:` is honored. Returns
    /// `{ constraints: [ids...], procedures: [names...] }` describing what was
    /// loaded (procedures are reported but not persisted as constraints).
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// db.pxLoadPxSource('constraint cap:\n  require: amount <= 100\n  severity: error\n');
    /// ```
    #[napi]
    pub fn px_load_px_source(&self, text: String) -> Result<serde_json::Value> {
        let doc =
            px_parse(&text).map_err(|e| map_node_error(CoreErrorCode::InvalidInput.as_str(), e))?;

        let mut loaded_ids: Vec<String> = Vec::new();
        for stmt in &doc.statements {
            if let PxStatement::Constraint(pc) = stmt {
                let constraint = px_constraint_to_schema(pc);
                persist_constraint(&self.store, &self.broadcaster, &self.actor_id, &constraint)?;
                loaded_ids.push(constraint.id);
            }
        }
        // Report procedure names (dataflow + legacy) but do not persist them as
        // constraints. px-ast splits procedures into two statement variants.
        let procedure_names: Vec<String> = doc
            .statements
            .iter()
            .filter_map(|s| match s {
                PxStatement::DataflowProcedure(p) => Some(p.name.name.clone()),
                PxStatement::LegacyProcedure(p) => Some(p.name.name.clone()),
                _ => None,
            })
            .collect();

        Ok(serde_json::json!({
            "constraints": loaded_ids,
            "procedures": procedure_names,
        }))
    }

    /// Insert a single constraint and persist it to the CrdtStore.
    ///
    /// `constraint` accepts EITHER:
    /// - a fully-structured `Constraint` JSON object (`{ id, description, when,
    ///   require, fix, evidence, severity }`) — inserted as-is, or
    /// - a compile request `{ id, text }` — `text` is compiled to a real
    ///   enforcing constraint via the canonical `compile_nl` path.
    ///
    /// Returns the persisted `Constraint` as JSON.
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// db.pxInsertConstraint({ id: 'C-CAP', text: 'amount <= 100' });
    /// ```
    #[napi]
    pub fn px_insert_constraint(&self, constraint: serde_json::Value) -> Result<serde_json::Value> {
        // Path A: a structured Constraint (must have a `require` field).
        let parsed: PxConstraint = if constraint.get("require").is_some() {
            serde_json::from_value(constraint)
                .map_err(|e| map_node_error(CoreErrorCode::InvalidInput.as_str(), e))?
        } else {
            // Path B: a `{ id, text }` compile request.
            let id = constraint
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    node_error(
                        CoreErrorCode::InvalidInput.as_str(),
                        "pxInsertConstraint requires either a structured constraint (with `require`) or `{ id, text }`",
                    )
                })?
                .to_string();
            let text = constraint
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    node_error(
                        CoreErrorCode::InvalidInput.as_str(),
                        "pxInsertConstraint `{ id, text }` form requires a `text` string",
                    )
                })?;
            px_procedures::compile_nl(text, id)
        };
        persist_constraint(&self.store, &self.broadcaster, &self.actor_id, &parsed)?;
        serde_json::to_value(&parsed)
            .map_err(|e| map_node_error(CoreErrorCode::SerializationError.as_str(), e))
    }

    /// Build the HNSW vector index from hydrated embeddings.
    /// Call after init to enable vector search without blocking startup.
    #[napi]
    pub fn build_vector_index(&self) -> u32 {
        let store = self.store.lock();
        store.build_vector_index() as u32
    }

    /// Get database statistics without loading all nodes into memory.
    #[napi]
    pub fn stats(&self) -> Result<serde_json::Value> {
        let store = self.store.clone();
        let store = store.lock();

        let mut total_nodes = 0u64;
        let mut type_counts: HashMap<String, u32> = HashMap::new();

        store.for_each_sync(&mut |record| {
            total_nodes += 1;
            if let Some(t) = record.data.get("type").and_then(|v| v.as_str()) {
                *type_counts.entry(t.to_string()).or_insert(0) += 1;
            }
            true
        });

        Ok(serde_json::json!({
            "totalNodes": total_nodes,
            "typeCounts": type_counts,
        }))
    }

    // -----------------------------------------------------------------------
    // Agens Runtime — reactive event system
    // -----------------------------------------------------------------------

    /// Emit an event into the Agens reactive runtime.
    ///
    /// The event is persisted as a CRDT node and can be polled by
    /// [`agensListEvents`] or dispatched to handlers registered in Rust.
    ///
    /// `event` must be a JSON object with an `event_type` field (one of:
    /// `message`, `timer`, `state_change`, `model_response`, `tool_result`,
    /// `praxis_analysis_ready`, `praxis_analysis_failed`,
    /// `praxis_guidance_updated`) and the corresponding fields for that type.
    ///
    /// Returns the CRDT node ID assigned to the event.
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// const nodeId = db.agensEmit({
    ///   event_type: 'message',
    ///   id: 'msg-1',
    ///   payload: { text: 'hello' },
    /// });
    /// ```
    #[napi]
    pub fn agens_emit(&self, event: serde_json::Value) -> Result<String> {
        let ev: AgensEvent = serde_json::from_value(event)
            .map_err(|e| map_node_error(CoreErrorCode::InvalidInput.as_str(), e))?;
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        Ok(runtime.emit_event(&ev))
    }

    /// Emit a Praxis lifecycle event with **idempotent** storage.
    ///
    /// Unlike [`agensEmit`], re-emitting the same logical event (same `id`)
    /// converges to a single CRDT node rather than creating duplicates.
    ///
    /// Use for `praxis_analysis_ready`, `praxis_analysis_failed`, and
    /// `praxis_guidance_updated` events.
    ///
    /// Returns the deterministic CRDT node ID (`praxis:cmd:{event_id}`).
    #[napi]
    pub fn agens_emit_praxis(&self, event: serde_json::Value) -> Result<String> {
        let ev: AgensEvent = serde_json::from_value(event)
            .map_err(|e| map_node_error(CoreErrorCode::InvalidInput.as_str(), e))?;
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        Ok(runtime.emit_praxis_event(&ev))
    }

    /// Poll the Agens command table for events after `sinceIso`.
    ///
    /// `since_iso` must be an ISO 8601 timestamp (e.g. `"2026-04-05T12:00:00Z"`).
    /// Returns events oldest-first as a JSON array.
    ///
    /// # Performance
    ///
    /// This method delegates to [`AgensRuntime::poll_events`], which may scan all
    /// CRDT nodes via store listing and then filter matching events in memory.
    /// As a result, the cost is O(n) in the total store size, including persisted
    /// storage, not just in the number of events returned.
    ///
    /// Avoid high-frequency polling on large stores. Prefer polling on a bounded
    /// interval, processing results in batches, and then advancing `since_iso`
    /// from the newest processed event timestamp to reduce repeated work.
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// const events = db.agensListEvents("2026-04-05T00:00:00Z");
    /// for (const ev of events) {
    ///   console.log(ev.event_type, ev.id);
    /// }
    /// ```
    #[napi]
    pub fn agens_list_events(&self, since_iso: String) -> Result<Vec<serde_json::Value>> {
        let since: chrono::DateTime<chrono::Utc> = since_iso
            .parse()
            .map_err(|e| map_node_error(CoreErrorCode::InvalidInput.as_str(), e))?;
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        let events = runtime.poll_events(since);
        events
            .into_iter()
            .map(|ev| {
                serde_json::to_value(&ev)
                    .map_err(|e| map_node_error(CoreErrorCode::SerializationError.as_str(), e))
            })
            .collect()
    }

    /// Get a value from the Agens reactive state table.
    ///
    /// Returns `null` if the key is not set.
    #[napi]
    pub fn agens_state_get(&self, key: String) -> Result<serde_json::Value> {
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        Ok(runtime.state().get(&key).unwrap_or(serde_json::Value::Null))
    }

    /// Set a value in the Agens reactive state table.
    ///
    /// Automatically emits a `state_change` event visible to
    /// [`agensListEvents`].
    #[napi]
    pub fn agens_state_set(&self, key: String, value: serde_json::Value) -> Result<()> {
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        runtime.state().set(&key, value);
        Ok(())
    }

    /// Watch the state table for entries updated since `sinceIso`.
    ///
    /// Returns an array of `{ key, value }` objects.
    #[napi]
    pub fn agens_state_watch(&self, since_iso: String) -> Result<Vec<serde_json::Value>> {
        let since: chrono::DateTime<chrono::Utc> = since_iso
            .parse()
            .map_err(|e| map_node_error(CoreErrorCode::InvalidInput.as_str(), e))?;
        let watch_results = {
            let store = self.store.lock();
            let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
            runtime.state().watch(since)
        };

        Ok(watch_results
            .into_iter()
            .map(|(k, v)| serde_json::json!({ "key": k, "value": v }))
            .collect())
    }

    /// Schedule a recurring timer in the Agens timer table.
    ///
    /// Returns the timer node ID (use with [`agensTimerCancel`]).
    #[napi]
    pub fn agens_timer_schedule(
        &self,
        name: String,
        interval_secs: u32,
        payload: serde_json::Value,
    ) -> Result<String> {
        if interval_secs == 0 {
            return Err(node_error(
                CoreErrorCode::InvalidInput.as_str(),
                "interval_secs must be greater than 0",
            ));
        }
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        Ok(runtime
            .timers()
            .schedule(&name, interval_secs as u64, payload))
    }

    /// Cancel a timer by its ID. Returns `true` if it existed.
    #[napi]
    pub fn agens_timer_cancel(&self, timer_id: String) -> Result<bool> {
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        Ok(runtime.timers().cancel(&timer_id))
    }

    /// List all scheduled timers.
    ///
    /// Returns an array of `{ id, name, intervalSecs, nextFireAt, payload }`.
    #[napi]
    pub fn agens_timer_list(&self) -> Result<Vec<serde_json::Value>> {
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        Ok(runtime
            .timers()
            .list()
            .into_iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "name": t.name,
                    "intervalSecs": t.interval_secs,
                    "nextFireAt": t.next_fire_at.to_rfc3339(),
                    "payload": t.payload,
                })
            })
            .collect())
    }

    /// Return timers that are due (next_fire_at <= now).
    ///
    /// Call this in a tick loop to process due timers.
    #[napi]
    pub fn agens_timer_due(&self) -> Result<Vec<serde_json::Value>> {
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        let now = chrono::Utc::now();
        Ok(runtime
            .timers()
            .due_timers(now)
            .into_iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "name": t.name,
                    "intervalSecs": t.interval_secs,
                    "nextFireAt": t.next_fire_at.to_rfc3339(),
                    "payload": t.payload,
                })
            })
            .collect())
    }

    /// Reschedule a timer by advancing its next_fire_at by one interval.
    ///
    /// Returns `true` if the timer existed and was rescheduled.
    #[napi]
    pub fn agens_timer_reschedule(&self, timer_id: String) -> Result<bool> {
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        Ok(runtime.timers().reschedule(&timer_id))
    }
}

/// Initialize the module
#[napi]
pub fn init() -> Result<()> {
    Ok(())
}
