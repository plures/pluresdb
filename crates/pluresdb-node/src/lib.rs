//! Node.js bindings for PluresDB.
//!
//! This crate provides Node.js bindings using N-API to expose
//! PluresDB functionality to Node.js applications.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use parking_lot::Mutex;
use pluresdb_core::{CrdtStore, NodeRecord};
use pluresdb_procedures::agens::{AgensEvent, AgensRuntime};
use pluresdb_procedures::engine::ProcedureEngine;
use pluresdb_storage::{SledStorage, StorageEngine};
use pluresdb_sync::{SyncBroadcaster, SyncEvent};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "sqlite-compat")]
use pluresdb_core::{Database, DatabaseOptions, SqlValue};

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
                    .map_err(|e| Error::from_reason(format!("Failed to open storage: {}", e)))?,
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
                Error::from_reason(format!("Failed to open database: {}", e))
            })?))
        } else {
            None
        };

        Ok(Self {
            store: Arc::new(Mutex::new(store)),
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
                Error::from_reason(format!(
                    "Failed to initialize embedding model '{}': {}",
                    model, e
                ))
            })?;
            let mut store = CrdtStore::default().with_embedder(Arc::new(embedder));

            // Open persistent storage if db_path provided.
            let storage: Option<Arc<dyn StorageEngine>> = if let Some(ref path) = db_path {
                let sled_storage =
                    Arc::new(SledStorage::open(path).map_err(|e| {
                        Error::from_reason(format!("Failed to open storage: {}", e))
                    })?);
                store = store.with_persistence(sled_storage.clone() as Arc<dyn StorageEngine>);
                Some(sled_storage as Arc<dyn StorageEngine>)
            } else {
                None
            };

            #[cfg(feature = "sqlite-compat")]
            let db = if let Some(ref path) = db_path {
                let options = DatabaseOptions::with_file(path).create_if_missing(true);
                Some(Arc::new(Database::open(options).map_err(|e| {
                    Error::from_reason(format!("Failed to open database: {}", e))
                })?))
            } else {
                None
            };

            Ok(Self {
                store: Arc::new(Mutex::new(store)),
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
            Err(Error::from_reason(
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
        let _ = broadcaster.publish(SyncEvent::NodeUpsert {
            id: node_id.clone(),
        });

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
            store
                .delete(&id)
                .map_err(|e| Error::from_reason(format!("Delete error: {}", e)))?;
        }

        // Publish sync event
        let _ = broadcaster.publish(SyncEvent::NodeDelete { id: id.clone() });

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
                Error::from_reason(
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
                                    Error::from_reason(format!("Failed to serialize param: {}", e))
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
                .map_err(|e| Error::from_reason(format!("Query error: {}", e)))?;

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
            Err(Error::from_reason(
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
                Error::from_reason(
                    "SQL execution requires a database (provide db_path in constructor)"
                        .to_string(),
                )
            })?;

            let result = db
                .exec(&sql)
                .map_err(|e| Error::from_reason(format!("Execution error: {}", e)))?;

            return Ok(serde_json::json!({
                "changes": result.changes,
                "lastInsertRowid": result.last_insert_rowid
            }));
        }

        #[cfg(not(feature = "sqlite-compat"))]
        {
            let _ = sql;
            Err(Error::from_reason(
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

        matches.sort_by(|a, b| b.1.cmp(&a.1));
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
            return Err(Error::from_reason("embedding must not be empty"));
        }
        if embedding.iter().any(|v| !v.is_finite()) {
            return Err(Error::from_reason(
                "embedding contains non-finite values (NaN or Inf)",
            ));
        }
        let threshold_val = threshold.unwrap_or(0.0);
        if !threshold_val.is_finite() || !(0.0..=1.0).contains(&threshold_val) {
            return Err(Error::from_reason(
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
            return Err(Error::from_reason("embedding must not be empty"));
        }
        if embedding.iter().any(|v| !v.is_finite()) {
            return Err(Error::from_reason(
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

        let _ = broadcaster.publish(SyncEvent::NodeUpsert {
            id: node_id.clone(),
        });

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
            Error::from_reason("embed() requires a database created with newWithEmbeddings()")
        })?;
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let vecs = embedder
            .embed(&refs)
            .map_err(|e| Error::from_reason(format!("embedding failed: {}", e)))?;
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
            .map_err(|e| Error::from_reason(format!("exec_dsl error: {}", e)))?;
        serde_json::to_value(&result)
            .map_err(|e| Error::from_reason(format!("serialisation error: {}", e)))
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
            .map_err(|e| Error::from_reason(format!("exec_ir error: {}", e)))?;
        serde_json::to_value(&result)
            .map_err(|e| Error::from_reason(format!("serialisation error: {}", e)))
    }

    /// Build the HNSW vector index from hydrated embeddings.
    /// Call after init to enable vector search without blocking startup.
    #[napi]
    pub fn build_vector_index(&self) -> u32 {
        let store = self.store.lock();
        store.build_vector_index() as u32
    }

    /// Get database statistics
    #[napi]
    pub fn stats(&self) -> Result<serde_json::Value> {
        let store = self.store.clone();

        let records = {
            let store = store.lock();
            store.list()
        };

        let mut type_counts: HashMap<String, u32> = HashMap::new();
        for record in &records {
            if let Some(t) = record.data.get("type").and_then(|v| v.as_str()) {
                *type_counts.entry(t.to_string()).or_insert(0) += 1;
            }
        }

        Ok(serde_json::json!({
            "totalNodes": records.len(),
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
            .map_err(|e| Error::from_reason(format!("invalid AgensEvent JSON: {}", e)))?;
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
            .map_err(|e| Error::from_reason(format!("invalid AgensEvent JSON: {}", e)))?;
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        Ok(runtime.emit_praxis_event(&ev))
    }

    /// Poll the Agens command table for events after `sinceIso`.
    ///
    /// `since_iso` must be an ISO 8601 timestamp (e.g. `"2026-04-05T12:00:00Z"`).
    /// Returns events oldest-first as a JSON array.
    ///
    /// # Example (JavaScript)
    ///
    /// ```js
    /// const events = db.agensListEvents('2026-04-05T00:00:00Z');
    /// for (const ev of events) {
    ///   console.log(ev.event_type, ev.id);
    /// }
    /// ```
    #[napi]
    pub fn agens_list_events(
        &self,
        since_iso: String,
    ) -> Result<Vec<serde_json::Value>> {
        let since: chrono::DateTime<chrono::Utc> = since_iso
            .parse()
            .map_err(|e| Error::from_reason(format!("invalid ISO 8601 timestamp: {}", e)))?;
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        let events = runtime.poll_events(since);
        events
            .into_iter()
            .map(|ev| {
                serde_json::to_value(&ev)
                    .map_err(|e| Error::from_reason(format!("serialization error: {}", e)))
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
    pub fn agens_state_watch(
        &self,
        since_iso: String,
    ) -> Result<Vec<serde_json::Value>> {
        let since: chrono::DateTime<chrono::Utc> = since_iso
            .parse()
            .map_err(|e| Error::from_reason(format!("invalid ISO 8601 timestamp: {}", e)))?;
        let store = self.store.lock();
        let runtime = AgensRuntime::new(&store, self.actor_id.as_str());
        Ok(runtime
            .state()
            .watch(since)
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
            return Err(Error::from_reason(
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
