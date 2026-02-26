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
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use pluresdb_core::CrdtStore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use uuid::Uuid;
use tracing::warn;

// Node type tags used to namespace Agens data inside the CRDT store.
const TYPE_COMMAND: &str = "agens:command";
const TYPE_STATE: &str = "agens:state";
const TYPE_TIMER: &str = "agens:timer";

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
}

impl AgensEvent {
    /// Return the event-type string used as a handler lookup key.
    ///
    /// One of: `"message"`, `"timer"`, `"state_change"`,
    /// `"model_response"`, `"tool_result"`.
    pub fn event_type(&self) -> &'static str {
        match self {
            AgensEvent::Message { .. } => "message",
            AgensEvent::Timer { .. } => "timer",
            AgensEvent::StateChange { .. } => "state_change",
            AgensEvent::ModelResponse { .. } => "model_response",
            AgensEvent::ToolResult { .. } => "tool_result",
        }
    }

    /// Return the unique ID of this event.
    pub fn id(&self) -> &str {
        match self {
            AgensEvent::Message { id, .. }
            | AgensEvent::Timer { id, .. }
            | AgensEvent::StateChange { id, .. }
            | AgensEvent::ModelResponse { id, .. }
            | AgensEvent::ToolResult { id, .. } => id.as_str(),
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
        if node
            .data
            .get("_type")
            .and_then(|v| v.as_str())
            != Some(TYPE_STATE)
        {
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
    /// Firing interval in seconds.
    pub interval_secs: u64,
    /// UTC timestamp of the next scheduled firing.
    pub next_fire_at: DateTime<Utc>,
    /// Arbitrary payload forwarded to the handler when the timer fires.
    pub payload: JsonValue,
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

    /// Schedule a recurring timer named `name` to fire every `interval_secs`
    /// seconds.  Returns the timer node ID (use it with [`cancel`][Self::cancel]
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
        // Validate interval to avoid zero-duration loops and integer wrap-around.
        if interval_secs == 0 {
            panic!("TimerTable::schedule: interval_secs must be greater than 0");
        }

        let interval_secs_i64 =
            i64::try_from(interval_secs).expect("TimerTable::schedule: interval_secs too large");

        let id = format!("timer:{}", Uuid::new_v4());
        let next_fire_at =
            Utc::now() + chrono::Duration::seconds(interval_secs_i64);
        self.store.put(
            id.clone(),
            self.actor.as_str(),
            json!({
                "_type": TYPE_TIMER,
                "name": name,
                "interval_secs": interval_secs,
                "next_fire_at": next_fire_at.to_rfc3339(),
                "payload": payload,
            }),
        );
        id
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
            .filter(|n| {
                n.data.get("_type").and_then(|v| v.as_str()) == Some(TYPE_TIMER)
            })
            .filter_map(|n| self.entry_from_data(&n.id, &n.data))
            .collect()
    }

    /// Return timers whose `next_fire_at` is at or before `now`.
    ///
    /// Delegates to [`list`][Self::list]; see its documentation for
    /// performance characteristics.
    pub fn due_timers(&self, now: DateTime<Utc>) -> Vec<TimerEntry> {
        self.list().into_iter().filter(|t| t.next_fire_at <= now).collect()
    }

    /// Advance a timer's `next_fire_at` by one interval.
    ///
    /// Call this after the timer has been processed to re-arm it for the next
    /// cycle.  Returns `false` if `timer_id` does not exist.
    pub fn reschedule(&self, timer_id: &str) -> bool {
        let Some(node) = self.store.get(timer_id) else {
            return false;
        };
        let node_type = node.data.get("_type").and_then(|v| v.as_str());
        if node_type != Some(TYPE_TIMER) {
            debug_assert!(
                false,
                "TimerTable::reschedule called with non-timer node id `{}` (type: {:?})",
                timer_id,
                node_type,
            );
            return false;
        }
        let Some(entry) = self.entry_from_data(&node.id, &node.data) else {
            return false;
        let Some(node) = self.store.get(timer_id) else {
            eprintln!(
                "Agens timer reschedule failed: timer with id '{}' not found",
                timer_id
            );
            return false;
        };
        let Some(entry) = self.entry_from_data(&node.id, &node.data) else {
            eprintln!(
                "Agens timer reschedule failed: invalid timer entry for id '{}'",
                node.id
            );
            return false;
        };
        // Convert the stored interval to i64 in a checked way to avoid overflow.
        let Ok(interval_i64) = i64::try_from(entry.interval_secs) else {
            eprintln!(
                "Agens timer reschedule failed: interval_secs '{}' for timer '{}' \
                 exceeds i64::MAX",
                entry.interval_secs,
                entry.name
            );
            return false;
        };
        let duration = chrono::Duration::seconds(interval_i64);
        // Use checked_add_signed to avoid overflowing the DateTime.
        let Some(next) = entry.next_fire_at.checked_add_signed(duration) else {
            eprintln!(
                "Agens timer reschedule failed: next_fire_at overflow for timer '{}' \
                 (id '{}') when adding {} seconds",
                entry.name,
                node.id,
                interval_i64
            );
            warn!(
                timer_id,
                ?duration,
                "TimerTable::reschedule: next_fire_at would overflow DateTime bounds, \
                 skipping reschedule"
            );
            return false;
        };
        self.store.put(
            timer_id,
            self.actor.as_str(),
            json!({
                "_type": TYPE_TIMER,
                "name": entry.name,
                "interval_secs": entry.interval_secs,
                "next_fire_at": next.to_rfc3339(),
                "payload": entry.payload,
            }),
        );
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
        let interval_secs = data.get("interval_secs")?.as_u64()?;
        let next_fire_at: DateTime<Utc> = data
            .get("next_fire_at")?
            .as_str()
            .and_then(|s| s.parse().ok())?;
        let payload = data.get("payload").cloned().unwrap_or(JsonValue::Null);
        Some(TimerEntry {
            id: id.to_string(),
            name,
            interval_secs,
            next_fire_at,
            payload,
        })
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
                "logical_id": event.id(),
                "event": serde_json::to_value(event)
                    .expect("AgensEvent serialization should not fail"),
            }),
        );
        node_id
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
                let ev: AgensEvent =
                    serde_json::from_value(n.data.get("event")?.clone()).ok()?;
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
        let ev = AgensEvent::Message { id: "1".to_string(), payload: json!({}) };
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

        let ev = AgensEvent::ModelResponse { id: "4".to_string(), payload: json!({}) };
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
        let ev = AgensEvent::Message { id: "m0".to_string(), payload: json!({}) };
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
        let ev = AgensEvent::Message { id: "m2".to_string(), payload: json!({}) };
        runtime.execute_procedure(&ev).unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn execute_procedure_missing_handler_returns_error() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let ev = AgensEvent::ModelResponse { id: "r1".to_string(), payload: json!({}) };
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
        let ev = AgensEvent::Message { id: "dup".to_string(), payload: json!({}) };
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
            AgensEvent::StateChange { key, new_value, old_value, .. } => {
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
            AgensEvent::StateChange { old_value, new_value, .. } => {
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
}
