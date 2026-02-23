# PluresDB Architecture

> Reflects PluresDB v1.11.1 and later.

This document describes how PluresDB works internally — from the Rust core to
the TypeScript/Node.js layers and the P2P synchronization protocol.

---

## Table of Contents

1. [Overview](#overview)
2. [Crate Structure](#crate-structure)
3. [Rust Core vs Legacy TypeScript Layer](#rust-core-vs-legacy-typescript-layer)
4. [Data Flow](#data-flow)
5. [Storage Backends](#storage-backends)
6. [CRDT Merge Strategy and Vector Clocks](#crdt-merge-strategy-and-vector-clocks)
7. [HNSW Vector Index](#hnsw-vector-index)
8. [P2P Sync Protocol](#p2p-sync-protocol)

---

## Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Consumers                                   │
│  Node.js app   Deno app   Rust app   CLI   REST client          │
└────────┬────────────┬──────────┬──────┬──────────┬─────────────┘
         │            │          │      │           │
         ▼            ▼          │      ▼           ▼
┌──────────────┐ ┌──────────┐   │  ┌───────┐  ┌────────┐
│ pluresdb-node│ │pluresdb- │   │  │ CLI   │  │  HTTP  │
│  (N-API)     │ │  deno    │   │  │       │  │  API   │
└──────┬───────┘ └────┬─────┘   │  └───┬───┘  └───┬────┘
       └──────────────┴──────────┘      │          │
                       │                │          │
                       ▼                ▼          ▼
              ┌─────────────────────────────────────────┐
              │           pluresdb-core                 │
              │  CrdtStore · Database · VectorIndex     │
              └────────────────┬────────────────────────┘
                               │
               ┌───────────────┼───────────────┐
               ▼               ▼               ▼
     ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
     │   SQLite     │  │  SledStorage │  │  MemStorage  │
     │ (rusqlite)   │  │  (sled KV)   │  │  (in-proc)   │
     └──────────────┘  └──────────────┘  └──────────────┘
                               │
                               ▼
              ┌─────────────────────────────────────────┐
              │           pluresdb-sync                 │
              │  SyncBroadcaster · Transport trait      │
              └────────────────┬────────────────────────┘
                               │
               ┌───────────────┼───────────────┐
               ▼               ▼               ▼
     ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
     │  Hyperswarm  │  │    Relay     │  │   Disabled   │
     │  (DHT/UDP)   │  │ (WebSocket)  │  │ (local-only) │
     └──────────────┘  └──────────────┘  └──────────────┘
```

---

## Crate Structure

| Crate | Path | Responsibility |
|---|---|---|
| `pluresdb-core` | `crates/pluresdb-core` | CRDT store, `Database` (rusqlite), `VectorIndex` (HNSW), `EmbedText` trait |
| `pluresdb-storage` | `crates/pluresdb-storage` | Pluggable storage backends (Sled, in-memory) |
| `pluresdb-sync` | `crates/pluresdb-sync` | `SyncBroadcaster`, `Transport` trait, Hyperswarm / relay / disabled impls |
| `pluresdb-cli` | `crates/pluresdb-cli` | `pluresdb` binary — `init`, `serve`, `put`, `get`, `delete`, `list`, `query`, `status` |
| `pluresdb-node` | `crates/pluresdb-node` | N-API bindings (`PluresDatabase`) for Node.js |
| `pluresdb-deno` | `crates/pluresdb-deno` | Deno FFI bindings |
| `pluresdb-wasm` | `crates/pluresdb-wasm` | `wasm-bindgen` bindings for browsers |
| `pluresdb-ipc` | `crates/pluresdb-ipc` | Shared-memory IPC server/client for native apps |

### Legacy TypeScript layer (`legacy/`)

The `legacy/` directory contains a TypeScript/Deno implementation that is
**gradually being replaced** by the Rust crates above.  It is still used for:

- The `PluresDB` TypeScript class (Deno + Node.js)
- The `SQLiteCompatibleAPI` and `better-sqlite3` compatibility wrappers
- Hyperswarm integration (`legacy/network/hyperswarm-sync.ts`)
- The REST / WebSocket API server (`legacy/api/`)
- The Svelte web UI backend

New code should prefer the Rust N-API bindings (`pluresdb-node`) for Node.js
and the Rust Deno FFI bindings (`pluresdb-deno`) for Deno.

---

## Rust Core vs Legacy TypeScript Layer

```
Feature               Rust crates         Legacy TypeScript
──────────────────────────────────────────────────────────
CRDT store            pluresdb-core       legacy/core/crdt-store.ts
SQLite access         pluresdb-core       legacy/core/sqlite-compat.ts
Vector search         pluresdb-core       legacy/core/vector-search.ts
P2P sync              pluresdb-sync       legacy/network/hyperswarm-sync.ts
Node.js bindings      pluresdb-node       legacy/*.ts (compiled to JS)
Deno bindings         pluresdb-deno       mod.ts / jsr package
CLI                   pluresdb-cli        (no TS equivalent)
Browser / WASM        pluresdb-wasm       (no TS equivalent)
```

---

## Data Flow

### Write Path

```
caller: store.put("user:1", "actor-a", { name: "Alice" })
         │
         ▼
CrdtStore::put()
 ├─ node exists?
 │   ├─ yes → NodeRecord::merge_update(actor, data)
 │   │         └─ increment vector clock for actor
 │   └─ no  → NodeRecord::new(id, actor, data)
 │             └─ initialise clock { actor: 1 }
 │
 ├─ embedder attached?
 │   └─ yes → extract text → EmbedText::embed() → put_with_embedding()
 │             └─ VectorIndex::insert(id, embedding)
 │
 └─ return NodeId
         │
         ▼ (Node.js / CLI path)
SyncBroadcaster::publish(SyncEvent::NodeUpsert { id })
         │
         ▼
Transport::announce() / broadcast to connected peers
```

### Read Path

```
caller: store.get("user:1")
         │
         ▼
DashMap::get(id) → Option<NodeRecord>
         │
         └─ NodeRecord { id, data, clock, timestamp, embedding? }
```

### Sync Path

```
Local write → SyncBroadcaster::publish(NodeUpsert)
         │
         ▼
SyncBroadcaster subscriber (background task)
         │
         ├─ serialise CrdtOperation::Put { id, actor, data }
         └─ Transport::send(peer_conn, payload)
                  │
                  ▼ (remote peer)
         Transport::receive()
                  │
                  ▼
         CrdtStore::apply(CrdtOperation::Put)
                  │
                  └─ merge_update() or insert as new node
```

---

## Storage Backends

PluresDB supports three storage backends, selected at construction time:

### SQLite (via rusqlite) — `pluresdb-core`

The default production backend.  A `Database` wraps a `rusqlite::Connection`
behind a `parking_lot::Mutex` so it is safe to share across threads.

Default pragmas applied automatically:

| Pragma | Value | Effect |
|---|---|---|
| `journal_mode` | `WAL` | Write-ahead log for concurrent reads |
| `synchronous` | `NORMAL` | Safe and fast durability |
| `temp_store` | `MEMORY` | Temporary tables in RAM |
| `mmap_size` | `30000000000` | Memory-mapped I/O |
| `page_size` | `4096` | Standard page size |
| `cache_size` | `-64000` | 64 MB page cache |

### Sled — `pluresdb-storage`

An embedded key-value store backed by [sled](https://github.com/spacejam/sled).
Useful for lightweight deployments that don't need SQL.

### In-memory — `pluresdb-storage`

A `HashMap`-backed store for testing and ephemeral workloads.  No persistence.

---

## CRDT Merge Strategy and Vector Clocks

PluresDB uses **Last-Write-Wins (LWW) CRDTs** with **vector clocks** for
conflict-free replication.

### NodeRecord

```rust
pub struct NodeRecord {
    pub id:        NodeId,           // String key
    pub data:      JsonValue,        // Arbitrary JSON payload
    pub clock:     VectorClock,      // HashMap<ActorId, u64>
    pub timestamp: DateTime<Utc>,    // Wall-clock time (informational)
    pub embedding: Option<Vec<f32>>, // Optional HNSW embedding
}

pub type VectorClock = HashMap<ActorId, u64>;
```

### Merge Rules

1. **First write** — a new `NodeRecord` is created with `clock: { actor: 1 }`.
2. **Local update** — `merge_update(actor, new_data)` increments the counter for
   `actor` and replaces `data` with `new_data`.
3. **Remote update (sync)** — the incoming `CrdtOperation::Put` is fed to
   `CrdtStore::apply()`, which calls the same `merge_update` path.  The highest
   counter value per actor wins.

Because every actor tracks its own monotonically increasing counter, concurrent
writes by different actors are always distinguishable without coordination.

### CrdtOperation

```rust
pub enum CrdtOperation {
    Put    { id: NodeId, actor: ActorId, data: NodeData },
    Delete { id: NodeId },
}
```

Operations are serialised (e.g. as JSON) and transmitted to peers via the sync
transport.  Peers replay them through `CrdtStore::apply()`.

---

## HNSW Vector Index

PluresDB embeds a **Hierarchical Navigable Small World (HNSW)** graph index
([hnsw_rs](https://github.com/jean-pierreBoth/hnsw_rs)) for approximate
nearest-neighbour search.

### Key Parameters

| Parameter | Value | Notes |
|---|---|---|
| Distance metric | `DistCosine` | Cosine similarity |
| `max_nb_connection` (M) | `16` | HNSW graph degree |
| `ef_construction` | `200` | Build-time exploration |
| `ef_search` | `16` | Query-time exploration |
| Default capacity | `1 000 000` | Pre-allocated slots |

### Score Computation

`DistCosine` returns `1 − cos(θ)` in `[0, 2]`.  PluresDB maps this to a
similarity score in `[0, 1]`:

```
score = max(0.0,  1.0 − distance)
```

| distance | cos(θ) | score |
|---|---|---|
| 0 | 1 (identical) | 1.0 |
| 1 | 0 (orthogonal) | 0.0 |
| 2 | −1 (opposite) | 0.0 (clamped) |

### Auto-Embedding

When an `EmbedText` backend is attached via `CrdtStore::with_embedder()`, every
`put()` call that finds extractable text in the JSON payload automatically calls
`embed()` and indexes the resulting vector.

The `FastEmbedder` implementation (behind the `embeddings` cargo feature) uses
[FastEmbed / ONNX Runtime](https://github.com/Anush008/fastembed-rs) to run
HuggingFace sentence-transformer models locally.

### Capacity Management

- Each insert (including updates) consumes one slot.
- Inserts beyond `max_elements` are **silently dropped** (logged at `debug!`).
- A warning is emitted at 90% capacity.

---

## P2P Sync Protocol

### Transport Trait

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self, topic: TopicHash) -> Result<Receiver<Box<dyn Connection>>>;
    async fn announce(&mut self, topic: TopicHash) -> Result<()>;
    async fn lookup(&self,  topic: TopicHash) -> Result<Vec<PeerInfo>>;
    async fn disconnect(&mut self) -> Result<()>;
    fn    name(&self) -> &str;
}
```

### Transport Modes

| Mode | Enum variant | Description |
|---|---|---|
| Hyperswarm | `TransportMode::Hyperswarm` | DHT peer discovery, UDP holepunching |
| Relay | `TransportMode::Relay` | WebSocket relay, port 443 (corporate-friendly) |
| Disabled | `TransportMode::Disabled` | Local-only, no network activity |

### Topic Derivation

Peers find each other by announcing and looking up a **topic hash** derived from
the database ID using **BLAKE2b-256**:

```rust
pub fn derive_topic(database_id: &str) -> TopicHash /* [u8; 32] */ {
    Blake2b::<U32>::new()
        .chain_update(database_id)
        .finalize()
        .into()
}
```

Two instances sharing the same `database_id` will derive the same topic hash
and therefore discover each other in the DHT without any pre-configured address.

### SyncBroadcaster

`SyncBroadcaster` is an in-process `tokio::sync::broadcast` hub.  Each write
(and delete) publishes a `SyncEvent` that background tasks can subscribe to:

```rust
pub enum SyncEvent {
    NodeUpsert      { id: String },
    NodeDelete      { id: String },
    PeerConnected   { peer_id: String },
    PeerDisconnected { peer_id: String },
}
```

A background replication task subscribes, retrieves the full `NodeRecord`, and
forwards it to connected peers.

### Hyperswarm Transport

Uses [Hyperswarm](https://github.com/hypercore-protocol/hyperswarm) (Node.js)
via the `legacy/network/hyperswarm-sync.ts` wrapper when running under Node.js.
The Rust `HyperswarmTransport` (`crates/pluresdb-sync/src/hyperswarm.rs`)
provides the equivalent implementation for native Rust builds.

Features:
- **DHT discovery** — peers find each other automatically from the topic hash.
- **NAT traversal** — UDP holepunching works through most home/office firewalls.
- **Noise encryption** — every connection is encrypted with the Noise protocol.
- **Broadcast loop prevention** — sender peer ID is excluded when relaying
  messages to prevent message storms.

### Relay Transport

For environments where UDP is blocked (corporate networks), the relay transport
uses a WebSocket server on port 443.  All traffic is end-to-end encrypted before
being forwarded by the relay.
