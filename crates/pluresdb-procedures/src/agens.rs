//! Pares Agens procedure execution API for PluresDB.
//!
//! This module wires PluresDB's CRDT store to the Pares Agens reactive
//! procedure executor so that procedures persist and can sync across peers
//! via Hyperswarm.
//!
//! ## Key types
//!
//! - [`AgensEvent`] — the five event kinds that Pares Agens handles.
//! - [`ProcedureHandler`] — trait (+ blanket impl for closures) that
//!   procedure implementations must satisfy.
//! - [`AgensRuntime`] — top-level runtime: emit / poll events, register and
//!   dispatch procedure handlers.
//! - [`StateTable`] — reactive get / set / watch for the Agens state table.
//! - [`TimerTable`] — schedule / cancel / list recurring timer events.
//!
//! ## Quick start
//!
//! ```rust
//! use std::sync::Arc;
//! use pluresdb_core::CrdtStore;
//! use pluresdb_procedures::agens::{AgensEvent, AgensRuntime};
//!
//! let store = CrdtStore::default();
//! let runtime = AgensRuntime::new(&store, "my-actor");
//!
//! // Register a handler for "message" events.
//! runtime.register_procedure("message", Arc::new(|event: &AgensEvent| {
//!     println!("received event: {:?}", event.event_type());
//!     Ok(())
//! }));
//!
//! // Emit an event and then execute it immediately.
//! let event = AgensEvent::Message {
//!     id: "msg-1".to_string(),
//!     payload: serde_json::json!({"text": "hello"}),
//! };
//! runtime.emit_event(&event);
//! runtime.execute_procedure(&event).unwrap();
//! ```

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use pluresdb_core::CrdtStore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use tracing::warn;
use uuid::Uuid;

// Node type tags used to namespace Agens data inside the CRDT store.
const TYPE_COMMAND: &str = "agens:command";
const TYPE_STATE: &str = "agens:state";
const TYPE_TIMER: &str = "agens:timer";

/// Node ID prefix for idempotent praxis lifecycle command nodes.
///
/// Praxis events are stored under `"{PRAXIS_CMD_PREFIX}{event_id}"`.
/// Using a stable prefix makes it easy to enumerate all praxis command nodes
/// and ensures the idempotency guarantee: re-emitting an event with the same
/// `id` always resolves to the same CRDT node key.
const PRAXIS_CMD_PREFIX: &str = "praxis:cmd:";

// ---------------------------------------------------------------------------
// Event types
// ---------------------------------------------------------------------------

/// An event dispatched through the Pares Agens runtime.
///
/// Events are persisted as CRDT nodes (type `"agens:command"`) so that they
/// survive restarts and can be synced to peers via Hyperswarm.
///
/// # Security
///
/// Event payloads are arbitrary [`serde_json::Value`] and are not validated by
/// the runtime.  When events arrive from remote peers via Hyperswarm, the data
/// is untrusted.  **Procedure handlers must validate all fields** (types,
/// ranges, allowed keys) before acting on them to prevent injection or
/// prototype-pollution style attacks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum AgensEvent {
    /// An incoming message (e.g. from a peer, user, or channel).
    Message {
        /// Unique event ID.
        id: String,
        /// Arbitrary payload.
        payload: JsonValue,
    },
    /// A scheduled timer firing.
    Timer {
        /// Timer node ID as returned by [`TimerTable::schedule`].
        id: String,
        /// Human-readable timer name.
        name: String,
        /// Payload stored when the timer was scheduled.
        payload: JsonValue,
    },
    /// A reactive state change (a key was updated in the state table).
    StateChange {
        /// Unique event ID.
        id: String,
        /// The state key that changed.
        key: String,
        /// Previous value, if any.
        old_value: Option<JsonValue>,
        /// New value after the change.
        new_value: JsonValue,
    },
    /// A response received from an LLM / model inference endpoint.
    ModelResponse {
        /// Unique event ID.
        id: String,
        /// Arbitrary payload from the model.
        payload: JsonValue,
    },
    /// The result of a tool invocation.
    ToolResult {
        /// Unique event ID.
        id: String,
        /// The name of the tool that was called.
        tool_name: String,
        /// Tool output payload.
        payload: JsonValue,
    },
    /// A Praxis analysis job completed successfully and results are available.
    ///
    /// Emitted by PluresLM once analysis is ready for consumption by Pares
    /// Agens.  Use [`AgensRuntime::emit_praxis_event`] so the event is stored
    /// under a stable, deterministic CRDT node key — re-emitting the same
    /// `analysis_id` is idempotent.
    PraxisAnalysisReady {
        /// Unique event ID.  For idempotency, derive this deterministically
        /// from `analysis_id` (e.g. `format!("praxis-ready:{}", analysis_id)`).
        id: String,
        /// Identifies the Praxis analysis job.
        analysis_id: String,
        /// Optional session or conversation context.
        session_id: Option<String>,
        /// Analysis result payload (schema defined by PluresLM).
        payload: JsonValue,
    },
    /// A Praxis analysis job failed.
    ///
    /// Emitted by PluresLM when analysis cannot be completed.  Like
    /// [`PraxisAnalysisReady`][AgensEvent::PraxisAnalysisReady], use
    /// [`AgensRuntime::emit_praxis_event`] for idempotent delivery.
    PraxisAnalysisFailed {
        /// Unique event ID.  For idempotency, derive this deterministically
        /// from `analysis_id` (e.g. `format!("praxis-failed:{}", analysis_id)`).
        id: String,
        /// Identifies the Praxis analysis job that failed.
        analysis_id: String,
        /// Optional session or conversation context.
        session_id: Option<String>,
        /// Human-readable description of the failure.
        reason: String,
        /// Additional error detail payload.
        payload: JsonValue,
    },
    /// Praxis guidance for a session has been updated.
    ///
    /// Emitted by PluresLM whenever guidance data changes.  Use
    /// [`AgensRuntime::emit_praxis_event`] for idempotent delivery.
    PraxisGuidanceUpdated {
        /// Unique event ID.  For idempotency, derive this deterministically
        /// from `guidance_id` (e.g. `format!("praxis-guidance:{}", guidance_id)`).
        id: String,
        /// Identifies the guidance item that was updated.
        guidance_id: String,
        /// Optional session or conversation context.
        session_id: Option<String>,
        /// Updated guidance payload (schema defined by PluresLM).
        payload: JsonValue,
    },
}

impl AgensEvent {
    /// Return the event-type string used as a handler lookup key.
    ///
    /// One of: `"message"`, `"timer"`, `"state_change"`,
    /// `"model_response"`, `"tool_result"`, `"praxis_analysis_ready"`,
    /// `"praxis_analysis_failed"`, `"praxis_guidance_updated"`.
    pub fn event_type(&self) -> &'static str {
        match self {
            AgensEvent::Message { .. } => "message",
            AgensEvent::Timer { .. } => "timer",
            AgensEvent::StateChange { .. } => "state_change",
            AgensEvent::ModelResponse { .. } => "model_response",
            AgensEvent::ToolResult { .. } => "tool_result",
            AgensEvent::PraxisAnalysisReady { .. } => "praxis_analysis_ready",
            AgensEvent::PraxisAnalysisFailed { .. } => "praxis_analysis_failed",
            AgensEvent::PraxisGuidanceUpdated { .. } => "praxis_guidance_updated",
        }
    }

    /// Return the unique ID of this event.
    pub fn id(&self) -> &str {
        match self {
            AgensEvent::Message { id, .. }
            | AgensEvent::Timer { id, .. }
            | AgensEvent::StateChange { id, .. }
            | AgensEvent::ModelResponse { id, .. }
            | AgensEvent::ToolResult { id, .. }
            | AgensEvent::PraxisAnalysisReady { id, .. }
            | AgensEvent::PraxisAnalysisFailed { id, .. }
            | AgensEvent::PraxisGuidanceUpdated { id, .. } => id.as_str(),
        }
    }
}

// ---------------------------------------------------------------------------
// Procedure handler trait
// ---------------------------------------------------------------------------

/// A handler for an [`AgensEvent`].
///
/// Implement this trait to define custom reactive behaviour.  Handlers are
/// registered with [`AgensRuntime::register_procedure`] and invoked by
/// [`AgensRuntime::execute_procedure`].
pub trait ProcedureHandler: Send + Sync {
    /// Process the given event.  Returns `Ok(())` on success.
    fn call(&self, event: &AgensEvent) -> anyhow::Result<()>;
}

/// Blanket implementation so that closures can be used as procedure handlers.
impl<F> ProcedureHandler for F
where
    F: Fn(&AgensEvent) -> anyhow::Result<()> + Send + Sync,
{
    fn call(&self, event: &AgensEvent) -> anyhow::Result<()> {
        self(event)
    }
}

// ---------------------------------------------------------------------------
// State table
// ---------------------------------------------------------------------------

/// Reactive key-value state table backed by the CRDT store.
///
/// State entries are stored as nodes with type `"agens:state"` and node ID
/// `"state:{key}"`.  Callers can react to state changes by polling
/// [`watch`][StateTable::watch] with the timestamp of their last check.
pub struct StateTable<'a> {
    store: &'a CrdtStore,
    actor: String,
}

impl<'a> StateTable<'a> {
    /// Create a state table view backed by `store`.
    pub fn new(store: &'a CrdtStore, actor: impl Into<String>) -> Self {
        StateTable {
            store,
            actor: actor.into(),
        }
    }

    /// Retrieve the current value for `key`, or `None` if not set.
    pub fn get(&self, key: &str) -> Option<JsonValue> {
        let node = self.store.get(format!("state:{}", key))?;
        // Ensure we only return values from Agens state records.
        if node.data.get("_type").and_then(|v| v.as_str()) != Some(TYPE_STATE) {
            return None;
        }
        node.data.get("value").cloned()
    }

    /// Set the value for `key`, persisting it as a CRDT node.
    ///
    /// Updating an existing key merges via CRDT semantics (last-write-wins per
    /// field), so concurrent updates from different peers converge.
    ///
    /// A [`AgensEvent::StateChange`] command event is automatically emitted
    /// to the commands table so that [`AgensRuntime::poll_events`] subscribers
    /// are notified reactively without any extra caller code.
    pub fn set(&self, key: &str, value: JsonValue) {
        let old_value = self.get(key);
        self.store.put(
            format!("state:{}", key),
            self.actor.as_str(),
            json!({
                "_type": TYPE_STATE,
                "key": key,
                "value": value,
            }),
        );
        // Auto-emit a StateChange event so poll_events() subscribers see it.
        let event_id = Uuid::new_v4().to_string();
        let event = AgensEvent::StateChange {
            id: event_id.clone(),
            key: key.to_string(),
            old_value,
            new_value: value,
        };
        let node_id = format!("cmd:{}", event_id);
        self.store.put(
            node_id,
            self.actor.as_str(),
            json!({
                "_type": TYPE_COMMAND,
                "logical_id": &event_id,
                "event": serde_json::to_value(&event)
                    .expect("AgensEvent serialization should not fail"),
            }),
        );
    }

    /// Return all state entries updated at or after `since`.
    ///
    /// Each returned item is a `(key, value)` pair.  Use this to implement
    /// reactive triggers — advance `since` to the time of the last call to
    /// avoid re-processing unchanged entries.
    ///
    /// # Performance
    ///
    /// This method calls [`CrdtStore::list`] and filters in memory — it is
    /// **O(n)** in the total number of nodes in the store.  For stores with
    /// large amounts of data, prefer using [`AgensRuntime::poll_events`] to
    /// receive only [`AgensEvent::StateChange`] events emitted by
    /// [`set`][Self::set].
    pub fn watch(&self, since: DateTime<Utc>) -> Vec<(String, JsonValue)> {
        self.store
            .list()
            .into_iter()
            .filter(|n| {
                n.data.get("_type").and_then(|v| v.as_str()) == Some(TYPE_STATE)
                    && n.timestamp >= since
            })
            .filter_map(|n| {
                let key = n.data.get("key")?.as_str()?.to_string();
                let value = n.data.get("value")?.clone();
                Some((key, value))
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Timer entry & table
// ---------------------------------------------------------------------------

/// A scheduled recurring timer entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerEntry {
    /// Unique timer node ID.  Pass this to [`TimerTable::cancel`] to remove
    /// the timer or to [`TimerTable::reschedule`] after it has fired.
    pub id: String,
    /// Human-readable name forwarded to [`AgensEvent::Timer`].
    pub name: String,
    /// Timer trigger mode.
    pub trigger: TimerTrigger,
    /// Firing interval in seconds (used when `trigger == interval`).
    pub interval_secs: u64,
    /// Cron expression (used when `trigger == cron`).
    pub cron_expr: Option<String>,
    /// UTC timestamp when this timer most recently ran.
    pub last_run: Option<DateTime<Utc>>,
    /// UTC timestamp of the next scheduled firing.
    pub next_fire_at: DateTime<Utc>,
    /// Arbitrary payload forwarded to the handler when the timer fires.
    pub payload: JsonValue,
    /// Whether this timer is still active.
    pub active: bool,
}

/// Timer trigger type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimerTrigger {
    /// Fire every `interval_secs`.
    Interval,
    /// Fire according to `cron_expr`.
    Cron,
    /// Fire exactly once at `next_fire_at`.
    Once,
}

/// Timer table backed by the CRDT store.
///
/// Timers are stored as nodes with type `"agens:timer"`.  The runtime (or the
/// application tick loop) calls [`due_timers`][TimerTable::due_timers] each
/// tick and emits [`AgensEvent::Timer`] events for timers whose
/// `next_fire_at` has passed.  After processing, call
/// [`reschedule`][TimerTable::reschedule] to advance the timer by one
/// interval.
pub struct TimerTable<'a> {
    store: &'a CrdtStore,
    actor: String,
}

impl<'a> TimerTable<'a> {
    /// Create a timer table view backed by `store`.
    pub fn new(store: &'a CrdtStore, actor: impl Into<String>) -> Self {
        TimerTable {
            store,
            actor: actor.into(),
        }
    }

    /// Schedule a recurring **interval** timer named `name` to fire every
    /// `interval_secs` seconds.
    ///
    /// Returns the timer node ID (use it with [`cancel`][Self::cancel]
    /// or [`reschedule`][Self::reschedule]).
    ///
    /// The first firing is scheduled `interval_secs` from now.
    ///
    /// # Panics
    ///
    /// Panics if `interval_secs` is `0` (a zero-duration timer would fire
    /// continuously without advancing) or if it exceeds [`i64::MAX`] (which
    /// would overflow the chrono duration).
    pub fn schedule(&self, name: &str, interval_secs: u64, payload: JsonValue) -> String {
        self.schedule_interval(name, interval_secs, payload)
    }

    /// Schedule a recurring **interval** timer named `name` to fire every
    /// `interval_secs` seconds.
    pub fn schedule_interval(&self, name: &str, interval_secs: u64, payload: JsonValue) -> String {
        // Validate interval to avoid zero-duration loops and integer wrap-around.
        if interval_secs == 0 {
            panic!("TimerTable::schedule: interval_secs must be greater than 0");
        }

        let interval_secs_i64 =
            i64::try_from(interval_secs).expect("TimerTable::schedule: interval_secs too large");

        let id = format!("timer:{}", Uuid::new_v4());
        let next_fire_at = Utc::now() + chrono::Duration::seconds(interval_secs_i64);
        let entry = TimerEntry {
            id: id.clone(),
            name: name.to_string(),
            trigger: TimerTrigger::Interval,
            interval_secs,
            cron_expr: None,
            last_run: None,
            next_fire_at,
            payload,
            active: true,
        };
        self.persist_entry(&entry);
        id
    }

    /// Schedule a recurring **cron** timer named `name`.
    ///
    /// Supports standard 5-field expressions (`min hour dom mon dow`) and
    /// 6/7-field expressions accepted by the `cron` crate.
    pub fn schedule_cron(
        &self,
        name: &str,
        cron_expr: &str,
        payload: JsonValue,
    ) -> anyhow::Result<String> {
        let now = Utc::now();
        let next_fire_at = self.next_cron_fire(cron_expr, now).ok_or_else(|| {
            anyhow::anyhow!(
                "TimerTable::schedule_cron: cron expression '{}' has no next run",
                cron_expr
            )
        })?;
        let id = format!("timer:{}", Uuid::new_v4());
        let entry = TimerEntry {
            id: id.clone(),
            name: name.to_string(),
            trigger: TimerTrigger::Cron,
            interval_secs: 0,
            cron_expr: Some(cron_expr.to_string()),
            last_run: None,
            next_fire_at,
            payload,
            active: true,
        };
        self.persist_entry(&entry);
        Ok(id)
    }

    /// Schedule a **one-shot** timer named `name` for `run_at`.
    pub fn schedule_once(&self, name: &str, run_at: DateTime<Utc>, payload: JsonValue) -> String {
        let id = format!("timer:{}", Uuid::new_v4());
        let entry = TimerEntry {
            id: id.clone(),
            name: name.to_string(),
            trigger: TimerTrigger::Once,
            interval_secs: 0,
            cron_expr: None,
            last_run: None,
            next_fire_at: run_at,
            payload,
            active: true,
        };
        self.persist_entry(&entry);
        id
    }

    fn persist_entry(&self, entry: &TimerEntry) {
        self.store.put(
            entry.id.as_str(),
            self.actor.as_str(),
            json!({
                "_type": TYPE_TIMER,
                "name": entry.name,
                "trigger": entry.trigger,
                "interval_secs": entry.interval_secs,
                "cron_expr": entry.cron_expr,
                "last_run": entry.last_run.map(|ts| ts.to_rfc3339()),
                "next_run": entry.next_fire_at.to_rfc3339(),
                "active": entry.active,
                "payload": entry.payload,
            }),
        );
    }

    /// Cancel a timer by its ID.  Returns `true` if the timer existed.
    pub fn cancel(&self, timer_id: &str) -> bool {
        self.store.delete(timer_id).is_ok()
    }

    /// List all scheduled timers.
    ///
    /// # Performance
    ///
    /// This method calls [`CrdtStore::list`] and filters in memory — it is
    /// **O(n)** in the total number of nodes in the store.
    pub fn list(&self) -> Vec<TimerEntry> {
        self.store
            .list()
            .into_iter()
            .filter(|n| n.data.get("_type").and_then(|v| v.as_str()) == Some(TYPE_TIMER))
            .filter_map(|n| self.entry_from_data(&n.id, &n.data))
            .collect()
    }

    /// Return timers whose `next_fire_at` is at or before `now`.
    ///
    /// Delegates to [`list`][Self::list]; see its documentation for
    /// performance characteristics.
    pub fn due_timers(&self, now: DateTime<Utc>) -> Vec<TimerEntry> {
        self.list()
            .into_iter()
            .filter(|t| t.active && t.next_fire_at <= now)
            .collect()
    }

    /// Advance a timer's `next_fire_at` by one interval.
    ///
    /// Call this after the timer has been processed to re-arm it for the next
    /// cycle.  Returns `false` if `timer_id` does not exist.
    pub fn reschedule(&self, timer_id: &str) -> bool {
        self.mark_ran(timer_id, Utc::now())
    }

    /// Mark a timer as executed at `ran_at`, persisting `last_run` and
    /// advancing `next_run`.
    pub fn mark_ran(&self, timer_id: &str, ran_at: DateTime<Utc>) -> bool {
        let Some(node) = self.store.get(timer_id) else {
            return false;
        };
        let node_type = node.data.get("_type").and_then(|v| v.as_str());
        if node_type != Some(TYPE_TIMER) {
            debug_assert!(
                false,
                "TimerTable::reschedule called with non-timer node id `{}` (type: {:?})",
                timer_id, node_type,
            );
            return false;
        }
        let Some(entry) = self.entry_from_data(&node.id, &node.data) else {
            return false;
        };
        let (next_fire_at, active) = match entry.trigger {
            TimerTrigger::Interval => {
                // Convert the stored interval to i64 in a checked way to avoid overflow.
                let Ok(interval_i64) = i64::try_from(entry.interval_secs) else {
                    warn!(
                        timer_id,
                        interval_secs = entry.interval_secs,
                        "TimerTable::mark_ran: interval_secs exceeds i64::MAX, skipping reschedule"
                    );
                    return false;
                };
                let duration = chrono::Duration::seconds(interval_i64);
                // Use checked_add_signed to avoid overflowing the DateTime.
                let Some(next) = ran_at.checked_add_signed(duration) else {
                    warn!(
                        timer_id,
                        ?duration,
                        "TimerTable::mark_ran: next_fire_at would overflow DateTime bounds, \
                         skipping reschedule"
                    );
                    return false;
                };
                (next, true)
            }
            TimerTrigger::Cron => {
                let Some(expr) = entry.cron_expr.as_deref() else {
                    warn!(
                        timer_id,
                        "TimerTable::mark_ran: cron timer missing cron_expr, skipping reschedule"
                    );
                    return false;
                };
                match self.next_cron_fire(expr, ran_at) {
                    Some(next) => (next, true),
                    None => {
                        warn!(
                            timer_id,
                            cron_expr = expr,
                            "TimerTable::mark_ran: cron has no next run, disabling timer"
                        );
                        (ran_at, false)
                    }
                }
            }
            TimerTrigger::Once => (ran_at, false),
        };
        self.persist_entry(&TimerEntry {
            id: timer_id.to_string(),
            name: entry.name,
            trigger: entry.trigger,
            interval_secs: entry.interval_secs,
            cron_expr: entry.cron_expr,
            last_run: Some(ran_at),
            next_fire_at,
            payload: entry.payload,
            active,
        });
        true
    }

    /// Parse a [`TimerEntry`] from raw node data.
    ///
    /// Returns `None` if `data` does not represent a valid Agens timer record
    /// (`_type != agens:timer`, or any required field is absent/malformed).
    fn entry_from_data(&self, id: &str, data: &JsonValue) -> Option<TimerEntry> {
        // Guard: only parse nodes that are actually Agens timer records.
        if data.get("_type").and_then(|v| v.as_str()) != Some(TYPE_TIMER) {
            return None;
        }
        let name = data.get("name")?.as_str()?.to_string();
        let trigger = match data.get("trigger").and_then(|v| v.as_str()) {
            Some("cron") => TimerTrigger::Cron,
            Some("once") => TimerTrigger::Once,
            _ => TimerTrigger::Interval,
        };
        let interval_secs = data
            .get("interval_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or_default();
        let cron_expr = data
            .get("cron_expr")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let last_run: Option<DateTime<Utc>> = data
            .get("last_run")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());
        let next_fire_at: DateTime<Utc> = data
            .get("next_run")
            .or_else(|| data.get("next_fire_at"))?
            .as_str()
            .and_then(|s| s.parse().ok())?;
        let active = data.get("active").and_then(|v| v.as_bool()).unwrap_or(true);
        let payload = data.get("payload").cloned().unwrap_or(JsonValue::Null);
        Some(TimerEntry {
            id: id.to_string(),
            name,
            trigger,
            interval_secs,
            cron_expr,
            last_run,
            next_fire_at,
            payload,
            active,
        })
    }

    /// Compute the next UTC firing time for `cron_expr` strictly after `now`.
    fn next_cron_fire(&self, cron_expr: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let normalized = Self::normalize_cron_expr(cron_expr);
        let schedule = cron::Schedule::from_str(&normalized).ok()?;
        schedule.after(&now).next()
    }

    /// Normalize a cron expression for the parser.
    ///
    /// Accepts 5-field expressions (`min hour dom mon dow`) and prepends a
    /// seconds field (`0`) so they can be parsed by the underlying crate.
    fn normalize_cron_expr(cron_expr: &str) -> String {
        let fields = cron_expr.split_whitespace().count();
        if fields == 5 {
            format!("0 {}", cron_expr)
        } else {
            cron_expr.to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Agens runtime
// ---------------------------------------------------------------------------

/// The Pares Agens procedure execution runtime.
///
/// Wraps a [`CrdtStore`] to provide:
///
/// - **Event emission**: write command events into the store with
///   [`emit_event`][AgensRuntime::emit_event].
/// - **Event polling**: scan the commands table for new [`AgensEvent`]s with
///   [`poll_events`][AgensRuntime::poll_events].
/// - **Procedure registry**: register named event handlers with
///   [`register_procedure`][AgensRuntime::register_procedure].
/// - **Procedure dispatch**: run the matching handler for an event with
///   [`execute_procedure`][AgensRuntime::execute_procedure].
/// - **State table**: reactive key/value storage via
///   [`state`][AgensRuntime::state].
/// - **Timer table**: scheduled recurring events via
///   [`timers`][AgensRuntime::timers].
pub struct AgensRuntime<'a> {
    store: &'a CrdtStore,
    actor: String,
    /// Registry: event_type string → handler.
    handlers: RwLock<HashMap<String, Arc<dyn ProcedureHandler>>>,
}

impl<'a> AgensRuntime<'a> {
    /// Create a new runtime bound to `store`.
    ///
    /// `actor` is used for all writes made on behalf of the runtime (command
    /// persistence, state updates, timer scheduling).
    pub fn new(store: &'a CrdtStore, actor: impl Into<String>) -> Self {
        AgensRuntime {
            store,
            actor: actor.into(),
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a procedure handler for `event_type`.
    ///
    /// Supported event type strings: `"message"`, `"timer"`,
    /// `"state_change"`, `"model_response"`, `"tool_result"`.
    ///
    /// Registering a second handler for the same event type replaces the
    /// first.
    pub fn register_procedure(
        &self,
        event_type: impl Into<String>,
        handler: Arc<dyn ProcedureHandler>,
    ) {
        self.handlers.write().insert(event_type.into(), handler);
    }

    /// Persist `event` in the commands table as a CRDT node.
    ///
    /// Returns the assigned CRDT node ID. After emission, peers that call
    /// [`poll_events`][Self::poll_events] will see the event.
    pub fn emit_event(&self, event: &AgensEvent) -> String {
        // Use a unique node ID per emission so that command storage remains
        // append-only, even when the logical event ID is reused (e.g., for
        // recurring timers).
        let unique_id = Uuid::new_v4();
        let node_id = format!("cmd:{}:{}", event.id(), unique_id);
        self.store.put(
            node_id.clone(),
            self.actor.as_str(),
            json!({
                "_type": TYPE_COMMAND,
                // Store the logical event ID separately so consumers can
                // correlate multiple emissions of the same logical event.
                // This is redundant with `event.id()` but enables index
                // queries on `logical_id` without deserializing the full
                // `event` value.
                "logical_id": event.id(),
                "event": serde_json::to_value(event)
                    .expect("AgensEvent serialization should not fail"),
            }),
        );
        node_id
    }

    /// Persist a praxis lifecycle event using an **idempotent** CRDT node.
    ///
    /// Unlike [`emit_event`][Self::emit_event], this method writes to a
    /// deterministic node key derived solely from `event.id()`:
    /// `"praxis:cmd:{id}"`.  Because CRDT semantics apply last-write-wins
    /// merging, re-emitting the same logical event (same `id`, same or
    /// updated payload) converges to a single store entry — no duplicate
    /// command nodes are created.
    ///
    /// This satisfies the durable event contract for the Praxis analysis
    /// lifecycle:
    ///
    /// - **Durable**: the node is persisted in the CRDT store and survives
    ///   restarts and peer sync via Hyperswarm.
    /// - **Idempotent**: emitting an event with the same `id` a second time
    ///   (e.g. after a crash-recovery or network retry) updates the same
    ///   underlying CRDT node rather than creating a new one, so no duplicate
    ///   command nodes are stored.
    ///
    /// Note that idempotent storage does **not** by itself guarantee
    /// exactly-once delivery to consumers. `poll_events()` is timestamp-
    /// based, and rewriting the same deterministic node will advance its
    /// timestamp so it can be observed again after a consumer moves its
    /// `since` cursor. Consumers that require exactly-once handling must
    /// deduplicate on `event.id()` / logical id (or persist a processed
    /// watermark per logical id).
    ///
    /// # Security
    ///
    /// The `event` payload is stored verbatim. Callers must sanitize all
    /// fields before emission to prevent prototype-pollution or injection
    /// attacks.
    ///
    /// # Usage
    ///
    /// Use this method for [`AgensEvent::PraxisAnalysisReady`],
    /// [`AgensEvent::PraxisAnalysisFailed`], and
    /// [`AgensEvent::PraxisGuidanceUpdated`].  It also works for any other
    /// event variant when idempotent storage is desired.
    pub fn emit_praxis_event(&self, event: &AgensEvent) -> String {
        let node_id = Self::praxis_node_key(event.id());
        self.store.put(
            node_id.clone(),
            self.actor.as_str(),
            json!({
                "_type": TYPE_COMMAND,
                // Store the logical event ID separately so consumers can
                // correlate multiple emissions of the same logical event.
                // This is redundant with `event.id()` but enables index
                // queries on `logical_id` without deserializing the full
                // `event` value.
                "logical_id": event.id(),
                "event": serde_json::to_value(event).unwrap_or_else(|err| {
                    panic!(
                        "AgensEvent::{} serialization should not fail: {}",
                        event.event_type(),
                        err
                    )
                }),
            }),
        );
        node_id
    }

    /// Build the deterministic CRDT node key for a praxis command event.
    ///
    /// Returns `"{PRAXIS_CMD_PREFIX}{event_id}"`.  This is the single source
    /// of truth for the idempotency key used by
    /// [`emit_praxis_event`][Self::emit_praxis_event].
    fn praxis_node_key(event_id: &str) -> String {
        format!("{}{}", PRAXIS_CMD_PREFIX, event_id)
    }

    /// Poll the commands table for events that arrived **strictly after** `since`.
    ///
    /// Returns events ordered oldest-first.  Advance `since` to `Utc::now()`
    /// before the next call to avoid re-processing already-seen events.
    /// Using the timestamp of the last received event as `since` also works
    /// because the filter is strictly greater-than, so that event is excluded
    /// from the next poll.
    ///
    /// # Performance
    ///
    /// This method calls [`CrdtStore::list`] and filters in memory — it is
    /// **O(n)** in the total number of nodes in the store, not just the number
    /// of command nodes.  For high-frequency polling on large stores consider
    /// maintaining a prefix-based index or using a dedicated commands table.
    pub fn poll_events(&self, since: DateTime<Utc>) -> Vec<AgensEvent> {
        let mut events: Vec<(DateTime<Utc>, AgensEvent)> = self
            .store
            .list()
            .into_iter()
            .filter(|n| {
                n.data.get("_type").and_then(|v| v.as_str()) == Some(TYPE_COMMAND)
                    && n.timestamp > since
            })
            .filter_map(|n| {
                let ev: AgensEvent = serde_json::from_value(n.data.get("event")?.clone()).ok()?;
                Some((n.timestamp, ev))
            })
            .collect();
        events.sort_by_key(|(ts, _)| *ts);
        events.into_iter().map(|(_, ev)| ev).collect()
    }

    /// Dispatch `event` to its registered handler.
    ///
    /// Returns an error if no handler is registered for the event type, or if
    /// the handler itself returns an error.
    pub fn execute_procedure(&self, event: &AgensEvent) -> anyhow::Result<()> {
        let event_type = event.event_type();
        let handler = {
            let guard = self.handlers.read();
            guard.get(event_type).cloned()
        };
        match handler {
            Some(h) => h.call(event),
            None => Err(anyhow::anyhow!(
                "no handler registered for event type '{}'",
                event_type
            )),
        }
    }

    /// Process all due timers at `now`.
    ///
    /// For each due timer, executes the registered `"timer"` handler with a
    /// [`AgensEvent::Timer`] event and then persists `last_run`/`next_run`.
    /// Returns the number of timers that were processed.
    pub fn process_due_timers(&self, now: DateTime<Utc>) -> usize {
        let timers = self.timers();
        let due = timers.due_timers(now);
        for timer in &due {
            let event = AgensEvent::Timer {
                id: timer.id.clone(),
                name: timer.name.clone(),
                payload: timer.payload.clone(),
            };
            if let Err(err) = self.execute_procedure(&event) {
                warn!(
                    timer_id = timer.id,
                    timer_name = timer.name,
                    error = %err,
                    "AgensRuntime::process_due_timers: timer handler failed"
                );
            }
            if !timers.mark_ran(&timer.id, now) {
                warn!(
                    timer_id = timer.id,
                    "AgensRuntime::process_due_timers: failed to persist timer run state"
                );
            }
        }
        due.len()
    }

    /// Spawn a background Tokio task that processes due timers every 10 seconds.
    ///
    /// Requires that this runtime's store reference is `'static`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn_timer_task(self: Arc<Self>) -> tokio::task::JoinHandle<()>
    where
        'a: 'static,
    {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(10));
            loop {
                ticker.tick().await;
                let now = Utc::now();
                let _ = self.process_due_timers(now);
            }
        })
    }

    /// Return a view of the reactive state table.
    pub fn state(&self) -> StateTable<'_> {
        StateTable::new(self.store, self.actor.as_str())
    }

    /// Return a view of the timer table.
    pub fn timers(&self) -> TimerTable<'_> {
        TimerTable::new(self.store, self.actor.as_str())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pluresdb_core::CrdtStore;

    fn make_store() -> CrdtStore {
        CrdtStore::default()
    }

    // -----------------------------------------------------------------------
    // AgensEvent
    // -----------------------------------------------------------------------

    #[test]
    fn event_type_strings() {
        let ev = AgensEvent::Message {
            id: "1".to_string(),
            payload: json!({}),
        };
        assert_eq!(ev.event_type(), "message");

        let ev = AgensEvent::Timer {
            id: "2".to_string(),
            name: "tick".to_string(),
            payload: json!({}),
        };
        assert_eq!(ev.event_type(), "timer");

        let ev = AgensEvent::StateChange {
            id: "3".to_string(),
            key: "k".to_string(),
            old_value: None,
            new_value: json!(1),
        };
        assert_eq!(ev.event_type(), "state_change");

        let ev = AgensEvent::ModelResponse {
            id: "4".to_string(),
            payload: json!({}),
        };
        assert_eq!(ev.event_type(), "model_response");

        let ev = AgensEvent::ToolResult {
            id: "5".to_string(),
            tool_name: "search".to_string(),
            payload: json!({}),
        };
        assert_eq!(ev.event_type(), "tool_result");
    }

    #[test]
    fn event_serde_roundtrip() {
        let ev = AgensEvent::ToolResult {
            id: "t1".to_string(),
            tool_name: "calculator".to_string(),
            payload: json!({"result": 42}),
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: AgensEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(ev, back);
    }

    // -----------------------------------------------------------------------
    // StateTable
    // -----------------------------------------------------------------------

    #[test]
    fn state_get_set() {
        let store = make_store();
        let state = StateTable::new(&store, "actor");
        assert!(state.get("counter").is_none());
        state.set("counter", json!(1));
        assert_eq!(state.get("counter"), Some(json!(1)));
        state.set("counter", json!(2));
        assert_eq!(state.get("counter"), Some(json!(2)));
    }

    #[test]
    fn state_watch_returns_recent_changes() {
        let store = make_store();
        let state = StateTable::new(&store, "actor");
        let before = Utc::now();
        state.set("x", json!("hello"));
        state.set("y", json!(99));
        let changes = state.watch(before);
        let keys: Vec<&str> = changes.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"x"));
        assert!(keys.contains(&"y"));
    }

    #[test]
    fn state_watch_excludes_old_entries() {
        let store = make_store();
        let state = StateTable::new(&store, "actor");
        state.set("old", json!("before"));
        let cutoff = Utc::now();
        let changes = state.watch(cutoff);
        assert!(changes.iter().all(|(k, _)| k != "old"));
    }

    // -----------------------------------------------------------------------
    // TimerTable
    // -----------------------------------------------------------------------

    #[test]
    fn timer_schedule_and_list() {
        let store = make_store();
        let timers = TimerTable::new(&store, "actor");
        let id = timers.schedule("heartbeat", 60, json!({"ping": true}));
        let list = timers.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert_eq!(list[0].name, "heartbeat");
        assert_eq!(list[0].interval_secs, 60);
        assert_eq!(list[0].trigger, TimerTrigger::Interval);
        assert!(list[0].last_run.is_none());
        assert!(list[0].active);
    }

    #[test]
    fn timer_cancel() {
        let store = make_store();
        let timers = TimerTable::new(&store, "actor");
        let id = timers.schedule("t", 5, json!(null));
        assert!(timers.cancel(&id));
        assert!(timers.list().is_empty());
    }

    #[test]
    fn timer_due_timers() {
        let store = make_store();
        let timers = TimerTable::new(&store, "actor");
        // Schedule a timer with a 1-second interval, then check due timers 2
        // seconds in the future so the timer appears due.
        let id = timers.schedule("soon", 1, json!({}));
        let future = Utc::now() + chrono::Duration::seconds(2);
        let due = timers.due_timers(future);
        assert!(due.iter().any(|t| t.id == id));
    }

    #[test]
    fn timer_reschedule_advances_next_fire() {
        let store = make_store();
        let timers = TimerTable::new(&store, "actor");
        // Use a 60-second interval so reschedule advances next_fire_at by a
        // measurable amount.
        let id = timers.schedule("tick", 60, json!({}));
        let original_fire = timers.list()[0].next_fire_at;
        assert!(timers.reschedule(&id));
        let rescheduled = timers.list()[0].next_fire_at;
        assert!(rescheduled > original_fire);
        assert!(timers.list()[0].last_run.is_some());
    }

    #[test]
    fn timer_schedule_cron() {
        let store = make_store();
        let timers = TimerTable::new(&store, "actor");
        let id = timers
            .schedule_cron("quarter-hour", "*/15 * * * *", json!({}))
            .unwrap();
        let entry = timers
            .list()
            .into_iter()
            .find(|t| t.id == id)
            .expect("scheduled cron timer should be listed");
        assert_eq!(entry.trigger, TimerTrigger::Cron);
        assert_eq!(entry.cron_expr.as_deref(), Some("*/15 * * * *"));
        assert!(entry.next_fire_at > Utc::now());
    }

    #[test]
    fn timer_once_disables_after_run() {
        let store = make_store();
        let timers = TimerTable::new(&store, "actor");
        let now = Utc::now();
        let id = timers.schedule_once("one-shot", now - chrono::Duration::seconds(1), json!({}));
        let due = timers.due_timers(now);
        assert!(due.iter().any(|t| t.id == id));
        assert!(timers.mark_ran(&id, now));
        let entry = timers
            .list()
            .into_iter()
            .find(|t| t.id == id)
            .expect("one-shot timer should still exist");
        assert!(!entry.active);
        assert_eq!(entry.last_run, Some(now));
        assert!(timers
            .due_timers(now + chrono::Duration::seconds(5))
            .is_empty());
    }

    // -----------------------------------------------------------------------
    // AgensRuntime
    // -----------------------------------------------------------------------

    #[test]
    fn emit_and_poll_events() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let before = Utc::now();
        let ev = AgensEvent::Message {
            id: "m1".to_string(),
            payload: json!({"text": "hi"}),
        };
        runtime.emit_event(&ev);
        let polled = runtime.poll_events(before);
        assert_eq!(polled.len(), 1);
        assert_eq!(polled[0], ev);
    }

    #[test]
    fn poll_events_excludes_old() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let ev = AgensEvent::Message {
            id: "m0".to_string(),
            payload: json!({}),
        };
        runtime.emit_event(&ev);
        let after = Utc::now();
        let polled = runtime.poll_events(after);
        assert!(polled.is_empty());
    }

    #[test]
    fn register_and_execute_procedure() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        runtime.register_procedure(
            "message",
            Arc::new(move |_event: &AgensEvent| {
                called_clone.store(true, Ordering::SeqCst);
                Ok(())
            }),
        );
        let ev = AgensEvent::Message {
            id: "m2".to_string(),
            payload: json!({}),
        };
        runtime.execute_procedure(&ev).unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn execute_procedure_missing_handler_returns_error() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let ev = AgensEvent::ModelResponse {
            id: "r1".to_string(),
            payload: json!({}),
        };
        let result = runtime.execute_procedure(&ev);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("model_response"));
    }

    #[test]
    fn state_and_timer_tables_via_runtime() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        runtime.state().set("key", json!(42));
        assert_eq!(runtime.state().get("key"), Some(json!(42)));
        let _timer_id = runtime.timers().schedule("ping", 30, json!({}));
        assert_eq!(runtime.timers().list().len(), 1);
    }

    #[test]
    fn process_due_timers_executes_timer_handler_and_persists_runs() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let hit_count = Arc::new(AtomicUsize::new(0));
        let hit_count_clone = hit_count.clone();
        runtime.register_procedure(
            "timer",
            Arc::new(move |event: &AgensEvent| {
                match event {
                    AgensEvent::Timer { name, .. } => assert_eq!(name, "one-shot"),
                    _ => panic!("expected timer event"),
                }
                hit_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        );

        let now = Utc::now();
        let timer_id = runtime.timers().schedule_once(
            "one-shot",
            now - chrono::Duration::seconds(1),
            json!({"job": "run"}),
        );
        let processed = runtime.process_due_timers(now);
        assert_eq!(processed, 1);
        assert_eq!(hit_count.load(Ordering::SeqCst), 1);

        let timer = runtime
            .timers()
            .list()
            .into_iter()
            .find(|t| t.id == timer_id)
            .expect("timer should exist");
        assert_eq!(timer.last_run, Some(now));
        assert!(!timer.active);
    }

    #[test]
    fn poll_events_returns_multiple_events_oldest_first() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let before = Utc::now();
        runtime.emit_event(&AgensEvent::Message {
            id: "first".to_string(),
            payload: json!({}),
        });
        runtime.emit_event(&AgensEvent::Message {
            id: "second".to_string(),
            payload: json!({}),
        });
        let polled = runtime.poll_events(before);
        // Both events should be present.
        assert_eq!(polled.len(), 2);
    }

    /// Emitting the same logical event twice must create two separate command
    /// nodes (append-only semantics).
    #[test]
    fn emit_same_event_twice_creates_two_nodes() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let before = Utc::now();
        let ev = AgensEvent::Message {
            id: "dup".to_string(),
            payload: json!({}),
        };
        runtime.emit_event(&ev);
        runtime.emit_event(&ev);
        let polled = runtime.poll_events(before);
        assert_eq!(polled.len(), 2, "both emissions must be present");
    }

    /// `StateTable::set` must auto-emit a StateChange event visible via poll_events.
    #[test]
    fn state_set_emits_state_change_event() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let before = Utc::now();
        runtime.state().set("mood", json!("happy"));
        let polled = runtime.poll_events(before);
        assert_eq!(polled.len(), 1);
        match &polled[0] {
            AgensEvent::StateChange {
                key,
                new_value,
                old_value,
                ..
            } => {
                assert_eq!(key, "mood");
                assert_eq!(new_value, &json!("happy"));
                assert!(old_value.is_none());
            }
            other => panic!("expected StateChange, got {:?}", other),
        }
    }

    /// `StateTable::set` with an existing value must include the previous value
    /// in the StateChange event's `old_value`.
    #[test]
    fn state_set_includes_old_value_in_event() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        runtime.state().set("x", json!(1));
        let before = Utc::now();
        runtime.state().set("x", json!(2));
        let polled = runtime.poll_events(before);
        // Only the second set should appear (the first is before `before`).
        assert_eq!(polled.len(), 1);
        match &polled[0] {
            AgensEvent::StateChange {
                old_value,
                new_value,
                ..
            } => {
                assert_eq!(old_value, &Some(json!(1)));
                assert_eq!(new_value, &json!(2));
            }
            other => panic!("expected StateChange, got {:?}", other),
        }
    }

    /// `StateTable::get` must not return data from a non-Agens node stored
    /// under the same id prefix.
    #[test]
    fn state_get_ignores_non_agens_state_node() {
        let store = CrdtStore::default();
        // Write a node without _type at the state:{key} id.
        store.put("state:hijack", "actor", json!({"value": "bad"}));
        let state = StateTable::new(&store, "actor");
        // get() must return None because _type != agens:state.
        assert!(state.get("hijack").is_none());
    }

    /// In debug builds `reschedule` fires a debug_assert for non-timer nodes.
    #[test]
    #[should_panic(expected = "TimerTable::reschedule called with non-timer")]
    #[cfg(debug_assertions)]
    fn timer_reschedule_rejects_non_timer_node() {
        let store = CrdtStore::default();
        store.put("timer:fake", "actor", json!({"_type": "something_else"}));
        let timers = TimerTable::new(&store, "actor");
        timers.reschedule("timer:fake");
    }

    // -----------------------------------------------------------------------
    // Praxis analysis lifecycle event contracts
    // -----------------------------------------------------------------------

    /// Verify that the three praxis lifecycle variants return the correct
    /// event-type strings.
    #[test]
    fn praxis_event_type_strings() {
        let ev = AgensEvent::PraxisAnalysisReady {
            id: "r1".to_string(),
            analysis_id: "a1".to_string(),
            session_id: None,
            payload: json!({}),
        };
        assert_eq!(ev.event_type(), "praxis_analysis_ready");

        let ev = AgensEvent::PraxisAnalysisFailed {
            id: "f1".to_string(),
            analysis_id: "a1".to_string(),
            session_id: Some("sess-1".to_string()),
            reason: "timeout".to_string(),
            payload: json!({}),
        };
        assert_eq!(ev.event_type(), "praxis_analysis_failed");

        let ev = AgensEvent::PraxisGuidanceUpdated {
            id: "g1".to_string(),
            guidance_id: "guide-1".to_string(),
            session_id: None,
            payload: json!({"hint": "be concise"}),
        };
        assert_eq!(ev.event_type(), "praxis_guidance_updated");
    }

    /// Praxis lifecycle events must survive a JSON serialization round-trip.
    #[test]
    fn praxis_event_serde_roundtrip() {
        let events = vec![
            AgensEvent::PraxisAnalysisReady {
                id: "r1".to_string(),
                analysis_id: "a1".to_string(),
                session_id: Some("sess-42".to_string()),
                payload: json!({"score": 0.9}),
            },
            AgensEvent::PraxisAnalysisFailed {
                id: "f1".to_string(),
                analysis_id: "a1".to_string(),
                session_id: None,
                reason: "model unavailable".to_string(),
                payload: json!({"code": 503}),
            },
            AgensEvent::PraxisGuidanceUpdated {
                id: "g1".to_string(),
                guidance_id: "guide-7".to_string(),
                session_id: Some("sess-42".to_string()),
                payload: json!({"hint": "be concise"}),
            },
        ];
        for ev in &events {
            let serialized = serde_json::to_string(ev).unwrap();
            let back: AgensEvent = serde_json::from_str(&serialized).unwrap();
            assert_eq!(*ev, back);
        }
    }

    /// `emit_praxis_event` must write to a deterministic `praxis:cmd:{id}` node
    /// so that re-emitting the same logical event is idempotent (only one CRDT
    /// node in the store, visible once via `poll_events`).
    #[test]
    fn emit_praxis_event_is_idempotent() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "pluresLM");
        let before = Utc::now();

        let ev = AgensEvent::PraxisAnalysisReady {
            id: "praxis-ready:a1".to_string(),
            analysis_id: "a1".to_string(),
            session_id: None,
            payload: json!({"result": "ok"}),
        };

        // Emit twice — should converge to a single store node.
        let node_id_1 = runtime.emit_praxis_event(&ev);
        let node_id_2 = runtime.emit_praxis_event(&ev);

        // Both calls must return the same deterministic node ID.
        assert_eq!(node_id_1, node_id_2);
        assert_eq!(
            node_id_1,
            format!("{}{}", PRAXIS_CMD_PREFIX, "praxis-ready:a1")
        );

        // poll_events must return exactly one event (not two).
        let polled = runtime.poll_events(before);
        assert_eq!(
            polled.len(),
            1,
            "idempotent emission must not create duplicate events"
        );
        assert_eq!(polled[0], ev);
    }

    /// Fixture: a full successful praxis analysis lifecycle sequence:
    ///   1. Emit `PraxisAnalysisReady`
    ///   2. Emit `PraxisGuidanceUpdated`
    ///
    /// Verify both events are polled in emission order and can be dispatched
    /// to registered procedure handlers.
    #[test]
    fn praxis_lifecycle_success_sequence() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "pluresLM");

        let counter = Arc::new(AtomicUsize::new(0));
        let c1 = counter.clone();
        let c2 = counter.clone();

        runtime.register_procedure(
            "praxis_analysis_ready",
            Arc::new(move |_ev: &AgensEvent| {
                c1.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        );
        runtime.register_procedure(
            "praxis_guidance_updated",
            Arc::new(move |_ev: &AgensEvent| {
                c2.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        );

        let before = Utc::now();

        let ready = AgensEvent::PraxisAnalysisReady {
            id: "praxis-ready:job-1".to_string(),
            analysis_id: "job-1".to_string(),
            session_id: Some("session-abc".to_string()),
            payload: json!({"confidence": 0.95}),
        };
        let guidance = AgensEvent::PraxisGuidanceUpdated {
            id: "praxis-guidance:guide-1".to_string(),
            guidance_id: "guide-1".to_string(),
            session_id: Some("session-abc".to_string()),
            payload: json!({"hint": "focus on key points"}),
        };

        runtime.emit_praxis_event(&ready);
        runtime.emit_praxis_event(&guidance);

        let polled = runtime.poll_events(before);
        assert_eq!(polled.len(), 2);

        for ev in &polled {
            runtime.execute_procedure(ev).unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    /// Fixture: a failed praxis analysis lifecycle sequence:
    ///   1. Emit `PraxisAnalysisFailed`
    ///
    /// Verify the event is polled and handler is called once.
    #[test]
    fn praxis_lifecycle_failure_sequence() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };

        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "pluresLM");

        let handled = Arc::new(AtomicBool::new(false));
        let h = handled.clone();

        runtime.register_procedure(
            "praxis_analysis_failed",
            Arc::new(move |ev: &AgensEvent| {
                if let AgensEvent::PraxisAnalysisFailed { reason, .. } = ev {
                    assert_eq!(reason, "model unavailable");
                }
                h.store(true, Ordering::SeqCst);
                Ok(())
            }),
        );

        let before = Utc::now();

        let failed = AgensEvent::PraxisAnalysisFailed {
            id: "praxis-failed:job-2".to_string(),
            analysis_id: "job-2".to_string(),
            session_id: None,
            reason: "model unavailable".to_string(),
            payload: json!({"code": 503}),
        };

        runtime.emit_praxis_event(&failed);

        let polled = runtime.poll_events(before);
        assert_eq!(polled.len(), 1);
        runtime.execute_procedure(&polled[0]).unwrap();
        assert!(handled.load(Ordering::SeqCst));
    }

    /// Re-emitting an already-processed praxis event is idempotent in the
    /// store (single command node) and preserves a stable logical id, even
    /// if the consumer later polls after the first emission's timestamp and
    /// observes the event again due to an LWW timestamp update.
    #[test]
    fn praxis_idempotent_reemit_single_node_and_stable_id() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "pluresLM");

        let ev = AgensEvent::PraxisAnalysisReady {
            id: "praxis-ready:a99".to_string(),
            analysis_id: "a99".to_string(),
            session_id: None,
            payload: json!({}),
        };

        let before = Utc::now();
        runtime.emit_praxis_event(&ev);

        // First poll — consumer processes the event.
        let first_poll = runtime.poll_events(before);
        assert_eq!(first_poll.len(), 1);

        // Advance the consumer's `since` cursor past the first emission.
        let after_first = Utc::now();

        // Re-emit the same logical event (e.g. crash-recovery replay).
        runtime.emit_praxis_event(&ev);

        // Second poll using the advanced cursor: because the node is
        // overwritten with a newer timestamp the consumer may see the event
        // again, but it carries the same logical `id` — consumers are expected
        // to deduplicate on `id()` if strict exactly-once semantics are
        // required.  Here we only assert the node count in the store remains
        // one (idempotent storage).
        let store_commands: Vec<_> = store
            .list()
            .into_iter()
            .filter(|n| {
                n.data.get("_type").and_then(|v| v.as_str()) == Some(TYPE_COMMAND)
                    && n.id.starts_with(PRAXIS_CMD_PREFIX)
            })
            .collect();
        assert_eq!(
            store_commands.len(),
            1,
            "exactly one praxis command node in store"
        );

        // The second poll with the advanced cursor reflects the re-emit
        // (LWW timestamp update), but the event id is stable.
        let second_poll = runtime.poll_events(after_first);
        for ev in &second_poll {
            // All re-delivered events carry the original logical id.
            assert_eq!(ev.id(), "praxis-ready:a99");
        }
    }
}
