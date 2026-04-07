# PluresDB Conflict Resolution and Split-Brain Policy

## Overview

PluresDB is a local-first, offline-first database that uses **CRDT (Conflict-free Replicated Data Type)** semantics for automatic conflict resolution.  Because any peer can write to any key at any time — including during network partitions — the system must deterministically converge to the same state without central coordination.

This document defines:

1. The conflict resolution algorithm and its guarantees.
2. The split-brain scenario and how PluresDB handles it.
3. Expected outcomes for common conflict patterns.
4. How to reason about field-level convergence in your application.

---

## Conflict Resolution Algorithm

### Hypothetical Amnesia Machine (HAM)

PluresDB uses the **HAM (Hypothetical Amnesia Machine)** algorithm, compatible with GUN.js wire protocol, to resolve conflicts at a **per-field** granularity.

Each field of every graph node carries an independent HAM state timestamp (milliseconds since Unix epoch, as a 64-bit float).  When two peers hold different values for the same field, the merge rule is:

| Condition | Winner |
|-----------|--------|
| `other.timestamp > self.timestamp` | `other` (newer write wins) |
| `other.timestamp == self.timestamp` | Lexicographically greater JSON serialization wins (deterministic tie-break) |
| `other.timestamp < self.timestamp` | `self` (local value is newer; keep it) |

This is a **Last-Write-Wins (LWW)** policy applied independently per field, making the outcome deterministic, commutative, and associative — i.e., independent of merge order.

### Merge Semantics

```
merge(A, B) == merge(B, A)                 // commutative
merge(merge(A, B), C) == merge(A, merge(B, C))  // associative
merge(A, A) == A                           // idempotent
```

All three properties hold for all field values and all timestamp combinations, including ties.

### Soul Isolation

The merge algorithm is **soul-scoped**: a `GunNode` representing key `user:alice` is never merged with a node representing key `user:bob`.  If the souls differ, the merge is a no-op.  This prevents silent cross-node data corruption during bulk gossip.

---

## Split-Brain Scenarios

A **split-brain** occurs when the peer network is partitioned into two or more isolated groups that each continue accepting writes to the same keys.  When the partition heals and peers reconnect, PluresDB must converge their diverged states without data loss.

### Two-Peer Partition

```
Partition phase (no network):
  Peer A  ──[writes field="X" @ ts=100]──  (isolated)
  Peer B  ──[writes field="Y" @ ts=200]──  (isolated)

Reconnect:
  A <──sync──> B

Result (LWW per field):
  Both peers: field="Y" @ ts=200  (B's higher timestamp wins)
```

### Split-Brain with Multiple Fields

Each field converges independently:

```
Peer A writes: { "status": "online",  "score": 42  }  @ ts=100
Peer B writes: { "status": "offline", "score": 100 }  @ ts=200

After merge:
  "status" → "offline"  (B's ts=200 > A's ts=100)
  "score"  → 100        (B's ts=200 > A's ts=100)
```

If peers write **different** fields at the same or different timestamps:

```
Peer A writes: { "color": "red"  }  @ ts=100
Peer B writes: { "size": "large" }  @ ts=200

After merge:
  "color" → "red"    (only A wrote this field; B's merge adds it unchanged)
  "size"  → "large"  (only B wrote this field; A's merge adds it unchanged)
```

Fields written exclusively by one peer are preserved regardless of relative timestamps, because the other peer simply has no conflicting value for that field.

### Three-Peer Mesh Partition

```
Partition phase:
  {A, B} isolated from C
  A writes: score=10 @ ts=100
  B writes: score=20 @ ts=200
  C writes: score=30 @ ts=300   ← highest timestamp

Reconnect (gossip rounds):
  Round 1: A <──> B   → both converge to score=20 @ ts=200
  Round 2: B <──> C   → both converge to score=30 @ ts=300
  Round 3: A <──> C   → A learns score=30 @ ts=300

Final state on all peers: score=30 @ ts=300
```

The number of gossip rounds needed to fully converge equals the diameter of the reconnected peer graph.

---

## Expected Outcomes Reference

| Scenario | Expected Winner | Rationale |
|----------|----------------|-----------|
| Same field, different timestamps | Higher timestamp | LWW |
| Same field, same timestamp, different values | Lexicographically greater JSON string | Deterministic tie-break |
| Different fields, no overlap | Both values preserved | No conflict |
| Different fields, overlapping fields | Per-field LWW applied independently | Per-field CRDT |
| Soul mismatch | No-op (unchanged) | Soul isolation |
| Partition + reconnect (N peers) | Highest per-field timestamp across all peers | Gossip convergence |

---

## Application Guidelines

### Choosing Timestamps

Because HAM uses timestamps to resolve conflicts, **clock accuracy matters**:

- Use the **highest precision monotonic clock** available on each peer.
- Prefer **logical clocks** (Lamport clocks, HLC) over wall-clock time in adversarial environments where clocks may be skewed.
- PluresDB's `now_ms()` uses the system wall clock (`SystemTime::now()`).  If peers have significant clock drift (>1 s), LWW behavior may appear unexpected.

### Designing Conflict-Friendly Schemas

Use these patterns to reduce harmful conflicts:

| Pattern | Description |
|---------|-------------|
| **Separate fields per author** | Instead of one `value` field, use `value_peerId` per peer |
| **Append-only sets** | Represent sets as a map of `element → tombstone` booleans |
| **Counter via accumulation** | Track per-peer deltas rather than absolute values |
| **Intent fields** | Add an `intent` field alongside a `value` field so a later merge can interpret meaning |

### Detecting Post-Merge Divergence

If your application needs to detect that a value was overwritten during a partition, store an additional **vector clock** or **author** field alongside the data field.  After a merge you can compare the `author` field to determine which peer's write survived.

---

## Test Coverage

The split-brain and conflict-resolution policies described in this document are verified by deterministic tests in:

```
crates/pluresdb-sync/tests/p2p_harness_test.rs
```

Key test functions:

| Test | Scenario |
|------|---------|
| `test_split_brain_isolated_partitions_converge` | Two isolated peers, concurrent writes → reconnect → LWW convergence |
| `test_split_brain_per_field_independent_convergence` | Per-field LWW with mixed timestamps |
| `test_split_brain_three_peer_partition_and_full_reconnect` | Three peers, two-partition, full reconnect |
| `test_three_peer_crdt_concurrent_writes_convergence` | Merge-order independence |
| `test_three_peer_crdt_convergence_via_transport` | Transport-layer HAM timestamp preservation |

---

## Related Documentation

- [`docs/HYPERSWARM_SYNC.md`](HYPERSWARM_SYNC.md) — Hyperswarm P2P transport details
- [`docs/SYNC_TRANSPORT.md`](SYNC_TRANSPORT.md) — Relay and direct transport architecture
- [`docs/DURABILITY.md`](DURABILITY.md) — Durability and write-ahead-log guarantees
- [`docs/GUN_WIRE_PROTOCOL.md`](GUN_WIRE_PROTOCOL.md) — GUN wire protocol specification
