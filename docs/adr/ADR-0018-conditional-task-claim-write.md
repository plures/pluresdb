# ADR-0018: Conditional Task Claim-Write (Revision/Digest-Guarded Atomic Mutation)

- **Status:** Proposed (design-only; no implementation in this pass)
- **Epic:** `pluresdb:conditional-task-claim-write`
- **Stage:** DESIGN
- **Owners:** pluresdb-core, pluresdb-storage, pluresdb-procedures
- **Related docs:** `docs/CONFLICT_RESOLUTION.md`, `docs/DURABILITY.md`, `docs/ARCHITECTURE.md`

## 1. Problem

Callers that implement work-queue / task-claim patterns on top of PluresDB
(e.g. "claim the next open task, mark it `in_progress`, assign to me") need
an atomic **read-check-write** primitive: *"update this node only if it is
still in the state I last observed."* Today, `CrdtStore` exposes only
unconditional last-write-wins mutation:

- `CrdtStore::put(id, actor, data)` — always applies, merges via per-field
  HAM (Hypothetical Amnesia Machine) timestamp comparison
  (`NodeRecord::merge_update`).
- `apply_mutate` (in `pluresdb-procedures::ops::mutate`) — batch put / merge /
  delete with an `atomic` flag that only does an **existence** pre-check
  (`store.get(id).is_none()`), not a value/version pre-check. Two concurrent
  callers can both pass the "exists" check and both `put()`, and HAM's
  per-field LWW will silently let the later-timestamped write win — there is
  no signal to either caller that they raced, and no way to abort or retry.

This is a **lost update** problem: with concurrent claimants, the design
intent ("only one claimant should win") is not enforced. HAM guarantees
convergence, not mutual exclusion or claim semantics — those are different,
compatible-but-additional properties this ADR must supply.

### Requirements

1. Provide a `CrdtStore`-level API to perform a mutation **conditioned on**
   the current revision/digest of the target node, atomically w.r.t. other
   callers on the same store (single process) and safely w.r.t. concurrent
   CRDT sync merges from other peers.
2. On precondition mismatch, return a typed conflict error carrying the
   *current* record (revision + data) so the caller can decide to retry,
   re-render, or abandon — no partial writes.
3. Preserve existing LWW/CRDT convergence semantics for normal
   (non-conditional) `put`/`merge` — this is additive, not a replacement.
4. Work uniformly across storage backends behind `StorageEngine` /
   `SyncStorageEngine` (in-memory `DashMap`, SQLite-backed `rad.rs`,
   WAL-backed persistence) without requiring backend-specific
   compare-and-swap support (single-process in-memory serialization is
   sufficient; durability is via existing WAL/persist path).
5. Support both single-node conditional writes and small atomic multi-op
   batches (claim = "check A, then write A and B together") reusing the
   `apply_mutate` batch shape where practical.
4. No implementation in this pass — API shape, conflict-resolution
   strategy, and test plan only.

## 2. Current State Analysis

### 2.1 `CrdtStore` (crates/pluresdb-core/src/lib.rs)

- Backing store: `DashMap<NodeId, NodeRecord>` (in-memory, per-key locking
  via DashMap shard locks — no cross-key or read-then-write atomicity
  guarantee across the "check" and "write" steps for a caller today).
- `NodeRecord { id, data, clock: VectorClock, timestamp: DateTime<Utc>, embedding, quality_score }`.
  - `clock` is a `VectorClock` (per-actor counters) — already tracks causal
    history per node, but is **not currently exposed** as a
    caller-comparable "revision" for CAS purposes (no `revision()` /
    `digest()` accessor, no way to pass "the clock/digest I last saw" back
    in).
  - `timestamp` is the HAM wall-clock timestamp, used for per-field merge
    tie-breaking (`merge_update`), not a monotonic per-write revision.
- `put()` unconditionally does
  `entry(id).and_modify(|r| r.merge_update(...)).or_insert_with(...)`,
  followed by best-effort persistence (`persist_node`) — no compare step.
- `apply(op: CrdtOperation)` — dispatches `Put`/`Delete`, same
  unconditional semantics.
- No optimistic-concurrency primitive exists in `pluresdb-core` today.

### 2.2 Storage backends (crates/pluresdb-storage)

- `SyncStorageEngine` / `StorageEngine` traits: `put`, `get`, `delete`,
  `list`, `count`, `for_each`. `StoredNode { id: String, payload: Value }` —
  a plain overwrite-oriented KV contract; no revision/ETag/version column,
  no compare-and-swap verb.
- Backends: `rad.rs` (SQLite/"rad" persistence), `wal.rs` (write-ahead log),
  `bridge/{snapshot,restorer,wal_flusher}.rs` (snapshot/replay), `blob.rs`,
  `encryption.rs`. All are "last writer wins at the storage layer" — they
  persist whatever `CrdtStore` hands them, after `CrdtStore`'s in-memory
  merge has already resolved the value. Consequently, the **conditional
  check must happen at the `CrdtStore` layer** (which owns the canonical
  in-memory state and merge logic), not by pushing CAS semantics down into
  each storage backend. Storage backends remain simple sinks.

### 2.3 `apply_mutate` (crates/pluresdb-procedures/src/ops/mutate.rs)

- Batch-oriented `Put | Delete | Merge` with an `atomic: bool` flag.
- "Atomic" today == existence pre-check only (`store.get(id).is_none()`
  short-circuits before any writes are applied) — a good pattern to extend,
  but it has no notion of "value must match expected revision."
- This is the natural integration point for a **conditional batch** variant
  (`apply_mutate_conditional`) that adds per-op expected-revision guards
  alongside the existing existence guards, reusing its pre-flight-then-apply
  structure.

### 2.4 CRDT merge / conflict resolution (docs/CONFLICT_RESOLUTION.md)

- HAM per-field LWW is commutative/associative/idempotent and soul-scoped.
  It guarantees **eventual convergence** across peers, not **claim
  exclusivity** at write time. A conditional-write API is a client-side
  concurrency-control layer *on top of* HAM: it changes when a `put` is
  attempted, not how merges resolve once two `put`s have both landed. This
  distinction drives the design in §3: the guard must be checked and the
  write applied as one atomic critical section relative to other local
  `put`s, but the resulting `NodeRecord` still flows through the same
  `merge_update`/HAM machinery for sync/replication.

## 3. Proposed Design

### 3.1 Revision/digest model

Introduce a caller-visible, cheap-to-compare **revision token** derived from
existing `NodeRecord` state — no new wire format, no schema migration:

```rust
/// Opaque, comparable snapshot marker for a node's current state.
/// Two revisions are equal iff the record's (clock, timestamp, data-digest)
/// triple is bit-identical. Cheap to compute (no crypto hash of full data
/// required for the common case — see NodeRevision::digest below).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRevision {
    /// Vector clock at time of read (causal position).
    pub clock: VectorClock,
    /// HAM timestamp at time of read (tie-break / freshness).
    pub timestamp: DateTime<Utc>,
    /// FNV-1a/xxhash digest of the canonical JSON serialization of `data`.
    /// Guards against same (clock, timestamp) but different data in edge
    /// cases (e.g. clock/timestamp collision under clock skew) — cheap,
    /// non-cryptographic, collision-resistant enough for a local
    /// concurrency guard (not a security boundary).
    pub digest: u64,
}

impl NodeRevision {
    pub fn of(record: &NodeRecord) -> Self { /* compute from record */ }
}
```

`NodeRecord` gains:
```rust
impl NodeRecord {
    pub fn revision(&self) -> NodeRevision { NodeRevision::of(self) }
}
```

Rationale for (clock, timestamp, digest) rather than a single monotonic
counter: `CrdtStore` is multi-writer/CRDT by design, so there is no single
global sequence number to hand out cheaply without introducing a
coordinator. The tuple is: (a) already materialized on every record, (b)
strictly more discriminating than timestamp alone (guards the rare
same-millisecond race), and (c) trivially comparable with `==` — no need for
callers to reason about ordering, only "has this specific node moved since I
looked at it."

### 3.2 `CrdtStore` API additions

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConditionalWriteError {
    /// The node existed but its revision did not match `expected`.
    #[error("revision mismatch for '{id}'")]
    RevisionMismatch {
        id: NodeId,
        expected: NodeRevision,
        current: NodeRecord,
    },
    /// Caller expected the node to exist (passed `Some(revision)`) but it
    /// does not.
    #[error("node '{id}' not found, expected revision {expected:?}")]
    NotFound { id: NodeId, expected: NodeRevision },
    /// Caller expected the node to be absent (passed `None`, i.e. "create
    /// only if missing") but it already exists.
    #[error("node '{id}' already exists")]
    AlreadyExists { id: NodeId, current: NodeRecord },
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl CrdtStore {
    /// Atomically (w.r.t. other local `put`/`put_if`/`delete_if` calls on
    /// this store) apply `data` to `id` only if the node's current revision
    /// equals `expected`. Pass `expected = None` to require the node be
    /// absent (create-only / "claim if unclaimed").
    ///
    /// On success returns the new `NodeRevision` (post-write) so the caller
    /// can chain subsequent conditional writes without a re-read.
    /// On mismatch returns `ConditionalWriteError` carrying the current
    /// record so the caller can inspect/retry without a second round trip.
    pub fn put_if(
        &self,
        id: impl Into<NodeId>,
        actor: impl Into<ActorId>,
        data: NodeData,
        expected: Option<NodeRevision>,
    ) -> Result<NodeRevision, ConditionalWriteError>;

    /// Same guard semantics as `put_if`, but applies a JSON-merge-patch
    /// style partial update to the existing data instead of a full replace.
    /// Convenience wrapper for the common "claim = patch {status, owner}"
    /// case; internally: read -> guard -> merge patch -> put_if.
    pub fn merge_if(
        &self,
        id: impl Into<NodeId>,
        actor: impl Into<ActorId>,
        patch: NodeData,
        expected: NodeRevision,
    ) -> Result<NodeRevision, ConditionalWriteError>;

    /// Guarded delete: succeeds only if current revision equals `expected`.
    pub fn delete_if(
        &self,
        id: impl AsRef<str>,
        expected: NodeRevision,
    ) -> Result<(), ConditionalWriteError>;

    /// Convenience for the canonical claim pattern: read current record
    /// (if any) and revision in one call, so callers don't need a separate
    /// `get()` + manual `NodeRecord::revision()` before their first
    /// `put_if`.
    pub fn get_with_revision(
        &self,
        id: impl AsRef<str>,
    ) -> Option<(NodeRecord, NodeRevision)>;
}
```

**Atomicity mechanism (single process):** `DashMap::entry(id)` already
returns a per-shard-locked `Entry` API. `put_if` is implemented as a single
`entry()` call whose closure performs *both* the revision comparison and
the mutation under the same shard lock — i.e. reuse
`DashMap::Entry::and_modify`/match-on-`Entry` (`Occupied`/`Vacant`) instead
of the current two-step `get()`-based check in `apply_mutate`. This removes
the TOCTOU window that exists in today's `apply_mutate` atomic pre-check
(which does `store.get(id)` then, separately, `store.put(...)`, allowing another
thread's `put` to land in between). No new lock types, no external
coordinator — this is a correctness fix using primitives already in the
dependency graph (`dashmap` is already a `pluresdb-core` dependency).

**Interaction with CRDT sync (multi-process/multi-peer):** `put_if`'s
guard is a **local** optimistic-concurrency check against this store's
current view. It does not (and cannot, without a consensus protocol) 
prevent a *remote* peer from concurrently claiming the same logical task in
its own local store during a network partition — that is a split-brain
scenario, explicitly out of scope for mutual exclusion (per
CONFLICT_RESOLUTION.md, PluresDB is local-first/AP, not CP). What `put_if`
guarantees:
- **Within one store/process**, concurrent local callers cannot both
  "win" a claim on the same expected revision — exactly one succeeds, and
  losers get a fast, informative `RevisionMismatch`/`NotFound` instead of a
  silent overwrite.
- **Across sync**, after a claim write lands locally, its `clock` advances
  (new vector-clock entry for `actor`), so a stale `expected` revision held
  by a peer's in-flight conditional write will correctly fail to match once
  that peer's local view merges the new state in — the guard degrades
  gracefully into "your write will be rejected locally on next attempt"
  rather than silently corrupting data. Document this clearly as an AP
  design choice: **conditional-write is a local mutual-exclusion +
  correctness aid, not a distributed lock**. Split-brain reconciliation
  after partition-heal still follows existing per-field HAM LWW on the
  `data` payload; applications building claim/task queues that must survive
  partitions should additionally embed the claim `owner`/`status` fields
  understanding they follow LWW convergence rules if two peers race across
  a partition (documented in §3.5 as an explicit limitation + mitigation).

### 3.3 Batch conditional mutation (procedures layer)

Extend `pluresdb-procedures::ops::mutate` with a conditional-aware sibling
of `apply_mutate`, reusing its pre-flight/apply structure:

```rust
pub enum ConditionalMutateOp {
    /// Apply MutateOp::Put only if the node's current revision matches
    /// `expected` (None => must not exist).
    PutIf { id: NodeId, data: NodeData, expected: Option<NodeRevision> },
    MergeIf { id: NodeId, patch: NodeData, expected: NodeRevision },
    DeleteIf { id: NodeId, expected: NodeRevision },
}

/// All-or-nothing: computes every guard first (single pass over `store`),
/// and only if *all* guards currently pass does it apply all writes. This
/// generalizes the existing `atomic` existence-check pattern in
/// `apply_mutate` to revision-guarded batches — e.g. "claim task A only if
/// still open AND write audit-log entry B" as one unit.
///
/// NOTE: guard-check-then-apply is still two passes over `store`, so it is
/// atomic in the sense of "all guards pass or nothing is written," but a
/// concurrent local `put_if`/`put` on one of the same ids between the two
/// passes will be caught by re-checking each guard immediately before its
/// own write inside the same DashMap `entry()` critical section (i.e. the
/// second pass reuses the same per-key `entry()`-based CAS as `put_if`, so
/// TOCTOU is bounded to "did anything change between pass 1 and pass 2,"
/// which is still detected and reported, just not pre-empted at pass-1
/// time). Document this precisely in the API doc comment.
pub fn apply_mutate_conditional(
    store: &CrdtStore,
    actor: &str,
    ops: &[ConditionalMutateOp],
) -> Result<usize, ConditionalBatchError>;

pub struct ConditionalBatchError {
    /// Which op(s) failed their guard, with current-state detail per op.
    pub failures: Vec<(usize, ConditionalWriteError)>,
}
```

This is the primary primitive for the motivating **task claim** use case:

```rust
// Claim pattern using the new primitives:
let (record, rev) = store.get_with_revision("task:42")
    .ok_or(ClaimError::NotFound)?;
if record.data["status"] != "open" {
    return Err(ClaimError::AlreadyClaimed);
}
match store.merge_if(
    "task:42",
    my_actor_id,
    json!({"status": "in_progress", "owner": my_actor_id}),
    rev,
) {
    Ok(_new_rev) => Ok(Claimed),
    Err(ConditionalWriteError::RevisionMismatch { current, .. }) => {
        // Someone else claimed it first (locally) — inspect `current` and
        // decide whether to retry against a different task or give up.
        Err(ClaimError::Contended { current })
    }
    Err(e) => Err(e.into()),
}
```

### 3.4 Public surface: TS/npm & CLI bindings (naming only — no impl this pass)

To keep this consistent with existing documented API (`docs/API.md`,
`development-guide/tools/pluresdb.md`), the eventual TS-facing shape should
mirror ETag-style conditional requests, since that idiom is already
familiar to consumers of `db.update`:

```typescript
// Future (not implemented in this pass):
const claimed = await db.updateIf('tasks', id, expectedRevision, {
  status: 'in_progress',
  owner: myId,
});
// throws PluresDBError { code: 'REVISION_MISMATCH', current }
```

This ADR only proposes the Rust-level `CrdtStore`/`pluresdb-procedures`
primitives; wiring to `pluresdb-node`/`pluresdb-deno`/CLI bindings is a
follow-up implementation task, out of scope for this design pass.

### 3.5 Explicit limitations & mitigations

| Limitation | Mitigation |
|---|---|
| `put_if`/`merge_if` guard is local-store-only; does not prevent two *different peers* from both locally "winning" a claim during a network partition | Document as AP tradeoff (consistent with existing HAM policy). Recommend applications add a `claimant_history: Vec<{actor, ts}>` append-only field (per existing "conflict-friendly schema" guidance in CONFLICT_RESOLUTION.md) so post-partition-heal reconciliation can detect and resolve double-claims at the application layer (e.g. deterministic tie-break: lowest actor id wins, requeue the loser's work). |
| `NodeRevision::digest` uses a non-cryptographic hash | Acceptable: this is a local concurrency guard, not a security/integrity boundary. Document explicitly; do not use for authentication or tamper detection. |
| Batch `apply_mutate_conditional` guard-check pass (pass 1) is separate from apply pass (pass 2) | Second pass re-validates each guard inside the same `entry()` CAS as `put_if`, so no write can be silently accepted after violating its guard, even though pass 1 is only a fast-fail advisory check. Document the two-pass semantics precisely. |
| No distributed lock / lease / TTL semantics (e.g. "claim expires after 60s if not completed") | Explicitly out of scope for this ADR; can be layered on top by applications storing an `expires_at` field and treating stale claims as `status == "open"` for claim purposes — a follow-up epic if needed. |

## 4. Alternatives Considered

1. **Global monotonic sequence number per node (single-writer counter).**
   Rejected: requires a coordinator or centralized counter allocation,
   contrary to PluresDB's local-first/multi-writer CRDT model; would force
   a consistency bottleneck exactly where the architecture avoids one.
2. **Cryptographic hash (SHA-256) digest instead of fast non-crypto hash.**
   Rejected for the default: revision comparison is a local concurrency
   guard, not a security boundary; SHA-256 adds unnecessary CPU cost on
   every write/read for a comparison need that a 64-bit hash already
   satisfies. Leave room to swap the hash function later behind
   `NodeRevision::digest`'s implementation without an API break.
3. **Push CAS down into `StorageEngine`/`SyncStorageEngine` trait (add
   `compare_and_swap` to the storage trait).** Rejected: storage backends
   don't own the canonical in-memory merge state (`CrdtStore` does); adding
   CAS at the storage layer would require every backend (SQLite/rad, WAL,
   future backends) to reimplement version tracking redundantly with
   `NodeRecord.clock`/`timestamp`, and would not solve the multi-writer
   in-memory race in `CrdtStore.nodes: DashMap` itself, which is where the
   actual local race occurs today (`apply_mutate`'s `get()`-then-`put()`
   TOCTOU).
4. **Lock-based mutex per node id (explicit `lock()`/`unlock()` API).**
   Rejected: pessimistic locking adds liveness risk (forgotten unlock,
   deadlock across batch ops) and a stateful session concept foreign to the
   rest of the `CrdtStore` API, which is currently stateless-call-based.
   Optimistic CAS matches the existing `put`/`get`/`delete` call shape and
   composes with the batch `apply_mutate` pattern already in place.

## 5. Test Plan

All tests target `crates/pluresdb-core` (unit) and
`crates/pluresdb-procedures` (integration), following existing test file
conventions (`tests/integration.rs`, `tests/agent_simulation_tests.rs`
patterns already present in the workspace).

### 5.1 Unit tests — `CrdtStore::put_if` / `merge_if` / `delete_if`

1. `put_if` succeeds when `expected == Some(current_revision)` and the
   node's data/clock/timestamp are updated as with plain `put`.
2. `put_if` succeeds with `expected = None` only when the node does not yet
   exist (create-only / "claim if unclaimed"); fails with `AlreadyExists`
   (carrying current record) if it does.
3. `put_if` fails with `RevisionMismatch { expected, current }` when the
   node exists but its live revision differs from `expected` — assert the
   returned `current` record reflects the actual in-store state (no write
   applied, `nodes` map unchanged byte-for-byte).
4. `put_if` fails with `NotFound` when `expected = Some(rev)` but the node
   does not exist.
5. `merge_if` applies a partial JSON patch only on matching revision;
   verify unspecified fields are preserved (same merge semantics as
   existing `apply_mutate`'s `Merge` op) and the vector clock advances by
   exactly one entry for `actor`.
6. `delete_if` succeeds only on matching revision; fails with
   `RevisionMismatch` otherwise, and node remains present.
7. `NodeRevision::of` produces equal revisions for two reads of an
   unmodified record, and unequal revisions after any `put`/`merge_update`
   call that changes `data`, `clock`, or `timestamp`.
8. `NodeRevision` digest collision resistance smoke test: two records with
   different `data` but crafted identical `(clock, timestamp)` must still
   produce different `digest` values (guards the edge case motivating
   including digest in the tuple).

### 5.2 Concurrency tests — TOCTOU elimination

9. **Race test:** spawn N threads all calling `put_if(id, actor_i, data_i,
   Some(shared_initial_revision))` concurrently against the same freshly
   created node. Assert **exactly one** thread succeeds and the other
   N-1 receive `RevisionMismatch`; assert the final stored value equals
   the winner's `data` (no interleaved/partial write, no lost update).
   Repeat with high thread count (e.g. 64) and loom/miri or stress-loop
   (1000+ iterations) to catch rare interleavings given `DashMap`'s
   sharded locking.
10. **Claim-loop simulation:** model the motivating use case directly —
    a pool of N "worker" threads each attempt `get_with_revision` +
    `merge_if(status: open -> in_progress)` in a retry loop against a
    shared pool of M open tasks (N > M). Assert: every task is claimed by
    exactly one worker, no task is claimed by two workers, no task is left
    unclaimed if any worker still retrying, and total successful claims
    equals `min(N, M)`.
11. **Batch atomicity test (`apply_mutate_conditional`):** two threads each
    submit a two-op conditional batch touching an overlapping node id with
    correct-at-submit-time expected revisions; assert exactly one batch
    fully applies (both ops) and the other fully fails (zero ops applied),
    matching the "all guards pass or nothing is written" contract — verify
    via a spy/counter that no op from the losing batch's second op was
    applied even though its first guard might have independently passed at
    check time (tests the pass-1/pass-2 TOCTOU-bounding behavior in §3.5).

### 5.3 CRDT/sync interaction tests

12. **Post-merge revision invalidation:** peer A's local `put_if` succeeds
    with revision R1. Simulate a remote merge from peer B
    (`NodeRecord::merge_update`) that advances the record. Assert a
    subsequent `put_if(..., expected = Some(R1))` on peer A now fails with
    `RevisionMismatch` reflecting the merged state — proves the guard
    correctly observes sync-driven changes, not just local ones.
13. **Split-brain claim scenario (documented limitation, not a bug):** two
    isolated peers both successfully `merge_if`-claim the same logical task
    id during a partition (each against its own local unclaimed state).
    After reconnect/merge, assert the per-field HAM LWW outcome is exactly
    as predicted by `docs/CONFLICT_RESOLUTION.md` (higher-timestamp claim
    wins on the `status`/`owner` fields) — this test documents/pins the
    expected (not "fixed") behavior described in §3.5, and should live
    alongside the existing `test_split_brain_*` tests in
    `crates/pluresdb-sync/tests/p2p_harness_test.rs`.

### 5.4 Persistence/durability tests

14. `put_if`/`merge_if`/`delete_if` successes are durably persisted through
    the existing `persist_node`/WAL path exactly like unconditional `put`
    (extend `crates/pluresdb-storage/tests/durability_tests.rs` style
    coverage) — i.e. conditional guard logic sits above persistence and
    does not change durability guarantees.
15. Failed conditional writes (`RevisionMismatch`/`NotFound`/`AlreadyExists`)
    must **not** write anything to the WAL or backing storage — assert
    storage engine call counts (`put`/`delete`) are zero on guard failure
    (mockable `StorageEngine`/`SyncStorageEngine` test double).

### 5.5 Procedures/engine integration tests

16. `apply_mutate_conditional` end-to-end via the procedures engine
    (`pluresdb-procedures::engine`), following existing patterns in
    `crates/pluresdb-core/tests/integration.rs`, covering: successful
    all-guards-pass batch; single-guard-failure batch (assert zero
    ops applied, `ConditionalBatchError.failures` lists the correct
    index/reason); mixed `PutIf`/`MergeIf`/`DeleteIf` batch.

### 5.6 Benchmarks (non-blocking, informational)

17. Extend `crates/pluresdb-core/benches/crdt_benchmarks.rs` with a
    `put_if` benchmark comparing throughput/latency against plain `put`,
    to quantify the overhead of revision computation + comparison (target:
    single-digit percent overhead, since it reuses the existing `entry()`
    critical section rather than adding a second lock acquisition).

## 6. Rollout / Follow-up (out of scope for this ADR)

- Implementation PR(s) for `NodeRevision`, `CrdtStore::{put_if, merge_if,
  delete_if, get_with_revision}`.
- Implementation PR for `pluresdb-procedures::ops::mutate::{ConditionalMutateOp,
  apply_mutate_conditional}`.
- Bindings PR(s) for `pluresdb-node`/`pluresdb-deno`/CLI exposing
  `db.updateIf(...)` with `REVISION_MISMATCH` error code, per §3.4.
- Documentation PR updating `docs/API.md`, `docs/CONFLICT_RESOLUTION.md`
  (cross-reference §3.5 limitations), and
  `development-guide/tools/pluresdb.md` with the new conditional-write API
  reference once implemented.
- Optional future epic: claim lease/TTL semantics (see §3.5 limitations
  table) if application feedback shows it's needed beyond app-layer
  `expires_at` fields.

## 7. Decision

Adopt the `NodeRevision` + `put_if`/`merge_if`/`delete_if` design in §3 as
the basis for implementation. The design:

- Solves the stated lost-update problem for local concurrent
  claim/write by closing the TOCTOU window in the current
  `apply_mutate` atomic-existence-check pattern, using the same
  `DashMap::entry()` primitive already available in `pluresdb-core`.
- Is additive: existing `put`/`merge`/`apply_mutate` behavior and the
  HAM/CRDT convergence guarantees documented in `CONFLICT_RESOLUTION.md`
  are unchanged.
- Has documented, tested limitations for the cross-peer/split-brain case
  rather than over-promising distributed mutual exclusion the AP
  architecture cannot provide.

Implementation is a separate, follow-up pass per the epic's stage gating.