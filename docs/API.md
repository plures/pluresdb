# PluresDB API Reference

Complete reference for all public APIs across Rust, Node.js, Deno, CLI, and REST.

---

## Table of Contents

1. [Rust API — `pluresdb-core`](#rust-api--pluresdb-core)
   - [CrdtStore](#crdtstore)
   - [Database](#database)
   - [VectorIndex](#vectorindex)
   - [EmbedText trait](#embedtext-trait)
2. [Rust API — `pluresdb-sync`](#rust-api--pluresdb-sync)
   - [SyncBroadcaster](#syncbroadcaster)
   - [Transport trait](#transport-trait)
3. [Node.js API (N-API bindings)](#nodejs-api-n-api-bindings)
   - [PluresDatabase](#pluresdatabase)
4. [Deno / JSR API](#deno--jsr-api)
5. [TypeScript / Legacy API](#typescript--legacy-api)
   - [PluresDB class](#pluresdatabase-typescript-class)
   - [SQLiteCompatibleAPI](#sqlitecompatibleapi)
   - [better-sqlite3 compat](#better-sqlite3-compat)
6. [REST API](#rest-api)
7. [CLI Commands](#cli-commands)

---

## Rust API — `pluresdb-core`

Add to `Cargo.toml`:

```toml
[dependencies]
pluresdb-core = "0.1"
```

### CrdtStore

An in-memory, conflict-free replicated store backed by a concurrent `DashMap`.

```rust
use pluresdb_core::CrdtStore;

let store = CrdtStore::default();
```

#### Methods

##### `put`

```rust
pub fn put(
    &self,
    id:    impl Into<NodeId>,
    actor: impl Into<ActorId>,
    data:  NodeData,          // serde_json::Value
) -> NodeId
```

Inserts or updates a node using CRDT semantics.  If an `EmbedText` backend is
attached (via `with_embedder`), text content in `data` is automatically
embedded.

##### `put_with_embedding`

```rust
pub fn put_with_embedding(
    &self,
    id:        impl Into<NodeId>,
    actor:     impl Into<ActorId>,
    data:      NodeData,
    embedding: Vec<f32>,
) -> NodeId
```

Stores a node together with a pre-computed embedding vector.  The embedding is
indexed in the HNSW graph immediately.

##### `get`

```rust
pub fn get(&self, id: impl AsRef<str>) -> Option<NodeRecord>
```

Returns the node record for `id`, or `None` if it does not exist.

##### `delete`

```rust
pub fn delete(&self, id: impl AsRef<str>) -> Result<(), StoreError>
```

Removes a node.  Returns `StoreError::NotFound` if the node does not exist.

##### `list`

```rust
pub fn list(&self) -> Vec<NodeRecord>
```

Returns all nodes currently stored.  Order is unspecified.

##### `apply`

```rust
pub fn apply(&self, op: CrdtOperation) -> Result<Option<NodeId>, StoreError>
```

Applies a serialised CRDT operation.  Used by the sync layer to replay remote
writes.

##### `vector_search`

```rust
pub fn vector_search(
    &self,
    query_embedding: &[f32],
    limit:           usize,
    min_score:       f32,    // 0.0 – 1.0
) -> Vec<VectorSearchResult>
```

Returns up to `limit` nodes whose cosine similarity to `query_embedding` is
`≥ min_score`, ordered highest-first.

##### `with_embedder`

```rust
pub fn with_embedder(self, embedder: Arc<dyn EmbedText>) -> Self
```

Attaches an automatic text-embedding backend.  After this call, every `put()`
will auto-embed extractable text content.

---

#### NodeRecord

```rust
pub struct NodeRecord {
    pub id:        NodeId,           // String
    pub data:      NodeData,         // serde_json::Value
    pub clock:     VectorClock,      // HashMap<ActorId, u64>
    pub timestamp: DateTime<Utc>,
    pub embedding: Option<Vec<f32>>,
}
```

#### VectorSearchResult

```rust
pub struct VectorSearchResult {
    pub record: NodeRecord,
    /// Cosine similarity in [0, 1].  1 = identical direction.
    pub score: f32,
}
```

#### CrdtOperation

```rust
pub enum CrdtOperation {
    Put    { id: NodeId, actor: ActorId, data: NodeData },
    Delete { id: NodeId },
}
```

---

### Database

A thin, thread-safe wrapper around `rusqlite::Connection`.

```rust
use pluresdb_core::{Database, DatabaseOptions};

// File-based database
let db = Database::open(
    DatabaseOptions::with_file("./data.db").create_if_missing(true)
)?;

// In-memory database
let db = Database::open(DatabaseOptions::in_memory())?;
```

#### DatabaseOptions

| Builder method | Default | Description |
|---|---|---|
| `in_memory()` | — | Use an in-memory SQLite database |
| `with_file(path)` | — | Use a file-based SQLite database |
| `read_only(bool)` | `false` | Open in read-only mode |
| `create_if_missing(bool)` | `true` | Create the file if it does not exist |
| `apply_default_pragmas(bool)` | `true` | Apply WAL + performance pragmas |
| `add_pragma(name, value)` | — | Add a custom SQLite pragma |
| `busy_timeout(Option<Duration>)` | `5 000 ms` | SQLite busy timeout |
| `with_embedding_model(model_id)` | `None` | Auto-embed via model (needs `embeddings` feature) |

#### Database Methods

```rust
// Execute DDL / multi-statement SQL
db.exec("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")?;

// Parameterised query — returns all rows
let result: QueryResult = db.query(
    "SELECT * FROM users WHERE name = ?",
    &[SqlValue::Text("Alice".into())]
)?;

// Prepared statement
let stmt = db.prepare("INSERT INTO users (name) VALUES (?)")?;
stmt.run(&[SqlValue::Text("Bob".into())])?;
let rows = stmt.all(&[])?;

// Transaction
db.transaction(|tx| {
    tx.execute("INSERT INTO users (name) VALUES (?)", ["Charlie"])?;
    Ok(())
})?;

// PRAGMA helper
let wal_info = db.pragma("journal_mode")?;
```

#### QueryResult

```rust
pub struct QueryResult {
    pub columns:          Vec<String>,
    pub rows:             Vec<Vec<SqlValue>>,
    pub changes:          u64,
    pub last_insert_rowid: i64,
}

// Convenience conversion
result.rows_as_maps();  // Vec<HashMap<String, SqlValue>>
result.rows_as_json();  // Vec<serde_json::Value>
```

#### SqlValue

```rust
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}
```

---

### VectorIndex

Usually accessed through `CrdtStore`.  Available directly for advanced use:

```rust
use pluresdb_core::VectorIndex;

let index = VectorIndex::new(1_000_000); // capacity
index.insert("node-1", &[0.1, 0.2, 0.3]);
let results: Vec<(String, f32)> = index.search(&[0.1, 0.2, 0.3], 10);
// (node_id, cosine_similarity_score)
```

---

### EmbedText trait

```rust
pub trait EmbedText: Send + Sync + std::fmt::Debug {
    fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>>;
    fn dimension(&self) -> usize;
}
```

Implement this trait to plug in any embedding backend.

**Built-in:** `FastEmbedder` (behind the `embeddings` cargo feature):

```toml
pluresdb-core = { version = "0.1", features = ["embeddings"] }
```

```rust
use pluresdb_core::FastEmbedder;
use std::sync::Arc;

let embedder = FastEmbedder::new("BAAI/bge-small-en-v1.5")?;
let store = CrdtStore::default().with_embedder(Arc::new(embedder));
```

---

## Rust API — `pluresdb-sync`

```toml
[dependencies]
pluresdb-sync = "0.1"
```

### SyncBroadcaster

An in-process Tokio broadcast hub for CRDT events.

```rust
use pluresdb_sync::{SyncBroadcaster, SyncEvent};

let hub = SyncBroadcaster::new(1024);  // or ::default() (capacity 1024)

// Subscribe before publishing (messages sent before subscribe are lost)
let mut rx = hub.subscribe();

hub.publish(SyncEvent::NodeUpsert { id: "node-1".into() })?;

let event = rx.recv().await?;
```

#### SyncEvent

```rust
pub enum SyncEvent {
    NodeUpsert       { id: String },
    NodeDelete       { id: String },
    PeerConnected    { peer_id: String },
    PeerDisconnected { peer_id: String },
}
```

---

### Transport trait

```rust
use pluresdb_sync::{Transport, TopicHash, PeerInfo};

#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self, topic: TopicHash)
        -> Result<mpsc::Receiver<Box<dyn Connection>>>;
    async fn announce(&mut self, topic: TopicHash) -> Result<()>;
    async fn lookup(&self, topic: TopicHash) -> Result<Vec<PeerInfo>>;
    async fn disconnect(&mut self) -> Result<()>;
    fn name(&self) -> &str;
}
```

#### TransportConfig

```rust
let config = TransportConfig {
    mode:      TransportMode::Hyperswarm,  // Hyperswarm | Relay | Disabled
    relay_url: Some("wss://relay.example.com".into()),
    timeout_ms: 30_000,
    encryption: true,
};
```

#### Topic derivation

```rust
use pluresdb_sync::derive_topic;

let topic: [u8; 32] = derive_topic("my-database-id");
```

---

## Node.js API (N-API bindings)

Install:

```bash
npm install @plures/pluresdb
```

### PluresDatabase

All methods are **synchronous** (no `async`/`await`).

```js
const { PluresDatabase } = require("@plures/pluresdb");

// In-memory store
const db = new PluresDatabase();

// With optional actor ID and file-backed SQLite
const db = new PluresDatabase("my-actor", "./data.db");
```

#### `PluresDatabase.newWithEmbeddings(model, actorId?)` _(factory)_

Creates a store with automatic text embedding.

```js
const db = PluresDatabase.newWithEmbeddings("BAAI/bge-small-en-v1.5");
```

Requires `pluresdb-node` compiled with the `embeddings` feature.

---

#### `put(id, data)` → `string`

```js
db.put("user:1", { name: "Alice", role: "admin" });
```

#### `get(id)` → `object | null`

```js
const user = db.get("user:1");
// { name: "Alice", role: "admin" }
```

#### `getWithMetadata(id)` → `object | null`

```js
const meta = db.getWithMetadata("user:1");
// { id, data, clock, timestamp }
```

#### `delete(id)` → `void`

```js
db.delete("user:1");
```

#### `list()` → `object[]`

```js
const all = db.list();
// [{ id, data, timestamp }, ...]
```

#### `listByType(type)` → `object[]`

```js
const users = db.listByType("user");
// Filters nodes where data.type === "user"
```

#### `search(query, limit?)` → `object[]`

Full-text substring search over serialised JSON values.

```js
const results = db.search("Alice", 5);
// [{ id, data, score, timestamp }, ...]
```

#### `vectorSearch(embedding, limit?, threshold?)` → `object[]`

Approximate nearest-neighbour search using a pre-computed embedding.

```js
const results = db.vectorSearch(queryEmbedding, 10, 0.3);
// [{ id, data, score, timestamp }, ...]
// score is cosine similarity in [0, 1]
```

#### `query(sql, params?)` → `object`

Executes a SQL `SELECT` statement (requires `db_path` in constructor).

```js
const result = db.query(
  "SELECT * FROM users WHERE name = ?",
  ["Alice"]
);
// { columns, rows, changes, lastInsertRowid }
```

#### `exec(sql)` → `object`

Executes a SQL DDL or DML statement (requires `db_path` in constructor).

```js
db.exec("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)");
// { changes, lastInsertRowid }
```

#### `stats()` → `object`

```js
const s = db.stats();
// { nodeCount, vectorIndexSize, actorId }
```

---

## Deno / JSR API

Install:

```bash
deno add @plures/pluresdb
```

The Deno package re-exports the TypeScript legacy API.  Key exports:

```typescript
import {
  PluresDB,       // Main database class
  PluresNode,     // Higher-level wrapper
  startApiServer, // Express/Oak REST server factory
  SQLiteCompatibleAPI,
} from "@plures/pluresdb";
```

See [TypeScript / Legacy API](#typescript--legacy-api) below for method details.

---

## TypeScript / Legacy API

Available in both Node.js (`npm install pluresdb`) and Deno (`jsr:@plures/pluresdb`).

### PluresDB (TypeScript class) {#pluresdatabase-typescript-class}

```typescript
import { PluresDB } from "pluresdb";

const db = new PluresDB();
await db.ready();
```

#### `db.ready()` → `Promise<void>`

Waits until the database is initialised.

#### `db.put(key, value)` → `Promise<string>`

```typescript
await db.put("user:alice", { name: "Alice" });
```

#### `db.get(key)` → `Promise<any>`

```typescript
const user = await db.get("user:alice");
```

#### `db.delete(key)` → `Promise<void>`

```typescript
await db.delete("user:alice");
```

#### `db.list(prefix?)` → `Promise<any[]>`

```typescript
const users = await db.list("user:");
```

#### `db.enableSync(options)` → `Promise<void>`

Enables Hyperswarm P2P synchronisation.

```typescript
const key = PluresDB.generateSyncKey();
await db.enableSync({ key });
```

#### `PluresDB.generateSyncKey()` → `string` _(static)_

Generates a cryptographically random 64-hex-character sync key.

#### `db.getSyncStats()` → `object`

```typescript
const stats = db.getSyncStats();
// { peersConnected, messagesSent, messagesReceived, syncKey }
```

#### `db.on(event, handler)`

```typescript
db.on("peer:connected",    (info) => console.log(info.peerId));
db.on("peer:disconnected", (info) => console.log(info.peerId));
```

#### `db.serve(options)` → `void`

Starts the built-in HTTP/WebSocket server.

```typescript
db.serve({ port: 34567 });
```

---

### SQLiteCompatibleAPI

A `Promise`-based API modelled on the [better-sqlite3](https://github.com/WiseLibs/better-sqlite3)
and [node-sqlite3](https://github.com/TryGhost/node-sqlite3) interfaces.

```typescript
import { SQLiteCompatibleAPI } from "pluresdb";

const db = new SQLiteCompatibleAPI({ config: { dataDir: "./data" } });
```

#### `db.exec(sql)` → `Promise<void>`

Executes one or more SQL statements.

#### `db.run(sql, params?)` → `Promise<{ changes, lastID }>`

Runs a parameterised statement.

#### `db.get(sql, params?)` → `Promise<object | undefined>`

Returns the first row.

#### `db.all(sql, params?)` → `Promise<object[]>`

Returns all rows.

#### `db.put(key, value)` → `Promise<void>`

Key-value shorthand.

#### `db.getValue(key)` → `Promise<any>`

Key-value shorthand.

#### `db.delete(key)` → `Promise<void>`

Key-value shorthand.

#### `db.vectorSearch(query, limit)` → `Promise<object[]>`

Semantic similarity search.

```typescript
const results = await db.vectorSearch("machine learning", 10);
```

---

### better-sqlite3 compat

A synchronous-style API that matches the
[better-sqlite3](https://github.com/WiseLibs/better-sqlite3) interface:

```typescript
import Database from "pluresdb/better-sqlite3";

const db = await new Database("./data.db", { autoStart: true }).open();

const insert = db.prepare("INSERT INTO users (name) VALUES (?)");
await insert.run("Ada Lovelace");

const select = db.prepare("SELECT * FROM users");
const users = await select.all();
```

---

## REST API

Start the server:

```bash
pluresdb serve --port 34569
# or via Node.js
npm start
```

All endpoints accept and return JSON.  Base URL: `http://localhost:34569`

### `POST /api/put`

Insert or update a node.

```bash
curl -X POST http://localhost:34569/api/put \
  -H "Content-Type: application/json" \
  -d '{"id": "user:1", "data": {"name": "Alice"}}'
```

Response: `{ "id": "user:1" }`

### `GET /api/get?id=<id>`

Retrieve a node.

```bash
curl http://localhost:34569/api/get?id=user:1
```

Response: `{ "id": "user:1", "data": { "name": "Alice" }, "timestamp": "..." }`

### `DELETE /api/delete?id=<id>`

Delete a node.

```bash
curl -X DELETE http://localhost:34569/api/delete?id=user:1
```

### `GET /api/list`

List all nodes.

```bash
curl http://localhost:34569/api/list
```

Response: `[{ "id": "...", "data": {...}, "timestamp": "..." }, ...]`

### `POST /api/search`

Text or vector search.

```bash
# Text search
curl -X POST http://localhost:34569/api/search \
  -H "Content-Type: application/json" \
  -d '{"query": "machine learning", "limit": 10}'

# Vector search (pre-computed embedding)
curl -X POST http://localhost:34569/api/search \
  -H "Content-Type: application/json" \
  -d '{"embedding": [0.1, 0.2, ...], "limit": 10, "threshold": 0.3}'
```

### `POST /api/query`

Execute a SQL query.

```bash
curl -X POST http://localhost:34569/api/query \
  -H "Content-Type: application/json" \
  -d '{"sql": "SELECT * FROM users WHERE name = ?", "params": ["Alice"]}'
```

### `GET /api/status`

Server and database health.

```bash
curl http://localhost:34569/api/status
```

---

## CLI Commands

Install the CLI:

```bash
cargo install pluresdb-cli
# or download a pre-built binary from GitHub Releases
```

### `pluresdb init [path]`

Initialise a new database at `path` (default: `./pluresdb-data`).

```bash
pluresdb init ./my-db
pluresdb init ./my-db --force   # overwrite existing
```

### `pluresdb serve`

Start the HTTP API server.

```bash
pluresdb serve --port 34569 --bind 127.0.0.1
pluresdb --data-dir ./my-db serve
```

### `pluresdb put <id> <json>`

Insert or update a node.

```bash
pluresdb put "user:1" '{"name": "Alice"}'
pluresdb put "doc:1"  @document.json        # read from file
```

### `pluresdb get <id>`

Retrieve a node.

```bash
pluresdb get "user:1"
```

### `pluresdb delete <id>`

Delete a node.

```bash
pluresdb delete "user:1"
```

### `pluresdb list`

List all nodes (outputs JSONL).

```bash
pluresdb list
pluresdb list | jq '.id'
```

### `pluresdb query <sql> [params...]`

Execute a SQL statement.

```bash
pluresdb query "SELECT * FROM users"
pluresdb query "SELECT * FROM users WHERE name = ?" "Alice"
```

### `pluresdb status`

Show database statistics.

```bash
pluresdb status
pluresdb status --detailed
```

### Global flags

| Flag | Description |
|---|---|
| `--data-dir <path>` | Path to data directory (default: in-memory) |
| `--verbose` / `-v` | Enable verbose logging |
| `--log-level <level>` | Log level: `error`, `warn`, `info`, `debug`, `trace` |
