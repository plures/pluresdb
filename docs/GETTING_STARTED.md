# Getting Started with PluresDB

Choose your platform:

- [Node.js (npm)](#nodejs-npm)
- [Deno (JSR)](#deno-jsr)
- [Rust (crates.io)](#rust-cratesio)
- [CLI](#cli)
- [Docker](#docker)
- [Windows](#windows)

---

## Node.js (npm)

### Requirements

- Node.js 18 or later

### Install

```bash
npm install @plures/pluresdb
```

### CRDT key-value store

```js
const { PluresDatabase } = require("@plures/pluresdb");

const db = new PluresDatabase("my-actor");

// Write
db.put("user:1", { name: "Alice", role: "admin" });

// Read
const user = db.get("user:1");
console.log(user); // { name: "Alice", role: "admin" }

// List all
const all = db.list();
console.log(all);

// Delete
db.delete("user:1");
```

### SQLite-compatible API

```js
const { SQLiteCompatibleAPI } = require("@plures/pluresdb");

const db = new SQLiteCompatibleAPI({ config: { dataDir: "./data" } });

// DDL
await db.exec("CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, body TEXT)");

// Insert
await db.run("INSERT INTO notes (body) VALUES (?)", ["Hello, PluresDB!"]);

// Query
const notes = await db.all("SELECT * FROM notes");
console.log(notes);
```

### Vector search (auto-embedding)

Requires `pluresdb-node` compiled with the `embeddings` feature.

```js
const { PluresDatabase } = require("@plures/pluresdb");

const db = PluresDatabase.newWithEmbeddings("BAAI/bge-small-en-v1.5");

// Text is auto-embedded on put
db.put("doc:1", { content: "Rust makes systems programming safe." });
db.put("doc:2", { content: "TypeScript adds types to JavaScript." });

// Supply the query embedding (compute with your preferred library)
const queryEmbedding = /* Float32Array | number[] */ getEmbedding("fast safe language");
const results = db.vectorSearch(queryEmbedding, 5, 0.3);
console.log(results); // [{ id, data, score }, ...]
```

### P2P sync (Node.js only)

```js
const { PluresDB } = require("@plures/pluresdb");

const db1 = new PluresDB();
await db1.ready();

const key = PluresDB.generateSyncKey();
console.log("Share this key:", key);

await db1.enableSync({ key });

db1.on("peer:connected", (info) => console.log("peer joined:", info.peerId));

await db1.put("shared:note", { text: "Hello from device 1" });
```

On device 2:

```js
const db2 = new PluresDB();
await db2.ready();
await db2.enableSync({ key }); // same key as device 1

// After DHT discovery (1–5 s):
const note = await db2.get("shared:note");
console.log(note); // { text: "Hello from device 1" }
```

---

## Deno (JSR)

### Requirements

- Deno 2.x

### Install

```bash
deno add @plures/pluresdb
```

### Basic usage

```typescript
import { PluresDB } from "@plures/pluresdb";

const db = new PluresDB();
await db.ready();

await db.put("user:alice", { name: "Alice" });

const user = await db.get("user:alice");
console.log(user);
```

### With HTTP server

```typescript
import { PluresDB, startApiServer } from "@plures/pluresdb";

const db = new PluresDB();
await db.ready();

db.serve({ port: 34567 });          // P2P / WebSocket port
const api = startApiServer({ port: 8080, db });

console.log("Web UI:   http://localhost:34568"); // served by db.serve()
console.log("REST API:     http://localhost:8080");
```

### Run tests

```bash
deno test -A --unstable-kv
```

---

## Rust (crates.io)

### Requirements

- Rust 1.75 or later (stable)

### Add dependencies

```toml
# Cargo.toml
[dependencies]
pluresdb-core = "0.1"
```

Optional extras:

```toml
# File-backed storage with SQLite WAL
pluresdb-core = "0.1"

# With automatic text embedding (downloads ONNX model on first use)
pluresdb-core = { version = "0.1", features = ["embeddings"] }

# P2P synchronisation
pluresdb-sync = "0.1"
```

### CRDT store (in-memory)

```rust
use pluresdb_core::{CrdtStore, CrdtOperation};
use serde_json::json;

fn main() {
    let store = CrdtStore::default();

    // Insert
    store.put("user:1", "actor-a", json!({ "name": "Alice" }));

    // Read
    if let Some(record) = store.get("user:1") {
        println!("{}", record.data);
    }

    // Delete
    store.delete("user:1").unwrap();
}
```

### File-backed SQLite database

```rust
use pluresdb_core::{Database, DatabaseOptions, SqlValue};

fn main() -> anyhow::Result<()> {
    let db = Database::open(
        DatabaseOptions::with_file("./plures.db")
            .create_if_missing(true),
    )?;

    db.exec("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)")?;

    let stmt = db.prepare("INSERT INTO users (name) VALUES (?)")?;
    stmt.run(&[SqlValue::Text("Alice".into())])?;

    let result = db.query("SELECT * FROM users", &[])?;
    for row in result.rows_as_json() {
        println!("{}", row);
    }

    Ok(())
}
```

### Vector search

```rust
use pluresdb_core::{CrdtStore, FastEmbedder};
use std::sync::Arc;

fn main() -> anyhow::Result<()> {
    let embedder = FastEmbedder::new("BAAI/bge-small-en-v1.5")?;
    let store = CrdtStore::default().with_embedder(Arc::new(embedder));

    store.put("doc:1", "writer", serde_json::json!({
        "content": "Rust makes systems programming safe."
    }));

    // Query embedding (compute from your text)
    let query_vec: Vec<f32> = embedder.embed(&["safe programming language"])?
        .into_iter().next().unwrap();

    let results = store.vector_search(&query_vec, 5, 0.3);
    for r in results {
        println!("{} — score: {:.3}", r.record.id, r.score);
    }

    Ok(())
}
```

### Run tests

```bash
cargo test --workspace
```

---

## CLI

### Install

```bash
# From crates.io
cargo install pluresdb-cli

# Pre-built binary (Linux / macOS / Windows)
# Download from https://github.com/plures/pluresdb/releases
```

### Quick start

```bash
# Initialise a database
pluresdb init ./my-db

# Start the HTTP server (port 34569 by default)
pluresdb --data-dir ./my-db serve

# In another terminal — basic CRUD
pluresdb --data-dir ./my-db put  "user:1" '{"name":"Alice"}'
pluresdb --data-dir ./my-db get  "user:1"
pluresdb --data-dir ./my-db list
pluresdb --data-dir ./my-db delete "user:1"

# SQL query
pluresdb --data-dir ./my-db query "SELECT name FROM users"

# Status
pluresdb --data-dir ./my-db status --detailed

# Health diagnostics (storage + WAL + sync)
pluresdb --data-dir ./my-db doctor
pluresdb --data-dir ./my-db doctor --json
```

### Help

```bash
pluresdb --help
pluresdb serve --help
```

---

## Docker

### Pull the image

```bash
docker pull pluresdb/pluresdb:latest
```

### Run in-memory (ephemeral)

```bash
docker run -p 34569:34569 pluresdb/pluresdb:latest
```

### Run with persistent storage

```bash
docker run -p 34569:34569 \
  -v $(pwd)/data:/data \
  pluresdb/pluresdb:latest \
  --data-dir /data serve
```

### Docker Compose

```yaml
version: "3.9"
services:
  pluresdb:
    image: pluresdb/pluresdb:latest
    ports:
      - "34569:34569"
      - "34568:34568"   # web UI
    volumes:
      - plures_data:/data
    command: ["--data-dir", "/data", "serve"]

volumes:
  plures_data:
```

```bash
docker compose up
```

### Health check

```bash
curl http://localhost:34569/api/status
```

---

## Windows

### Winget

```powershell
winget install pluresdb.pluresdb
```

### Manual installer

Download `PluresDB-Setup-x64.msi` from the
[latest GitHub release](https://github.com/plures/pluresdb/releases/latest)
and run it.

The installer places `pluresdb.exe` in `%ProgramFiles%\PluresDB\bin` and adds
it to your `PATH`.

### From source (requires libclang for Rust bindgen dependencies)

```powershell
# 1. Configure libclang (one-time setup)
pwsh ./scripts/setup-libclang.ps1 -ConfigureCurrentProcess

# 2. Build
cargo build --release

# 3. Run
./target/release/pluresdb.exe serve
```

### Verifying the installation

```powershell
pluresdb --version
pluresdb status
```

### PowerShell module (optional)

```powershell
Import-Module PluresDB
Initialize-PluresDBHistory
Enable-PluresDBHistoryIntegration

# Query command history stored in PluresDB
Get-PluresDBHistory -Last 10
```

For a full Windows guide including WSL support and NixOS integration, see
[docs/WINDOWS_GETTING_STARTED.md](WINDOWS_GETTING_STARTED.md).
