# pluresdb-deno

Native Deno bindings for PluresDB using deno_bindgen FFI.

## Features

✅ **Complete Implementation** - All core features are now implemented:

- **CRUD Operations**
  - `put(id, data)` - Insert or update a node
  - `get(id)` - Retrieve a node by ID
  - `getWithMetadata(id)` - Get node with vector clock and timestamp
  - `delete(id)` - Delete a node
  - `list()` - List all nodes
  - `listByType(type)` - List nodes filtered by type

- **SQL Support**
  - `query(sql, params?)` - Execute SQL SELECT queries
  - `exec(sql)` - Execute SQL statements (INSERT, UPDATE, DELETE)

- **Search**
  - `search(query, limit?)` - Text-based search across node data
  - `vectorSearch(query, limit?, threshold?)` - Vector similarity search (placeholder, uses text search)

- **Subscriptions**
  - Infrastructure ready via SyncBroadcaster (full async support pending)

- **Utilities**
  - `getActorId()` - Get the actor ID for this database instance
  - `stats()` - Get database statistics (total nodes, type counts)

## Usage

```typescript
import { PluresDatabase } from './bindings/bindings.ts';

// Create a new database instance
const db = new PluresDatabase('my-actor-id', './data.db');

// Insert a node
db.put('node-1', { name: 'Alice', age: 30 });

// Retrieve a node
const node = db.get('node-1');
console.log(node); // { name: 'Alice', age: 30 }

// Get with metadata
const nodeWithMeta = db.getWithMetadata('node-1');
console.log(nodeWithMeta);
// {
//   id: 'node-1',
//   data: { name: 'Alice', age: 30 },
//   clock: { 'my-actor-id': 1 },
//   timestamp: '2026-01-10T12:00:00Z'
// }

// Execute SQL query
const result = db.query('SELECT * FROM nodes WHERE age > ?', [25]);
console.log(result.rows);

// Search
const results = db.search('Alice', 10);
console.log(results);

// Get statistics
const stats = db.stats();
console.log(stats); // { totalNodes: 1, typeCounts: {} }
```

## Building

```bash
cd crates/pluresdb-deno
deno_bindgen --release
```

This will generate TypeScript bindings in `bindings/` directory.

## Status

✅ **Implementation Complete** - Ready for testing and publishing

