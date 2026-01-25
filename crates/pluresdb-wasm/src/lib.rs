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

use wasm_bindgen::prelude::*;
use pluresdb_core::CrdtStore;
use serde_json::Value;
use std::cell::RefCell;

mod indexeddb;
use indexeddb::IndexedDBStore;

// Set panic hook for better error messages in the browser
#[cfg(feature = "console_error_panic_hook")]
pub use console_error_panic_hook::set_once as set_panic_hook;

/// Initialize WASM module (call this before using PluresDB)
#[wasm_bindgen(start)]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

/// PluresDB browser database instance
///
/// This is the main entry point for using PluresDB in the browser.
/// It wraps the core CRDT store and provides a JavaScript-friendly API.
/// Data is persisted to IndexedDB automatically.
#[wasm_bindgen]
pub struct PluresDBBrowser {
    store: RefCell<CrdtStore>,
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
        let store = CrdtStore::default();
        
        Ok(PluresDBBrowser {
            store: RefCell::new(store),
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

        let mut store = self.store.borrow_mut();
        let node_id = store.put(id.clone(), "wasm".to_string(), data_value.clone());
        drop(store); // Release borrow before async operation
        
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
        
        match store.get(id) {
            Some(record) => {
                serde_wasm_bindgen::to_value(&record.data)
                    .map_err(|e| JsValue::from_str(&format!("Failed to serialize data: {}", e)))
            }
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
        let mut store = self.store.borrow_mut();
        store.delete(&id)
            .map_err(|e| JsValue::from_str(&format!("Failed to delete: {}", e)))?;
        drop(store); // Release borrow before async operation
        
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
                    "timestamp": record.timestamp.to_rfc3339(),
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
        // Clear in-memory store
        let mut store = self.store.borrow_mut();
        let keys: Vec<String> = store.list().iter().map(|r| r.id.clone()).collect();
        for key in keys {
            let _ = store.delete(&key);
        }
        drop(store);
        
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
        let db = PluresDBBrowser::new("test-db".to_string()).unwrap();
        
        let data = serde_wasm_bindgen::to_value(&serde_json::json!({
            "name": "Alice",
            "email": "alice@example.com"
        })).unwrap();
        
        let id = db.put("user:1".to_string(), data).unwrap();
        assert_eq!(id, "user:1");
        
        let retrieved = db.get("user:1".to_string()).unwrap();
        assert!(!retrieved.is_null());
    }

    #[wasm_bindgen_test]
    fn test_delete() {
        let db = PluresDBBrowser::new("test-db".to_string()).unwrap();
        
        let data = serde_wasm_bindgen::to_value(&serde_json::json!({
            "name": "Bob"
        })).unwrap();
        
        db.put("user:2".to_string(), data).unwrap();
        assert_eq!(db.count(), 1);
        
        db.delete("user:2".to_string()).unwrap();
        
        let retrieved = db.get("user:2".to_string()).unwrap();
        assert!(retrieved.is_null());
    }

    #[wasm_bindgen_test]
    fn test_list() {
        let db = PluresDBBrowser::new("test-db".to_string()).unwrap();
        
        let data1 = serde_wasm_bindgen::to_value(&serde_json::json!({"name": "Alice"})).unwrap();
        let data2 = serde_wasm_bindgen::to_value(&serde_json::json!({"name": "Bob"})).unwrap();
        
        db.put("user:1".to_string(), data1).unwrap();
        db.put("user:2".to_string(), data2).unwrap();
        
        assert_eq!(db.count(), 2);
        
        let list = db.list().unwrap();
        assert!(!list.is_null());
    }
}
