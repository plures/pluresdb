//! Deno bindings for PluresDB.
//!
//! This crate provides Deno bindings using deno_bindgen to expose
//! PluresDB functionality to Deno applications.

use deno_bindgen::deno_bindgen;
use pluresdb_core::{
    CrdtOperation, CrdtStore, Database, DatabaseOptions, NodeRecord, SqlValue,
};
use pluresdb_sync::{SyncBroadcaster, SyncEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Value>,
    pub changes: u64,
    pub last_insert_rowid: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub changes: u64,
    pub last_insert_rowid: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeWithMetadata {
    pub id: String,
    pub data: serde_json::Value,
    pub clock: HashMap<String, u64>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub data: serde_json::Value,
    pub score: usize,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub total_nodes: u64,
    pub type_counts: HashMap<String, u32>,
}

/// PluresDB database instance for Deno
#[deno_bindgen]
pub struct PluresDatabase {
    store: Arc<Mutex<CrdtStore>>,
    db: Option<Arc<Database>>,
    broadcaster: Arc<SyncBroadcaster>,
    actor_id: String,
}

#[deno_bindgen]
impl PluresDatabase {
    /// Create a new PluresDB instance
    #[deno_bindgen(constructor)]
    pub fn new(actor_id: Option<String>, db_path: Option<String>) -> Result<Self, String> {
        let actor_id = actor_id.unwrap_or_else(|| "deno-actor".to_string());
        let db = if let Some(path) = db_path {
            let options = DatabaseOptions::with_file(path).create_if_missing(true);
            Some(Arc::new(
                Database::open(options)
                    .map_err(|e| format!("Failed to open database: {}", e))?,
            ))
        } else {
            None
        };

        Ok(Self {
            store: Arc::new(Mutex::new(CrdtStore::default())),
            db,
            broadcaster: Arc::new(SyncBroadcaster::default()),
            actor_id,
        })
    }

    /// Insert or update a node
    #[deno_bindgen]
    pub fn put(&self, id: String, data: serde_json::Value) -> Result<String, String> {
        let store = self.store.clone();
        let broadcaster = self.broadcaster.clone();
        let actor_id = self.actor_id.clone();
        
        let node_id = {
            let store = store.lock();
            store.put(id.clone(), actor_id, data)
        };
        
        // Publish sync event
        let _ = broadcaster.publish(SyncEvent::NodeUpsert { id: node_id.clone() });
        
        Ok(node_id)
    }

    /// Retrieve a node by ID
    #[deno_bindgen]
    pub fn get(&self, id: String) -> Result<Option<serde_json::Value>, String> {
        let store = self.store.clone();
        
        let record = {
            let store = store.lock();
            store.get(id)
        };
        
        match record {
            Some(record) => Ok(Some(record.data)),
            None => Ok(None),
        }
    }

    /// Get a node with full metadata (including vector clock and timestamp)
    #[deno_bindgen]
    pub fn get_with_metadata(&self, id: String) -> Result<Option<NodeWithMetadata>, String> {
        let store = self.store.clone();
        
        let record = {
            let store = store.lock();
            store.get(id)
        };
        
        match record {
            Some(record) => {
                Ok(Some(NodeWithMetadata {
                    id: record.id,
                    data: record.data,
                    clock: record.clock,
                    timestamp: record.timestamp.to_rfc3339(),
                }))
            }
            None => Ok(None),
        }
    }

    /// Delete a node by ID
    #[deno_bindgen]
    pub fn delete(&self, id: String) -> Result<(), String> {
        let store = self.store.clone();
        let broadcaster = self.broadcaster.clone();
        
        {
            let store = store.lock();
            store.delete(&id)
                .map_err(|e| format!("Delete error: {}", e))?;
        }
        
        // Publish sync event
        let _ = broadcaster.publish(SyncEvent::NodeDelete { id: id.clone() });
        
        Ok(())
    }

    /// List all nodes
    #[deno_bindgen]
    pub fn list(&self) -> Result<Vec<serde_json::Value>, String> {
        let store = self.store.clone();
        
        let records = {
            let store = store.lock();
            store.list()
        };
        
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
    #[deno_bindgen]
    pub fn list_by_type(&self, node_type: String) -> Result<Vec<serde_json::Value>, String> {
        let store = self.store.clone();
        
        let records = {
            let store = store.lock();
            store.list()
        };
        
        let result: Vec<serde_json::Value> = records
            .into_iter()
            .filter(|record| {
                record.data
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
    #[deno_bindgen]
    pub fn query(
        &self,
        sql: String,
        params: Option<Vec<serde_json::Value>>,
    ) -> Result<QueryResult, String> {
        let db = self.db.as_ref()
            .ok_or_else(|| "SQL queries require a database (provide db_path in constructor)".to_string())?;
        
        let sql_params: Vec<SqlValue> = if let Some(p) = params {
            p.into_iter()
                .map(|v| -> Result<SqlValue, String> {
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
                            SqlValue::Text(serde_json::to_string(&v)
                                .map_err(|e| format!("Failed to serialize param: {}", e))?)
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![]
        };
        
        let result = db.query(&sql, &sql_params)
            .map_err(|e| format!("Query error: {}", e))?;
        
        Ok(QueryResult {
            columns: result.columns,
            rows: result.rows_as_json(),
            changes: result.changes,
            last_insert_rowid: result.last_insert_rowid,
        })
    }

    /// Execute SQL statement (INSERT, UPDATE, DELETE)
    #[deno_bindgen]
    pub fn exec(&self, sql: String) -> Result<ExecutionResult, String> {
        let db = self.db.as_ref()
            .ok_or_else(|| "SQL execution requires a database (provide db_path in constructor)".to_string())?;
        
        let result = db.exec(&sql)
            .map_err(|e| format!("Execution error: {}", e))?;
        
        Ok(ExecutionResult {
            changes: result.changes,
            last_insert_rowid: result.last_insert_rowid,
        })
    }

    /// Search nodes by text content
    #[deno_bindgen]
    pub fn search(&self, query: String, limit: Option<u32>) -> Result<Vec<SearchResult>, String> {
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
        
        let result: Vec<SearchResult> = matches
            .into_iter()
            .map(|(record, score)| {
                SearchResult {
                    id: record.id,
                    data: record.data,
                    score,
                    timestamp: record.timestamp.to_rfc3339(),
                }
            })
            .collect();
        
        Ok(result)
    }

    /// Vector similarity search (placeholder - returns text search results)
    #[deno_bindgen]
    pub fn vector_search(
        &self,
        query: String,
        limit: Option<u32>,
        _threshold: Option<f64>,
    ) -> Result<Vec<SearchResult>, String> {
        // For now, vector search falls back to text search
        // In the future, this will use actual vector embeddings
        self.search(query, limit)
    }

    /// Get the actor ID for this database instance
    #[deno_bindgen]
    pub fn get_actor_id(&self) -> String {
        self.actor_id.clone()
    }

    /// Get database statistics
    #[deno_bindgen]
    pub fn stats(&self) -> Result<DatabaseStats, String> {
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
        
        Ok(DatabaseStats {
            total_nodes: records.len() as u64,
            type_counts,
        })
    }
}

/// Initialize the module
#[deno_bindgen]
pub fn init() -> Result<(), String> {
    Ok(())
}
