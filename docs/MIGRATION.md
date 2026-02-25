# Migration Guide: v1.x → v2.0

PluresDB v2.0 removes the `rusqlite` dependency from the default build and
wires `CrdtStore` persistence to the native `pluresdb-storage` layer (sled).
This is a **breaking change** requiring updates to any code that used the
`Database` / `CrdtStore::with_persistence(Arc<Database>)` API.

---

## What Changed

### `CrdtStore::with_persistence`

**v1.x:**
```rust
use pluresdb_core::{CrdtStore, Database, DatabaseOptions};
use std::sync::Arc;

let db = Arc::new(Database::open(DatabaseOptions::default())?);
let store = CrdtStore::default()
    .with_persistence(db)?;           // returned Result<Self, DatabaseError>
```

**v2.0:**
```rust
use pluresdb_core::CrdtStore;
use pluresdb_storage::{SledStorage, StorageEngine};
use std::sync::Arc;

let storage = Arc::new(SledStorage::open("./data/db")?);
let store = CrdtStore::default()
    .with_persistence(storage as Arc<dyn StorageEngine>); // returns Self (infallible)
```

Key differences:
- Accepts `Arc<dyn StorageEngine>` instead of `Arc<Database>`
- Returns `Self` (no `Result` wrapper)
- Works with any `StorageEngine` implementation: `SledStorage`, `MemoryStorage`, or custom

### `rusqlite` / SQLite types

`Database`, `SqlValue`, `QueryResult`, `ExecutionResult`, `DatabasePath`,
`DatabaseOptions`, and `DatabaseError` are now gated behind the
`sqlite-compat` cargo feature.

To re-enable them:

```toml
# Cargo.toml
pluresdb-core = { version = "2.0", features = ["sqlite-compat"] }
```

Or using the umbrella crate:
```toml
pluresdb = { version = "2.0", features = ["sqlite-compat"] }
```

---

## Migrating Existing Data

If you have data in a v1.x SQLite database (`crdt_nodes` table), use the
built-in migration tool:

```bash
# Requires the sqlite-compat feature
cargo install pluresdb-cli --features sqlite-compat

pluresdb migrate-from-sqlite \
  --source /path/to/old/pluresdb.db \
  --target /path/to/new/pluresdb-data
```

This reads all rows from `crdt_nodes` (including embeddings) and writes them
to a sled store at `<target>/db/`.  The original SQLite file is left untouched.

---

## Node.js / N-API

The `query()` and `exec()` N-API methods now return an error at runtime unless
the crate is compiled with `--features sqlite-compat`.

The constructor no longer opens a SQLite file; it opens a sled store when
`db_path` is provided:

**v1.x:**
```js
// opened a SQLite .db file
const db = new PluresDatabase("my-actor", "./data/pluresdb.db");
db.exec("CREATE TABLE ...");
```

**v2.0:**
```js
// opens a sled directory (not a single file)
const db = new PluresDatabase("my-actor", "./data");
// db.exec() will throw unless the native module was built with sqlite-compat
```

---

## pluresLM Users

pluresLM already uses only the CRDT API (`put`, `get`, `delete`,
`vectorSearch`) and is unaffected by this change at the API level.

Update your `@plures/pluresdb` dependency to `^2.0.0` and rebuild the native
module.  Remove `better-sqlite3` from your `package.json` if present.

---

## Why This Change

- **Smaller binaries** — removes bundled SQLite C library (~1.5–2 MB)
- **Pure Rust build** — no C toolchain requirement; trivial cross-compilation
- **Encryption at rest** — sled storage already includes AES-GCM encryption
- **Mobile-friendly** — sled compiles to any target including Android/iOS NDK
- **Strategic alignment** — PluresDB positions itself as a SQLite replacement,
  not a wrapper

The `sqlite-compat` feature exists for existing users who need SQL access
during a transition period.
