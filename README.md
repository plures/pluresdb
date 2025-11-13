# PluresDB

[![npm version](https://badge.fury.io/js/pluresdb.svg)](https://badge.fury.io/js/pluresdb)
[![Deno version](https://img.shields.io/badge/deno-v2.x-blue)](https://deno.land)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](https://opensource.org/licenses/AGPL-3.0)

**P2P Graph Database with SQLite Compatibility** - A local-first, offline-first database for modern applications.

> 💡 **Perfect for Personal Use on Windows!** PluresDB is ideal for note-taking, knowledge management, personal wikis, task tracking, and more. [Get Started on Windows →](docs/WINDOWS_GETTING_STARTED.md)

## 🚀 Quick Start

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

## 🎯 Features

### Core Database

- **P2P Graph Database**: Distributed, peer-to-peer data storage
- **SQLite Compatibility**: 95% SQLite API compatibility for easy migration
- **CRDT Conflict Resolution**: Automatic conflict resolution for distributed data
- **Vector Search**: Built-in vector embeddings and similarity search
- **Local-First**: Offline-first data storage with sync when online

### P2P Ecosystem

- **Identity Management**: Public key infrastructure for peer identification
- **Encrypted Sharing**: End-to-end encrypted data sharing between peers
- **Cross-Device Sync**: Automatic synchronization across all your devices
- **Acceptance Policies**: Granular control over what data to accept from peers

### Developer Experience

- **TypeScript Support**: Full TypeScript definitions included
- **VSCode Integration**: Easy integration with VSCode extensions
- **Web UI**: Comprehensive 24-tab management interface
- **REST API**: Full REST API for web applications
- **WebSocket API**: Real-time updates and synchronization

## 📦 Installation Methods

### Windows (Personal Database Use)

**Recommended for Windows users who want a personal database:**

```powershell
# Option 1: Using winget (coming soon)
winget install plures.pluresdb

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

### P2P API

```typescript
// Identity management
await db.createIdentity({ name: "John", email: "john@example.com" });
await db.searchPeers("developer");

// Encrypted sharing
await db.shareNode(nodeId, targetPeerId, { accessLevel: "read-only" });
await db.acceptSharedNode(sharedNodeId);

// Cross-device sync
await db.addDevice({ name: "My Phone", type: "phone" });
await db.syncWithDevice(deviceId);
```

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

- **End-to-End Encryption**: All shared data is encrypted
- **Public Key Infrastructure**: Secure peer identification
- **Access Control**: Granular permissions and policies
- **Audit Trail**: Complete logging of all activities
- **Local-First**: Your data stays on your devices
- **Payload Sanitization**: Incoming records are scrubbed to neutralize prototype pollution and function injection attempts

## 🧪 Testing & Verification

PluresDB ships with a unified verification workflow that compiles the TypeScript entry points and runs every Deno test suite (integration, performance, security, and unit).

```powershell
npm run verify
```

The command executes `tsc -p tsconfig.json` followed by `deno test -A --unstable-kv`, ensuring shipping builds stay green.

## 📊 Performance

- **Vector Search**: Sub-millisecond similarity search
- **CRDT Sync**: Efficient conflict resolution
- **Local Storage**: Fast local operations
- **P2P Sync**: Optimized for bandwidth and latency
- **Memory Efficient**: Minimal memory footprint

## 🌍 Use Cases

### Personal Database & Knowledge Management 📝

PluresDB is **perfect for personal use on Windows** as a local-first database:

- **Digital Journal**: Daily logs, mood tracking, personal reflections
- **Note-taking System**: Organize notes with tags, relationships, and smart search
- **Personal Wiki**: Build your own knowledge base with linked concepts
- **Task Manager**: Track personal and work tasks with custom fields
- **Research Database**: Collect papers, articles, bookmarks with metadata
- **Contact Manager**: Store contacts with rich relationships
- **Recipe Collection**: Searchable recipes with ingredients and ratings
- **Password Vault**: Encrypted storage for sensitive information
- **Bookmark Manager**: Save and organize web links with AI-powered search

👉 **[Windows Getting Started Guide](docs/WINDOWS_GETTING_STARTED.md)** for personal database setup

### Application Development 🚀

- **VSCode Extensions**: Replace SQLite with P2P capabilities
- **Local-First Apps**: Offline-first applications
- **Collaborative Tools**: Real-time collaboration
- **IoT Applications**: Edge computing and sync
- **Research Projects**: Academic and research data
- **Personal Knowledge Management**: Personal wikis and notes

## 📚 Documentation

- [Installation Guide](packaging/INSTALLATION.md)
- [API Reference](docs/API.md)
- [VSCode Integration](examples/vscode-extension-integration.ts)
- [Migration Guide](docs/MIGRATION.md)
- [P2P Guide](docs/P2P.md)

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

**Ready to build the future of local-first applications?** 🚀

[Get Started](packaging/INSTALLATION.md) | [View Examples](examples/) | [GitHub Discussions](https://github.com/plures/pluresdb/discussions)
