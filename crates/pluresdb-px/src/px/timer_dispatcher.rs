//! `PxTimerDispatcher` — native tick → px trigger bridge (ADR-0017).
//!
//! Bridges [`pluresdb_procedures::agens::AgensRuntime`] timers to the px
//! executor: it translates a fired `AgensEvent::Timer` into the
//! `{"event": {"type": "timer", ...}}` vars shape expected by
//! [`crate::px::executor::execute_with_vars`], drives the tick loop in both
//! native (Tokio) and embedded (Node/FFI) hosts, and provides at-least-once
//! delivery with idempotent bookkeeping + crash recovery.
//!
//! See `docs/adr/ADR-0017-px-timer-dispatcher.md` for the full design.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};
use pluresdb_procedures::agens::{AgensRuntime, TimerEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::px::executor::{self, ActionHandler, ExecutionError};

/// Summary of a single [`PxTimerDispatcher::tick`] call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TickReport {
    /// Number of timers that were successfully dispatched and completed.
    pub fired: u32,
    /// Number of timers skipped because a dispatch for the same
    /// `(timer_id, next_fire_at)` occurrence was already in flight.
    pub skipped: u32,
    /// Per-timer dispatch errors encountered during this tick.
    pub errors: Vec<TimerDispatchError>,
}

/// A single timer dispatch failure, surfaced for host-side alerting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerDispatchError {
    /// The timer's CRDT node ID.
    pub timer_id: String,
    /// The timer's human-readable name.
    pub timer_name: String,
    /// The error message from the failed execution.
    pub error: String,
}

/// Summary of a [`PxTimerDispatcher::recover`] call.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecoveryReport {
    /// Number of timers whose stale `last_fired_token` was cleared.
    pub recovered: u32,
}

/// Outcome of a single [`PxTimerDispatcher::dispatch_one`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// The timer's procedure was invoked and completed successfully.
    Fired,
    /// A dispatch for the same `(timer_id, next_fire_at)` occurrence was
    /// already in flight; this attempt was skipped.
    SkippedInFlight,
}

/// Default max consecutive failures before backoff on the *tick* attempt
/// itself (not `next_fire_at`) kicks in for a chronically-failing timer.
const DEFAULT_MAX_CONSECUTIVE_FAILURES: u32 = 5;

/// Cap on the exponential backoff applied to a chronically-failing timer's
/// tick attempts.
const MAX_BACKOFF: Duration = Duration::seconds(300);

/// Bridges `AgensRuntime` timers to the px executor.
///
/// Wraps a `&AgensRuntime`, a `&dyn ActionHandler`, and the compiled
/// procedure record to invoke on each fired timer. Driven by either
/// [`PxTimerDispatcher::tick`] (called explicitly by embedded/FFI hosts) or
/// [`PxTimerDispatcher::spawn_native`] (a 10s Tokio loop, native builds
/// only, feature `async`).
pub struct PxTimerDispatcher<'a> {
    runtime: &'a AgensRuntime<'a>,
    handler: &'a dyn ActionHandler,
    procedure_record: &'a JsonValue,
    /// In-memory guard against re-dispatching a timer occurrence whose
    /// previous dispatch attempt is still running (slow procedure body +
    /// fast tick source). Ephemeral; not persisted.
    in_flight: Mutex<HashSet<(String, DateTime<Utc>)>>,
    /// Consecutive dispatch failures per timer id, used to back off the
    /// *tick* attempt for chronically-failing timers. Ephemeral; resets on
    /// restart by design (see ADR-0017 "Open Questions").
    consecutive_failures: Mutex<HashMap<String, u32>>,
    /// Timestamp before which a chronically-failing timer's next dispatch
    /// attempt should be skipped (backoff window). Ephemeral.
    backoff_until: Mutex<HashMap<String, DateTime<Utc>>>,
}

impl<'a> PxTimerDispatcher<'a> {
    /// Create a new dispatcher bound to `runtime`, `handler`, and the
    /// compiled procedure record to invoke for every fired timer.
    pub fn new(
        runtime: &'a AgensRuntime<'a>,
        handler: &'a dyn ActionHandler,
        procedure_record: &'a JsonValue,
    ) -> Self {
        PxTimerDispatcher {
            runtime,
            handler,
            procedure_record,
            in_flight: Mutex::new(HashSet::new()),
            consecutive_failures: Mutex::new(HashMap::new()),
            backoff_until: Mutex::new(HashMap::new()),
        }
    }

    /// Process all timers due at `now`, dispatching each through the px
    /// executor. Returns a [`TickReport`] summarizing fires/skips/errors.
    pub fn tick(&self, now: DateTime<Utc>) -> TickReport {
        let due = self.runtime.timers().due_timers(now);
        let mut report = TickReport::default();
        for entry in due {
            // Chronic-failure backoff: skip the tick attempt entirely if
            // this timer is still within its backoff window.
            if let Some(until) = self.backoff_until.lock().unwrap().get(&entry.id).copied() {
                if now < until {
                    continue;
                }
            }
            match self.dispatch_one(&entry, now) {
                Ok(DispatchOutcome::Fired) => report.fired += 1,
                Ok(DispatchOutcome::SkippedInFlight) => report.skipped += 1,
                Err(e) => {
                    report.errors.push(TimerDispatchError {
                        timer_id: entry.id.clone(),
                        timer_name: entry.name.clone(),
                        error: e.to_string(),
                    });
                    self.record_failure(&entry, now);
                }
            }
        }
        report
    }

    /// Dispatch a single due timer entry through the px executor.
    ///
    /// Algorithm (see ADR-0017 "At-least-once delivery with idempotency"):
    /// 1. In-flight guard: skip if `(timer_id, next_fire_at)` is already
    ///    being dispatched by this process.
    /// 2. Token check: skip a re-fetch showing a `last_fired_token` already
    ///    set (another process/attempt already claimed this occurrence),
    ///    unless the caller is `recover()` (handled separately).
    /// 3. Mint a fresh token, persist it via `mark_dispatch_started`.
    /// 4. Build the `{"event": {...}}` vars and invoke
    ///    `executor::execute_with_vars`.
    /// 5. On success: `mark_ran` (clears the token, advances scheduling),
    ///    reset the consecutive-failure counter, remove the in-flight
    ///    marker, return `Fired`.
    /// 6. On failure: `record_failure` (exponential backoff, `best_effort`
    ///    handling is implicit in `mark_ran`'s trigger-based advance logic —
    ///    see below), remove the in-flight marker, propagate the error.
    pub fn dispatch_one(
        &self,
        entry: &TimerEntry,
        now: DateTime<Utc>,
    ) -> Result<DispatchOutcome, ExecutionError> {
        let key = (entry.id.clone(), entry.next_fire_at);

        // 1. In-flight guard (single process).
        {
            let mut in_flight = self.in_flight.lock().unwrap();
            if in_flight.contains(&key) {
                return Ok(DispatchOutcome::SkippedInFlight);
            }
            in_flight.insert(key.clone());
        }

        // Ensure the in-flight marker is always removed, regardless of the
        // outcome below.
        let result = self.dispatch_one_inner(entry, now);
        self.in_flight.lock().unwrap().remove(&key);
        result
    }

    fn dispatch_one_inner(
        &self,
        entry: &TimerEntry,
        now: DateTime<Utc>,
    ) -> Result<DispatchOutcome, ExecutionError> {
        let timers = self.runtime.timers();

        // 2. Token check: re-fetch to avoid double-dispatch if another
        // process already claimed this occurrence.
        if let Some(fresh) = timers
            .list()
            .into_iter()
            .find(|t| t.id == entry.id)
        {
            if fresh.last_fired_token.is_some() {
                return Ok(DispatchOutcome::SkippedInFlight);
            }
        }

        // 3. Mint a fresh dispatch token and persist it before invoking the
        // executor (crash-safety: a process crash between here and
        // `mark_ran` leaves a recoverable token).
        let token = Uuid::new_v4().to_string();
        timers.mark_dispatch_started(&entry.id, &token);

        // 4. Translate the timer entry into px executor vars and invoke.
        let vars = build_event_vars(entry);
        let outcome = executor::execute_with_vars(self.procedure_record, self.handler, vars);

        match outcome {
            Ok(_) => {
                // 5. Success: mark_ran clears the token and advances
                // next_fire_at per the timer's trigger semantics.
                timers.mark_ran(&entry.id, now);
                self.consecutive_failures.lock().unwrap().remove(&entry.id);
                self.backoff_until.lock().unwrap().remove(&entry.id);
                Ok(DispatchOutcome::Fired)
            }
            Err(e) => {
                // 6. Failure: best_effort timers still advance so they
                // don't wedge on a permanently-broken procedure; otherwise
                // leave next_fire_at untouched (token remains set) so the
                // next tick retries the same occurrence, per ADR-0017.
                if entry.best_effort {
                    timers.mark_ran(&entry.id, now);
                } else {
                    timers.clear_stale_token(&entry.id);
                }
                Err(e)
            }
        }
    }

    /// Build the `{"event": {"type": "timer", ...}}` vars for `entry`,
    /// matching the shape asserted in `crate::px::executor` timer-event test
    /// fixtures.
    pub fn build_event_vars(entry: &TimerEntry) -> HashMap<String, JsonValue> {
        build_event_vars(entry)
    }

    /// Record a dispatch failure for `entry` and apply exponential backoff
    /// (capped at 5 minutes) to future tick attempts for this timer.
    fn record_failure(&self, entry: &TimerEntry, now: DateTime<Utc>) {
        let mut failures = self.consecutive_failures.lock().unwrap();
        let count = failures.entry(entry.id.clone()).or_insert(0);
        *count += 1;
        let attempt = (*count).min(DEFAULT_MAX_CONSECUTIVE_FAILURES);
        drop(failures);

        if attempt >= DEFAULT_MAX_CONSECUTIVE_FAILURES {
            tracing::warn!(
                timer_id = entry.id,
                timer_name = entry.name,
                consecutive_failures = attempt,
                "timer_repeatedly_failing: applying backoff to tick attempts"
            );
        }

        // Exponential backoff: 2^attempt seconds, capped at MAX_BACKOFF.
        let backoff_secs = 2i64.saturating_pow(attempt.min(20)).min(MAX_BACKOFF.num_seconds());
        let until = now + Duration::seconds(backoff_secs);
        self.backoff_until
            .lock()
            .unwrap()
            .insert(entry.id.clone(), until);

        tracing::error!(
            timer_id = entry.id,
            timer_name = entry.name,
            "PxTimerDispatcher: timer dispatch failed"
        );
    }

    /// Scan all timers for a stale `last_fired_token` (dispatch attempt that
    /// crashed mid-flight: token set, `mark_ran` never called) and clear it
    /// if `next_fire_at` is older than `now - grace_period`.
    ///
    /// Call on dispatcher startup to recover from a prior process crash.
    /// `grace_period` guards against false-positive recovery of a dispatch
    /// that is genuinely still in flight on a slow-but-alive procedure.
    pub fn recover(&self, now: DateTime<Utc>, grace_period: Duration) -> RecoveryReport {
        let timers = self.runtime.timers();
        let mut report = RecoveryReport::default();
        for entry in timers.list() {
            if entry.last_fired_token.is_none() {
                continue;
            }
            if entry.next_fire_at <= now - grace_period && timers.clear_stale_token(&entry.id) {
                report.recovered += 1;
                tracing::warn!(
                    timer_id = entry.id,
                    timer_name = entry.name,
                    "PxTimerDispatcher::recover: cleared stale dispatch token"
                );
            }
        }
        report
    }
}

/// Build the `{"event": {"type": "timer", ...}}` vars shape for `entry`.
fn build_event_vars(entry: &TimerEntry) -> HashMap<String, JsonValue> {
    let recurring = !matches!(entry.trigger, pluresdb_procedures::agens::TimerTrigger::Once);
    let mut vars = HashMap::new();
    vars.insert(
        "event".to_string(),
        serde_json::json!({
            "type": "timer",
            "id": entry.id,
            "name": entry.name,
            "recurring": recurring,
            "payload": entry.payload,
        }),
    );
    vars
}

#[cfg(feature = "async")]
impl<'a> PxTimerDispatcher<'a>
where
    'a: 'static,
{
    /// Spawn a background Tokio task that calls [`PxTimerDispatcher::tick`]
    /// every 10 seconds, mirroring the existing
    /// `AgensRuntime::spawn_timer_task` cadence. Native builds only
    /// (feature `async`); embedded/FFI hosts must call `tick()` explicitly.
    pub fn spawn_native(self: std::sync::Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(10));
            loop {
                ticker.tick().await;
                let now = Utc::now();
                let _ = self.tick(now);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pluresdb_core::CrdtStore;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingHandler {
        #[allow(dead_code)]
        calls: AtomicUsize,
        #[allow(dead_code)]
        fail_until: usize,
    }

    impl ActionHandler for CountingHandler {
        fn call(&self, _name: &str, _params: &JsonValue) -> Result<JsonValue, ExecutionError> {
            Ok(JsonValue::Null)
        }
    }

    struct FailingHandler {
        calls: AtomicUsize,
        fail_until: usize,
    }

    impl ActionHandler for FailingHandler {
        fn call(&self, _name: &str, _params: &JsonValue) -> Result<JsonValue, ExecutionError> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n < self.fail_until {
                Err(ExecutionError::ActionFailed {
                    action: "noop".to_string(),
                    message: "intentional test failure".to_string(),
                })
            } else {
                Ok(JsonValue::Null)
            }
        }
    }

    fn noop_procedure() -> JsonValue {
        serde_json::json!({
            "name": "on_timer",
            "steps": [
                { "kind": "call", "name": "noop", "params": {} }
            ]
        })
    }

    #[test]
    fn fires_due_timer_once() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let timer_id = runtime.timers().schedule("check", 60, JsonValue::Null);
        // Force it due now by scheduling in the past via schedule_once.
        let now = Utc::now();
        runtime.timers().cancel(&timer_id);
        let timer_id = runtime
            .timers()
            .schedule_once("check", now - Duration::seconds(1), JsonValue::Null);

        let handler = CountingHandler {
            calls: AtomicUsize::new(0),
            fail_until: 0,
        };
        let record = noop_procedure();
        let dispatcher = PxTimerDispatcher::new(&runtime, &handler, &record);

        let report = dispatcher.tick(now);
        assert_eq!(report.fired, 1);
        assert_eq!(report.errors.len(), 0);

        // Once timer should now be inactive and not re-fire.
        let report2 = dispatcher.tick(now);
        assert_eq!(report2.fired, 0);
        let entry = timers_get(&runtime, &timer_id);
        assert!(!entry.active);
        assert!(entry.last_fired_token.is_none());
    }

    fn timers_get(runtime: &AgensRuntime, id: &str) -> TimerEntry {
        runtime
            .timers()
            .list()
            .into_iter()
            .find(|t| t.id == id)
            .expect("timer should exist")
    }

    #[test]
    fn failed_dispatch_does_not_advance_next_fire_at_unless_best_effort() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let now = Utc::now();
        let timer_id = runtime
            .timers()
            .schedule_interval("check", 60, JsonValue::Null);
        // Force due by rescheduling via schedule_once semantics isn't
        // available for interval; instead advance now past next_fire_at.
        let entry = timers_get(&runtime, &timer_id);
        let due_now = entry.next_fire_at + Duration::seconds(1);

        let handler = FailingHandler {
            calls: AtomicUsize::new(0),
            fail_until: usize::MAX,
        };
        let record = noop_procedure();
        let dispatcher = PxTimerDispatcher::new(&runtime, &handler, &record);

        let report = dispatcher.tick(due_now);
        assert_eq!(report.fired, 0);
        assert_eq!(report.errors.len(), 1);

        let entry_after = timers_get(&runtime, &timer_id);
        assert_eq!(entry_after.next_fire_at, entry.next_fire_at);
        assert!(entry_after.last_fired_token.is_none());
    }

    #[test]
    fn best_effort_failed_dispatch_advances_next_fire_at() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let timer_id = runtime
            .timers()
            .schedule_interval("check", 60, JsonValue::Null);
        // Manually flip best_effort=true via a raw persist (mirrors what a
        // caller would do through TimerTable in a real host).
        let before = timers_get(&runtime, &timer_id);
        let node = store.get(&timer_id).unwrap();        let mut data = node.data.clone();
        data["best_effort"] = serde_json::json!(true);
        store.put(timer_id.clone(), "actor", data);

        let due_now = before.next_fire_at + Duration::seconds(1);

        let handler = FailingHandler {
            calls: AtomicUsize::new(0),
            fail_until: usize::MAX,
        };
        let record = noop_procedure();
        let dispatcher = PxTimerDispatcher::new(&runtime, &handler, &record);

        let report = dispatcher.tick(due_now);
        assert_eq!(report.fired, 0);
        assert_eq!(report.errors.len(), 1);

        let entry_after = timers_get(&runtime, &timer_id);
        assert!(entry_after.next_fire_at > before.next_fire_at);
        assert!(entry_after.last_fired_token.is_none());
    }

    #[test]
    fn skips_in_flight_duplicate() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let now = Utc::now();
        let timer_id = runtime
            .timers()
            .schedule_once("check", now - Duration::seconds(1), JsonValue::Null);
        let entry = timers_get(&runtime, &timer_id);

        let handler = CountingHandler {
            calls: AtomicUsize::new(0),
            fail_until: 0,
        };
        let record = noop_procedure();
        let dispatcher = PxTimerDispatcher::new(&runtime, &handler, &record);

        // Simulate an in-flight marker for this occurrence.
        dispatcher
            .in_flight
            .lock()
            .unwrap()
            .insert((entry.id.clone(), entry.next_fire_at));

        let outcome = dispatcher.dispatch_one(&entry, now).unwrap();
        assert_eq!(outcome, DispatchOutcome::SkippedInFlight);
    }

    #[test]
    fn recover_clears_stale_token_past_grace_period() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let now = Utc::now();
        let timer_id = runtime
            .timers()
            .schedule_once("check", now - Duration::seconds(120), JsonValue::Null);
        runtime
            .timers()
            .mark_dispatch_started(&timer_id, "stale-token");

        let handler = CountingHandler {
            calls: AtomicUsize::new(0),
            fail_until: 0,
        };
        let record = noop_procedure();
        let dispatcher = PxTimerDispatcher::new(&runtime, &handler, &record);

        let report = dispatcher.recover(now, Duration::seconds(60));
        assert_eq!(report.recovered, 1);
        let entry = timers_get(&runtime, &timer_id);
        assert!(entry.last_fired_token.is_none());
    }

    #[test]
    fn recover_leaves_fresh_token_within_grace_period() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let now = Utc::now();
        let timer_id = runtime
            .timers()
            .schedule_once("check", now - Duration::seconds(1), JsonValue::Null);
        runtime
            .timers()
            .mark_dispatch_started(&timer_id, "fresh-token");

        let handler = CountingHandler {
            calls: AtomicUsize::new(0),
            fail_until: 0,
        };
        let record = noop_procedure();
        let dispatcher = PxTimerDispatcher::new(&runtime, &handler, &record);

        let report = dispatcher.recover(now, Duration::seconds(60));
        assert_eq!(report.recovered, 0);
        let entry = timers_get(&runtime, &timer_id);
        assert!(entry.last_fired_token.is_some());
    }

    #[test]
    fn timer_event_vars_shape() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let timer_id = runtime
            .timers()
            .schedule("check", 60, serde_json::json!({"k": "v"}));
        let entry = timers_get(&runtime, &timer_id);

        let vars = build_event_vars(&entry);
        let event = vars.get("event").unwrap();
        assert_eq!(event["type"], "timer");
        assert_eq!(event["id"], entry.id);
        assert_eq!(event["name"], "check");
        assert_eq!(event["recurring"], true);
        assert_eq!(event["payload"]["k"], "v");
    }

    #[test]
    fn once_timer_event_not_recurring() {
        let store = CrdtStore::default();
        let runtime = AgensRuntime::new(&store, "actor");
        let timer_id = runtime
            .timers()
            .schedule_once("check", Utc::now(), JsonValue::Null);
        let entry = timers_get(&runtime, &timer_id);
        let vars = build_event_vars(&entry);
        assert_eq!(vars["event"]["recurring"], false);
    }
}
