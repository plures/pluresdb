# PluresDB

[![npm version](https://badge.fury.io/js/pluresdb.svg)](https://badge.fury.io/js/@plures/pluresdb)
[![crates.io](https://img.shields.io/crates/v/pluresdb-core.svg)](https://crates.io/crates/pluresdb-core)
[![Deno version](https://img.shields.io/badge/deno-v2.x-blue)](https://deno.land)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://opensource.org/licenses/AGPL-3.0)

**Graph Database with SQLite Compatibility** - CRDT-based graph storage with vector search and TypeScript/Rust support.

> 💡 **Perfect for Personal Use on Windows!** SQLite-compatible database with graph capabilities, vector search, and a comprehensive web UI. [Get Started on Windows →](docs/WINDOWS_GETTING_STARTED.md)

## 🚀 Quick Start

### Release channels (current)

- **Winget:** Published as `pluresdb.pluresdb` ([manifest](https://github.com/plures/pluresdb/blob/main/packaging/winget/pluresdb.yaml)) for Windows installs
- **npm:** [`pluresdb`](https://www.npmjs.com/package/pluresdb) (Node.js + better-sqlite3 compatibility)
- **crates.io:** [`pluresdb-core`](https://crates.io/crates/pluresdb-core), [`pluresdb-storage`](https://crates.io/crates/pluresdb-storage), [`pluresdb-sync`](https://crates.io/crates/pluresdb-sync), [`pluresdb-cli`](https://crates.io/crates/pluresdb-cli) (Rust crates)
- **JSR:** [`@plures/pluresdb`](https://jsr.io/@plures/pluresdb) (Deno module)
- **Docker Hub:** [`pluresdb/pluresdb`](https://hub.docker.com/r/pluresdb/pluresdb) (containerized deployment)
- **GitHub Releases:** Pre-built binaries for Windows, macOS, and Linux

For version-specific information and release notes, see the [CHANGELOG](CHANGELOG.md).

### Install

```bash
# npm
npm install pluresdb

# yarn
yarn add pluresdb

# pnpm
pnpm add pluresdb

# Deno (JSR)
deno add @plures/pluresdb

# Rust (Cargo)
cargo add pluresdb-core pluresdb-storage pluresdb-sync

# Docker
docker pull pluresdb/pluresdb:latest

# Windows (Winget)
winget install pluresdb.pluresdb
```

### Development Prerequisites (Rust Components)

If you plan to build or test the Rust crates in this repository, make sure `libclang` is available so
bindgen-based dependencies (like `zstd-sys`) can compile. On Windows you can automate this setup with:

```powershell
pwsh ./scripts/setup-libclang.ps1 -ConfigureCurrentProcess
```

The script will detect or install LLVM (via `winget`/`choco`), set the `LIBCLANG_PATH` environment variable,
and update the current session so that `cargo build` / `cargo test` can run without manual configuration.
Restart your terminal if you omit the `-ConfigureCurrentProcess` flag.

## 📦 Packaging Artifacts

Generate Windows, MSI, Deno, and Nix release bundles with the helper script:

```powershell
pwsh ./packaging/scripts/build-packages.ps1
```

The script automatically reads the release version from `Cargo.toml`. Override it for pre-release cuts if needed:

```powershell
pwsh ./packaging/scripts/build-packages.ps1 -Version 1.1.0-rc1
```

### Basic Usage

```typescript
import { PluresNode, SQLiteCompatibleAPI } from "pluresdb";

// Start the database
const db = new PluresNode({
  config: {
    port: 34567,
    host: "localhost",
    dataDir: "./data",
  },
  autoStart: true,
});

// Use SQLite-compatible API
const sqlite = new SQLiteCompatibleAPI();

// Create tables
await sqlite.exec(`
  CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT,
    email TEXT
  )
`);

// Insert data
await sqlite.run("INSERT INTO users (name, email) VALUES (?, ?)", [
  "John",
  "john@example.com",
]);

// Query data
const users = await sqlite.all("SELECT * FROM users");

// Vector search
const results = await sqlite.vectorSearch("machine learning", 10);
```

#### In Deno

```ts
import { GunDB, startApiServer } from "jsr:@plures/pluresdb";

const db = new GunDB();
await db.ready();

// start the mesh listener and optional HTTP API
db.serve({ port: 34567 });
const api = startApiServer({ port: 8080, db });

await db.put("user:alice", { name: "Alice", email: "alice@example.com" });
const record = await db.get("user:alice");
console.log(record);

// remember to close when the process exits
await db.close();
api.close();
```

#### Network Mode

PluresDB operates in network mode using HTTP REST API:

```typescript
import { PluresDBLocalFirst } from "@plures/pluresdb/local-first";

// Network mode provides backward compatibility
const db = new PluresDBLocalFirst({ mode: "network", port: 34567 });

await db.put("user:1", { name: "Alice", email: "alice@example.com" });
const user = await db.get("user:1");
const results = await db.vectorSearch("Find users in London", 10);
```

> **Note**: Local-first modes (WASM, IPC, Tauri) are implemented in Rust but TypeScript integration is in progress. See [Local-First Integration Status](docs/LOCAL_FIRST_INTEGRATION.md) for details.

## 🎯 Features

### Core Database

- **Graph Database**: CRDT-based graph storage with conflict resolution
- **SQLite Compatibility**: 95% SQLite API compatibility for easy migration  
- **Vector Search**: Built-in vector embeddings and similarity search
- **REST API**: HTTP API for CRUD operations and queries
- **TypeScript/JavaScript**: Full Node.js and Deno support

### Web Interface

- **24-Tab Management UI**: Comprehensive Svelte-based web interface
- **Data Explorer**: Browse and edit nodes with JSON editing
- **Graph Visualization**: Interactive Cytoscape.js graph views
- **Vector Search UI**: Semantic search with similarity scoring
- **Real-Time Updates**: Server-Sent Events for live data changes

### Developer Experience

- **TypeScript Definitions**: Complete type safety
- **VSCode Integration**: SQLite-compatible API for extensions
- **Rust Core**: High-performance Rust implementation
- **Multiple Runtimes**: Node.js, Deno, and Rust bindings
- **Docker Support**: Containerized deployment available

## 📦 Installation Methods

### Windows (Personal Database Use)

**Recommended for Windows users who want a personal database (`pluresdb.pluresdb`):**

```powershell
# Option 1: Using winget
winget install pluresdb.pluresdb

# Option 2: Using PowerShell installer
irm https://raw.githubusercontent.com/plures/pluresdb/main/install.ps1 | iex

# Option 3: Download ZIP from releases
# Extract and run start.bat
```

📖 **[Complete Windows Getting Started Guide →](docs/WINDOWS_GETTING_STARTED.md)**

### Package Managers

```bash
# macOS
brew install plures/pluresdb/pluresdb

# Linux (NixOS)
nix-env -iA nixpkgs.pluresdb

# Universal install script
curl -fsSL https://raw.githubusercontent.com/plures/pluresdb/main/install.sh | bash
```

### Docker

```bash
docker pull plures/pluresdb:latest
docker run -p 34567:34567 -p 34568:34568 plures/pluresdb:latest
```

## 🔧 VSCode Extension Integration

Perfect for VSCode extensions that currently use SQLite:

```typescript
import { SQLiteCompatibleAPI } from "pluresdb";

export function activate(context: vscode.ExtensionContext) {
  // Replace your SQLite database with PluresDB
  const db = new SQLiteCompatibleAPI({
    config: {
      dataDir: path.join(context.globalStorageUri.fsPath, "pluresdb"),
    },
  });

  // Use the same SQLite API you're familiar with
  await db.exec("CREATE TABLE settings (key TEXT, value TEXT)");
  await db.run("INSERT INTO settings VALUES (?, ?)", ["theme", "dark"]);
  const settings = await db.all("SELECT * FROM settings");
}
```

## 🌐 Web UI

Access the comprehensive web interface at `http://localhost:34568`:

- **Data Explorer**: Browse and edit your data
- **Graph Visualization**: Interactive graph view of relationships
- **Vector Search**: Semantic search across your data
- **P2P Management**: Manage peers and encrypted sharing
- **Performance Monitoring**: Real-time performance metrics
- **Settings**: Configure database and P2P settings

## 🔌 API Reference

### SQLite-Compatible API

```typescript
// Database operations
await sqlite.exec(sql); // Execute SQL
await sqlite.run(sql, params); // Run SQL with parameters
await sqlite.get(sql, params); // Get single row
await sqlite.all(sql, params); // Get all rows

// Key-value operations
await sqlite.put(key, value); // Store key-value pair
await sqlite.getValue(key); // Get value by key
await sqlite.delete(key); // Delete key

// Vector search
await sqlite.vectorSearch(query, limit); // Semantic search
```

### better-sqlite3-Compatible API

Need the synchronous ergonomics of [`better-sqlite3`](https://github.com/WiseLibs/better-sqlite3)?
The Node package now ships a compatibility layer that mirrors its familiar
`Database`/`Statement` workflow while proxying calls to PluresDB.

> **Note:** PluresDB operations run through HTTP and therefore return Promises.
> You can still keep the same control flow by awaiting each call.

```typescript
import Database from "pluresdb/better-sqlite3";

const db = await new Database("./data.db", { autoStart: true }).open();
await db.exec(
  "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)",
);

const insert = db.prepare("INSERT INTO users (name) VALUES (?)");
await insert.run("Ada Lovelace");

const select = db.prepare("SELECT id, name FROM users ORDER BY id");
const people = await select.all();
```

Statements support `run`, `get`, `all`, `iterate`, and common mode toggles like
`.raw()`, `.pluck()`, and `.expand(true)` for dotted column names.

```typescript
const singleColumnValues = await select.pluck().all();
const nestedRows = await select.expand().all();
```

Transaction helpers mirror `better-sqlite3` as well:

```typescript
const write = db.transaction(async (users) => {
  for (const user of users) {
    await insert.run(user.name);
  }
});

await write([{ name: "Grace Hopper" }, { name: "Margaret Hamilton" }]);
```

### REST API

```typescript
// Via HTTP client
const response = await fetch("http://localhost:34567/api/put", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ id: "user:1", data: { name: "Alice" } })
});

// Get node
const node = await fetch("http://localhost:34567/api/get?id=user:1")
  .then(r => r.json());

// List all nodes
const nodes = await fetch("http://localhost:34567/api/list")
  .then(r => r.json());
```

## 🗂️ Repository Structure

PluresDB is a Rust-first monorepo with TypeScript/JavaScript bindings:

- **`crates/`**: Rust core implementation
  - `pluresdb-core`: CRDT storage engine
  - `pluresdb-storage`: Storage backends (Sled, SQLite, RocksDB)
  - `pluresdb-sync`: P2P synchronization (in development)
  - `pluresdb-cli`: Command-line interface
  - `pluresdb-wasm`: WebAssembly bindings (Rust complete, TS integration pending)
  - `pluresdb-ipc`: IPC shared memory (Rust complete, TS integration pending)
- **`legacy/`**: TypeScript/JavaScript source
  - Node.js and Deno API implementations
  - HTTP/WebSocket server
  - better-sqlite3 compatibility layer
- **`web/svelte/`**: Web UI components
- **`packaging/`**: Distribution packages (MSI, winget, Docker)

## 🚀 Migration from SQLite

Migrating from SQLite is straightforward:

1. **Install PluresDB**: `npm install pluresdb`
2. **Replace imports**: Change `sqlite3` to `pluresdb`
3. **Update initialization**: Use `SQLiteCompatibleAPI` instead of `sqlite3.Database`
4. **Keep your queries**: All SQL queries work the same way

```typescript
// Before (SQLite)
import sqlite3 from "sqlite3";
const db = new sqlite3.Database("./data.db");

// After (PluresDB)
import { SQLiteCompatibleAPI } from "pluresdb";
const db = new SQLiteCompatibleAPI();
```

## 🔒 Security

- **Input Validation**: All user inputs are validated
- **Payload Sanitization**: Protection against prototype pollution and injection attacks
- **AGPL v3 License**: Ensures modifications remain open source
- **Audit Trail**: Complete logging of database operations
- **Local Storage**: Data stored locally with optional network sync

For security issues, see our [Security Policy](SECURITY.md).

## 🧪 Testing & Verification

PluresDB ships with a unified verification workflow that compiles the TypeScript entry points and runs every Deno test suite (integration, performance, security, and unit).

```powershell
npm run verify
```

The command executes `tsc -p tsconfig.json` followed by `deno test -A --unstable-kv`, ensuring shipping builds stay green.

### Azure Relay Testing

For testing P2P relay functionality in a cloud environment, PluresDB includes Azure infrastructure automation:

```bash
# Deploy test environment with 3 nodes
cd azure/scripts
./deploy.sh --environment test --node-count 3

# Run relay tests
npm run test:azure:relay

# Clean up
./destroy.sh --environment test
```

See [Azure Testing Guide](azure/README.md) and [Quick Start](azure/QUICKSTART.md) for detailed instructions.

## 📊 Performance

- **Vector Search**: HNSW-based similarity search in Rust
- **CRDT Operations**: Efficient conflict-free data structures
- **Multiple Backends**: Sled, SQLite, RocksDB storage options
- **HTTP REST API**: ~5-10ms latency for local operations
- **Rust Core**: High-performance implementation with TypeScript bindings

## 🌍 Use Cases

### Personal Database & Knowledge Management

PluresDB works well for personal Windows applications:

- **Note-taking**: Organize notes with graph relationships and vector search
- **Knowledge Base**: Build interconnected wikis with semantic search
- **Task Tracking**: Store tasks with custom fields and queries
- **Research Data**: Collect and search research materials
- **Bookmark Manager**: Save and organize links with full-text search

See [Windows Getting Started Guide](docs/WINDOWS_GETTING_STARTED.md) for setup.

### Application Development

- **VSCode Extensions**: SQLite-compatible API for extension storage
- **Desktop Applications**: Embedded database with web UI
- **Data Tools**: Graph data structures with REST API access
- **Prototyping**: Quick database setup with TypeScript/Rust support

## 📚 Documentation

- [Windows Getting Started](docs/WINDOWS_GETTING_STARTED.md)
- [Local-First Integration Status](docs/LOCAL_FIRST_INTEGRATION.md)
- [VSCode Extension Example](examples/vscode-extension-integration.ts)
- [CHANGELOG](CHANGELOG.md)
- [Contributing Guide](CONTRIBUTING.md)

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

**Note**: By contributing to PluresDB, you agree that your contributions will be licensed under the AGPL v3 license.

## 📄 License

This project is licensed under the GNU Affero General Public License v3.0 (AGPL v3). This ensures that all modifications to PluresDB remain open source. See [LICENSE](LICENSE) for details.

## 🆘 Support

- **Issues**: [GitHub Issues](https://github.com/plures/pluresdb/issues)
- **Discussions**: [GitHub Discussions](https://github.com/plures/pluresdb/discussions)

For security issues, please see our [Security Policy](SECURITY.md).

## 🙏 Acknowledgments

- Built with [Deno](https://deno.land/)
- Inspired by [Gun.js](https://gun.eco/)
- Web UI built with [Svelte](https://svelte.dev/)
- Vector search powered by [HNSW](https://github.com/nmslib/hnswlib)

---

**Built with Rust and TypeScript** 🚀

[Windows Setup](docs/WINDOWS_GETTING_STARTED.md) | [Examples](examples/) | [Contributing](CONTRIBUTING.md)
