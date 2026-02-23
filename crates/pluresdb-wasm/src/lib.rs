/*!
 * PluresDB WebAssembly Bindings
 *
 * This crate provides WebAssembly bindings for PluresDB, enabling
 * local-first database functionality directly in the browser.
 *
 * # Features
 *
 * - Zero network overhead - runs directly in browser
 * - No server process required
 * - Offline-first by default
 * - IndexedDB for persistence
 *
 * # Example
 *
 * ```javascript
 * import init, { PluresDBBrowser } from "@plures/pluresdb-wasm";
 *
 * await init(); // Initialize WASM
 * const db = new PluresDBBrowser("my-app-db");
 *
 * await db.put("user:1", { name: "Alice", email: "alice@example.com" });
 * const user = await db.get("user:1");
 * ```
 */

use std::cell::RefCell;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasm_bindgen::prelude::*;

mod indexeddb;
use indexeddb::IndexedDBStore;

// Set panic hook for better error messages in the browser
pub use console_error_panic_hook::set_once as set_panic_hook;

/// Unique identifier for a stored node.
pub type NodeId = String;

/// Logical actor identifier used when merging CRDT updates.
pub type ActorId = String;

/// A key-value map of logical clocks per actor.
pub type VectorClock = HashMap<ActorId, u64>;

/// Metadata associated with a persisted node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRecord {
    pub id: NodeId,
    pub data: Value,
    pub clock: VectorClock,
    /// Unix milliseconds timestamp (from js_sys::Date::now()).
    pub timestamp_ms: f64,
}

impl NodeRecord {
    fn new(id: NodeId, actor: ActorId, data: Value) -> Self {
        let mut clock = VectorClock::default();
        clock.insert(actor, 1);
        Self {
            id,
            data,
            clock,
            timestamp_ms: js_sys::Date::now(),
        }
    }

    fn merge_update(&mut self, actor: ActorId, data: Value) {
        let counter = self.clock.entry(actor).or_insert(0);
        *counter += 1;
        self.timestamp_ms = js_sys::Date::now();
        self.data = data;
    }
}

/// Simple in-memory CRDT store (no native threads, safe for WASM).
#[derive(Default)]
struct SimpleStore {
    nodes: HashMap<NodeId, NodeRecord>,
}

impl SimpleStore {
    fn put(&mut self, id: NodeId, actor: ActorId, data: Value) -> NodeId {
        if let Some(record) = self.nodes.get_mut(&id) {
            record.merge_update(actor, data);
        } else {
            self.nodes.insert(id.clone(), NodeRecord::new(id.clone(), actor, data));
        }
        id
    }

    fn get(&self, id: &str) -> Option<&NodeRecord> {
        self.nodes.get(id)
    }

    fn delete(&mut self, id: &str) -> bool {
        self.nodes.remove(id).is_some()
    }

    fn list(&self) -> Vec<&NodeRecord> {
        self.nodes.values().collect()
    }
}

/// Initialize WASM module (call this before using PluresDB)
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// PluresDB browser database instance
///
/// This is the main entry point for using PluresDB in the browser.
/// It wraps a simple CRDT store and provides a JavaScript-friendly API.
/// Data is persisted to IndexedDB automatically.
#[wasm_bindgen]
pub struct PluresDBBrowser {
    store: RefCell<SimpleStore>,
    db_name: String,
    persistence: Option<IndexedDBStore>,
}

#[wasm_bindgen]
impl PluresDBBrowser {
    /// Create a new PluresDB instance
    ///
    /// # Arguments
    ///
    /// * `db_name` - Name of the database (used for IndexedDB storage)
    ///
    /// Note: This creates the instance but persistence is initialized lazily.
    /// Call `init_persistence()` to enable IndexedDB persistence.
    #[wasm_bindgen(constructor)]
    pub fn new(db_name: String) -> Result<PluresDBBrowser, JsValue> {
        Ok(PluresDBBrowser {
            store: RefCell::new(SimpleStore::default()),
            db_name,
            persistence: None,
        })
    }

    /// Initialize IndexedDB persistence
    ///
    /// This should be called after creating the instance to enable
    /// automatic persistence to IndexedDB.
    #[wasm_bindgen]
    pub async fn init_persistence(&mut self) -> Result<(), JsValue> {
        let idb = IndexedDBStore::open(&self.db_name).await?;

        // Load existing data from IndexedDB into the store
        let keys = idb.get_all_keys().await?;
        for key in keys {
            if let Some(value) = idb.get(&key).await? {
                let mut store = self.store.borrow_mut();
                store.put(key, "browser".to_string(), value);
            }
        }

        self.persistence = Some(idb);
        Ok(())
    }

    /// Insert or update a node in the database
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the node
    /// * `data` - JSON data to store (will be serialized)
    ///
    /// # Returns
    ///
    /// Node ID on success
    #[wasm_bindgen]
    pub async fn put(&mut self, id: String, data: JsValue) -> Result<String, JsValue> {
        let data_value: Value = serde_wasm_bindgen::from_value(data)
            .map_err(|e| JsValue::from_str(&format!("Failed to deserialize data: {}", e)))?;

        let node_id = {
            let mut store = self.store.borrow_mut();
            store.put(id.clone(), "wasm".to_string(), data_value.clone())
        };

        // Persist to IndexedDB if enabled
        if let Some(ref idb) = self.persistence {
            idb.put(&node_id, &data_value).await?;
        }

        Ok(node_id)
    }

    /// Retrieve a node from the database
    ///
    /// # Arguments
    ///
    /// * `id` - Node identifier
    ///
    /// # Returns
    ///
    /// Node data as JSON, or null if not found
    #[wasm_bindgen]
    pub fn get(&self, id: String) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();

        match store.get(&id) {
            Some(record) => serde_wasm_bindgen::to_value(&record.data)
                .map_err(|e| JsValue::from_str(&format!("Failed to serialize data: {}", e))),
            None => Ok(JsValue::NULL),
        }
    }

    /// Delete a node from the database
    ///
    /// # Arguments
    ///
    /// * `id` - Node identifier
    #[wasm_bindgen]
    pub async fn delete(&mut self, id: String) -> Result<(), JsValue> {
        {
            let mut store = self.store.borrow_mut();
            store.delete(&id);
        }

        // Delete from IndexedDB if enabled
        if let Some(ref idb) = self.persistence {
            idb.delete(&id).await?;
        }

        Ok(())
    }

    /// List all nodes in the database
    ///
    /// # Returns
    ///
    /// Array of all nodes with their data
    #[wasm_bindgen]
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let store = self.store.borrow();
        let records = store.list();

        let nodes: Vec<Value> = records
            .into_iter()
            .map(|record| {
                serde_json::json!({
                    "id": record.id,
                    "data": record.data,
                    "timestamp_ms": record.timestamp_ms,
                })
            })
            .collect();

        serde_wasm_bindgen::to_value(&nodes)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize list: {}", e)))
    }

    /// Get the database name
    #[wasm_bindgen(getter)]
    pub fn db_name(&self) -> String {
        self.db_name.clone()
    }

    /// Get the number of nodes in the database
    #[wasm_bindgen]
    pub fn count(&self) -> usize {
        let store = self.store.borrow();
        store.list().len()
    }

    /// Clear all data from the database (memory and IndexedDB)
    #[wasm_bindgen]
    pub async fn clear(&mut self) -> Result<(), JsValue> {
        {
            let mut store = self.store.borrow_mut();
            store.nodes.clear();
        }

        // Clear IndexedDB if enabled
        if let Some(ref idb) = self.persistence {
            idb.clear().await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_put_and_get() {
        let mut db = PluresDBBrowser::new("test-db".to_string()).unwrap();

        // put is async; exercise it via the inner store directly in unit tests
        {
            let mut store = db.store.borrow_mut();
            store.put("user:1".to_string(), "test".to_string(), serde_json::json!({"name": "Alice"}));
        }

        let retrieved = db.get("user:1".to_string()).unwrap();
        assert!(!retrieved.is_null());
    }

    #[wasm_bindgen_test]
    fn test_delete() {
        let mut db = PluresDBBrowser::new("test-db".to_string()).unwrap();

        {
            let mut store = db.store.borrow_mut();
            store.put("user:2".to_string(), "test".to_string(), serde_json::json!({"name": "Bob"}));
        }
        assert_eq!(db.count(), 1);

        {
            let mut store = db.store.borrow_mut();
            store.delete("user:2");
        }

        let retrieved = db.get("user:2".to_string()).unwrap();
        assert!(retrieved.is_null());
    }

    #[wasm_bindgen_test]
    fn test_list() {
        let mut db = PluresDBBrowser::new("test-db".to_string()).unwrap();

        {
            let mut store = db.store.borrow_mut();
            store.put("user:1".to_string(), "test".to_string(), serde_json::json!({"name": "Alice"}));
            store.put("user:2".to_string(), "test".to_string(), serde_json::json!({"name": "Bob"}));
        }

        assert_eq!(db.count(), 2);

        let list = db.list().unwrap();
        assert!(!list.is_null());
    }
}

