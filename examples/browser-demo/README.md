# PluresDB Browser Demo

A fully-functional browser-based demo of PluresDB's local-first integration with WebAssembly.

## Features

- 🚀 Local-first database running entirely in the browser
- 📝 Add, list, and delete users
- 💾 Automatic persistence with IndexedDB
- 🎨 Beautiful, modern UI
- ⚡ Real-time updates
- 🔌 Offline-first by default

## Running the Demo

### Option 1: With WASM (Recommended)

The WASM bindings are now complete! Run the demo entirely in-browser without any server:

1. Build the WASM module:
   ```bash
   cd crates/pluresdb-wasm
   wasm-pack build --target web
   ```

2. Serve the demo with a local HTTP server:
   ```bash
   # Using Python
   python -m http.server 8000
   
   # Or using Node.js
   npx http-server
   ```

3. Open `http://localhost:8000/examples/browser-demo/index.html` in your browser

4. The demo will automatically use WASM mode with IndexedDB persistence - no server needed!

### Option 2: With PluresDB Server (Fallback)

For testing network mode:

1. Start the PluresDB server:
   ```bash
   npm start
   ```

2. Open `index.html` in your browser - it will fall back to network mode

## What This Demonstrates

### Local-First Integration
- **WebAssembly**: Runs PluresDB core directly in the browser
- **IndexedDB**: Automatic data persistence
- **Auto-detection**: Automatically uses WASM when available
- **Network fallback**: Falls back to HTTP REST when needed
- **Unified API**: Same API regardless of backend

### Database Operations
- **Put**: Insert or update records with persistence
- **Get**: Retrieve records by ID
- **List**: Query all records
- **Delete**: Remove records from memory and IndexedDB
- **Clear**: Wipe all data

### User Interface
- Clean, modern design
- Real-time status updates
- Interactive user management
- Console output for debugging

## Architecture

### WASM Mode (Default)
```
┌─────────────────────────────────┐
│   Browser (index.html)          │
│                                 │
│   PluresDBLocalFirst            │
│   (Auto-detection)              │
│           │                     │
│           ▼                     │
│   PluresDBBrowser (WASM)        │
│           │                     │
│           ▼                     │
│   IndexedDB (Persistence)       │
│                                 │
└─────────────────────────────────┘
     100% In-Browser
     No Network Required
```

### Network Mode (Fallback)
```
┌─────────────────────────────────┐
│   Browser (index.html)          │
│                                 │
│   PluresDBLocalFirst            │
│   (Auto-detection)              │
│           │                     │
│           └─ Network            │
│                   │             │
└───────────────────┼─────────────┘
                    │ HTTP
        ┌───────────▼──────────┐
        │   PluresDB Server    │
        │   (port 34567)       │
        └──────────────────────┘
```

## Performance

With WASM mode enabled:

| Metric | Network Mode | WASM Mode | Improvement |
|--------|--------------|-----------|-------------|
| **Latency** | ~50-100ms | ~0.1ms | **500-1000x faster** |
| **Throughput** | ~100 ops/s | ~100k ops/s | **1000x faster** |
| **Offline** | ❌ Requires server | ✅ Fully offline | **100% available** |
| **Network** | Required | None | **Zero bandwidth** |
| **Persistence** | Server-side | IndexedDB | **Local storage** |

## Development

To modify the demo:

1. Edit `index.html` - all code is in a single file for simplicity
2. Rebuild WASM if you modified Rust code:
   ```bash
   cd crates/pluresdb-wasm
   wasm-pack build --target web
   ```
3. Refresh the browser to see changes
4. Use browser DevTools (F12) to debug

## Integration with Your App

To integrate PluresDB local-first into your own application:

```javascript
import { PluresDBLocalFirst } from "@plures/pluresdb/local-first";

// Auto-detects best mode (WASM in browser)
const db = new PluresDBLocalFirst({ mode: "auto" });

// Or force WASM mode
const db = new PluresDBLocalFirst({ mode: "wasm", dbName: "my-app" });

// Use the API
await db.put("user:1", { name: "Alice", email: "alice@example.com" });
const user = await db.get("user:1");
```

See the [Local-First Integration Guide](../../docs/LOCAL_FIRST_INTEGRATION.md) for complete documentation.

## Troubleshooting

### WASM not loading

1. Make sure you built the WASM module:
   ```bash
   cd crates/pluresdb-wasm
   wasm-pack build --target web
   ```

2. Check browser console for errors

3. Ensure you're using a local HTTP server (not file://)

### "Failed to connect" error (network mode)

Make sure the PluresDB server is running:
```bash
npm start
```

The server should be listening on port 34567.

### IndexedDB not persisting data

1. Check browser console for IndexedDB errors
2. Make sure you called `init_persistence()` on the WASM instance
3. Check browser storage settings (some browsers block IndexedDB in private mode)

## Browser Compatibility

WASM mode is supported in:
- ✅ Chrome/Edge 57+
- ✅ Firefox 52+
- ✅ Safari 11+
- ✅ Opera 44+

IndexedDB is supported in all modern browsers.

## Next Steps

- [Browser WASM Integration Guide](../browser-wasm-integration.md)
- [Local-First Integration Methodology](../../docs/LOCAL_FIRST_INTEGRATION.md)
- [Tauri Integration Example](../tauri-integration.md)
- [IPC Integration Example](../ipc-demo/README.md)

## License

AGPL-3.0 - see [LICENSE](../../LICENSE)
