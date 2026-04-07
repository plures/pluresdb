//! WebAssembly bindings for PluresDB.
//!
//! Thin wasm-bindgen wrapper around `pluresdb_core::CrdtStore` with
//! `pluresdb_storage::MemoryStorage`. IndexedDB persistence is planned
//! for a future release; the module is kept on disk but not wired in yet.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use js_sys::{Function, Object};
use pluresdb_core::CrdtStore;
use pluresdb_procedures::agens::{AgensEvent, AgensRuntime, StateTable, TimerTable};
use pluresdb_procedures::engine::ProcedureEngine;
use pluresdb_procedures::ir::Step;
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
    store: Arc<CrdtStore>,
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
        let store = Arc::new(CrdtStore::default().with_persistence(storage));
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
        let node_id = self
            .store
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
        let results = self
            .store
            .vector_search(&query_embedding, limit, min_score.unwrap_or(0.0));
        // VectorSearchResult doesn't derive Serialize, so build JS array manually.
        let arr = js_sys::Array::new();
        for r in results {
            let obj = js_sys::Object::new();
            let record_js = to_value(&r.record).map_err(|e| JsValue::from_str(&e.to_string()))?;
            js_sys::Reflect::set(&obj, &JsValue::from_str("record"), &record_js)?;
            js_sys::Reflect::set(
                &obj,
                &JsValue::from_str("score"),
                &JsValue::from_f64(r.score as f64),
            )?;
            arr.push(&obj);
        }
        Ok(arr.into())
    }

    /// Number of records currently stored.
    pub fn node_count(&self) -> usize {
        self.store.list().len()
    }
}

/// Shared CRDT store for wasm runtimes.
#[wasm_bindgen]
pub struct WasmCrdtStore {
    store: Arc<CrdtStore>,
}

#[wasm_bindgen]
impl WasmCrdtStore {
    /// Create a new in-memory CRDT store.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        let storage = Arc::new(MemoryStorage::default());
        let store = CrdtStore::default().with_persistence(storage);
        Self {
            store: Arc::new(store),
        }
    }

    /// Create another wasm handle to the same underlying CRDT store.
    #[wasm_bindgen(js_name = cloneStore)]
    pub fn clone_store(&self) -> Self {
        Self {
            store: Arc::clone(&self.store),
        }
    }
}

impl WasmCrdtStore {
    /// Return a cloned shared store for internal Rust-side delegation.
    ///
    /// This helper is intentionally kept for upcoming Rust-side integrations
    /// that need access to the underlying shared store without exposing it to
    /// the wasm-bindgen public API.
    #[allow(dead_code)]
    pub(crate) fn shared(&self) -> Arc<CrdtStore> {
        Arc::clone(&self.store)
    }
}
/// wasm-bindgen wrapper around the Agens runtime.
#[wasm_bindgen]
pub struct WasmAgensRuntime {
    store: Arc<CrdtStore>,
    actor: String,
    handlers: RefCell<HashMap<String, Function>>,
    state_watchers: RefCell<HashMap<String, Vec<Function>>>,
}

#[wasm_bindgen]
impl WasmAgensRuntime {
    /// Internal constructor: wraps an existing shared store reference.
    fn from_shared_store(store: Arc<CrdtStore>, actor: String) -> Self {
        console_error_panic_hook::set_once();
        Self {
            store,
            actor,
            handlers: RefCell::new(HashMap::new()),
            state_watchers: RefCell::new(HashMap::new()),
        }
    }

    /// Create a new Agens runtime bound to a shared CRDT store.
    #[wasm_bindgen(constructor)]
    pub fn new(store: &WasmCrdtStore, actor: String) -> Self {
        Self::from_shared_store(store.store.clone(), actor)
    }

    /// Create a new Agens runtime bound to the same CRDT store used by a browser wrapper.
    #[wasm_bindgen(js_name = fromBrowser)]
    pub fn from_browser(browser: &PluresDBBrowser, actor: String) -> Self {
        Self::from_shared_store(browser.store.clone(), actor)
    }
    /// Emit an Agens event into the command log.
    pub fn emit_event(&self, event: JsValue) -> Result<String, JsValue> {
        let event: AgensEvent = from_value(event).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let runtime = AgensRuntime::new(self.store.as_ref(), self.actor.as_str());
        Ok(runtime.emit_event(&event))
    }

    /// Emit a Praxis lifecycle event using idempotent storage.
    pub fn emit_praxis_event(&self, event: JsValue) -> Result<String, JsValue> {
        let event: AgensEvent = from_value(event).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let runtime = AgensRuntime::new(self.store.as_ref(), self.actor.as_str());
        Ok(runtime.emit_praxis_event(&event))
    }

    /// Register a JS handler for the given event type.
    pub fn register_procedure(&self, event_type: &str, callback: Function) {
        self.handlers
            .borrow_mut()
            .insert(event_type.to_string(), callback);
    }

    /// Unregister the JS handler for the given event type.
    ///
    /// Returns `true` if a handler was removed.
    pub fn unregister_procedure(&self, event_type: &str) -> bool {
        self.handlers.borrow_mut().remove(event_type).is_some()
    }

    /// Clear all registered JS procedure handlers.
    pub fn clear_procedures(&self) {
        self.handlers.borrow_mut().clear();
    }
    /// Execute the registered procedure handler for the given event.
    pub fn execute_procedure(&self, event: JsValue) -> Result<(), JsValue> {
        let parsed: AgensEvent =
            from_value(event.clone()).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let event_type = parsed.event_type();
        let handler = { self.handlers.borrow().get(event_type).cloned() };
        match handler {
            Some(cb) => {
                cb.call1(&JsValue::NULL, &event)?;
                Ok(())
            }
            None => Err(JsValue::from_str(&format!(
                "no handler registered for event type '{}'",
                event_type
            ))),
        }
    }

    /// Poll events that arrived after the given UTC timestamp (ms since epoch).
    pub fn poll_events(&self, since_ms: f64) -> Result<JsValue, JsValue> {
        let since = datetime_from_millis(since_ms)?;
        let runtime = AgensRuntime::new(self.store.as_ref(), self.actor.as_str());
        let events = runtime.poll_events(since);
        to_value(&events).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get a state value by key.
    pub fn state_get(&self, key: &str) -> Result<JsValue, JsValue> {
        let table = StateTable::new(self.store.as_ref(), self.actor.as_str());
        match table.get(key) {
            Some(val) => to_value(&val).map_err(|e| JsValue::from_str(&e.to_string())),
            None => Ok(JsValue::NULL),
        }
    }

    /// Set a state value and notify JS watchers.
    pub fn state_set(&self, key: &str, value: JsValue) -> Result<(), JsValue> {
        let json: serde_json::Value =
            from_value(value.clone()).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let table = StateTable::new(self.store.as_ref(), self.actor.as_str());
        let old_value = table.get(key);
        table.set(key, json);

        if let Some(callbacks) = self.state_watchers.borrow().get(key).cloned() {
            let new_js = value;
            let old_js = match old_value {
                Some(val) => to_value(&val).map_err(|e| JsValue::from_str(&e.to_string()))?,
                None => JsValue::NULL,
            };
            let mut first_err: Option<JsValue> = None;
            for cb in callbacks {
                if let Err(err) = cb.call2(&JsValue::NULL, &new_js, &old_js) {
                    if first_err.is_none() {
                        first_err = Some(err);
                    }
                }
            }
            if let Some(err) = first_err {
                return Err(err);
            }
        }

        Ok(())
    }

    /// Register a JS callback for state changes on `key`.
    pub fn state_watch(&self, key: &str, callback: Function) {
        let mut watchers = self.state_watchers.borrow_mut();
        let callbacks = watchers.entry(key.to_string()).or_default();

        if callbacks
            .iter()
            .any(|existing| Object::is(existing, &callback))
        {
            return;
        }

        callbacks.push(callback);
    }

    /// Unregister a JS callback for state changes on `key`.
    pub fn state_unwatch(&self, key: &str, callback: Function) -> bool {
        let mut watchers = self.state_watchers.borrow_mut();
        let mut should_remove_key = false;
        let mut removed = false;

        if let Some(callbacks) = watchers.get_mut(key) {
            let original_len = callbacks.len();
            callbacks.retain(|existing| !Object::is(existing, &callback));
            removed = callbacks.len() != original_len;
            should_remove_key = callbacks.is_empty();
        }

        if should_remove_key {
            watchers.remove(key);
        }

        removed
    }

    /// Clear all JS callbacks registered for state changes on `key`.
    pub fn state_clear_watchers(&self, key: &str) -> usize {
        self.state_watchers
            .borrow_mut()
            .remove(key)
            .map(|callbacks| callbacks.len())
            .unwrap_or(0)
    }

    /// Schedule a recurring timer.
    pub fn timer_schedule(
        &self,
        name: &str,
        interval_secs: f64,
        payload: JsValue,
    ) -> Result<String, JsValue> {
        if !interval_secs.is_finite() {
            return Err(JsValue::from_str("interval_secs must be a finite number"));
        }
        if interval_secs.trunc() != interval_secs {
            return Err(JsValue::from_str("interval_secs must be an integer"));
        }
        if interval_secs <= 0.0 {
            return Err(JsValue::from_str("interval_secs must be greater than 0"));
        }
        if interval_secs > i64::MAX as f64 {
            return Err(JsValue::from_str(
                "interval_secs exceeds maximum allowed value",
            ));
        }
        let interval_secs = interval_secs as u64;
        let payload: serde_json::Value =
            from_value(payload).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let timers = TimerTable::new(self.store.as_ref(), self.actor.as_str());
        Ok(timers.schedule(name, interval_secs, payload))
    }

    /// Cancel a timer by id.
    pub fn timer_cancel(&self, timer_id: &str) -> bool {
        let timers = TimerTable::new(self.store.as_ref(), self.actor.as_str());
        timers.cancel(timer_id)
    }

    /// List all scheduled timers.
    pub fn timer_list(&self) -> Result<JsValue, JsValue> {
        let timers = TimerTable::new(self.store.as_ref(), self.actor.as_str());
        to_value(&timers.list()).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Return timers due at or before `now_ms` (UTC ms since epoch).
    pub fn timers_due(&self, now_ms: f64) -> Result<JsValue, JsValue> {
        let now = datetime_from_millis(now_ms)?;
        let timers = TimerTable::new(self.store.as_ref(), self.actor.as_str());
        to_value(&timers.due_timers(now)).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Reschedule a timer by advancing its next fire time.
    pub fn timer_reschedule(&self, timer_id: &str) -> bool {
        let timers = TimerTable::new(self.store.as_ref(), self.actor.as_str());
        timers.reschedule(timer_id)
    }

    /// Execute a DSL query string.
    pub fn exec_dsl(&self, query: &str) -> Result<JsValue, JsValue> {
        let engine = ProcedureEngine::new(self.store.as_ref(), self.actor.as_str());
        let result = engine
            .exec_dsl(query)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Execute a pipeline of steps provided as JSON.
    pub fn exec(&self, steps: JsValue) -> Result<JsValue, JsValue> {
        let steps: Vec<Step> = from_value(steps).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let engine = ProcedureEngine::new(self.store.as_ref(), self.actor.as_str());
        let result = engine
            .exec(&steps)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

/// wasm-bindgen wrapper around the ProcedureEngine.
#[wasm_bindgen]
pub struct WasmProcedureEngine {
    store: Arc<CrdtStore>,
    actor: String,
}

#[wasm_bindgen]
impl WasmProcedureEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(store: &WasmCrdtStore, actor: String) -> Self {
        console_error_panic_hook::set_once();
        Self {
            store: store.store.clone(),
            actor,
        }
    }

    pub fn exec_dsl(&self, query: &str) -> Result<JsValue, JsValue> {
        let engine = ProcedureEngine::new(self.store.as_ref(), self.actor.as_str());
        let result = engine
            .exec_dsl(query)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn exec(&self, steps: JsValue) -> Result<JsValue, JsValue> {
        let steps: Vec<Step> = from_value(steps).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let engine = ProcedureEngine::new(self.store.as_ref(), self.actor.as_str());
        let result = engine
            .exec(&steps)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

fn datetime_from_millis(ms: f64) -> Result<DateTime<Utc>, JsValue> {
    if !ms.is_finite() {
        return Err(JsValue::from_str("timestamp must be finite"));
    }
    if ms.trunc() != ms {
        return Err(JsValue::from_str(
            "timestamp must be an integer number of milliseconds",
        ));
    }
    if ms < i64::MIN as f64 || ms > i64::MAX as f64 {
        return Err(JsValue::from_str("timestamp out of range"));
    }
    let ms_i64 = ms as i64;
    Utc.timestamp_millis_opt(ms_i64)
        .single()
        .ok_or_else(|| JsValue::from_str("invalid timestamp"))
}
