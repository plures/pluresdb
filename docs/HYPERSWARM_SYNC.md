# Hyperswarm P2P Sync

This document describes the Hyperswarm P2P sync feature for PluresDB, which enables zero-configuration peer-to-peer database synchronization with automatic NAT traversal.

## Overview

PluresDB's Hyperswarm sync transport provides:

- **DHT-based discovery**: Peers find each other using a distributed hash table without central servers
- **NAT traversal**: UDP holepunching works through most firewalls and NATs
- **Encryption**: All P2P connections are encrypted using the Noise protocol
- **CRDT merge**: Automatic conflict resolution using conflict-free replicated data types
- **Zero configuration**: No port forwarding or relay servers required

## Architecture

```
┌─────────────┐                    ┌─────────────┐
│  Database A │                    │  Database B │
│             │                    │             │
│ ┌─────────┐ │                    │ ┌─────────┐ │
│ │  CRDT   │ │                    │ │  CRDT   │ │
│ │ Storage │ │                    │ │ Storage │ │
│ └────┬────┘ │                    │ └────┬────┘ │
│      │      │                    │      │      │
│ ┌────▼────┐ │                    │ ┌────▼────┐ │
│ │Hyperswm │ │    DHT Network     │ │Hyperswm │ │
│ │  Sync   │ ├──────announce──────┤ │  Sync   │ │
│ └────┬────┘ │                    │ └────┬────┘ │
└──────┼──────┘                    └──────┼──────┘
       │                                  │
       └──────UDP Holepunch───────────────┘
                (Encrypted)
```

## Quick Start

### Generate a Sync Key

```javascript
// Node.js
const { GunDB } = require("@plures/pluresdb");
const syncKey = GunDB.generateSyncKey();
console.log(syncKey); // 64-char hex string
```

```typescript
// Deno
import { GunDB } from "https://deno.land/x/pluresdb/mod.ts";
const syncKey = GunDB.generateSyncKey();
```

### Enable Sync

```javascript
const db = new GunDB();
await db.ready("/path/to/db");

// Enable P2P sync with a shared key
await db.enableSync({ key: syncKey });

// Listen for peer events
db.on("peer:connected", (info) => {
  console.log("Peer connected:", info.peerId);
});

db.on("peer:disconnected", (info) => {
  console.log("Peer disconnected:", info.peerId);
});

db.on("sync:complete", (stats) => {
  console.log("Sync stats:", stats);
});
```

### Sync Data

Once sync is enabled, all database changes are automatically replicated to connected peers:

```javascript
// Device A
await dbA.put("user:alice", { name: "Alice", age: 30 });

// After a short delay, the data appears on Device B automatically
const user = await dbB.get("user:alice");
console.log(user); // { name: "Alice", age: 30 }
```

### Disable Sync

```javascript
await db.disableSync();
```

## API Reference

### Static Methods

#### `GunDB.generateSyncKey()`

Generates a new random sync key (32 bytes as 64-character hex string).

```javascript
const key = GunDB.generateSyncKey();
// Returns: "a1b2c3d4e5f6..."
```

### Instance Methods

#### `db.enableSync(options)`

Enable P2P sync with Hyperswarm.

**Parameters:**
- `options.key` (string, optional): 64-character hex sync key. If not provided, generates a new one.

**Returns:** Promise<void>

```javascript
// With existing key
await db.enableSync({ key: "a1b2c3d4..." });

// Generate new key
await db.enableSync({});
const key = db.getSyncKey(); // Retrieve the generated key
```

#### `db.disableSync()`

Disable P2P sync and disconnect from all peers.

**Returns:** Promise<void>

```javascript
await db.disableSync();
```

#### `db.getSyncStats()`

Get current sync statistics.

**Returns:** SyncStats | null

```javascript
const stats = db.getSyncStats();
console.log(stats);
// {
//   peersConnected: 2,
//   messagesSent: 150,
//   messagesReceived: 200,
//   bytesTransmitted: 45000,
//   bytesReceived: 60000
// }
```

#### `db.getSyncPeers()`

Get list of connected peers.

**Returns:** PeerInfo[]

```javascript
const peers = db.getSyncPeers();
console.log(peers);
// [
//   {
//     peerId: "abc123...",
//     connected: true,
//     remotePublicKey: "def456..."
//   }
// ]
```

#### `db.isSyncEnabled()`

Check if sync is currently enabled.

**Returns:** boolean

```javascript
if (db.isSyncEnabled()) {
  console.log("Sync is active");
}
```

#### `db.getSyncKey()`

Get the current sync key (if sync is enabled).

**Returns:** string | null

```javascript
const key = db.getSyncKey();
if (key) {
  console.log("Current sync key:", key);
}
```

### Events

#### `peer:connected`

Emitted when a new peer connects.

```javascript
db.on("peer:connected", (info: PeerInfo) => {
  console.log("Peer connected:", info.peerId);
});
```

#### `peer:disconnected`

Emitted when a peer disconnects.

```javascript
db.on("peer:disconnected", (info: PeerInfo) => {
  console.log("Peer disconnected:", info.peerId);
});
```

#### `sync:complete`

Emitted when sync operations complete (periodic).

```javascript
db.on("sync:complete", (stats: SyncStats) => {
  console.log("Sync complete:", stats);
});
```

## Use Cases

### 1. Shared Memory for AI Agents

```javascript
// Agent 1 on Machine A
const agent1Memory = new GunDB({ peerId: "agent-1" });
await agent1Memory.ready();
const sharedKey = GunDB.generateSyncKey();
await agent1Memory.enableSync({ key: sharedKey });

// Store experiences
await agent1Memory.put("memory:task-1", {
  task: "Process data",
  result: "success",
  timestamp: Date.now(),
});

// Agent 2 on Machine B (different network)
const agent2Memory = new GunDB({ peerId: "agent-2" });
await agent2Memory.ready();
await agent2Memory.enableSync({ key: sharedKey }); // Same key!

// Automatically syncs across the internet
const experience = await agent2Memory.get("memory:task-1");
```

### 2. Decentralized Commerce (OASIS)

```javascript
const merchantDb = new GunDB({ peerId: "merchant-alice" });
await merchantDb.enableSync({ key: marketplaceKey });

// List a product
await merchantDb.put("product:widget", {
  name: "Amazing Widget",
  price: 29.99,
  seller: "alice",
});

// Buyer's database automatically syncs the product catalog
const buyerDb = new GunDB({ peerId: "buyer-bob" });
await buyerDb.enableSync({ key: marketplaceKey });

// Search for products
const widget = await buyerDb.get("product:widget");
```

### 3. Team Collaboration

```javascript
const teamSyncKey = GunDB.generateSyncKey();

// Share the key with team members via secure channel
// Each member enables sync:

const member1 = new GunDB({ peerId: "alice" });
await member1.enableSync({ key: teamSyncKey });

const member2 = new GunDB({ peerId: "bob" });
await member2.enableSync({ key: teamSyncKey });

// Collaborative editing
await member1.put("doc:1", { title: "Q1 Planning", content: "..." });
// Automatically appears on member2's database
```

## Security Considerations

### Sync Key Security

- **Keep keys secret**: The sync key is the only authentication mechanism
- **Use secure channels**: Share keys via encrypted messaging, not email/SMS
- **Key rotation**: Generate new keys periodically for long-running networks
- **Per-group keys**: Use different keys for different collaboration groups

### Network Security

- **Encryption**: All P2P connections use Noise protocol encryption
- **No metadata leakage**: The DHT topic is derived via SHA-256 hash
- **Firewall-friendly**: Uses UDP holepunching, works through NATs

## Limitations

### Platform Support

- **Node.js**: Full support (Hyperswarm is Node.js native)
- **Deno**: Limited (Hyperswarm not available in Deno yet)
- **Browser**: Not supported (Hyperswarm requires UDP)

For Deno and browser environments, use WebSocket-based mesh networking instead:

```javascript
// Deno fallback: WebSocket mesh
await db.serve({ port: 8080 });
// Other peer connects via:
db.connect("ws://peer-ip:8080");
```

### Network Considerations

- **Discovery time**: DHT lookup can take 1-5 seconds
- **Connection limits**: Practical limit ~50-100 active peers
- **Bandwidth**: Each peer broadcasts to all connected peers
- **Mobile**: May not work on mobile networks with strict NAT policies

## Troubleshooting

### Peers not connecting

1. **Check sync keys match**:
   ```javascript
   console.log(dbA.getSyncKey() === dbB.getSyncKey());
   ```

2. **Wait for DHT announcement**:
   ```javascript
   await db.enableSync({ key });
   await new Promise((r) => setTimeout(r, 5000)); // Wait 5s
   ```

3. **Check firewall**: Ensure UDP traffic is allowed

### Sync not working

1. **Verify peers connected**:
   ```javascript
   console.log(db.getSyncPeers().length); // Should be > 0
   ```

2. **Check for errors**:
   ```javascript
   db.on("error", (err) => console.error("Sync error:", err));
   ```

3. **Monitor stats**:
   ```javascript
   setInterval(() => {
     console.log(db.getSyncStats());
   }, 5000);
   ```

## Examples

See [examples/hyperswarm-p2p-sync.js](../examples/hyperswarm-p2p-sync.js) for complete working examples including:

- Basic P2P sync between two devices
- Multi-device mesh networks
- CRDT conflict resolution
- Private sync channels with different keys
- Real-time event handling

## Related Documentation

- [CRDT Implementation](./CRDT.md) - How conflict resolution works
- [Mesh Networking](./MESH_NETWORKING.md) - WebSocket-based alternative
- [Security](../SECURITY.md) - Security best practices
- [API Documentation](./API.md) - Complete API reference

## References

- [Hyperswarm](https://github.com/holepunchto/hyperswarm) - DHT and holepunching library
- [Noise Protocol](https://noiseprotocol.org/) - Encryption framework
- [CRDTs](https://crdt.tech/) - Conflict-free replicated data types
