//! WebAssembly bindings for PluresDB.
//!
//! Thin wasm-bindgen wrapper around `pluresdb_core::CrdtStore` with
//! `pluresdb_storage::MemoryStorage`. IndexedDB persistence is planned
//! for v3.1; the module is kept on disk but not wired in yet.

use std::sync::Arc;

use pluresdb_core::CrdtStore;
use pluresdb_storage::MemoryStorage;
use serde_wasm_bindgen::{from_value, to_value};
use wasm_bindgen::prelude::*;

// Keep the indexeddb module on disk for future use.
#[cfg(any())]
mod indexeddb;

/// Initialise panic hook so Rust panics show up in the browser console.
#[wasm_bindgen]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Browser-side PluresDB backed by an in-memory CRDT store.
#[wasm_bindgen]
pub struct PluresDBBrowser {
    store: CrdtStore,
    actor_id: String,
}

#[wasm_bindgen]
impl PluresDBBrowser {
    /// Create a new in-memory PluresDB instance.
    ///
    /// `db_name` is reserved for future IndexedDB persistence.
    /// `actor_id` defaults to `"browser"` when not supplied.
    #[wasm_bindgen(constructor)]
    pub fn new(_db_name: &str, actor_id: Option<String>) -> Self {
        console_error_panic_hook::set_once();
        let actor = actor_id.unwrap_or_else(|| "browser".to_string());
        let storage = Arc::new(MemoryStorage::default());
        let store = CrdtStore::default().with_persistence(storage);
        Self {
            store,
            actor_id: actor,
        }
    }

    /// Insert or update a record. Returns the node id.
    pub fn put(&self, id: &str, data: JsValue) -> Result<String, JsValue> {
        let json: serde_json::Value =
            from_value(data).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let node_id = self.store.put(id, &self.actor_id, json);
        Ok(node_id)
    }

    /// Insert or update a record with a pre-computed embedding vector.
    pub fn put_with_embedding(
        &self,
        id: &str,
        data: JsValue,
        embedding: Vec<f32>,
    ) -> Result<String, JsValue> {
        let json: serde_json::Value =
            from_value(data).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let node_id =
            self.store
                .put_with_embedding(id, &self.actor_id, json, embedding);
        Ok(node_id)
    }

    /// Retrieve a record by id. Returns `null` if not found.
    pub fn get(&self, id: &str) -> Result<JsValue, JsValue> {
        match self.store.get(id) {
            Some(record) => to_value(&record).map_err(|e| JsValue::from_str(&e.to_string())),
            None => Ok(JsValue::NULL),
        }
    }

    /// Delete a record. Silently succeeds if the id does not exist.
    pub fn delete(&self, id: &str) {
        let _ = self.store.delete(id);
    }

    /// List all records as a JS array.
    pub fn list(&self) -> Result<JsValue, JsValue> {
        let records = self.store.list();
        to_value(&records).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Approximate nearest-neighbour search over stored embeddings.
    ///
    /// Returns a JS array of `{ record, score }` objects.
    pub fn vector_search(
        &self,
        query_embedding: Vec<f32>,
        limit: usize,
        min_score: Option<f32>,
    ) -> Result<JsValue, JsValue> {
        let results =
            self.store
                .vector_search(&query_embedding, limit, min_score.unwrap_or(0.0));
        // VectorSearchResult doesn't derive Serialize, so build JS array manually.
        let arr = js_sys::Array::new();
        for r in results {
            let obj = js_sys::Object::new();
            let record_js =
                to_value(&r.record).map_err(|e| JsValue::from_str(&e.to_string()))?;
            js_sys::Reflect::set(&obj, &JsValue::from_str("record"), &record_js)
                .map_err(|e| e)?;
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("score"),
                &JsValue::from_f64(r.score as f64),
            )
            .map_err(|e| e)?;
            arr.push(&obj);
        }
        Ok(arr.into())
    }

    /// Number of records currently stored.
    pub fn node_count(&self) -> usize {
        self.store.list().len()
    }
}
