//! Node.js bindings for PluresDB.
//!
//! This crate provides Node.js bindings using N-API to expose
//! PluresDB functionality to Node.js applications.

use napi::bindgen_prelude::*;
use napi_derive::napi;
use pluresdb_core::CrdtStore;
use std::sync::Arc;
use parking_lot::Mutex;

/// PluresDB database instance for Node.js
#[napi]
pub struct PluresDatabase {
    store: Arc<Mutex<CrdtStore>>,
    actor_id: String,
}

#[napi]
impl PluresDatabase {
    /// Create a new PluresDB instance
    #[napi(constructor)]
    pub fn new(actor_id: Option<String>) -> Self {
        Self {
            store: Arc::new(Mutex::new(CrdtStore::default())),
            actor_id: actor_id.unwrap_or_else(|| "node-actor".to_string()),
        }
    }

    /// Insert or update a node
    #[napi]
    pub fn put(&self, id: String, data: serde_json::Value) -> Result<String> {
        let store = self.store.clone();
        let actor_id = self.actor_id.clone();
        
        let node_id = {
            let store = store.lock();
            store.put(id, actor_id, data)
        };
        
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

    /// Delete a node by ID
    #[napi]
    pub fn delete(&self, id: String) -> Result<()> {
        let store = self.store.clone();
        
        {
            let store = store.lock();
            store.delete(id)
                .map_err(|e| Error::from_reason(format!("Delete error: {}", e)))?;
        }
        
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

    /// Get the actor ID for this database instance
    #[napi]
    pub fn get_actor_id(&self) -> String {
        self.actor_id.clone()
    }
}

/// Initialize the module
#[napi]
pub fn init() -> Result<()> {
    Ok(())
}
