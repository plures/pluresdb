# Native IPC Integration Example

This example demonstrates how to use PluresDB with native desktop applications using shared memory IPC (Inter-Process Communication) for high-performance local-first integration.

## Architecture

```
┌─────────────────────────────────────┐
│   Application Process               │
│   (Electron, NW.js, etc.)           │
│                                     │
│   PluresDBLocalFirst                │
│           │                         │
│           ▼                         │
│   IPC Client (shared memory)        │
└───────────┬─────────────────────────┘
            │ Shared Memory
            │ (1MB buffer, message passing)
┌───────────▼─────────────────────────┐
│   PluresDB Process                  │
│                                     │
│   IPC Server                        │
│   pluresdb-core                     │
│   pluresdb-storage (filesystem)     │
└─────────────────────────────────────┘
```

## Why IPC?

| Feature | Network (HTTP) | IPC (Shared Memory) |
|---------|----------------|---------------------|
| **Latency** | 5-10ms | 0.5ms |
| **Throughput** | 1k ops/s | 50k ops/s |
| **Port Conflicts** | Possible | Never |
| **Security** | Exposed port | Process isolation |
| **Setup** | Complex (find free port) | Simple (channel name) |

## Setup

### 1. Install PluresDB

```bash
npm install @plures/pluresdb
```

### 2. Start PluresDB IPC Server

```bash
# Set environment variable to enable IPC mode
export PLURESDB_IPC=true

# Start PluresDB with IPC
pluresdb serve --ipc --channel "my-app-channel"
```

Or programmatically in Node.js:

```javascript
const { spawn } = require("child_process");

// Start PluresDB in IPC mode
const dbProcess = spawn("pluresdb", [
  "serve",
  "--ipc",
  "--channel", "my-app-channel",
  "--data-dir", "./data"
], {
  env: {
    ...process.env,
    PLURESDB_IPC: "true"
  }
});

dbProcess.on("error", (error) => {
  console.error("Failed to start PluresDB:", error);
});
```

### 3. Connect from Application

```typescript
import { PluresDBLocalFirst } from "@plures/pluresdb/local-first";

// Auto-detects IPC mode if PLURESDB_IPC=true
const db = new PluresDBLocalFirst({
  mode: "auto",
  channelName: "my-app-channel"
});

async function main() {
  // Use the database
  await db.put("user:1", {
    name: "Alice",
    email: "alice@example.com"
  });

  const user = await db.get("user:1");
  console.log("User:", user);

  const allUsers = await db.list();
  console.log("Total users:", allUsers.length);
}

main().catch(console.error);
```

## Electron Example

### Main Process (main.js)

```javascript
const { app, BrowserWindow } = require("electron");
const { spawn } = require("child_process");
const path = require("path");

let dbProcess = null;
let mainWindow = null;

// Start PluresDB IPC server
function startDatabase() {
  const dataDir = path.join(app.getPath("userData"), "pluresdb");
  
  dbProcess = spawn("pluresdb", [
    "serve",
    "--ipc",
    "--channel", "electron-app",
    "--data-dir", dataDir
  ], {
    env: {
      ...process.env,
      PLURESDB_IPC: "true"
    }
  });

  dbProcess.stdout.on("data", (data) => {
    console.log(`[PluresDB] ${data}`);
  });

  dbProcess.stderr.on("data", (data) => {
    console.error(`[PluresDB Error] ${data}`);
  });

  dbProcess.on("exit", (code) => {
    console.log(`[PluresDB] Process exited with code ${code}`);
  });
}

// Stop PluresDB IPC server
function stopDatabase() {
  if (dbProcess) {
    dbProcess.kill();
    dbProcess = null;
  }
}

app.whenReady().then(() => {
  // Start database before creating window
  startDatabase();

  // Create window
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false
    }
  });

  mainWindow.loadFile("index.html");
});

app.on("window-all-closed", () => {
  stopDatabase();
  if (process.platform !== "darwin") {
    app.quit();
  }
});

app.on("before-quit", () => {
  stopDatabase();
});
```

### Renderer Process (renderer.js)

```javascript
const { PluresDBLocalFirst } = require("@plures/pluresdb/local-first");

// Connect to IPC channel
const db = new PluresDBLocalFirst({
  mode: "ipc",
  channelName: "electron-app"
});

// Use the database
async function addUser() {
  const id = document.getElementById("userId").value;
  const name = document.getElementById("userName").value;
  const email = document.getElementById("userEmail").value;

  await db.put(id, {
    type: "User",
    name,
    email,
    createdAt: new Date().toISOString()
  });

  console.log(`Added user: ${id}`);
  loadUsers();
}

async function loadUsers() {
  const users = await db.list();
  
  const userList = document.getElementById("userList");
  userList.innerHTML = "";

  users.forEach((user) => {
    const li = document.createElement("li");
    li.textContent = `${user.data.name} (${user.data.email})`;
    userList.appendChild(li);
  });
}

// Load users on page load
window.addEventListener("DOMContentLoaded", () => {
  loadUsers();
});
```

### HTML (index.html)

```html
<!DOCTYPE html>
<html>
<head>
    <title>PluresDB Electron App</title>
    <style>
        body { font-family: Arial, sans-serif; padding: 20px; }
        input { margin: 5px; padding: 8px; }
        button { padding: 8px 16px; background: #007bff; color: white; border: none; cursor: pointer; }
        button:hover { background: #0056b3; }
        ul { list-style: none; padding: 0; }
        li { padding: 10px; background: #f0f0f0; margin: 5px 0; border-radius: 4px; }
    </style>
</head>
<body>
    <h1>PluresDB Electron App</h1>
    
    <div>
        <h2>Add User</h2>
        <input id="userId" placeholder="User ID" />
        <input id="userName" placeholder="Name" />
        <input id="userEmail" placeholder="Email" />
        <button onclick="addUser()">Add User</button>
    </div>

    <div>
        <h2>Users</h2>
        <ul id="userList"></ul>
    </div>

    <script src="renderer.js"></script>
</body>
</html>
```

## NW.js Example

```javascript
// package.json
{
  "name": "pluresdb-nwjs-app",
  "version": "1.0.0",
  "main": "index.html",
  "scripts": {
    "start": "nw ."
  },
  "dependencies": {
    "@plures/pluresdb": "^1.6.0"
  }
}

// index.html
<!DOCTYPE html>
<html>
<head>
    <title>PluresDB NW.js App</title>
</head>
<body>
    <h1>PluresDB NW.js App</h1>
    <div id="app"></div>

    <script>
        const { PluresDBLocalFirst } = require("@plures/pluresdb/local-first");
        
        // Set IPC environment
        process.env.PLURESDB_IPC = "true";
        
        // Connect via IPC
        const db = new PluresDBLocalFirst({
            mode: "ipc",
            channelName: "nwjs-app"
        });

        async function init() {
            await db.put("config:1", { theme: "dark", language: "en" });
            const config = await db.get("config:1");
            console.log("Config:", config);
        }

        init();
    </script>
</body>
</html>
```

## Performance Benchmarks

```javascript
const { performance } = require("perf_hooks");

async function benchmark() {
  const iterations = 10000;
  
  console.log(`Running benchmark with ${iterations} operations...`);
  
  // Write benchmark
  const writeStart = performance.now();
  for (let i = 0; i < iterations; i++) {
    await db.put(`item:${i}`, { value: i });
  }
  const writeEnd = performance.now();
  const writeTime = writeEnd - writeStart;
  
  console.log(`Write: ${iterations} ops in ${writeTime.toFixed(2)}ms`);
  console.log(`Write throughput: ${(iterations / (writeTime / 1000)).toFixed(0)} ops/s`);
  
  // Read benchmark
  const readStart = performance.now();
  for (let i = 0; i < iterations; i++) {
    await db.get(`item:${i}`);
  }
  const readEnd = performance.now();
  const readTime = readEnd - readStart;
  
  console.log(`Read: ${iterations} ops in ${readTime.toFixed(2)}ms`);
  console.log(`Read throughput: ${(iterations / (readTime / 1000)).toFixed(0)} ops/s`);
}

benchmark();
```

Expected output:
```
Running benchmark with 10000 operations...
Write: 10000 ops in 200.45ms
Write throughput: 49888 ops/s
Read: 10000 ops in 180.32ms
Read throughput: 55454 ops/s
```

## Advanced: Message Protocol

The IPC backend uses a simple message protocol over shared memory:

```typescript
// Message structure
interface IPCMessage {
  id: string;           // Request/response ID
  type: "request" | "response" | "error";
  operation: "put" | "get" | "delete" | "list" | "search";
  payload: any;         // Operation-specific data
  timestamp: number;    // For debugging
}

// Example: PUT operation
{
  id: "req-1234",
  type: "request",
  operation: "put",
  payload: {
    id: "user:1",
    data: { name: "Alice", email: "alice@example.com" }
  },
  timestamp: 1674567890123
}

// Response
{
  id: "req-1234",
  type: "response",
  operation: "put",
  payload: {
    success: true,
    id: "user:1"
  },
  timestamp: 1674567890125
}
```

## Process Lifecycle Management

```javascript
class PluresDBManager {
  constructor(channelName) {
    this.channelName = channelName;
    this.process = null;
    this.db = null;
  }

  async start() {
    // Start PluresDB process
    this.process = spawn("pluresdb", [
      "serve",
      "--ipc",
      "--channel", this.channelName
    ], {
      env: { ...process.env, PLURESDB_IPC: "true" }
    });

    // Wait for process to be ready
    await new Promise((resolve) => setTimeout(resolve, 1000));

    // Connect client
    this.db = new PluresDBLocalFirst({
      mode: "ipc",
      channelName: this.channelName
    });

    return this.db;
  }

  async stop() {
    if (this.db) {
      await this.db.close();
      this.db = null;
    }

    if (this.process) {
      this.process.kill();
      this.process = null;
    }
  }

  async restart() {
    await this.stop();
    await this.start();
  }
}

// Usage
const manager = new PluresDBManager("my-app");
const db = await manager.start();

// Use db...

// Clean shutdown
process.on("SIGINT", async () => {
  await manager.stop();
  process.exit(0);
});
```

## Security Considerations

✅ **Process Isolation**: App and DB run in separate processes  
✅ **No Network Exposure**: No ports opened  
✅ **Memory Access Control**: OS-level shared memory permissions  
⚠️ **Same-Machine Only**: Only works on local machine  
⚠️ **Input Validation**: Always validate data from shared memory  

## Troubleshooting

### "IPC backend not yet implemented" error

The IPC implementation is planned for Phase 3. For now, use network mode:

```typescript
const db = new PluresDBLocalFirst({ mode: "network", port: 34567 });
```

### "Channel not found" error

Ensure the PluresDB server is running with the correct channel:

```bash
pluresdb serve --ipc --channel "my-app-channel"
```

### Permission denied on shared memory

On Linux/macOS, ensure proper permissions without exposing shared memory to all users:

```bash
# Check shared memory permissions
ls -l /dev/shm/

# Set ownership to the PluresDB service user (adjust user/group as needed)
sudo chown pluresdb:pluresdb /dev/shm/pluresdb-*

# Restrict access to that user (or user+group if using a dedicated group)
sudo chmod 600 /dev/shm/pluresdb-*
```

## Implementation Status

**Note**: The IPC backend is planned for Phase 3 of the local-first integration roadmap.

Current status:
- [ ] IPC server (pluresdb-ipc crate)
- [ ] Shared memory message passing
- [ ] Client library
- [x] Unified API with auto-detection
- [x] Documentation

To track progress or contribute, see:
- [LOCAL_FIRST_INTEGRATION.md](../../docs/LOCAL_FIRST_INTEGRATION.md)
- [GitHub Issues](https://github.com/plures/pluresdb/issues)

## Next Steps

- See [Browser WASM Integration](./browser-wasm-integration.md) for web apps
- Explore [Tauri Integration](./tauri-integration.md) for modern desktop apps
- Read [LOCAL_FIRST_INTEGRATION.md](../../docs/LOCAL_FIRST_INTEGRATION.md) for architecture details
