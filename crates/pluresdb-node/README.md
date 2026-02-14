# @plures/pluresdb-native

Native Node.js bindings for PluresDB - a P2P Graph Database with SQLite compatibility.

[![npm version](https://badge.fury.io/js/@plures%2Fpluresdb-native.svg)](https://www.npmjs.com/package/@plures/pluresdb-native)
[![License](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](../../LICENSE)

This package provides high-performance N-API bindings to the PluresDB Rust core, enabling native database operations directly from Node.js.

## ✅ Features

- **CRUD Operations** - Create, read, update, and delete nodes with CRDT synchronization
- **SQL Support** - Execute SQL queries with parameterized statements (95% SQLite compatible)
- **BLOB Support** - Store binary data including embedding vectors
- **Search** - Text-based and vector similarity search  
- **Type Filtering** - Filter nodes by type
- **Subscriptions** - Subscribe to database changes
- **Native Performance** - Rust-powered N-API bindings for maximum speed

## Installation

```bash
npm install @plures/pluresdb-native
```

Pre-built binaries are available for:
- **Linux**: x86_64, aarch64 (GNU libc)
- **macOS**: x86_64 (Intel), aarch64 (Apple Silicon)  
- **Windows**: x86_64, aarch64 (MSVC)

## Quick Start

```javascript
const { PluresDatabase } = require('@plures/pluresdb-native');

// Create a new database instance
const db = new PluresDatabase('my-actor');

// Insert a node
const id = db.put('user-1', { 
  name: 'Alice', 
  email: 'alice@example.com',
  type: 'User' 
});

// Retrieve a node
const user = db.get('user-1');
console.log(user); // { name: 'Alice', email: 'alice@example.com', type: 'User' }

// List all nodes
const all = db.list();

// Delete a node
db.delete('user-1');
```

## SQL Support

```javascript
// Create database with SQL support (provide db_path)
const db = new PluresDatabase('my-actor', './data.db');

// Create table
db.exec(`
  CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    email TEXT UNIQUE,
    embedding BLOB
  )
`);

// Insert data
db.exec(`INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')`);

// Query with parameters (supports ? placeholders)
const result = db.query('SELECT * FROM users WHERE name = ?', ['Alice']);
console.log(result.rows); // [{ id: 1, name: 'Alice', email: 'alice@example.com', embedding: null }]
```

## API Reference

### Constructor

```typescript
new PluresDatabase(actorId?: string, dbPath?: string)
```

- `actorId` (optional): Unique identifier for this database instance. Default: `"node-actor"`
- `dbPath` (optional): Path to SQLite database file for SQL support

### Methods

#### CRUD Operations

- `put(id: string, data: any): string` - Insert or update a node
- `get(id: string): any | null` - Retrieve a node by ID
- `getWithMetadata(id: string): NodeWithMetadata | null` - Get node with vector clock and timestamp
- `delete(id: string): void` - Delete a node
- `list(): Array<{id, data, timestamp}>` - List all nodes
- `listByType(nodeType: string): Array<{id, data, timestamp}>` - Filter nodes by type

#### SQL Operations

- `query(sql: string, params?: any[]): QueryResult` - Execute SQL SELECT with parameters
- `exec(sql: string): ExecutionResult` - Execute SQL statement (INSERT, UPDATE, DELETE, CREATE)

**Example:**
```javascript
const result = db.query('SELECT * FROM users WHERE age > ?', [25]);
console.log(result.rows); // Array of matching rows
```

#### Search

- `search(query: string, limit?: number): SearchResult[]` - Text search across node data
- `vectorSearch(query: string, limit?: number, threshold?: number): SearchResult[]` - Vector similarity search

#### Utilities

- `subscribe(): string` - Subscribe to database changes
- `getActorId(): string` - Get the actor ID
- `stats(): DatabaseStats` - Get database statistics (`{totalNodes, typeCounts}`)

## TypeScript Support

TypeScript definitions are included in the package.

```typescript
import { PluresDatabase, QueryResult, ExecutionResult } from '@plures/pluresdb-native';

const db = new PluresDatabase('my-actor', './data.db');
const result: QueryResult = db.query('SELECT * FROM users WHERE id = ?', [1]);
```

## SLM (superlocalmemory) Compatibility

This package is designed as a drop-in replacement for better-sqlite3 in the superlocalmemory project:

- ✅ Parameterized SQL with `?` placeholders
- ✅ BLOB column support for embedding vectors
- ✅ Synchronous API (no async/await required)
- ✅ 95% SQLite compatibility

See the [SLM Migration Guide](https://github.com/plures/development-guide/blob/main/goals/SLM-PLURESDB-MIGRATION.md) for details.

## Building from Source

If a pre-built binary is not available for your platform:

```bash
# Clone the repository
git clone https://github.com/plures/pluresdb.git
cd pluresdb/crates/pluresdb-node

# Install dependencies
npm install

# Build
npm run build

# Test
npm test
```

### Requirements

- Rust 1.70 or later
- Node.js 20 or later
- C compiler (for rusqlite native dependencies)

## License

AGPL-3.0 - see [LICENSE](../../LICENSE) for details.

## Links

- [GitHub Repository](https://github.com/plures/pluresdb)
- [PluresDB Documentation](https://github.com/plures/pluresdb#readme)
- [Issue Tracker](https://github.com/plures/pluresdb/issues)
- [NPM Package](https://www.npmjs.com/package/@plures/pluresdb-native)
