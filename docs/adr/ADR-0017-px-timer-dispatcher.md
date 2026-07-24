# ADR-0017: PxTimerDispatcher — Native Tick → Px Trigger Bridge

**Status:** Proposed
**Date:** 2026-07-23
**Author:** epic-orchestrator (pluresdb:px-timer-dispatcher)

## Context

Two timer mechanisms exist in PluresDB today and are **not connected**:

1. **`AgensRuntime` timer table** (`crates/pluresdb-procedures/src/agens.rs`)
   - `TimerTable` persists `agens:timer` nodes in the CRDT store (`schedule`,
     `schedule_cron`, `schedule_once`, `due_timers`, `mark_ran`, `reschedule`, `cancel`).
   - `AgensRuntime::process_due_timers(now)` finds due timers, fires an
     `AgensEvent::Timer { id, name, payload }` through the registered `"timer"`
     handler (an in-process Rust closure), then persists `last_run` /
     `next_fire_at` via `mark_ran`.
   - `AgensRuntime::spawn_timer_task` (non-`wasm32`) spawns a Tokio task that calls
     `process_due_timers(Utc::now())` every 10 seconds. It requires a Tokio runtime
     and that the runtime's store reference is `'static` (see the bounds in `agens.rs`).
     This is the only built-in tick source.
   - The Node FFI (`crates/pluresdb-node/src/lib.rs`) exposes
     `agensTimerSchedule` / `agensTimerList` / `agensTimerDue` /
     `agensTimerReschedule` / `agensTimerCancel` but does **not** expose
     `process_due_timers` or the `"timer"` handler registration — JS callers
     must poll `agensTimerDue()` and manually re-implement dispatch.

2. **Px executor** (`crates/pluresdb-px/src/px/executor.rs`,
   `crates/pluresdb-px/src/px/watcher.rs`)
   - Px procedures declare triggers (`on_write`, event patterns, etc.) and are
     invoked with an `event` variable, e.g.
     `{ "event": { "type": "timer", "id": "t1", "name": "check", "recurring": false } }`
     (see `executor.rs` tests around line 4063). This shape is **already the
     execution contract** the dispatcher must produce — it is asserted by
     existing unit tests but nothing produces it outside of test fixtures.
   - `PxWatcher` (`watcher.rs`) is a hot-reload / file-watch mechanism for
     `.px` source files, not a runtime event source. It is unrelated to timer
     ticks and must not be confused with the dispatcher seam.
   - There is no code path today that takes a fired `AgensEvent::Timer` and
     turns it into a px procedure invocation. Nothing currently calls the
     executor with a `timer` event outside of hand-built test fixtures.

**The seam**: something must sit between `AgensRuntime::process_due_timers`
(the tick/fire source) and the px executor (the procedure-invocation sink),
translating `AgensEvent::Timer` → the executor's `{"event": {"type": "timer",
...}}` input shape, and driving the tick loop itself in both native (Tokio)
and embedded (Node/FFI, single-threaded) hosts. This component does not exist
in the codebase; this ADR names it `PxTimerDispatcher` and specifies its
implementation.

## Decision

Introduce `PxTimerDispatcher` in `pluresdb-px` (new module
`crates/pluresdb-px/src/px/timer_dispatcher.rs`) as the sole bridge between
`AgensRuntime` timers and the px executor. It owns **tick-driving**,
**event translation**, **exactly-once bookkeeping**, and **error
handling/recovery** for timer-triggered px procedures. It does not own
scheduling policy (interval/cron/once semantics stay in `TimerTable`) and
does not own procedure execution semantics (stays in `Executor`).

### Responsibilities

```
AgensRuntime::process_due_timers(now)
        │  (per due timer)
        ▼
PxTimerDispatcher::dispatch_timer(&entry, now)
        │  1. idempotency check (see below)
        │  2. translate TimerEntry -> px event JSON
        │  3. px::executor::execute_with_vars(procedure_record, handler, vars_with_event)
        │  4. record outcome (success/failure) + mark_ran
        ▼
Px procedure body executes with `event.type == "timer"`
```

`PxTimerDispatcher` wraps a `&AgensRuntime` and an `Executor` (or a handle to
one) and is driven by a **tick source**, of which there are exactly two
supported implementations in this design:

- `TokioTickSource` — native builds, feature `native`. Thin wrapper around
  the existing `spawn_timer_task` pattern, replaced to call
  `PxTimerDispatcher::tick(now)` instead of firing a bare handler.
- `ManualTickSource` — embedded/FFI builds (Node, WASM, tests). Exposes a
  single `pub fn tick(&self, now: DateTime<Utc>) -> TickReport` that host code
  (JS event loop, `setInterval`, WASM `requestAnimationFrame`, or a test
  harness) calls explicitly. This is a thin, already-almost-existing wrapper
  around `agensTimerDue()` + per-timer dispatch, now exposed as one FFI call
  instead of requiring JS to hand-roll the loop.

Both tick sources funnel through the same `PxTimerDispatcher::tick`
entry point, so dispatch semantics (idempotency, error handling) are
identical regardless of host.

### `PxTimerDispatcher::tick` algorithm

```rust
pub fn tick(&self, now: DateTime<Utc>) -> TickReport {
    let due = self.runtime.timers().due_timers(now);
    let mut report = TickReport::default();
    for entry in due {
        match self.dispatch_one(&entry, now) {
            Ok(DispatchOutcome::Fired) => report.fired += 1,
            Ok(DispatchOutcome::SkippedInFlight) => report.skipped += 1,
            Err(e) => {
                report.errors.push(TimerDispatchError {
                    timer_id: entry.id.clone(),
                    timer_name: entry.name.clone(),
                    error: e.to_string(),
                });
                self.record_failure(&entry, now, &e);
            }
        }
    }
    report
}
```

### Exactly-once / idempotency

Timers are **at-least-once** at the scheduling layer (a crash between
`dispatch_one` succeeding and `mark_ran` persisting could re-fire the same
logical tick), so `PxTimerDispatcher` makes the *procedure invocation*
idempotent rather than claiming true exactly-once delivery:

1. **In-flight guard (single process).** Before invoking the executor,
   `PxTimerDispatcher` inserts `(timer_id, next_fire_at)` into an in-memory
   `DashSet`/`Mutex<HashSet>` "in-flight" marker. If a tick for the same
   `(timer_id, next_fire_at)` pair is already in flight (only possible if a
   previous tick's executor call is still running when the next tick fires —
   e.g. slow px procedure + fast tick source), the new attempt is skipped and
   counted as `SkippedInFlight`, not silently dropped. Marker is removed after
   `dispatch_one` returns (success or failure).
2. **Fire-token in the store (multi-process / crash-safety).** Every
   `TimerEntry` gains a new persisted field `last_fired_token: Option<String>`
   (a UUID minted per fire attempt, written *before* invoking the executor via
   `TimerTable::mark_dispatch_started(timer_id, token)`, committed to the CRDT
   store). `dispatch_one` only proceeds if `entry.last_fired_token` is `None`
   or already resolved (see recovery below) — this prevents a second process
   or a retried tick from double-firing the same logical occurrence.
   `mark_ran` (existing) clears the token and advances `next_fire_at` only on
   confirmed success.
3. **Consequence for `Once` timers**: a one-shot timer whose dispatch fails
   remains `active` with its token set to `Some(..)`, so recovery (below)
   retries it instead of silently losing it or double-firing it.
4. **Consequence for `Interval`/`Cron` timers**: if dispatch fails, the timer
   is *not* rescheduled forward — `next_fire_at` stays at the same value so
   the next tick retries the same occurrence, bounded by the error/backoff
   policy below. This intentionally biases toward "run late" over "skip a
   scheduled occurrence" — the calling application can opt out per-timer via
   a `best_effort: bool` flag on `TimerEntry` (new field, default `false`)
   which, when `true`, advances `next_fire_at` even on failure (fire-and-
   forget telemetry-style timers).

This gives **at-least-once delivery with in-process duplicate suppression**,
which is the strongest guarantee achievable without a distributed
transaction across the CRDT store and the px executor's own effects (some of
which — e.g. tool calls — are not itself transactional). The ADR explicitly
does NOT claim exactly-once semantics for procedures with external side
effects; it documents that procedure authors must make their own bodies
idempotent (e.g. via `Assign`/dedup key) if the timer's target action is not
naturally idempotent (this mirrors the guidance already implied by
`AgensRuntime::process_due_timers`'s existing "processed" count semantics).

### Error handling & recovery

| Failure mode | Detection | Recovery |
|---|---|---|
| Px procedure body panics / returns `Err` | `Executor::run_triggered` returns `Result::Err` | Logged via `tracing::error!` with `timer_id`, `timer_name`, error; token left set; timer retried next tick (interval/cron) or remains pending (once), subject to backoff below |
| Process crash mid-dispatch (token set, `mark_ran` never called) | On dispatcher startup, `PxTimerDispatcher::recover()` scans `TimerTable::list()` for entries with `last_fired_token.is_some()` and `next_fire_at <= now - grace_period` | Clears stale tokens (treats as failed dispatch) and retries on next tick; `grace_period` default 60s, configurable, guards against false-positive recovery of an in-flight dispatch on a slow but alive procedure |
| Repeated failures on same timer | `PxTimerDispatcher` tracks `consecutive_failures` per timer id in memory (not persisted — resets on restart, intentionally, to avoid permanently wedging a timer across deploys) | After `max_consecutive_failures` (default 5), timer is **not** disabled automatically; instead it emits a `tracing::warn!` "timer_repeatedly_failing" event and applies exponential backoff to the *tick* attempt (not `next_fire_at`) — i.e. the dispatcher itself waits progressively longer before retrying a chronically-failing timer, capped at 5 minutes, while other timers continue ticking normally |
| Executor unavailable (e.g. px crate not linked, feature disabled) | `PxTimerDispatcher::new` requires an `Executor` handle at construction; if none is available the caller uses the pre-existing `AgensRuntime::process_due_timers` path directly (no px involved) — this is a deliberate compatibility fallback, not an error path | N/A — feature-gated at compile time via `pluresdb-px`'s existing `native`/no-`native` split |

All error paths must be observable without a debugger: every dispatch
failure, skip, and recovery action is a structured `tracing` event, and
`TickReport` (returned from every `tick()` call) surfaces the same
information programmatically for host-side alerting (this satisfies the
existing repo-wide expectation, see `docs/adr/ADR-0016-hardware-adaptive-
compute.md`'s precedent of returning inspectable reports rather than
swallowing state into logs only).

### PluresDB data model additions

`TimerEntry` (in `agens.rs`) gains two new optional fields, both persisted
under the existing `agens:timer` node type (`_type = "agens:timer"`), so no
migration is required for existing timers (fields default via `Option` /
`bool` default `false` on deserialize):

```rust
pub struct TimerEntry {
    // ...existing fields unchanged...
    /// Fire-in-progress token; Some(token) while a dispatch attempt for the
    /// current `next_fire_at` occurrence is outstanding. Cleared by
    /// `mark_ran`. Used by `PxTimerDispatcher` for crash recovery.
    pub last_fired_token: Option<String>,
    /// When true, a failed dispatch still advances `next_fire_at` (fire-and-
    /// forget semantics). Default false (retry-until-success semantics).
    pub best_effort: bool,
}
```

New `TimerTable` methods (additive, non-breaking):

```rust
impl<'a> TimerTable<'a> {
    /// Persist a fire-in-progress token before invoking the executor.
    pub fn mark_dispatch_started(&self, timer_id: &str, token: &str) -> bool;
    /// Clear a stale token without advancing next_fire_at (used by recovery).
    pub fn clear_stale_token(&self, timer_id: &str) -> bool;
}
```

`PxTimerDispatcher` itself is stateless with respect to the CRDT store aside
from the token field above — all durable state lives in `TimerEntry`, kept
consistent with the existing "timers are just CRDT nodes" model. In-memory
state (`in_flight` set, `consecutive_failures` map) is explicitly ephemeral
and rebuilt from the store's `last_fired_token` on `recover()`.

### Public API surface (native, `pluresdb-px`)

```rust
pub struct PxTimerDispatcher<'a> {
    pub fn new(runtime: &'a AgensRuntime<'a>, executor: Executor) -> Self;
    pub fn tick(&self, now: DateTime<Utc>) -> TickReport;
    pub fn recover(&self, now: DateTime<Utc>, grace_period: Duration) -> RecoveryReport;
    pub fn spawn_native(self: Arc<Self>) -> tokio::task::JoinHandle<()>; // 10s loop, feature = "native"
}
```

### FFI surface (`pluresdb-node`)

New napi methods on `PluresDatabase` (additive):

    // index.d.ts (PluresDatabase methods)
    export interface TickReport {
      fired: number;
      skipped: number;
      errors: Array<{ timerId: string; timerName: string; error: string }>;
    }
    pxTimerTick(): TickReport;          // calls PxTimerDispatcher::tick(now) once
    pxTimerRecover(gracePeriodSecs: number): { recovered: number };

This replaces the previous guidance ("call `agensTimerDue()` and hand-roll
dispatch in JS") with a single call that has the exactly-once/error
guarantees above. Existing `agensTimerDue`/`agensTimerReschedule` remain for
callers that want raw timer introspection without px execution (e.g. a
non-px consumer using `AgensRuntime` handlers directly) — this ADR does not
deprecate them.

## Alternatives Considered

1. **Have `AgensRuntime::process_due_timers` call the px executor directly.**
   Rejected — `pluresdb-procedures` does not (and should not) depend on
   `pluresdb-px`; the dependency must go the other direction (`pluresdb-px`
   depends on `pluresdb-procedures`, not vice versa) to keep the procedures
   crate px-agnostic for non-px consumers (pares-agens Tier 1 embed, per
   `development-guide/design/PLURESDB-NATIVE-PROCEDURES.md`).
2. **Push idempotency entirely into px procedure authoring convention (no
   token field).** Rejected — leaves crash recovery undefined and silently
   permits duplicate fires on process restart, which is worse than the
   documented at-least-once + dedup-token behavior above.
3. **True exactly-once via 2PC between store and executor.** Rejected as
   out of scope / disproportionate — no other part of the system offers
   transactional guarantees across store + effects, and px procedure bodies
   can themselves perform non-transactional side effects (tool calls),
   which no store-level protocol can make exactly-once anyway.

## Local Test Plan (build-the-binary)

This ADR requires a working local build to validate before merge, not just
unit tests in isolation:

1. **Build.**
   ```
   cargo build -p pluresdb-px --features native
   cargo build -p pluresdb-node --release
   ```
2. **Unit tests (pluresdb-px, new module).**
   ```
   cargo test -p pluresdb-px timer_dispatcher::
   ```
   Cover: fires due timer once; skips in-flight duplicate tick; failed
   dispatch does not advance `next_fire_at` (non-`best_effort`); failed
   dispatch does advance `next_fire_at` (`best_effort = true`); `recover()`
   clears a stale token past `grace_period` and allows re-fire; cron/once/
   interval trigger variants all reach the executor with `event.type ==
   "timer"` matching the shape already asserted in
   `crates/pluresdb-px/src/px/executor.rs` (~line 4063).
3. **Integration test (crate-level, `pluresdb-px/tests/`).**
   Spin up an in-memory `CrdtStore` + `AgensRuntime` + real `Executor`
   loaded with a `.px` fixture procedure (`trigger: timer`), schedule an
   interval timer, call `PxTimerDispatcher::tick()` twice at simulated
   times, assert the procedure ran exactly once per due tick and store
   state mutated by the procedure body is visible afterward.
4. **Native FFI smoke test (mirrors existing `pluresdb-node/__tests__`
   pattern, e.g. `subscribe.gate.mjs`).**
   ```
   node --experimental-vm-modules crates/pluresdb-node/__tests__/timer-dispatch.smoke.mjs
   ```
   Script: build the native module (`npm run build` in
   `crates/pluresdb-node`), schedule a timer via `agensTimerSchedule`,
   register a `.px` procedure with `trigger: timer`, call `pxTimerTick()`,
   assert `TickReport.fired === 1` and the expected store mutation occurred;
   call `pxTimerTick()` again before `next_fire_at` and assert `fired === 0`.
   Then simulate a crash: manually set `last_fired_token` via a raw store
   write, call `pxTimerRecover(0)`, assert token cleared and next `tick()`
   fires again.
5. **Manual verification command** (for reviewers without running the full
   suite):
   ```
   cargo run -p pluresdb-px --example run_px -- --with-timer-dispatch
   ```
   (new CLI flag on the existing `examples/run_px.rs`, prints `TickReport`
   JSON to stdout on each simulated tick so a reviewer can eyeball fire
   counts without instrumenting a test).

All of the above must pass locally (`cargo test --workspace` green, node
smoke script exits 0) before this ADR's companion implementation PR is
opened; this ADR PR itself contains no implementation, only the design.

## Consequences

- `pluresdb-px` gains a hard dependency on `pluresdb-procedures`'s
  `AgensRuntime`/`TimerTable` (already implied by px's timer-event test
  fixtures; this ADR makes it an explicit, real dependency edge).
- `TimerEntry`'s two new fields are additive/optional and do not break
  existing serialized timer nodes; `entry_from_data` must default them
  (`None` / `false`) when absent — this is a one-line change to the existing
  parse function (`agens.rs` ~line 639), not a migration.
- pares-agens Tier 1 (direct embed, no px) is unaffected: `AgensRuntime::
  process_due_timers` keeps working standalone exactly as documented in
  `PLURESDB-NATIVE-PROCEDURES.md`; `PxTimerDispatcher` is purely additive
  and only relevant to hosts that also load `pluresdb-px`.
- Node/FFI consumers get a real tick-driving primitive (`pxTimerTick`)
  instead of having to hand-roll dispatch loops against `agensTimerDue`.

## Open Questions (for implementation PR, not blocking this ADR)

- Should `consecutive_failures` backoff be persisted so it survives a
  restart within the backoff window, or is in-memory (reset on restart)
  acceptable long-term? This ADR chooses in-memory for simplicity; revisit
  if chronic-failure timers become an operational problem in practice.
- Multi-process deployments (more than one host ticking the same store)
  are out of scope for this ADR — the in-flight guard is per-process only.
  A follow-up ADR should address distributed leader election for timer
  dispatch if/when PluresDB is run with multiple writers against the same
  logical timer set.
