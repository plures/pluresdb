# PluresDB

[![npm version](https://badge.fury.io/js/pluresdb.svg)](https://badge.fury.io/js/@plures/pluresdb)
[![crates.io](https://img.shields.io/crates/v/pluresdb-core.svg)](https://crates.io/crates/pluresdb-core)
[![Deno version](https://img.shields.io/badge/deno-v2.x-blue)](https://deno.land)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://opensource.org/licenses/AGPL-3.0)

**Local-First Graph Database with SQLite Compatibility**

PluresDB is a CRDT-based graph database built with Rust and TypeScript. It
provides SQLite API compatibility while adding graph relationships, vector
search, and P2P synchronisation — ideal for desktop apps, VSCode extensions,
and personal knowledge management.

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

```typescript
// Node.js / Deno
import { SQLiteCompatibleAPI } from "@plures/pluresdb";

const db = new SQLiteCompatibleAPI({ config: { dataDir: "./data" } });

await db.exec("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)");
await db.run("INSERT INTO users (name) VALUES (?)", ["Alice"]);
const users = await db.all("SELECT * FROM users");
```

```rust
// Rust
use pluresdb_core::{CrdtStore, Database, DatabaseOptions};
use serde_json::json;

let store = CrdtStore::default();
store.put("user:1", "actor-a", json!({ "name": "Alice" }));
let record = store.get("user:1");
```

## Features

- **SQLite Compatibility** — 95% API-compatible drop-in replacement
- **CRDT Store** — conflict-free replicated data with vector clocks
- **Vector Search** — approximate nearest-neighbour via HNSW (cosine similarity)
- **Auto-Embedding** — pluggable `EmbedText` trait; `FastEmbedder` for local ONNX models
- **P2P Sync** — Hyperswarm DHT or WebSocket relay, end-to-end encrypted
- **Local-First** — full functionality offline; sync is opt-in
- **Multi-Platform** — Node.js (N-API), Deno (JSR), Rust, CLI, Docker, WASM

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
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guide |
| [CHANGELOG.md](CHANGELOG.md) | Release history |
| [SECURITY.md](SECURITY.md) | Security policy |

## Architecture

PluresDB is a Rust-first monorepo:

| Crate | Responsibility |
|---|---|
| `pluresdb-core` | CRDT store, SQLite (`rusqlite`), HNSW vector index, `EmbedText` trait |
| `pluresdb-storage` | Pluggable backends: Sled, in-memory |
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
