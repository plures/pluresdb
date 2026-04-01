# PluresDB — P2P Graph Database

[![npm (@plures/pluresdb)](https://img.shields.io/npm/v/@plures/pluresdb)](https://www.npmjs.com/package/@plures/pluresdb)
[![crates.io](https://img.shields.io/crates/v/pluresdb-core.svg)](https://crates.io/crates/pluresdb-core)
[![JSR](https://jsr.io/badges/@plures/pluresdb)](https://jsr.io/@plures/pluresdb)
[![License: BSL-1.1](https://img.shields.io/badge/License-BSL--1.1-blue.svg)](LICENSE)

**Local-first, offline-first. One codebase: Rust native, Node.js, browser, Deno.**

PluresDB v3.0.0 is a fully Rust-powered P2P graph database with CRDT synchronization, vector search, SQLite compatibility, and a procedures engine. The same Rust codebase compiles to:

- **Native binary** (Rust, via `cargo`)
- **Node.js native addon** (N-API, via `npm`)
- **Browser WASM** (via `npm` or CDN)
- **Deno WASM** (via JSR)

## Features

- **CRDT synchronization** — Conflict-free replicated data types for multi-device sync
- **Vector search** — Semantic search with embeddings
- **SQLite compatibility** — Familiar SQL interface over CRDT storage
- **Procedures engine** — Programmable business logic inside the database
- **P2P networking** — Built-in Hyperswarm integration for direct peer-to-peer sync
- **Local-first** — Works offline, syncs when connected
- **Zero configuration** — No servers to set up

## Quick Start

### Rust (Native)

```bash
cargo add pluresdb-core
```

```rust
use pluresdb_core::CrdtStore;

fn main() {
    let store = CrdtStore::new("./data").unwrap();
    store.insert("key", b"value").unwrap();
    println!("{:?}", store.get("key"));
}
```

### Node.js (N-API)

```bash
npm install @plures/pluresdb
```

```javascript
import { CrdtStore } from '@plures/pluresdb';

const store = new CrdtStore('./data');
store.insert('key', Buffer.from('value'));
console.log(store.get('key'));
```

### Browser (WASM)

```bash
npm install @plures/pluresdb
```

```javascript
import init, { CrdtStore } from '@plures/pluresdb/wasm';

await init(); // Initialize WASM module
const store = new CrdtStore();
store.insert('key', new Uint8Array([118, 97, 108, 117, 101]));
console.log(store.get('key'));
```

Or via CDN:

```html
<script type="module">
  import init, { CrdtStore } from 'https://unpkg.com/@plures/pluresdb@3/dist/wasm/pluresdb.js';
  await init();
  const store = new CrdtStore();
  store.insert('key', new Uint8Array([118, 97, 108, 117, 101]));
</script>
```

### Deno (WASM via JSR)

```bash
deno add jsr:@plures/pluresdb
```

```typescript
import { CrdtStore } from '@plures/pluresdb';

const store = new CrdtStore();
store.insert('key', new Uint8Array([118, 97, 108, 117, 101]));
console.log(store.get('key'));
```

## Architecture

PluresDB v3.0.0 is a **Rust-first** architecture. All core functionality lives in Rust crates, compiled to multiple targets:

```
pluresdb/
├── crates/
│   ├── pluresdb-core         # CRDT store, storage engine
│   ├── pluresdb-sync         # P2P sync via Hyperswarm
│   ├── pluresdb-vector       # Vector search (embeddings)
│   ├── pluresdb-sql          # SQLite compatibility layer
│   ├── pluresdb-procedures   # Programmable procedures engine
│   ├── pluresdb-http         # HTTP/REST server
│   ├── pluresdb-node         # N-API bindings for Node.js
│   └── pluresdb-wasm         # WASM bindings for browser/Deno
├── dist/
│   ├── node/                 # N-API build output
│   ├── wasm/                 # WASM build output
│   └── types/                # TypeScript type definitions
```

**Compilation targets:**

- `cargo build --release` → Native binary (Linux, macOS, Windows)
- `napi build --release` → Node.js `.node` addon
- `wasm-pack build --target web` → Browser/Deno WASM

## Migration from v2.x

PluresDB v3.0.0 removes all TypeScript source code. The legacy `./legacy` exports are gone.

**Removed exports:**

- `@plures/pluresdb/legacy`
- `@plures/pluresdb/node`
- `@plures/pluresdb/better-sqlite3`
- `@plures/pluresdb/embedded`

**New exports:**

- `@plures/pluresdb` — Auto-detects Node.js, browser, or Deno
- `@plures/pluresdb/wasm` — Explicit WASM import

**Breaking changes:**

- All TypeScript source removed — Rust-first implementation
- Package now ships pre-compiled Rust (N-API for Node, WASM for browser/Deno)
- API surface remains compatible for core CRDT operations
- Some legacy utilities (CLI tools, VSCode extension entry points) may require updates

See [CHANGELOG.md](CHANGELOG.md) for detailed migration instructions.

## Documentation

- [API Reference](https://docs.rs/pluresdb-core)
- [Examples](examples/)
- [Architecture Decisions](docs/architecture/)
- [Contributing](CONTRIBUTING.md)

## License

Business Source License 1.1 — See [LICENSE](LICENSE) for details.

The BSL allows free use for non-production purposes. After 4 years from release date, the code becomes Apache 2.0 licensed.

## Community

- [GitHub Issues](https://github.com/plures/pluresdb/issues)
- [GitHub Discussions](https://github.com/plures/pluresdb/discussions)

---

**Built with Rust. Runs everywhere.**
