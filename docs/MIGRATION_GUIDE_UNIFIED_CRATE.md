# Migration Guide: Unified Crate + Local-First API

This guide covers two migration paths that are commonly needed together:

1. **Individual sub-crates → unified `pluresdb` crate** — simplify your
   `Cargo.toml` by depending on the umbrella crate instead of multiple
   `pluresdb-*` crates.
2. **TypeScript REST/WebSocket → local-first unified API** — remove the
   network round-trip for single-process apps by using the new
   `PluresDBLocalFirst` class.

For the `v1.x → v2.0` breaking-change migration (Sled storage, SQLite feature
gate) see [MIGRATION.md](./MIGRATION.md).  
For the TypeScript → Rust-native bindings migration see
[MIGRATION_GUIDE_V2.md](./MIGRATION_GUIDE_V2.md).

---

## Part 1 — Individual crates → unified `pluresdb` crate

### Why migrate?

Before the unified crate, Rust applications needed to list every sub-crate
explicitly:

```toml
# OLD — four separate dependencies
[dependencies]
pluresdb-core    = "0.1"
pluresdb-storage = "0.1"
pluresdb-sync    = "0.1"
```

The `pluresdb` umbrella crate re-exports everything from these crates so your
`Cargo.toml` shrinks to one line:

```toml
# NEW — single dependency
[dependencies]
pluresdb = "0.1"
```

### Import path changes

Replace every import that reaches into a sub-crate with the equivalent from
the umbrella crate.

| Old import path | New import path |
|---|---|
| `pluresdb_core::CrdtStore` | `pluresdb::CrdtStore` |
| `pluresdb_core::CrdtOperation` | `pluresdb::CrdtOperation` |
| `pluresdb_core::NodeData` | `pluresdb::NodeData` |
| `pluresdb_core::NodeId` | `pluresdb::NodeId` |
| `pluresdb_core::NodeRecord` | `pluresdb::NodeRecord` |
| `pluresdb_core::VectorClock` | `pluresdb::VectorClock` |
| `pluresdb_core::VectorIndex` | `pluresdb::VectorIndex` |
| `pluresdb_core::VectorSearchResult` | `pluresdb::VectorSearchResult` |
| `pluresdb_core::ActorId` | `pluresdb::ActorId` |
| `pluresdb_core::EmbedText` | `pluresdb::EmbedText` |
| `pluresdb_core::NoOpPlugin` | `pluresdb::NoOpPlugin` |
| `pluresdb_core::PluresLmPlugin` | `pluresdb::PluresLmPlugin` |
| `pluresdb_core::FastEmbedder` _(embeddings feature)_ | `pluresdb::FastEmbedder` |
| `pluresdb_core::Database` _(sqlite-compat feature)_ | `pluresdb::Database` |
| `pluresdb_core::DatabaseOptions` | `pluresdb::DatabaseOptions` |
| `pluresdb_core::QueryResult` | `pluresdb::QueryResult` |
| `pluresdb_core::SqlValue` | `pluresdb::SqlValue` |
| `pluresdb_core::StoreError` | `pluresdb::CoreError` |
| `pluresdb_core::DatabaseError` _(sqlite-compat feature)_ | `pluresdb::DatabaseError` |
| `pluresdb_storage::StorageEngine` | `pluresdb::StorageEngine` |
| `pluresdb_storage::StoredNode` | `pluresdb::StoredNode` |
| `pluresdb_storage::MemoryStorage` | `pluresdb::MemoryStorage` |
| `pluresdb_storage::SledStorage` | `pluresdb::SledStorage` |
| `pluresdb_storage::EncryptionConfig` | `pluresdb::EncryptionConfig` |
| `pluresdb_storage::EncryptionMetadata` | `pluresdb::EncryptionMetadata` |
| `pluresdb_storage::WriteAheadLog` | `pluresdb::WriteAheadLog` |
| `pluresdb_storage::WalEntry` | `pluresdb::WalEntry` |
| `pluresdb_storage::WalOperation` | `pluresdb::WalOperation` |
| `pluresdb_storage::ReplayStats` | `pluresdb::ReplayStats` |
| `pluresdb_sync::SyncBroadcaster` | `pluresdb::SyncBroadcaster` |
| `pluresdb_sync::SyncEvent` | `pluresdb::SyncEvent` |
| `pluresdb_sync::GunRelayServer` | `pluresdb::GunRelayServer` |

### Feature flags

The sub-crate feature flags are available on the umbrella crate:

```toml
# SQLite compatibility layer
pluresdb = { version = "0.1", features = ["sqlite-compat"] }

# On-device embeddings (fastembed / ONNX)
pluresdb = { version = "0.1", features = ["embeddings"] }

# Both
pluresdb = { version = "0.1", features = ["sqlite-compat", "embeddings"] }
```

### Before / after comparison

```rust
// BEFORE — importing from three separate crates
use pluresdb_core::{CrdtStore, NodeData};
use pluresdb_storage::{MemoryStorage, SledStorage, StorageEngine};
use pluresdb_sync::{SyncBroadcaster, SyncEvent};

fn setup() -> (CrdtStore, MemoryStorage) {
    let store   = CrdtStore::default();
    let storage = MemoryStorage::default();
    (store, storage)
}
```

```rust
// AFTER — single unified crate
use pluresdb::{CrdtStore, MemoryStorage, new_memory_database};

fn setup() -> (CrdtStore, MemoryStorage) {
    new_memory_database()
}
```

### Convenience functions (new)

The umbrella crate adds two factory functions that were not available on the
individual sub-crates:

```rust
use pluresdb::{new_memory_database, new_persistent_database};

// In-memory — ideal for tests
let (store, storage) = new_memory_database();

// Persistent — opens a Sled database at the given path
let (store, storage) = new_persistent_database("./data")?;
```

### Keeping direct sub-crate dependencies

You can continue to depend on individual crates alongside the umbrella crate
if you need fine-grained control over features.  The umbrella crate does not
add any new types; it only re-exports.

---

## Part 2 — TypeScript REST API → local-first unified API

### Why migrate?

The original TypeScript API communicated over HTTP/WebSocket even for
single-process apps.  This introduced unnecessary network overhead, required
the server process to be running, and exposed a local port.

`PluresDBLocalFirst` detects the runtime environment and selects the fastest
available backend automatically:

| Runtime | Backend selected |
|---|---|
| Browser | WebAssembly (`pluresdb-wasm`) |
| Tauri desktop app | Native Rust via Tauri commands |
| Node.js / Deno with `PLURESDB_IPC=true` | Shared-memory IPC |
| Any other environment | HTTP REST (existing server) |

### Old import paths vs new import paths

#### Node.js

```typescript
// OLD — direct database class
import { PluresDB } from "@plures/pluresdb";
const db = new PluresDB();
await db.ready();
```

```typescript
// NEW — auto-detecting local-first API
import { PluresDBLocalFirst } from "@plures/pluresdb/local-first";
const db = new PluresDBLocalFirst({ mode: "auto" });
console.log(`Mode: ${db.getMode()}`); // wasm | tauri | ipc | network
```

#### Deno

```typescript
// OLD
import { PluresDB } from "https://deno.land/x/pluresdb/mod.ts";

// NEW
import { PluresDBLocalFirst } from "jsr:@plures/pluresdb";
// or with a lock-file-friendly URL:
import { PluresDBLocalFirst } from "@plures/pluresdb";
```

### Config options

```typescript
import { PluresDBLocalFirst } from "@plures/pluresdb/local-first";

const db = new PluresDBLocalFirst({
  // "auto"    — detect best backend (default)
  // "wasm"    — force browser WebAssembly
  // "tauri"   — force Tauri native commands
  // "ipc"     — force shared-memory IPC
  // "network" — force HTTP REST (legacy)
  mode: "auto",

  // Name used by IPC channel and IndexedDB store
  dbName: "my-app",

  // IPC channel name (only relevant when mode === "ipc")
  channelName: "pluresdb-ipc",

  // REST server URL (only relevant when mode === "network")
  networkUrl: "http://localhost:34569",

  // Port for the network fallback server
  port: 34569,
});
```

### API comparison

The `PluresDBLocalFirst` interface is a strict superset of the legacy
`PluresDB` async API.  Existing `await db.put(...)` call sites work without
changes.

| Operation | Old (`PluresDB`) | New (`PluresDBLocalFirst`) |
|---|---|---|
| Insert / update | `await db.put(id, data)` | `await db.put(id, data)` ✅ |
| Read | `await db.get(id)` | `await db.get(id)` ✅ |
| Delete | `await db.delete(id)` | `await db.delete(id)` ✅ |
| List all | `await db.list()` | `await db.list()` ✅ |
| Semantic search | `await db.vectorSearch(q, n)` | `await db.vectorSearch(q, n)` ✅ |
| Close | `await db.close()` | `await db.close()` ✅ |
| Which backend? | _(always network)_ | `db.getMode()` → string |

### Full migration example

```typescript
// BEFORE
import { PluresDB } from "@plures/pluresdb";

async function run() {
  const db = new PluresDB();
  await db.ready();             // waits for HTTP server connection

  await db.put("user:1", { name: "Alice" });
  const user = await db.get("user:1");
  console.log(user);

  await db.close();
}
```

```typescript
// AFTER
import { PluresDBLocalFirst } from "@plures/pluresdb/local-first";

async function run() {
  const db = new PluresDBLocalFirst({ mode: "auto" });
  // No ready() call needed — constructor resolves the backend synchronously

  await db.put("user:1", { name: "Alice" });
  const user = await db.get("user:1");
  console.log(user);

  await db.close();
}
```

### P2P sync with the local-first API

P2P sync is not yet exposed through the `PluresDBLocalFirst` interface.  If
you need sync, continue using the `PluresDB` class directly and enable
Hyperswarm transport:

```typescript
import { PluresDB } from "@plures/pluresdb";

const db = new PluresDB();
await db.ready();

const key = PluresDB.generateSyncKey();
await db.enableSync({ key });

// Share `key` with other devices (QR code, invite link, etc.)
```

See [`docs/HYPERSWARM_SYNC.md`](./HYPERSWARM_SYNC.md) and the runnable
[`examples/sync-configuration.ts`](../examples/sync-configuration.ts)
for detailed transport configuration.

### Storage encryption with the local-first API

Encryption is configured at the Rust storage layer.  When using the binary
distribution (`pluresdb serve`) set the `PLURESDB_PASSPHRASE` environment
variable; the TypeScript layer is unaffected:

```bash
PLURESDB_PASSPHRASE=hunter2 pluresdb serve --data-dir ./secure-db
```

```typescript
// TypeScript code is identical — encryption is transparent
import { PluresDBLocalFirst } from "@plures/pluresdb/local-first";
const db = new PluresDBLocalFirst({ mode: "network", networkUrl: "http://localhost:34569" });
```

For embedded Rust usage see the
[`EncryptionConfig` API reference](./API.md#encryptionconfig) and
[`examples/storage-encryption.ts`](../examples/storage-encryption.ts).

---

## FAQ

**Q: Can I mix the umbrella crate and individual sub-crates in the same
workspace?**

A: Yes.  Cargo will de-duplicate them.  Prefer the umbrella crate for
application code; keep direct sub-crate dependencies in library crates that
need to minimise the dependency surface.

**Q: Does migrating to `PluresDBLocalFirst` change the data format?**

A: No.  The data format is defined by the CRDT layer, which is the same
regardless of which transport backend is used.  You can switch between
`network`, `ipc`, and `wasm` modes without re-migrating data.

**Q: Do I need to update my `deno.json` import map?**

A: If you previously pinned to `https://deno.land/x/pluresdb@<version>/mod.ts`
you should switch to the JSR package for better tooling support:

```jsonc
// deno.json
{
  "imports": {
    "@plures/pluresdb": "jsr:@plures/pluresdb@^2"
  }
}
```

**Q: The old `PluresDB` constructor accepted a `DatabaseOptions` object.
Does `PluresDBLocalFirst` accept the same?**

A: `PluresDBLocalFirst` accepts a `LocalFirstOptions` object (see Config
options above).  Pass `mode: "network"` and `networkUrl` to replicate the
behaviour of the old constructor pointing at an HTTP server.
