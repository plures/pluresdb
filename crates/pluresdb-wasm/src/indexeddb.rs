/*!
 * IndexedDB persistence layer for PluresDB WASM
 *
 * This module provides IndexedDB integration for persisting CRDT data
 * in the browser's local storage.
 */

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{IdbDatabase, IdbObjectStore, IdbRequest, IdbTransaction, IdbTransactionMode};
use serde_json::Value;

/// IndexedDB database wrapper
pub struct IndexedDBStore {
    db: IdbDatabase,
    db_name: String,
}

impl IndexedDBStore {
    /// Open or create an IndexedDB database
    pub async fn open(db_name: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object"))?;
        let idb_factory = window
            .indexed_db()
            .map_err(|e| JsValue::from_str(&format!("IndexedDB not supported: {:?}", e)))?
            .ok_or_else(|| JsValue::from_str("IndexedDB not available"))?;

        // Open database with version 1
        let open_request = idb_factory
            .open_with_u32(db_name, 1)
            .map_err(|e| JsValue::from_str(&format!("Failed to open database: {:?}", e)))?;

        // Handle database upgrade (create object store if needed)
        let on_upgrade_needed = Closure::wrap(Box::new(move |event: web_sys::IdbVersionChangeEvent| {
            let target = event.target().expect("Event should have target");
            let request = target
                .dyn_into::<web_sys::IdbOpenDbRequest>()
                .expect("Event target should be IdbOpenDbRequest");
            let db = request
                .result()
                .expect("Request should have result")
                .dyn_into::<IdbDatabase>()
                .expect("Result should be IdbDatabase");

            // Create object store if it doesn't exist
            if !db.object_store_names().contains("nodes") {
                let _ = db.create_object_store("nodes");
            }
        }) as Box<dyn FnMut(_)>);

        open_request.set_onupgradeneeded(Some(on_upgrade_needed.as_ref().unchecked_ref()));
        on_upgrade_needed.forget();

        // Wait for the database to open
        let db_promise = JsFuture::from(open_request);
        let db_result = db_promise.await?;
        let db = db_result
            .dyn_into::<IdbDatabase>()
            .map_err(|_| JsValue::from_str("Failed to cast to IdbDatabase"))?;

        Ok(Self {
            db,
            db_name: db_name.to_string(),
        })
    }

    /// Get a value from IndexedDB
    pub async fn get(&self, key: &str) -> Result<Option<Value>, JsValue> {
        let transaction = self
            .db
            .transaction_with_str("nodes")
            .map_err(|e| JsValue::from_str(&format!("Failed to create transaction: {:?}", e)))?;
        
        let object_store = transaction
            .object_store("nodes")
            .map_err(|e| JsValue::from_str(&format!("Failed to get object store: {:?}", e)))?;

        let request = object_store
            .get(&JsValue::from_str(key))
            .map_err(|e| JsValue::from_str(&format!("Failed to get value: {:?}", e)))?;

        let result = JsFuture::from(request).await?;

        if result.is_null() || result.is_undefined() {
            return Ok(None);
        }

        // Convert JsValue to JSON Value
        let json_string = js_sys::JSON::stringify(&result)
            .map_err(|e| JsValue::from_str(&format!("Failed to stringify: {:?}", e)))?;
        let json_str = json_string
            .as_string()
            .ok_or_else(|| JsValue::from_str("Failed to convert to string"))?;
        let value: Value = serde_json::from_str(&json_str)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse JSON: {}", e)))?;

        Ok(Some(value))
    }

    /// Put a value into IndexedDB
    pub async fn put(&self, key: &str, value: &Value) -> Result<(), JsValue> {
        let transaction = self
            .db
            .transaction_with_str_and_mode("nodes", IdbTransactionMode::Readwrite)
            .map_err(|e| JsValue::from_str(&format!("Failed to create transaction: {:?}", e)))?;

        let object_store = transaction
            .object_store("nodes")
            .map_err(|e| JsValue::from_str(&format!("Failed to get object store: {:?}", e)))?;

        // Convert serde_json::Value to JsValue
        let js_value = serde_wasm_bindgen::to_value(value)
            .map_err(|e| JsValue::from_str(&format!("Failed to serialize: {}", e)))?;

        let request = object_store
            .put_with_key(&js_value, &JsValue::from_str(key))
            .map_err(|e| JsValue::from_str(&format!("Failed to put value: {:?}", e)))?;

        JsFuture::from(request).await?;

        Ok(())
    }

    /// Delete a value from IndexedDB
    pub async fn delete(&self, key: &str) -> Result<(), JsValue> {
        let transaction = self
            .db
            .transaction_with_str_and_mode("nodes", IdbTransactionMode::Readwrite)
            .map_err(|e| JsValue::from_str(&format!("Failed to create transaction: {:?}", e)))?;

        let object_store = transaction
            .object_store("nodes")
            .map_err(|e| JsValue::from_str(&format!("Failed to get object store: {:?}", e)))?;

        let request = object_store
            .delete(&JsValue::from_str(key))
            .map_err(|e| JsValue::from_str(&format!("Failed to delete value: {:?}", e)))?;

        JsFuture::from(request).await?;

        Ok(())
    }

    /// Get all keys from IndexedDB
    pub async fn get_all_keys(&self) -> Result<Vec<String>, JsValue> {
        let transaction = self
            .db
            .transaction_with_str("nodes")
            .map_err(|e| JsValue::from_str(&format!("Failed to create transaction: {:?}", e)))?;

        let object_store = transaction
            .object_store("nodes")
            .map_err(|e| JsValue::from_str(&format!("Failed to get object store: {:?}", e)))?;

        let request = object_store
            .get_all_keys()
            .map_err(|e| JsValue::from_str(&format!("Failed to get all keys: {:?}", e)))?;

        let result = JsFuture::from(request).await?;
        let keys_array = js_sys::Array::from(&result);

        // Use iterator methods for better performance
        let keys = (0..keys_array.length())
            .filter_map(|i| keys_array.get(i).as_string())
            .collect();

        Ok(keys)
    }

    /// Clear all data from IndexedDB
    pub async fn clear(&self) -> Result<(), JsValue> {
        let transaction = self
            .db
            .transaction_with_str_and_mode("nodes", IdbTransactionMode::Readwrite)
            .map_err(|e| JsValue::from_str(&format!("Failed to create transaction: {:?}", e)))?;

        let object_store = transaction
            .object_store("nodes")
            .map_err(|e| JsValue::from_str(&format!("Failed to get object store: {:?}", e)))?;

        let request = object_store
            .clear()
            .map_err(|e| JsValue::from_str(&format!("Failed to clear: {:?}", e)))?;

        JsFuture::from(request).await?;

        Ok(())
    }
}
