# PluresDB

[![npm version](https://badge.fury.io/js/pluresdb.svg)](https://badge.fury.io/js/@plures/pluresdb)
[![crates.io](https://img.shields.io/crates/v/pluresdb-core.svg)](https://crates.io/crates/pluresdb-core)
[![Deno version](https://img.shields.io/badge/deno-v2.x-blue)](https://deno.land)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://opensource.org/licenses/AGPL-3.0)

**Local-First Graph Database with SQLite Compatibility**

PluresDB is a CRDT-based graph database that speaks SQLite. Built with Rust for performance and TypeScript for accessibility, it provides SQLite API compatibility while adding graph relationships, vector search, and P2P synchronization. Perfect for desktop applications, VSCode extensions, and personal knowledge management.

> 💡 **Ideal for Windows Desktop Apps**: Drop-in SQLite replacement with graph capabilities, vector search, and a comprehensive web UI. [Get Started on Windows →](docs/WINDOWS_GETTING_STARTED.md)

## 🚀 Quick Start

### Install

```bash
# Node.js / npm
npm install pluresdb

# Deno
deno add @plures/pluresdb

# Rust
cargo add pluresdb-core pluresdb-storage

# Windows (Winget)
winget install pluresdb.pluresdb

# Docker
docker pull pluresdb/pluresdb:latest
```

### Use It

```typescript
import { PluresNode, SQLiteCompatibleAPI } from "pluresdb";

// Start the database
const db = new PluresNode({
  config: { port: 34567, dataDir: "./data" },
  autoStart: true
});

// Use SQLite-compatible API
const sqlite = new SQLiteCompatibleAPI();

// Create tables
await sqlite.exec(`
  CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)
`);

// Insert data
await sqlite.run(
  "INSERT INTO users (name, email) VALUES (?, ?)", 
  ["Alice", "alice@example.com"]
);

// Query data
const users = await sqlite.all("SELECT * FROM users");

// Vector search
const results = await sqlite.vectorSearch("machine learning", 10);
```

## 🎯 What PluresDB Does

### Core Capabilities

- **SQLite Compatibility**: Drop in for SQLite with 95% API compatibility
- **Graph Relationships**: Store and query connected data with CRDT conflict resolution
- **Vector Search**: Semantic similarity search with HNSW indexing
- **Local-First**: Runs entirely on your machine, syncs when you want
- **P2P Sync**: Encrypted data sharing across devices without servers
- **Web UI**: 24-tab management interface for data exploration

### Built For

- **Desktop Applications**: Embedded database with graph and vector capabilities
- **VSCode Extensions**: SQLite-compatible storage with enhanced features
- **Knowledge Management**: Personal wikis, note-taking, research databases
- **Offline-First Apps**: Full functionality without network connectivity
- **Prototyping**: Quick database setup with TypeScript/Rust support

## 📖 How to Use PluresDB

### For Node.js Applications

```typescript
import { SQLiteCompatibleAPI } from "pluresdb";

const db = new SQLiteCompatibleAPI({
  config: { dataDir: "./data" }
});

// Use familiar SQLite methods
await db.exec("CREATE TABLE ...");
await db.run("INSERT INTO ...", params);
const rows = await db.all("SELECT ...");
```

### For Deno Applications

```typescript
import { GunDB, startApiServer } from "jsr:@plures/pluresdb";

const db = new GunDB();
await db.ready();

db.serve({ port: 34567 });
const api = startApiServer({ port: 8080, db });

await db.put("user:alice", { name: "Alice" });
const user = await db.get("user:alice");
```

### For Rust Applications

```rust
use pluresdb_core::{Database, DatabaseOptions};

let db = Database::open(
    DatabaseOptions::with_file("./data/plures.db")
        .create_if_missing(true)
)?;

db.put("user:1", json!({"name": "Alice"}))?;
let user = db.get("user:1")?;
```

### For VSCode Extensions

Replace SQLite in your VSCode extension with PluresDB:

```typescript
import { SQLiteCompatibleAPI } from "pluresdb";

export function activate(context: vscode.ExtensionContext) {
  const db = new SQLiteCompatibleAPI({
    config: {
      dataDir: path.join(context.globalStorageUri.fsPath, "pluresdb")
    }
  });

  // Same SQLite API, enhanced capabilities
  await db.exec("CREATE TABLE settings (key TEXT, value TEXT)");
  await db.run("INSERT INTO settings VALUES (?, ?)", ["theme", "dark"]);
}
```

## 🌐 Web Interface

PluresDB includes a comprehensive Svelte-based web UI at `http://localhost:34568`:

- **Data Explorer**: Browse, edit, and manage your data with JSON editing
- **Graph Visualization**: Interactive Cytoscape.js graph views
- **Vector Search UI**: Semantic search with similarity scoring
- **Type Management**: Define schemas and validate data
- **P2P Controls**: Manage peers, encryption, and cross-device sync
- **Performance Monitoring**: Real-time metrics and profiling
- **History & Time Travel**: Version history with diff and restore

## 🔄 P2P Sync Transport

PluresDB supports pluggable sync transports for different network environments:

| Transport | Environment | Port | Details |
|-----------|-------------|------|---------|
| **Auto** (default) | Any | Various | Automatic fallback: Direct → Azure → Vercel |
| **Azure Relay** | Corporate | 443 | WSS on port 443, looks like HTTPS |
| **Vercel Relay** | Corporate | 443 | Edge WebSocket, universally whitelisted |
| **Direct** | Home/Personal | Various | P2P via Hyperswarm (best performance) |

### Why Multiple Transports?

Corporate networks often block:
- UDP traffic (used by Hyperswarm)
- Non-standard ports
- Direct P2P connections

Azure/Vercel relay transports use **WebSocket on port 443** (standard HTTPS port), making them indistinguishable from normal web traffic.

### Configuration

```json
{
  "syncTransport": {
    "mode": "auto",
    "azureRelayUrl": "wss://pluresdb-relay.azurewebsites.net",
    "vercelRelayUrl": "wss://pluresdb-relay.vercel.app",
    "connectionTimeoutMs": 30000
  }
}
```

**Learn more**: [Sync Transport Documentation](docs/SYNC_TRANSPORT.md)

## 🔌 API Options

### SQLite-Compatible API

```typescript
// Database operations
await sqlite.exec(sql);                    // Execute SQL
await sqlite.run(sql, params);             // Run with parameters
await sqlite.get(sql, params);             // Get single row
await sqlite.all(sql, params);             // Get all rows

// Key-value operations
await sqlite.put(key, value);              // Store data
await sqlite.getValue(key);                // Retrieve data
await sqlite.delete(key);                  // Remove data

// Vector search
await sqlite.vectorSearch(query, limit);   // Semantic search
```

### better-sqlite3-Compatible API

For synchronous-style ergonomics:

```typescript
import Database from "pluresdb/better-sqlite3";

const db = await new Database("./data.db", { autoStart: true }).open();

const insert = db.prepare("INSERT INTO users (name) VALUES (?)");
await insert.run("Ada Lovelace");

const select = db.prepare("SELECT * FROM users");
const users = await select.all();
```

### REST API

```bash
# Create/update node
curl -X POST http://localhost:34567/api/put \
  -H "Content-Type: application/json" \
  -d '{"id": "user:1", "data": {"name": "Alice"}}'

# Retrieve node
curl http://localhost:34567/api/get?id=user:1

# Delete node
curl -X DELETE http://localhost:34567/api/delete?id=user:1

# List all nodes
curl http://localhost:34567/api/list

# Vector search
curl -X POST http://localhost:34567/api/search \
  -H "Content-Type: application/json" \
  -d '{"query": "machine learning", "limit": 10}'
```

### Local-First APIs

PluresDB provides production-ready Rust implementations for local-first integration:

**WASM (Browser)** - Rust implementation complete, use directly:
```javascript
// Via wasm-bindgen (compile from source)
import { PluresDBBrowser } from "./pluresdb-wasm/pkg";
const db = new PluresDBBrowser("my-app");
await db.init_persistence();
await db.put("user:1", { name: "Alice" });
```

**Tauri (Desktop Apps)**
```rust
#[tauri::command]
async fn db_put(state: State<'_, AppState>, id: String, data: Value) -> Result<String> {
    state.db.lock().put(id, data)
}
```

**IPC (Native Apps)**
```rust
let mut server = IPCServer::new("my-app", store)?;
server.start()?;
// Client connects via shared memory
```

See [Local-First Integration](docs/LOCAL_FIRST_INTEGRATION.md) for complete guides.

## 🐚 Shell Integration

PluresDB includes PowerShell and Bash modules for command history tracking and system integration:

### PowerShell Module

```powershell
# Install and initialize
Import-Module PluresDB
Initialize-PluresDBHistory

# Enable automatic history capture
Enable-PluresDBHistoryIntegration

# Query your command history
Get-PluresDBHistory -Last 10
Get-PluresDBCommandFrequency -Top 20
Get-PluresDBFailedCommands -Last 5
```

### Bash Module

```bash
# Source the module
source /path/to/pluresdb/modules/bash/pluresdb.sh

# Initialize database
pluresdb_init

# Enable automatic history capture
pluresdb_enable_integration

# Query your command history
pluresdb_history --last 10
pluresdb_frequency 20
pluresdb_failed 5
```

### Features

- **Automatic History Capture** - Track every command you run
- **Advanced Querying** - Search, filter, and analyze history
- **Deduplication** - View unique commands and frequency stats
- **Cross-Device Sync** - Access history from multiple machines via P2P
- **NixOS Integration** - Track `nixos-rebuild` and nix commands
- **WSL Support** - Track commands across Windows and Linux

See [Command Line Integration Guide](docs/COMMAND_LINE_INTEGRATION.md) for comprehensive documentation.

## 🗂️ Architecture

PluresDB is built as a Rust-first monorepo:

- **`crates/pluresdb-core`**: CRDT storage engine
- **`crates/pluresdb-storage`**: Storage backends (Sled, SQLite, RocksDB)
- **`crates/pluresdb-sync`**: P2P synchronization
- **`crates/pluresdb-cli`**: Command-line interface
- **`crates/pluresdb-wasm`**: WebAssembly bindings
- **`crates/pluresdb-ipc`**: IPC shared memory
- **`legacy/`**: TypeScript/Node.js/Deno implementations
- **`web/svelte/`**: Web UI components

## 📦 Distribution

PluresDB is available through multiple channels:

- **npm**: [`pluresdb`](https://www.npmjs.com/package/pluresdb) - Node.js package
- **JSR**: [`@plures/pluresdb`](https://jsr.io/@plures/pluresdb) - Deno module
- **crates.io**: Rust crates ([pluresdb-core](https://crates.io/crates/pluresdb-core), [pluresdb-storage](https://crates.io/crates/pluresdb-storage), [pluresdb-sync](https://crates.io/crates/pluresdb-sync))
- **Winget**: `pluresdb.pluresdb` - Windows package manager
- **Docker Hub**: [`pluresdb/pluresdb`](https://hub.docker.com/r/pluresdb/pluresdb) - Container images
- **GitHub Releases**: Pre-built binaries for Windows, macOS, Linux

## 🔒 Security

PluresDB implements comprehensive security measures:

- **Input Validation**: All user inputs are validated and sanitized
- **Prototype Pollution Protection**: Safe object handling
- **Audit Logging**: Complete operation logs
- **Local Storage**: Data stays on your machine by default
- **End-to-End Encryption**: Secure P2P synchronization
- **AGPL v3 License**: Ensures modifications remain open source

Report security issues privately via our [Security Policy](SECURITY.md).

## 📊 Performance

- **CRDT Operations**: Efficient conflict-free data structures in Rust
- **Vector Search**: HNSW-based similarity search
- **Storage Backends**: Sled, SQLite, or RocksDB
- **Local Operations**: ~5-10ms REST API latency
- **Zero Network**: Full functionality without internet

## 🧪 Testing

Run the comprehensive test suite:

```bash
npm run verify
```

This executes TypeScript compilation and all Deno test suites (unit, integration, performance, security).

## 📚 Documentation

- [Windows Getting Started Guide](docs/WINDOWS_GETTING_STARTED.md)
- [Command Line Integration](docs/COMMAND_LINE_INTEGRATION.md)
- [Local-First Integration](docs/LOCAL_FIRST_INTEGRATION.md)
- [VSCode Extension Example](examples/vscode-extension-integration.ts)
- [Contributing Guide](CONTRIBUTING.md)
- [CHANGELOG](CHANGELOG.md)

## 🤝 Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

All contributions are licensed under AGPL v3.

## 📄 License

GNU Affero General Public License v3.0 (AGPL v3). See [LICENSE](LICENSE) for details.

## 🆘 Support

- **Issues**: [GitHub Issues](https://github.com/plures/pluresdb/issues)
- **Discussions**: [GitHub Discussions](https://github.com/plures/pluresdb/discussions)
- **Security**: [Security Policy](SECURITY.md)

---

**Built with Rust and TypeScript** 🚀
