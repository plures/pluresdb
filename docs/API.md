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
3. [Rust API — `pluresdb-storage` (Encryption)](#rust-api--pluresdb-storage-encryption)
   - [EncryptionConfig](#encryptionconfig)
   - [EncryptionMetadata](#encryptionmetadata)
4. [Unified `pluresdb` crate](#unified-pluresdb-crate)
5. [Node.js API (N-API bindings)](#nodejs-api-n-api-bindings)
   - [PluresDatabase](#pluresdatabase)
6. [Deno / JSR API](#deno--jsr-api)
7. [TypeScript / Legacy API](#typescript--legacy-api)
   - [PluresDB class](#pluresdatabase-typescript-class)
   - [SQLiteCompatibleAPI](#sqlitecompatibleapi)
   - [better-sqlite3 compat](#better-sqlite3-compat)
8. [REST API](#rest-api)
9. [CLI Commands](#cli-commands)

---

## Rust API — `pluresdb-core`

Add to `Cargo.toml`:

```toml
[dependencies]
pluresdb-core = "0.1"
```

### CrdtStore

A conflict-free replicated store backed by a concurrent `DashMap` for in-session
writes, with optional SQLite persistence for durable storage.  When persistence
is attached via `with_persistence`, read operations (`get`, `list`) query SQLite
directly — no records are loaded into memory at startup, enabling zero-cost
initialisation regardless of database size.

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

Inserts or updates a node using CRDT semantics.  The node is stored **immediately** — `put()` never blocks on embedding inference.  If an `EmbedText` backend is attached (via `with_embedder`) and the data contains extractable text, an [`EmbeddingTask`] is enqueued for the background worker started by [`spawn_embedding_worker`].  The vector index is updated eventually once the worker processes the task.

##### `spawn_embedding_worker`

```rust
pub fn spawn_embedding_worker(store: Arc<CrdtStore>) -> std::thread::JoinHandle<()>
```

Starts a background OS thread that drains the embedding task queue.  Must be called after wrapping the store in an `Arc` and before issuing writes that need auto-embedding.  The thread shuts down automatically when the last `Arc<CrdtStore>` is dropped.

##### `embedding_worker_stats`

```rust
pub fn embedding_worker_stats(&self) -> EmbeddingWorkerStats
```

Returns an observability snapshot: `queue_depth`, `last_processed` timestamp, and `dropped_tasks` counter.

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

## Rust API — `pluresdb-storage` (Encryption)

```toml
[dependencies]
pluresdb-storage = "0.1"
```

PluresDB provides **AES-256-GCM** encryption for all data at rest.  Keys are
derived from passwords using **Argon2id**.  Each encrypted blob stores a fresh
random nonce, so repeated writes of the same value produce different
ciphertexts.

### EncryptionConfig

```rust
use pluresdb_storage::EncryptionConfig;
```

#### Constructors

| Method | Description |
|---|---|
| `EncryptionConfig::new()` | Generate a random 256-bit master key |
| `EncryptionConfig::from_password(password)` | Derive key from password with a random salt |
| `EncryptionConfig::from_password_with_salt(password, salt)` | Deterministic key derivation (for reopening a database) |

```rust
// Random key (new database)
let enc = EncryptionConfig::new()?;

// Password-derived key (user-supplied passphrase)
let enc = EncryptionConfig::from_password("correct-horse-battery-staple")?;
```

#### Methods

```rust
/// Encrypt data using AES-256-GCM.  Returns nonce || ciphertext.
pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>

/// Decrypt a blob previously produced by encrypt().
pub fn decrypt(&self, ciphertext_with_nonce: &[u8]) -> Result<Vec<u8>>

/// Returns whether encryption is enabled.
pub fn is_enabled(&self) -> bool

/// Rotate to a new password (generates a new salt).
pub fn rotate_key(&mut self, new_password: &str) -> Result<()>

/// Returns the 16-byte Argon2id salt.
pub fn salt(&self) -> &[u8; 16]

/// Disable encryption (for testing only — default config uses zero-filled key).
pub fn disable(&mut self)
```

#### `Default` — disabled config

`EncryptionConfig::default()` creates a **disabled** config with a zero-filled
key.  It must never be used for real encryption; it exists for compatibility
and unit testing.

```rust
let mut config = EncryptionConfig::default(); // encryption disabled
config.disable(); // no-op but explicit
assert!(!config.is_enabled());
```

---

### EncryptionMetadata

Stores encryption scheme information (KDF, cipher, salt, revoked devices)
alongside the database directory.

```rust
use pluresdb_storage::{EncryptionConfig, EncryptionMetadata};
use std::path::Path;

let enc   = EncryptionConfig::from_password("my-passphrase")?;
let meta  = EncryptionMetadata::from_config(&enc);

// Persist so the database can be re-opened with the same passphrase
meta.save(Path::new("./db/encryption.json"))?;

// Later — re-open with deterministic key derivation
let loaded  = EncryptionMetadata::load(Path::new("./db/encryption.json"))?;
let salt    = loaded.salt_bytes()?;
let enc2    = EncryptionConfig::from_password_with_salt("my-passphrase", &salt)?;
```

#### Fields

```rust
pub struct EncryptionMetadata {
    pub version:         u32,    // Scheme version (currently 1)
    pub kdf:             String, // "argon2id"
    pub cipher:          String, // "aes-256-gcm"
    pub salt:            String, // Argon2id salt, base64-encoded
    pub revoked_devices: Vec<String>,
}
```

#### Methods

```rust
/// Create metadata from an EncryptionConfig.
pub fn from_config(config: &EncryptionConfig) -> Self

/// Load metadata from a JSON file.
pub fn load(path: &Path) -> Result<Self>

/// Save metadata to a JSON file.
pub fn save(&self, path: &Path) -> Result<()>

/// Add a device ID to the revocation list.
pub fn revoke_device(&mut self, device_id: String)

/// Check whether a device has been revoked.
pub fn is_device_revoked(&self, device_id: &str) -> bool

/// Decode and return the salt as raw bytes.
pub fn salt_bytes(&self) -> Result<Vec<u8>>
```

#### Key rotation example

```rust
use pluresdb_storage::{EncryptionConfig, EncryptionMetadata};
use std::path::Path;

fn rotate_database_key(
    db_path: &Path,
    old_password: &str,
    new_password: &str,
) -> anyhow::Result<()> {
    // Load existing salt
    let meta = EncryptionMetadata::load(&db_path.join("encryption.json"))?;
    let salt = meta.salt_bytes()?;

    // Re-derive with old password, then rotate
    let mut enc = EncryptionConfig::from_password_with_salt(old_password, &salt)?;
    enc.rotate_key(new_password)?;

    // Persist updated metadata (new salt)
    let new_meta = EncryptionMetadata::from_config(&enc);
    new_meta.save(&db_path.join("encryption.json"))?;

    Ok(())
}
```

> **Note:** Storage-level encryption integration with `SledStorage` via a
> dedicated `StorageEngine::with_encryption` API is on the roadmap.  See
> `docs/ROADMAP.md` for the current status.  The encryption primitives above
> are production-ready and can be used directly for custom storage backends.

---

## Unified `pluresdb` crate

The `pluresdb` umbrella crate re-exports the most commonly used types from
all sub-crates so you only need one dependency in your `Cargo.toml`.

```toml
[dependencies]
pluresdb = "0.1"

# Optional: SQLite compatibility layer
pluresdb = { version = "0.1", features = ["sqlite-compat"] }

# Optional: on-device embedding (fastembed)
pluresdb = { version = "0.1", features = ["embeddings"] }
```

### Re-exports

| Symbol | Origin crate |
|---|---|
| `CrdtStore`, `CrdtOperation`, `NodeData`, `NodeId`, `NodeRecord` | `pluresdb-core` |
| `VectorClock`, `VectorIndex`, `VectorSearchResult`, `ActorId` | `pluresdb-core` |
| `EmbedText`, `NoOpPlugin`, `PluresLmPlugin` | `pluresdb-core` |
| `FastEmbedder` _(feature = "embeddings")_ | `pluresdb-core` |
| `Database`, `DatabaseOptions`, `QueryResult`, `SqlValue` _(feature = "sqlite-compat")_ | `pluresdb-core` |
| `StorageEngine`, `StoredNode`, `MemoryStorage`, `SledStorage` | `pluresdb-storage` |
| `EncryptionConfig`, `EncryptionMetadata` | `pluresdb-storage` |
| `WriteAheadLog`, `WalEntry`, `WalOperation`, `ReplayStats` | `pluresdb-storage` |
| `SyncBroadcaster`, `SyncEvent`, `GunRelayServer` | `pluresdb-sync` |
| `CoreError` | `pluresdb-core` |
| `DatabaseError` _(feature = "sqlite-compat")_ | `pluresdb-core` |

### Convenience functions

```rust
use pluresdb::{new_memory_database, new_persistent_database};

// In-memory database (no persistence — useful for tests)
let (store, storage) = new_memory_database();

// File-backed database (persists to disk via Sled)
let (store, storage) = new_persistent_database("./my-db")?;
```

### Attaching a plugin

```rust
use pluresdb::{CrdtStore, NoOpPlugin, PluresLmPlugin, NodeId, NodeData};
use std::sync::Arc;

struct MyPlugin;

impl PluresLmPlugin for MyPlugin {
    fn plugin_id(&self) -> &str { "my-plugin" }
    fn on_node_written(&self, id: &NodeId, data: &NodeData) {
        println!("Written: {id}");
    }
    fn on_node_deleted(&self, id: &NodeId) {
        println!("Deleted: {id}");
    }
}

let store = CrdtStore::default().with_lm_plugin(Arc::new(MyPlugin));
```

---

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

#### `buildVectorIndex()` → `number`

Builds the HNSW approximate-nearest-neighbour index from the embeddings
currently stored in the node records.  Call this once after opening a
persistent database to enable vector search without blocking startup.

Returns the number of nodes that were indexed.

```js
const db = new PluresDatabase("actor", "./data.db");
const indexed = db.buildVectorIndex();
console.log(`Indexed ${indexed} nodes for vector search`);
```

> If you used `newWithEmbeddings()` the index is built incrementally via the
> background worker; explicit calls to `buildVectorIndex()` are only needed
> when embeddings were pre-computed and stored externally.

#### `embed(texts)` → `number[][]`

Embeds one or more strings using the model configured in `newWithEmbeddings`.
Each inner array is a float64 vector suitable for `putWithEmbedding()` or
`vectorSearch()`.

```js
const db = PluresDatabase.newWithEmbeddings("BAAI/bge-small-en-v1.5");
const [[vec]] = db.embed(["hello world"]);
// vec is a float64[] of length 384 (model-dependent)
```

Only available when the database was created via `newWithEmbeddings()`.

#### `embeddingDimension()` → `number | null`

Returns the configured embedding dimension, or `null` if no embedder is
attached.

```js
const dim = db.embeddingDimension(); // e.g. 384
```

#### `getActorId()` → `string`

Returns the actor ID for this database instance.

```js
const actorId = db.getActorId();
// "my-actor"
```

#### `subscribe()` → `string`

Subscribes to the internal CRDT broadcast channel and returns a subscription
identifier.  Full async event-streaming support (returning change events to
JavaScript callbacks) is planned; the current implementation registers the
internal receiver and returns a placeholder ID.

```js
const subId = db.subscribe();
// "subscription-1"
```

#### `execDsl(query)` → `object`

Executes a PluresDB DSL query string against the CRDT store.  Returns a
result object with a `nodes` array and optionally `aggregate` or `mutated`
fields.

**DSL syntax overview**

Pipe stages are separated by `|>`:

| Stage | Example |
|---|---|
| `filter(expr)` | `filter(category == "decision")` |
| `sort(by: field, dir: "asc"\|"desc")` | `sort(by: "score", dir: "desc")` |
| `limit(n)` | `limit(10)` |
| `skip(n)` | `skip(5)` |
| `project(fields...)` | `project(id, title, score)` |

```js
const result = db.execDsl(
  'filter(category == "decision") |> sort(by: "score", dir: "desc") |> limit(10)'
);
console.log(result.nodes);
// [{ id, data, ... }, ...]
```

#### `execIr(steps)` → `object`

Executes a JSON IR query (the machine-readable representation produced by the
DSL parser or the `pluresdb-procedures` builder).  `steps` must be a JSON
array of step objects.

```js
const result = db.execIr([
  { op: "filter", predicate: { field: "category", cmp: "==", value: "decision" } },
  { op: "sort",   by: "score", dir: "desc" },
  { op: "limit",  n: 5 }
]);
console.log(result.nodes);
```

---

## Deno / JSR API

Install:

```bash
deno add @plures/pluresdb
```

Or use the HTTPS import directly:

```typescript
import { PluresDB } from "https://deno.land/x/pluresdb/mod.ts";
```

The Deno package (`@plures/pluresdb` / `jsr:@plures/pluresdb`) re-exports the
full TypeScript legacy API via `mod.ts → legacy/index.ts`.

### Key exports

```typescript
import {
  PluresDB,              // Main database class (see TypeScript / Legacy API)
  PluresNode,            // Higher-level typed node wrapper
  startApiServer,        // Factory for the Express/Oak REST server
  SQLiteCompatibleAPI,   // Promise-based SQLite-compatible API
  generateSyncKey,       // Cryptographically random 64-hex sync key
} from "@plures/pluresdb";
```

### Deno-specific usage

```typescript
import { PluresDB } from "@plures/pluresdb";

const db = new PluresDB();
await db.ready();

// Persist data — works offline with no network dependency
await db.put("config:theme", { value: "dark" });
const theme = await db.get("config:theme");
console.log(theme); // { value: "dark" }

// P2P sync — auto-selects Hyperswarm or Relay transport
const key = PluresDB.generateSyncKey();
await db.enableSync({ key });
console.log(`Sharing key: ${key}`);

await db.close();
```

### Local-first integration (Deno / Node.js)

Use `PluresDBLocalFirst` for runtime auto-detection (browser WASM, Tauri,
IPC, or HTTP fallback):

```typescript
import { PluresDBLocalFirst } from "@plures/pluresdb/local-first";

// Automatically selects the fastest available backend
const db = new PluresDBLocalFirst({ mode: "auto" });
console.log(`Running in ${db.getMode()} mode`);

await db.put("note:1", { text: "Hello" });
const note = await db.get("note:1");
await db.close();
```

See [TypeScript / Legacy API](#typescript--legacy-api) for full method details.

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

### `pluresdb migrate-from-sqlite <sqlite-path>`

Migrate an existing SQLite database file into PluresDB.  Each SQLite row is
converted to a CRDT node keyed by `<table>:<rowid>`.

```bash
pluresdb migrate-from-sqlite ./legacy.db --data-dir ./pluresdb-data
pluresdb migrate-from-sqlite ./legacy.db --data-dir ./pluresdb-data --dry-run
```

**Options**

| Flag | Description |
|---|---|
| `--data-dir <path>` | Target PluresDB data directory (created if absent) |
| `--dry-run` | Print what would be migrated without writing |
| `--table <name>` | Migrate only the named table (repeatable) |
| `--batch-size <n>` | Rows per CRDT write batch (default: 500) |

**Output**

```
Migrating ./legacy.db → ./pluresdb-data
  Table "users"   : 1,234 rows → CRDT nodes  (key prefix: users:)
  Table "posts"   : 5,678 rows → CRDT nodes  (key prefix: posts:)
  Total           : 6,912 nodes written
Migration complete.
```

### Global flags

| Flag | Description |
|---|---|
| `--data-dir <path>` | Path to data directory (default: in-memory) |
| `--verbose` / `-v` | Enable verbose logging |
| `--log-level <level>` | Log level: `error`, `warn`, `info`, `debug`, `trace` |
