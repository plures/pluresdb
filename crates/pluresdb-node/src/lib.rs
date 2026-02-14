//! Node.js bindings for PluresDB.
//!
//! This crate provides Node.js bindings using N-API to expose
//! PluresDB functionality to Node.js applications.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use pluresdb_core::{
    CrdtStore, Database, DatabaseOptions, NodeRecord, SqlValue,
};
use pluresdb_sync::{SyncBroadcaster, SyncEvent};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;

/// PluresDB database instance for Node.js
#[napi]
pub struct PluresDatabase {
    store: Arc<Mutex<CrdtStore>>,
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
        let db = if let Some(path) = db_path {
            let options = DatabaseOptions::with_file(path).create_if_missing(true);
            Some(Arc::new(
                Database::open(options)
                    .map_err(|e| Error::from_reason(format!("Failed to open database: {}", e)))?,
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
        let _ = broadcaster.publish(SyncEvent::NodeUpsert { id: node_id.clone() });
        
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
            Some(record) => {
                Ok(Some(serde_json::json!({
                    "id": record.id,
                    "data": record.data,
                    "clock": record.clock,
                    "timestamp": record.timestamp.to_rfc3339(),
                })))
            }
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
            store.delete(&id)
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
    #[napi]
    pub fn query(&self, sql: String, params: Option<Vec<serde_json::Value>>) -> Result<serde_json::Value> {
        let db = self.db.as_ref()
            .ok_or_else(|| Error::from_reason("SQL queries require a database (provide db_path in constructor)".to_string()))?;
        
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
                            SqlValue::Text(serde_json::to_string(&v)
                                .map_err(|e| Error::from_reason(format!("Failed to serialize param: {}", e)))?)
                        }
                    })
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            vec![]
        };
        
        let result = db.query(&sql, &sql_params)
            .map_err(|e| Error::from_reason(format!("Query error: {}", e)))?;
        
        Ok(serde_json::json!({
            "columns": result.columns,
            "rows": result.rows_as_json(),
            "changes": result.changes,
            "lastInsertRowid": result.last_insert_rowid
        }))
    }

    /// Execute SQL statement (INSERT, UPDATE, DELETE)
    #[napi]
    pub fn exec(&self, sql: String) -> Result<serde_json::Value> {
        let db = self.db.as_ref()
            .ok_or_else(|| Error::from_reason("SQL execution requires a database (provide db_path in constructor)".to_string()))?;
        
        let result = db.exec(&sql)
            .map_err(|e| Error::from_reason(format!("Execution error: {}", e)))?;
        
        Ok(serde_json::json!({
            "changes": result.changes,
            "lastInsertRowid": result.last_insert_rowid
        }))
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

    /// Vector similarity search (placeholder - returns text search results)
    #[napi]
    pub fn vector_search(&self, query: String, limit: Option<u32>, _threshold: Option<f64>) -> Result<Vec<serde_json::Value>> {
        // For now, vector search falls back to text search
        // In the future, this will use actual vector embeddings
        self.search(query, limit)
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

    /// Get the actor ID for this database instance
    #[napi]
    pub fn get_actor_id(&self) -> String {
        self.actor_id.clone()
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
}

/// Initialize the module
#[napi]
pub fn init() -> Result<()> {
    Ok(())
}
