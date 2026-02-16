# PluresDB Design Architecture

## Core Vision

PluresDB is a **local-first graph database with SQLite compatibility**, designed to bridge the gap between traditional SQL databases and modern P2P applications. It provides a familiar SQL interface while adding graph relationships, vector search, and CRDT-based synchronization.

## Architecture Overview

```
┌─ Language Bindings ─────────────────────────────────────┐
│  TypeScript (Node/Deno)  │  Rust Native  │  WASM/Browser │
├─ API Layers ────────────────────────────────────────────┤
│  SQLite Compatible  │  GunDB-style  │  REST  │  GraphQL │
├─ Core Features ─────────────────────────────────────────┤
│  CRDT Merge  │  Vector Search  │  Graph Relations      │
├─ Storage Engines ───────────────────────────────────────┤
│  Sled (default)  │  SQLite  │  RocksDB  │  Memory      │
├─ Sync Transports ───────────────────────────────────────┤
│  Hyperswarm P2P  │  Azure Relay  │  WebSocket Direct   │
└─ Platform Targets ─────────────────────────────────────┘
   Desktop │ Browser │ Mobile │ Server │ VSCode Extension
```

## Design Principles

### 1. Local-First Architecture

**Data Ownership**: All data resides on user devices by default
- Full functionality offline
- Optional P2P sync when desired  
- No central servers required
- User controls data location and sharing

**Performance**: Local operations are fast (5-10ms)
- In-memory caching for hot data
- Efficient storage backends
- Lazy loading for large datasets
- Background sync doesn't block UI

### 2. SQLite Compatibility

**API Compatibility**: 95% compatible with SQLite
- `exec()`, `run()`, `get()`, `all()` methods
- Prepared statements
- Parameter binding
- Transaction support

**Migration Path**: Easy adoption from existing SQLite apps
- Drop-in replacement capability
- Gradual migration to graph features
- Backward compatibility maintained

### 3. Multi-Transport Sync

**Problem**: Corporate networks block P2P
- UDP traffic blocked
- Non-standard ports blocked
- Direct connections blocked

**Solution**: Automatic fallback transport selection
- **Direct**: Hyperswarm P2P (best performance)
- **Azure Relay**: WebSocket on port 443 (corporate-friendly)
- **Vercel Edge**: Global WebSocket mesh
- **Auto mode**: Attempts transports in order

### 4. CRDT-Based Conflict Resolution

**Conflict-Free Replicated Data Types**:
- Automatic merge without user intervention
- Last-writer-wins with vector clocks
- Graph edge consensus
- Schema evolution support

**Consistency Model**:
- Eventually consistent across peers
- Strong local consistency
- Causal ordering preserved
- Byzantine fault tolerance

## Core Components

### 1. Storage Engine

**Rust-First Implementation**:
- Primary: Sled embedded database
- Fallback: SQLite for compatibility
- Optional: RocksDB for large datasets
- Memory: For testing and temporary data

**Data Model**:
```rust
pub struct Node {
    pub id: String,
    pub data: Value,
    pub metadata: Metadata,
    pub vector: Option<Vec<f32>>,
}

pub struct Edge {
    pub from: String,
    pub to: String,
    pub relation: String,
    pub metadata: Metadata,
}
```

### 2. Synchronization Layer

**Hyperswarm P2P** (preferred):
- DHT-based peer discovery
- UDP hole punching for NAT traversal
- Noise protocol encryption
- Automatic reconnection

**Relay Fallbacks**:
- Azure Service Bus Relay (enterprise)
- Vercel Edge Functions (global)
- WebSocket direct connections
- HTTP polling (last resort)

### 3. Vector Search

**HNSW Implementation**:
- Hierarchical Navigable Small World graphs
- Configurable distance metrics (cosine, euclidean)
- Incremental indexing
- Embedding dimension flexibility

**Integration**:
- Automatic embedding generation
- SQLite `vector_search()` function
- Semantic similarity queries
- Hybrid text + vector search

### 4. Web Interface

**Svelte-Based UI** (24 tabs):
1. **Data Explorer**: Browse/edit JSON data
2. **Graph Visualization**: Interactive Cytoscape.js
3. **Vector Search**: Semantic query interface
4. **Schema Management**: Type definitions
5. **P2P Status**: Peer connections, sync stats
6. **Performance**: Real-time metrics
7. **History**: Version control and time travel
8. **Settings**: Configuration management

## Language Bindings

### TypeScript/JavaScript

**Node.js Package**:
```typescript
import { SQLiteCompatibleAPI } from "pluresdb";
const db = new SQLiteCompatibleAPI({ dataDir: "./data" });
```

**Deno Module**:
```typescript
import { GunDB } from "jsr:@plures/pluresdb";
const db = new GunDB();
```

**Browser WASM** (via wasm-bindgen):
```javascript
import { PluresDBBrowser } from "./pluresdb-wasm/pkg";
const db = new PluresDBBrowser("app-name");
```

### Rust Native

**Core Crates**:
- `pluresdb-core`: CRDT engine
- `pluresdb-storage`: Backend abstractions
- `pluresdb-sync`: P2P synchronization
- `pluresdb-wasm`: Browser bindings

**Usage**:
```rust
use pluresdb_core::{Database, DatabaseOptions};

let db = Database::open(
    DatabaseOptions::with_file("./data.db")
)?;
```

### Platform Integration

**VSCode Extensions**:
- SQLite replacement with graph features
- Global storage path management
- Extension-specific databases

**Tauri Desktop Apps**:
- Native Rust performance
- Cross-platform compatibility
- IPC command integration

**Desktop Applications**:
- Embedded database
- No external dependencies
- Windows/macOS/Linux support

## Security Model

### Data Protection

**Local Storage**:
- Data never leaves device without explicit consent
- No telemetry or analytics
- User controls all data access

**Encryption**:
- P2P connections use Noise protocol
- At-rest encryption optional
- Key management per-device

### Network Security

**Transport Encryption**:
- All sync connections encrypted
- Perfect forward secrecy
- Resistance to MITM attacks

**Relay Security**:
- Azure/Vercel relays are tunnels only
- End-to-end encryption preserved
- No data stored on relay servers

### Input Validation

**SQL Injection Protection**:
- Parameterized queries enforced
- Input sanitization
- Type validation

**CRDT Safety**:
- Schema validation
- Malformed data rejection
- Byzantine fault tolerance

## Performance Characteristics

### Local Operations
- **Read Latency**: <5ms (in-memory cache)
- **Write Latency**: <10ms (with persistence)
- **Vector Search**: <50ms (10K vectors)
- **Graph Traversal**: <20ms (3 hops)

### Sync Performance
- **Initial Sync**: ~1MB/sec (local network)
- **Incremental**: <100ms (small changes)
- **Peer Discovery**: 1-5 seconds (DHT lookup)
- **Reconnection**: <1 second (established peers)

### Scalability Limits
- **Single Database**: ~100GB (Sled limitation)
- **Vector Index**: ~1M vectors (memory bound)
- **Concurrent Peers**: ~100 (network bound)
- **Concurrent Writes**: ~1K/sec (CRDT overhead)

## Error Handling Strategy

### Graceful Degradation

**Network Failures**:
- Continue local operations
- Queue sync operations
- Retry with exponential backoff
- User notification of sync status

**Storage Failures**:
- Automatic corruption detection
- Background repair attempts
- Data recovery tools
- User-initiated integrity checks

### Development Experience

**Error Messages**:
- Clear, actionable error descriptions
- Suggested fixes for common issues
- Debug information in development mode
- Performance profiling hooks

**Testing Support**:
- Mock sync transports
- Deterministic CRDT testing
- Performance benchmarks
- Integration test utilities

## Future Architecture Evolution

### V2.0 Vision

**Distributed Computing**:
- Query across multiple peers
- Map-reduce style operations
- Distributed consensus
- Load balancing

**Advanced Features**:
- Time-series data optimization
- Machine learning model storage
- Real-time collaboration
- Blockchain integration

**Enterprise Features**:
- Role-based access control
- Audit logging
- Compliance reporting
- High availability clustering

## Development Workflow

### Rust-First Development

**Core Logic**: All critical functionality in Rust
- Performance-critical paths
- CRDT implementations
- Storage engines
- Cryptographic operations

**Language Bindings**: Generated from Rust
- TypeScript types from Rust structs
- WASM bindings via wasm-bindgen
- Python bindings via PyO3
- Mobile bindings via uniffi

### Testing Strategy

**Multi-Language Testing**:
- Rust unit tests for core logic
- TypeScript integration tests
- Browser compatibility tests
- Performance regression tests

**Sync Testing**:
- Multi-peer integration tests
- Network partition simulation
- Conflict resolution validation
- Transport failure scenarios

---

*This design evolves as PluresDB matures. See ROADMAP.md for implementation timeline.*