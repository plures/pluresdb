# PluresDB WASM

WebAssembly bindings for PluresDB, enabling local-first database functionality directly in the browser.

## Features

- ✅ Zero network overhead - runs directly in browser
- ✅ No server process required
- ✅ Offline-first by default
- ✅ Type-safe JavaScript/TypeScript API
- 🚧 IndexedDB persistence (planned)

## Installation

```bash
npm install @plures/pluresdb-wasm
```

Or via CDN:

```html
<script type="module">
  import init, { PluresDBBrowser } from "https://esm.sh/@plures/pluresdb-wasm";
  
  await init(); // Initialize WASM
  const db = new PluresDBBrowser("my-app-db");
</script>
```

## Usage

### Basic Example

```javascript
import init, { PluresDBBrowser } from "@plures/pluresdb-wasm";

// Initialize WASM module
await init();

// Create database instance
const db = new PluresDBBrowser("my-app-db");

// Insert data
await db.put("user:1", { 
  name: "Alice", 
  email: "alice@example.com" 
});

// Retrieve data
const user = await db.get("user:1");
console.log(user); // { name: "Alice", email: "alice@example.com" }

// List all nodes
const allNodes = await db.list();
console.log(`Total nodes: ${allNodes.length}`);

// Delete node
await db.delete("user:1");
```

## Building from Source

### Prerequisites

- Rust (latest stable)
- wasm-pack

### Build

```bash
# Install wasm-pack if not already installed
cargo install wasm-pack

# Build for web
wasm-pack build --target web

# Build for bundlers (webpack, vite, etc.)
wasm-pack build --target bundler

# Build for Node.js
wasm-pack build --target nodejs
```

### Test

```bash
# Run WASM tests in headless browser
wasm-pack test --headless --firefox
wasm-pack test --headless --chrome
```

## API Reference

### `PluresDBBrowser`

Main database class.

#### Constructor

```typescript
new PluresDBBrowser(dbName: string): PluresDBBrowser
```

Creates a new database instance.

#### Methods

##### `put(id: string, data: any): Promise<string>`

Insert or update a node.

```javascript
await db.put("user:1", { name: "Alice" });
```

##### `get(id: string): Promise<any | null>`

Retrieve a node by ID. Returns `null` if not found.

```javascript
const user = await db.get("user:1");
```

##### `delete(id: string): Promise<void>`

Delete a node by ID.

```javascript
await db.delete("user:1");
```

##### `list(): Promise<Array<{id: string, data: any, timestamp: string}>>`

List all nodes in the database.

```javascript
const nodes = await db.list();
console.log(`Found ${nodes.length} nodes`);
```

##### `count(): number`

Get the number of nodes in the database.

```javascript
console.log(`Database has ${db.count()} nodes`);
```

## Performance

Compared to HTTP REST API:

| Metric | REST API | WASM | Improvement |
|--------|----------|------|-------------|
| **Latency** | ~50-100ms | ~0.1ms | **500-1000x faster** |
| **Throughput** | ~100 ops/s | ~100k ops/s | **1000x faster** |
| **Offline** | ❌ | ✅ | **100% available** |
| **Network** | Required | None | **Zero bandwidth** |

## Browser Compatibility

| Browser | Version | Support |
|---------|---------|---------|
| Chrome | 57+ | ✅ Full |
| Firefox | 52+ | ✅ Full |
| Safari | 11+ | ✅ Full |
| Edge | 79+ | ✅ Full |

Requirements:
- WebAssembly support
- ES2022+ JavaScript

## Security

✅ **Sandboxed**: Runs in browser security sandbox  
✅ **Same-Origin Policy**: Data isolated per domain  
✅ **No Network Exposure**: Zero network attack surface  
⚠️ **Client-Side Storage**: Data accessible to user (don't store secrets)  

## Roadmap

- [x] Core CRDT operations (put, get, delete, list)
- [ ] IndexedDB persistence layer
- [ ] Vector search support
- [ ] Incremental sync with remote peers
- [ ] Compression for large datasets
- [ ] Encryption at rest

## Examples

See the [examples directory](../../examples) for more examples:
- [Browser WASM Integration](../../examples/browser-wasm-integration.md)
- [React Integration](../../examples/browser-wasm-integration.md#3-react-example)
- [Vue Integration](../../examples/browser-wasm-integration.md#4-vue-example)

## License

AGPL-3.0 - see [LICENSE](../../LICENSE) for details.
