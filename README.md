# PluresDB

[![npm version](https://badge.fury.io/js/pluresdb.svg)](https://badge.fury.io/js/@plures/pluresdb)
[![crates.io](https://img.shields.io/crates/v/pluresdb-core.svg)](https://crates.io/crates/pluresdb-core)
[![Deno version](https://img.shields.io/badge/deno-v2.x-blue)](https://deno.land)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://opensource.org/licenses/AGPL-3.0)

**Local-First P2P Graph Database — v2.0**

PluresDB is a CRDT-based graph database built with Rust and TypeScript. It
provides a native sled-backed storage layer with CRDT conflict resolution,
vector search, and P2P synchronisation — ideal for desktop apps, VSCode
extensions, and personal knowledge management.

> **v2.0 breaking change:** The `rusqlite` dependency has been removed from
> `pluresdb-core` by default.  `CrdtStore::with_persistence()` now accepts
> `Arc<dyn StorageEngine>` instead of `Arc<Database>`.  If you need the legacy
> SQL layer, enable the `sqlite-compat` Cargo feature.  See
> [MIGRATION.md](MIGRATION.md) for upgrade instructions.

## Install

```bash
# Node.js
npm install @plures/pluresdb

# Deno
deno add @plures/pluresdb

# Rust
cargo add pluresdb-core

# CLI
cargo install pluresdb-cli

# Windows
winget install pluresdb.pluresdb

# Docker
docker pull pluresdb/pluresdb:latest
```

## Quick Example

```rust
// Rust
use pluresdb_core::CrdtStore;
use pluresdb_storage::{MemoryStorage, StorageEngine};
use serde_json::json;
use std::sync::Arc;

let storage = Arc::new(MemoryStorage::default());
let store = CrdtStore::default()
    .with_persistence(storage as Arc<dyn StorageEngine>);

store.put("user:1", "actor-a", json!({ "name": "Alice" }));
let record = store.get("user:1");
```

## Features

- **CRDT Store** — conflict-free replicated data with vector clocks
- **Native Storage** — sled-backed persistence (WAL, encryption, replay)
- **Vector Search** — approximate nearest-neighbour via HNSW (cosine similarity)
- **Auto-Embedding** — pluggable `EmbedText` trait; `FastEmbedder` for local ONNX models
- **P2P Sync** — Hyperswarm DHT or WebSocket relay, end-to-end encrypted
- **Local-First** — full functionality offline; sync is opt-in
- **Multi-Platform** — Node.js (N-API), Deno (JSR), Rust, CLI, Docker, WASM
- **SQLite optional** — legacy SQL layer via `sqlite-compat` feature flag

## Documentation

| Document | Description |
|---|---|
| [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) | Quick start for Node.js, Deno, Rust, CLI, Docker, Windows |
| [docs/API.md](docs/API.md) | Complete API reference (Rust, Node.js, Deno, REST, CLI) |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Internals: CRDTs, storage, HNSW, P2P protocol |
| [docs/WINDOWS_GETTING_STARTED.md](docs/WINDOWS_GETTING_STARTED.md) | Windows-specific setup guide |
| [docs/HYPERSWARM_SYNC.md](docs/HYPERSWARM_SYNC.md) | P2P sync deep-dive |
| [docs/SYNC_TRANSPORT.md](docs/SYNC_TRANSPORT.md) | Relay transport for corporate networks |
| [docs/LOCAL_FIRST_INTEGRATION.md](docs/LOCAL_FIRST_INTEGRATION.md) | WASM, Tauri, IPC integration guides |
| [docs/TESTING.md](docs/TESTING.md) | Test suite and CI notes |
| [MIGRATION.md](MIGRATION.md) | Upgrade guide: v1.x → v2.0 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guide |
| [CHANGELOG.md](CHANGELOG.md) | Release history |
| [SECURITY.md](SECURITY.md) | Security policy |

## Architecture

PluresDB is a Rust-first monorepo:

| Crate | Responsibility |
|---|---|
| `pluresdb-core` | CRDT store, HNSW vector index, `EmbedText` trait; SQLite optional via `sqlite-compat` |
| `pluresdb-storage` | Pluggable backends: Sled (WAL, encryption), in-memory |
| `pluresdb-sync` | `SyncBroadcaster`, `Transport` trait, Hyperswarm / relay / disabled |
| `pluresdb-cli` | `pluresdb` binary |
| `pluresdb-node` | N-API bindings for Node.js |
| `pluresdb-deno` | Deno FFI bindings |
| `pluresdb-wasm` | `wasm-bindgen` bindings for browsers |
| `pluresdb-ipc` | Shared-memory IPC server/client |
| `legacy/` | TypeScript layer (being replaced by Rust crates) |

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for a full description with
data-flow diagrams.

## Testing

```bash
npm run verify        # TypeScript build + all Deno tests
cargo test --workspace  # Rust tests
```

Network-dependent Hyperswarm tests are skipped automatically in CI.
See [docs/TESTING.md](docs/TESTING.md) for details.

## Distribution

- **npm**: [`pluresdb`](https://www.npmjs.com/package/pluresdb)
- **JSR**: [`@plures/pluresdb`](https://jsr.io/@plures/pluresdb)
- **crates.io**: [`pluresdb-core`](https://crates.io/crates/pluresdb-core), [`pluresdb-sync`](https://crates.io/crates/pluresdb-sync)
- **Winget**: `pluresdb.pluresdb`
- **Docker**: [`pluresdb/pluresdb`](https://hub.docker.com/r/pluresdb/pluresdb)
- **GitHub Releases**: pre-built binaries for Windows, macOS, Linux

## Security

All inputs are validated and sanitised. P2P communications are end-to-end
encrypted. Report vulnerabilities privately — see [SECURITY.md](SECURITY.md).

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).
All contributions are licensed under AGPL v3.

## License

GNU Affero General Public License v3.0. See [LICENSE](LICENSE).

---

**Built with Rust and TypeScript** 🚀
