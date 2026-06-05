//! WASM bindings for pluresdb-chronos (state timeline).

use pluresdb_chronos::{ChronosAction, ChronosLevel, ChronosTimeline};
use serde_json::Value;
use wasm_bindgen::prelude::*;

use serde_wasm_bindgen::{from_value, to_value};

use crate::WasmCrdtStore;

/// Browser-side Chronos timeline backed by a shared CRDT store.
#[wasm_bindgen]
pub struct WasmChronosTimeline {
    timeline: ChronosTimeline,
}

// `WasmChronosTimeline` is `Send` automatically if `ChronosTimeline` is `Send`; avoid `unsafe impl Send` here.
#[wasm_bindgen]
impl WasmChronosTimeline {
    /// Create a new Chronos timeline bound to a shared CRDT store.
    #[wasm_bindgen(constructor)]
    pub fn new(store: &WasmCrdtStore) -> Self {
        let timeline = ChronosTimeline::new(store.shared());
        Self { timeline }
    }

    /// Record a state mutation. Returns true if recorded (not filtered by level).
    pub fn record(
        &self,
        key: &str,
        actor: &str,
        action: &str,
        data: JsValue,
        rationale: Option<String>,
    ) -> Result<bool, JsValue> {
        let data_val: Value =
            from_value(data).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let chronos_action = parse_action(action)?;
        let entry = self.timeline.build_entry(
            key,
            actor,
            chronos_action,
            &data_val,
            vec![],
            rationale,
        );
        Ok(self.timeline.record(&entry))
    }

    /// Get version history for a key (newest first).
    pub fn history(&self, key: &str, limit: usize) -> Result<JsValue, JsValue> {
        let entries = self.timeline.history(key, limit);
        to_value(&entries).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get recent entries across all keys.
    pub fn recent(&self, limit: usize) -> Result<JsValue, JsValue> {
        let entries = self.timeline.recent(limit);
        to_value(&entries).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get the timeline with optional filters.
    ///
    /// `since_ms` is a Unix timestamp in milliseconds.
    /// The core Rust timeline API stores timestamps in seconds, so this method
    /// converts milliseconds to seconds before filtering.
    pub fn timeline(
        &self,
        limit: usize,
        since_ms: Option<f64>,
        level: Option<String>,
    ) -> Result<JsValue, JsValue> {
        let since = since_ms.map(|ms| (ms / 1000.0) as u64);
        let level_filter = level
            .as_deref()
            .and_then(ChronosLevel::from_str_loose);
        let entries = self.timeline.timeline(limit, since, level_filter);
        to_value(&entries).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Set the minimum recording level.
    pub fn set_level(&self, level: &str) -> Result<(), JsValue> {
        let l = ChronosLevel::from_str_loose(level)
            .ok_or_else(|| JsValue::from_str("invalid level: use debug/info/warn/error"))?;
        self.timeline.set_level(l);
        Ok(())
    }
}

fn parse_action(s: &str) -> Result<ChronosAction, JsValue> {
    match s.to_lowercase().as_str() {
        "create" => Ok(ChronosAction::Create),
        "update" => Ok(ChronosAction::Update),
        "delete" => Ok(ChronosAction::Delete),
        "move" => Ok(ChronosAction::Move),
        "message_received" | "messagereceived" => Ok(ChronosAction::MessageReceived),
        "response_generated" | "responsegenerated" => Ok(ChronosAction::ResponseGenerated),
        "tool_invoked" | "toolinvoked" => Ok(ChronosAction::ToolInvoked),
        "context_managed" | "contextmanaged" => Ok(ChronosAction::ContextManaged),
        "model_called" | "modelcalled" => Ok(ChronosAction::ModelCalled),
        "outcome_recorded" | "outcomerecorded" => Ok(ChronosAction::OutcomeRecorded),
        _ => Err(JsValue::from_str(&format!("unknown action: {s}"))),
    }
}
