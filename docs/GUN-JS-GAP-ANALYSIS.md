# GUN.js → PluresDB Gap Analysis

_Last updated: 2026-02-27 (PST)_

## Executive Summary

PluresDB has a strong Rust foundation for **core CRDT storage**, **vector/embedding search**, and **storage abstraction**, but it is not yet wire-compatible with the GUN ecosystem.

Biggest blockers to full compatibility:
1. **No GUN wire-protocol implementation** in Rust (`get/put/ack` message model, graph deltas, lexical queries).
2. **No SEA implementation** in Rust (identity, signing, encryption, authz conventions).
3. **No working Rust transport implementation** (`pluresdb-sync` transport modules are stubs, including Hyperswarm and relay).
4. **No GUN relay/WebSocket compatibility layer** (needed so JS GUN clients can connect and sync).
5. **HAM parity is incomplete** (current Rust `merge_update()` is LWW-per-node, not full GUN field-state HAM behavior).

At the same time, PluresDB already exceeds GUN in several areas (typed nodes, embeddings, HNSW vector search, procedure engine direction), which should be preserved as a superset layer behind a compatibility shim.

---

## Scope and Sources

### Repo files reviewed
- `crates/IMPLEMENTATION_STATUS.md`
- `legacy/core/crdt.ts`
- `legacy/network/hyperswarm-sync.ts`
- `legacy/core/database.ts`
- `legacy/storage/kv-storage.ts`
- `legacy/types/index.ts`
- `crates/pluresdb-core/src/lib.rs`
- `crates/pluresdb-sync/src/lib.rs`
- `crates/pluresdb-sync/src/transport.rs`
- `crates/pluresdb-sync/src/hyperswarm.rs`
- `crates/pluresdb-sync/src/relay.rs`
- `crates/pluresdb-storage/src/lib.rs`
- `crates/pluresdb-storage/src/encryption.rs`
- `crates/pluresdb-storage/src/wal.rs`

### External reference points
- GUN wire wiki (`Wire-Protocols (v0.1.x)`), GUN storage adapter docs, RAD wiki.
- GUN SEA source files (`sea/pair.js`, `sign.js`, `verify.js`, `encrypt.js`, `decrypt.js`, `work.js`) for concrete primitives and payload shapes.

---

## 1) GUN Core (Graph + CRDT + HAM)

### What GUN.js does
- Graph DB where each node has:
  - soul (`_['#']`)
  - per-field state map (`_['>'][field] = state`)
  - field values (primitive or relation link `{'#': soul}`)
- Conflict resolution via **HAM** semantics around state/time/value checks at field granularity.
- Replication uses **node delta** updates (not full-document overwrite).

### PluresDB current state
- **Legacy TS**: `mergeNodes()` + `deepMergeWithDeletes()` with per-field `state` timestamps and vector clocks (closest to GUN semantics).
- **Rust core** (`pluresdb-core`): `NodeRecord { id, data, clock, timestamp, embedding }`, `merge_update()` increments actor clock but replaces `data` at node level; no explicit field-state HAM function yet.

### Gap table

| Feature | GUN.js | Legacy TS | Rust Crate | Status | Priority |
|---|---|---|---|---|---|
| Graph node metadata (`_['#']`, `_['>']`) | Native | Partial equivalent (`id`, `state`) | Partial (`id`, node-level clock/timestamp) | Partial | P0 |
| Field-level conflict resolution (HAM-like) | Yes | Yes (per-field timestamps) | No (node-level merge_update overwrite) | Gap | P0 |
| Delta-based merge (partial fields + tombstones) | Yes | Yes (`null` delete, deep merge) | Partial | Gap | P0 |
| Link semantics (`{'#': soul}` refs) | Yes | Implicit JSON | Implicit JSON | Partial | P1 |
| Deterministic tie-breaking compatible with GUN HAM | Yes | Approximate | No explicit HAM | Gap | P0 |

### Required compatibility work
- Implement a **HAM-compatible merge engine** in Rust with field-level state map and deterministic tie-breaking.
- Keep current vector clocks as additional metadata (PluresDB extension), but do not let them break GUN-visible behavior.

### Recommended Rust crates
- `serde_json` (already used) for graph nodes.
- `indexmap` for stable deterministic iteration where tie-break ordering matters.
- `smallvec` for perf-sensitive merge paths (optional).

---

## 2) SEA (Security, Encryption, Authorization)

### What GUN SEA does (from source)
- Key generation: `SEA.pair()` creates:
  - signing keys: **ECDSA P-256** (`pub`, `priv`)
  - encryption keys: **ECDH P-256** (`epub`, `epriv`)
- Sign/verify:
  - SHA-256 hash + ECDSA P-256 signature
  - signed envelope pattern: `{ m, s }` often serialized as `"SEA{...}"`
- Encrypt/decrypt:
  - AES-GCM with random salt/iv-derived key path
  - envelope fields: `{ ct, iv, s }`
- Work/proof helper:
  - PBKDF2 / SHA-* utilities (`SEA.work`)
- User/auth conventions layered over these primitives (`user().create/auth/grant/...`).

### PluresDB current state
- `pluresdb-storage/encryption.rs` provides AES-256-GCM + Argon2id KDF for at-rest storage encryption.
- No SEA-compatible identity model, signed envelopes, cert/grant conventions, or user graph semantics in Rust crates.

### Gap table

| Feature | GUN.js | Legacy TS | Rust Crate | Status | Priority |
|---|---|---|---|---|---|
| ECDSA P-256 sign/verify compatibility | Yes | No | No | Gap | P0 |
| ECDH P-256 shared-secret flow | Yes | No | No | Gap | P0 |
| SEA envelope formats (`SEA{}`, `{m,s}`, `{ct,iv,s}`) | Yes | No | No | Gap | P0 |
| User auth API conventions (`create/auth/leave/grant`) | Yes | No | No | Gap | P1 |
| Proof-of-work / PBKDF2 helper semantics | Yes | No | Partial (Argon2 for storage only) | Gap | P2 |
| At-rest encryption | Not core SEA scope | No | Yes (AES-GCM + Argon2id) | Superset | P2 |

### Required compatibility work
- Build a `pluresdb-sea` crate implementing SEA-compatible key formats and envelope serialization.
- Distinguish two layers:
  1. **SEA-compat layer** (for interop)
  2. **Plures-native security layer** (Argon2id, stronger defaults, policy engine)

### Recommended Rust crates
- `p256` + `ecdsa` (strict P-256 compatibility with SEA/WebCrypto behavior).
- `sha2` for SHA-256 hashing.
- `aes-gcm` (already present) for SEA-compatible symmetric crypto.
- `hkdf` and/or PBKDF2 crate (`pbkdf2`) for SEA work-function parity where needed.
- `base64` (URL-safe modes) for wire-compatible encoding.
- `rand_core` / `getrandom` for nonce/salt generation.

> Note: `ed25519-dalek` is excellent but not SEA-compatible for signature format expectations (SEA currently uses ECDSA P-256 semantics). Can be added as Plures-native option, not compatibility default.

---

## 3) RAD (Random Access Data / Radix adapter model)

### What GUN RAD does
- Radix-tree chunking layer over pluggable storage backends.
- Exposes simple store API (`put(key,data,cb)`, `get(key,cb)`) and handles batching/chunking/lexical range reads.
- Supports lexical/range queries used by GUN’s graph fetch patterns.

### PluresDB current state
- **Legacy TS**: `KvStorage` on Deno KV (simple node/history ops).
- **Rust**: `StorageEngine` trait with `put/get/delete/list`; implementations: `MemoryStorage`, `SledStorage`; plus WAL/replay/encryption modules.
- No radix chunking/lexical range API equivalent today.

### Gap table

| Feature | GUN.js | Legacy TS | Rust Crate | Status | Priority |
|---|---|---|---|---|---|
| Radix chunked storage engine | Yes | No | No | Gap | P1 |
| Lexical/range query semantics (`*`, `>`, `<`, `%`) | Yes | No | No | Gap | P0 |
| Pluggable backend interface | Yes | Minimal | Yes (`StorageEngine`) | Partial | P1 |
| Batch/flush tuning knobs (chunk, until, batch) | Yes | No | Partial (WAL durability controls) | Gap | P2 |
| WAL + replay durability | No (not first-class) | No | Yes | Superset | P2 |

### Required compatibility work
- Add a RAD-compat adapter trait above `StorageEngine`, or extend `StorageEngine` with range iterators and prefix scans.
- Implement lexical constraint parser and bounded-range reads for GUN-style get queries.

### Recommended Rust crates
- `radix_trie` or `qp-trie` for in-memory radix indexing.
- `sled` trees + prefix iterators (already available) for durable lexical scans.
- `tantivy` not required for RAD parity (could be separate search layer).

---

## 4) Wire Protocol Compatibility

### Canonical GUN message model (practical minimum)

For adapter and peer compatibility, Rust side must understand/emit these JSON structures:

#### GET request
```json
{ "#": "req-id", "get": { "#": "soul", ".": "field?" } }
```

#### GET ack/response
```json
{ "@": "req-id", "put": { "soul": { "_": { "#": "soul", ">": { "field": 123 } }, "field": "value" } }, "err": null }
```

#### PUT request (graph delta)
```json
{ "#": "msg-id", "put": { "soul": { "_": { "#": "soul", ">": { "field": 123 } }, "field": "value" } }
```

#### PUT ack
```json
{ "@": "msg-id", "ok": true, "err": null }
```

Also encountered in older HTTP/WS envelope flows: body/header wrappers and stateless/stateful hybrid IDs. Compatibility layer should normalize these into internal message structs.

### PluresDB current state
- Legacy TS wire messages are custom mesh format:
  - `{ type: "put", node }`
  - `{ type: "delete", id }`
  - `{ type: "sync_request" }`
- Rust sync currently exposes only `SyncEvent` enum and transport traits; no GUN message codec.

### Gap table

| Feature | GUN.js | Legacy TS | Rust Crate | Status | Priority |
|---|---|---|---|---|---|
| `#` request IDs + `@` ack correlation | Yes | No | No | Gap | P0 |
| Graph delta `put` with soul metadata | Yes | No | No | Gap | P0 |
| `get` lexical + field queries | Yes | No | No | Gap | P0 |
| `ok/err` ack semantics | Yes | Minimal | No | Gap | P1 |
| Backward WS/HTTP envelope normalization | Yes | No | No | Gap | P2 |

### Required compatibility work
- Create `pluresdb-gun-protocol` crate:
  - message enums + serde codecs
  - validation of graph/node metadata
  - ack tracking and dedup map
- Add protocol translator:
  - GUN JSON ↔ internal `SyncEvent`/CRDT ops
- Keep Hyperswarm transport payload-agnostic; run GUN protocol frames over any transport.

---

## 5) Relay / Peer Discovery / Connectivity

### What GUN ecosystem expects
- WebSocket relay servers are common rendezvous points and browser-friendly transport.
- GUN clients typically speak WS to `/gun`, exchange get/put/ack protocol frames.
- Relay assists peer discovery and connectivity in NAT-constrained environments.

### PluresDB current state
- Legacy TS:
  - Working WS mesh server (`legacy/network/websocket-server.ts`)
  - Working Hyperswarm sync transport (`legacy/network/hyperswarm-sync.ts`)
- Rust:
  - `pluresdb-sync` defines `Transport` trait (good architecture)
  - `HyperswarmTransport` and `RelayTransport` are stubs returning not implemented.

### Gap table

| Feature | GUN.js | Legacy TS | Rust Crate | Status | Priority |
|---|---|---|---|---|---|
| Browser-compatible WebSocket relay endpoint | Yes | Yes (legacy mesh) | No (stub) | Gap | P0 |
| Hyperswarm DHT transport | No (native) | Yes | Stub | Gap | P0 |
| Transport abstraction for multiple network modes | Limited | Partial | Yes (`Transport` trait) | Partial | P1 |
| NAT traversal path for browser clients | Via relay | Via WS | No | Gap | P0 |

### Required compatibility work
- Implement Rust relay server mode (Axum + WebSocket) exposing GUN protocol endpoint for JS clients.
- Implement Rust hyperswarm transport once `hyperswarm-rs` is ready.
- Add mode orchestration: try direct/hyperswarm, fallback to relay, with re-announce policy.

### Recommended Rust crates
- `axum` / `tokio-tungstenite` for relay WS endpoints.
- `serde_json` streaming frames.
- `dashmap` for peer/session maps.
- `hyperswarm-rs` (from `plures/hyperswarm`) for DHT + holepunch.

---

## HAM vs `merge_update()` (Critical Comparison)

### GUN HAM (conceptual)
- Decides conflicts at **field level** using state clocks and deterministic ordering rules.
- Allows safe merging of partial graph updates from different peers and times.

### PluresDB Rust today
- `NodeRecord::merge_update()` increments actor clock, updates timestamp, and replaces `data` payload.
- This is robust for local CRDT progression but not equivalent to GUN’s field-state HAM merge behavior.

### Consequence
Without a HAM-compatible layer, Rust PluresDB cannot safely claim wire-protocol compatibility with arbitrary GUN peers sending partial field deltas.

---

## What PluresDB already does that GUN does not (or not natively)

1. **Vector search + embeddings** (`hnsw_rs`, embedding pipelines).
2. **Typed node ergonomics** and richer Rust-native APIs.
3. **WAL + replay + durability controls** in storage crate.
4. **Rust-first safety/performance model** and multi-binding strategy (CLI/node/deno scaffolding).

These should remain as Plures-native extensions behind a compatibility boundary:
- `compat/gun/*` for strict interop
- `native/plures/*` for advanced features

---

## Prioritized Implementation Plan

### Phase 0 (Interop MVP)
1. Implement `pluresdb-gun-protocol` (parse/validate/serialize get/put/ack).
2. Implement field-level HAM-compatible merge in Rust core.
3. Implement WebSocket relay endpoint that GUN JS can connect to.
4. Hook protocol ops into CRDT store (`merge_update` replacement/augmentation).

### Phase 1 (Network parity)
1. Complete `HyperswarmTransport` using `hyperswarm-rs`.
2. Complete `RelayTransport` in Rust sync crate.
3. Add protocol-over-transport framing and dedup/ack tracking.

### Phase 2 (SEA parity)
1. Ship `pluresdb-sea` crate with P-256 sign/verify + ECDH + AES-GCM envelopes.
2. Add SEA user/auth compatibility conventions.
3. Add permission/cert model compatible with GUN usage patterns.

### Phase 3 (RAD parity)
1. Add lexical/range query support and optional radix index.
2. Implement RAD-style adapter shim over `StorageEngine`.
3. Add bounded range pagination semantics (`%` byte limits).

---

## Concrete crate recommendations by gap

| Gap | Crates |
|---|---|
| GUN wire codec + message schema | `serde`, `serde_json`, `thiserror` |
| HAM deterministic merge | `indexmap`, `serde_json`, `smallvec` (optional) |
| SEA signing/keys | `p256`, `ecdsa`, `sha2`, `base64` |
| SEA encryption | `aes-gcm`, `hkdf`, `pbkdf2` |
| Relay WS compat | `axum`, `tokio`, `tokio-tungstenite` |
| Hyperswarm transport | `hyperswarm-rs` |
| RAD lexical/range indexing | `radix_trie`/`qp-trie`, `sled` prefix iter |

---

## Bottom line

PluresDB is architecturally pointed in the right direction (especially `Transport` and modular crates), but **GUN compatibility is currently structural, not behavioral**. The critical path is:

**GUN protocol codec + HAM parity + WS relay compat** first,
then **hyperswarm transport completion**,
then **SEA + RAD compatibility layers**.

Once those land, PluresDB can act as a Rust-native superset that still interoperates with existing GUN.js clients over the expected wire formats.