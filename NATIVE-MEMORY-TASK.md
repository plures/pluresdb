# TASK: Native-First Memory — Reactive Subscriptions + Headroom Delivery

**Worktree:** `C:\Projects\_worktrees\pluresdb-native-memory` (branch `feat/native-memory-reactive` off `main` 0ec9523)\n**Directive (Paradox, 2026-07-02):** "improve the native bindings to deliver the features we need, not to be limited by them." + "Effort 1 & 2. We aren't looking for quick... we want complete."
**Mandate:** complete + gated dev-lifecycle, not a shortcut. NO STUBS (C-NOSTUB-001). `.px`-first where logic (C-DEV-001/C-PLURES-004). Build the binary + run the binary + verify on real store (C-TEST-002).

---

## Ground truth (verified from source this session)

**NAPI crate = `crates/pluresdb-node`** (napi 3.9, features `["napi6","serde-json"]`, NO tokio_rt yet).

### Headroom (Effort 1) — BUILT, not published
- `src/headroom.rs` real algorithm (no stubs, no agens dep). Committed `350dba6`; 31-test suite `bc43e50`; e2e proof `58b42b2`.
- `lib.rs` L1369-1439 `#[napi]` free fns wrap it: `compressText(content, contentType?)`, `countTokens(text)` (real tiktoken_rs cl100k), `detectContentType(content)`.
- Built `.node` (2026-07-01 11:11) already exposes them. Source tagged to v3.2.0.
- **Gap:** plugin `plureslm-openclaw` consumes published `@plures/pluresdb-native@2.0.0-alpha.1` (predates headroom). => DELIVERY: build/publish current native pkg -> bump plugin -> register `MemoryFlushPlan` resolver -> verify compression live.

### Reactive PUSH (Effort 2) — the real native stub
- `lib.rs` L719-726 `subscribe()`: grabs `self.broadcaster.subscribe()` then DROPS it (`_receiver`); returns "subscription-1". Comment: "Full async subscription support requires additional async infrastructure." NO delivery to JS.
- **Notification SOURCE already wired:** every write path (`put` L325+, `put_with_embedding` L700+, `persist_constraint`) calls `broadcaster.publish(SyncEvent::NodeUpsert{id})`.
- `SyncBroadcaster::subscribe() -> broadcast::Receiver<SyncEvent>` (tokio broadcast). `SyncEvent = NodeUpsert{id} | NodeDelete{id}` (`pluresdb-sync/src/lib.rs` L64-71).
- **Work:** real `subscribe(callback)` — drain the broadcast::Receiver into a `ThreadsafeFunction`, delivering `{kind,id}` events to JS. Decide async infra: add `napi/tokio_rt` feature + tokio, OR std::thread + blocking recv. Add `unsubscribe`/handle for lifecycle. NO leaked task, NO dropped events silently.

### Reactive `.px`-on-write (Effort 3, follows Effort 2)
- Wire `pxLoadPxSource`/`execIr` procedure execution to fire on the subscription path so a write reactively runs `.px` (C-PLURES-004: a write causes reactive procedure execution). `.px`-first: cadence/policy as `.px`.

---

## Stages (gated — each gate PASSES before next starts)

- **S0 baseline** — worktree builds green (`cargo build -p pluresdb-node --release` + node addon build) BEFORE changes. Warms cache. GATE: clean build + existing 31 headroom tests pass.
- **S1 Effort-1 delivery** — build current native pkg from worktree; wire plugin to consume it (local path or bumped version); register `MemoryFlushPlan` resolver in `plureslm-openclaw` invoking `compressText`. GATE: high-token memory context -> resolver fires -> compression measured (reserve floor honored), on REAL store. NO publish to npm without Paradox's explicit ok (org-wide side-effect) — stage the release, ask before pushing.
- **S2 Effort-2 subscribe** — implement real async `subscribe(callback)` + `unsubscribe` in pluresdb-node. GATE: Node test — `put` triggers the JS callback with `{kind:'NodeUpsert',id}` WITHOUT polling; unsubscribe stops delivery; no leaked thread/task. Build binary, run binary.
- **S3 Effort-3 reactive .px** — `.px` procedure fires on write via S2 path. GATE: a governing `.px` runs reactively on a real write, effect observable in store.
- **S4 integrate + verify live** — bump plugin to the reactive native build; restart gateway; verify headroom compression + (if surfaced) reactive behavior live in OpenClaw memory. Close the loop.

## Hard gates consulted
WORK-PRIORITIZATION (Level 0 Foundation — reactive procedures + working NAPI = this), PLURES-FOUNDATION (pluresdb-node is the NAPI seam; reactive procedures are by-design), session-workspace-isolation (this worktree). Redact push `babc70f` on plugin stays HELD. Native npm publish stays HELD pending explicit ok.

## Milestones log: `.native-memory-milestones.md`
