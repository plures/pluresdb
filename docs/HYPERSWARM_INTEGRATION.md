# Hyperswarm-rs Integration for PluresDB Sync

**Status**: 🚧 In Progress  
**Tracking Issue**: #70 (P2P sync), #71 (Pluggable transports)  
**Implementation PR**: TBD

## Overview

This document tracks the integration of hyperswarm-rs as the default P2P transport for PluresDB synchronization. The goal is to enable DHT-based peer discovery and NAT traversal for CRDT replication across PluresDB instances.

## Background

The [plures/hyperswarm](https://github.com/plures/hyperswarm) repository contains a functional Rust DHT client with:
- Kademlia DHT for peer discovery (KRPC/UDP)
- Noise XX encrypted streams (25519/ChaChaPoly/BLAKE2s)
- UDP holepunching for NAT traversal (LAN/WAN/Relay candidates)
- Bootstrap, announce, and lookup operations

This provides the transport layer PluresDB needs for P2P CRDT synchronization.

## Architecture

### Transport Abstraction Layer

PluresDB uses a pluggable transport architecture defined in `pluresdb-sync`:

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&mut self, topic: TopicHash) -> Result<mpsc::Receiver<Box<dyn Connection>>>;
    async fn announce(&mut self, topic: TopicHash) -> Result<()>;
    async fn lookup(&self, topic: TopicHash) -> Result<Vec<PeerInfo>>;
    async fn disconnect(&mut self) -> Result<()>;
    fn name(&self) -> &str;
}
```

### Topic Derivation

Database IDs are converted to DHT topics using BLAKE2b-256:

```rust
pub fn derive_topic(database_id: &str) -> TopicHash {
    // Returns 32-byte BLAKE2b-256 hash
}
```

This ensures:
- Deterministic topic generation
- Same database ID → same DHT topic
- Peers with the same database ID discover each other automatically

### Transport Modes

PluresDB supports three sync transport modes:

| Mode | Description | Use Case |
|------|-------------|----------|
| `hyperswarm` | DHT-based P2P | Home/personal networks |
| `relay` | WebSocket on port 443 | Corporate networks |
| `disabled` | Local-only | No sync needed |

Configuration:

```rust
let config = TransportConfig {
    mode: TransportMode::Hyperswarm,
    encryption: true,
    timeout_ms: 30000,
    ..Default::default()
};
```

## Implementation Progress

### ✅ Completed

1. **Transport Trait Definition** (`pluresdb-sync/src/transport.rs`)
   - `Transport` trait with async methods
   - `Connection` trait for bidirectional communication
   - `TransportConfig` and `TransportMode` enums
   - `PeerInfo` struct for peer metadata

2. **Topic Derivation** (`pluresdb-sync/src/transport.rs`)
   - BLAKE2b-256 hash function
   - Unit tests for deterministic hashing
   - 32-byte topic output

3. **Transport Factory** (`pluresdb-sync/src/transport.rs`)
   - `create_transport()` function
   - Factory pattern for transport instantiation
   - Configuration-based transport selection

4. **DisabledTransport** (`pluresdb-sync/src/disabled.rs`)
   - Local-only mode implementation
   - Graceful no-op behavior
   - Test coverage

5. **HyperswarmTransport Stub** (`pluresdb-sync/src/hyperswarm.rs`)
   - Struct definition ready for integration
   - Configuration (`HyperswarmConfig`)
   - Bootstrap node defaults
   - Placeholder implementation with helpful error messages
   - Integration points documented in comments

6. **RelayTransport Stub** (`pluresdb-sync/src/relay.rs`)
   - WebSocket relay stub
   - Ready for WebSocket client integration
   - Corporate-friendly port 443 design

7. **Integration Tests** (`pluresdb-sync/tests/integration_test.rs`)
   - Tests for disabled transport
   - Tests for topic derivation
   - Tests for transport factory
   - Placeholder tests for hyperswarm (marked `#[ignore]`)

8. **Documentation**
   - README.md for pluresdb-sync crate
   - Inline documentation for all public APIs
   - Usage examples
   - Architecture diagrams

### 🚧 In Progress

1. **Hyperswarm-rs Integration**
   - Waiting for hyperswarm-rs crate to be published
   - Alternative: Add as git dependency to Cargo.toml
   - Implementation plan ready in comments

2. **Connection Handling**
   - Stream wrapper for Hyperswarm connections
   - Message framing (newline-delimited JSON or length-prefixed)
   - Connection lifecycle management

### 📋 Pending

1. **CRDT Sync Integration**
   - Wire Transport to CRDT replication engine
   - Message serialization/deserialization
   - Conflict resolution over P2P streams

2. **Relay Transport Implementation**
   - WebSocket client library (tokio-tungstenite)
   - Relay server integration
   - TLS/SSL certificate handling

3. **Integration Tests**
   - Two-node sync test (write on A → appears on B)
   - Multi-peer sync (3+ nodes)
   - NAT traversal simulation
   - Network partition recovery

4. **Configuration Integration**
   - Add sync.transport to PluresDB config file
   - Environment variable overrides
   - Default transport selection logic

5. **Performance Optimization**
   - Connection pooling
   - Message batching
   - Compression (optional)
   - Metrics and monitoring

## Dependencies

### Required

- **hyperswarm-rs**: DHT client from [plures/hyperswarm](https://github.com/plures/hyperswarm)
  - Status: Merged in PR #4
  - Needs: Publication to crates.io OR git dependency

### Optional

- **tokio-tungstenite**: WebSocket client for relay transport
- **quinn**: QUIC transport (future alternative to Hyperswarm)

## Next Steps

### Immediate (This PR)

1. ✅ Define Transport trait
2. ✅ Implement topic derivation
3. ✅ Create transport stubs
4. ✅ Add integration tests
5. ✅ Document architecture

### Short-term (Next PR)

1. Add hyperswarm-rs as dependency
   - Option A: Git dependency in Cargo.toml
   - Option B: Wait for crates.io publication
2. Implement HyperswarmTransport methods
3. Wire to CRDT sync engine
4. Add two-node integration test
5. Update documentation

### Medium-term

1. Implement RelayTransport (WebSocket)
2. Add automatic transport fallback (auto mode)
3. Performance benchmarks
4. Security audit of P2P communication
5. NAT traversal improvements

## Usage Example (Future)

Once hyperswarm-rs is integrated:

```rust
use pluresdb_sync::{create_transport, derive_topic, TransportConfig, TransportMode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure hyperswarm transport
    let config = TransportConfig {
        mode: TransportMode::Hyperswarm,
        encryption: true,
        timeout_ms: 30000,
        ..Default::default()
    };
    
    let mut transport = create_transport(config);
    
    // Derive topic from database ID
    let db_id = "my-app-database";
    let topic = derive_topic(db_id);
    
    // Announce on DHT
    transport.announce(topic).await?;
    
    // Lookup peers
    let peers = transport.lookup(topic).await?;
    println!("Found {} peers", peers.len());
    
    // Listen for connections
    let mut connections = transport.connect(topic).await?;
    while let Some(mut conn) = connections.recv().await {
        println!("Peer connected: {}", conn.peer_id());
        
        // CRDT sync would happen here
        // sync_engine.replicate_with(conn).await?;
    }
    
    Ok(())
}
```

## Testing Strategy

### Unit Tests
- Topic derivation (BLAKE2b-256)
- Transport configuration
- Factory function
- Each transport mode

### Integration Tests
- Local-only mode (disabled transport)
- Two-node sync (when hyperswarm available)
- Multi-peer sync
- Peer discovery
- Connection lifecycle

### Network Tests
- NAT traversal scenarios
- Firewall simulation
- Corporate network conditions
- Relay fallback behavior

### Performance Tests
- Connection establishment latency
- Message throughput
- DHT lookup time
- Memory usage under load

## Security Considerations

### Already Implemented in hyperswarm-rs
- Noise XX encryption (25519/ChaChaPoly)
- BLAKE2s hashing
- UDP packet authentication

### PluresDB Responsibilities
- Topic derivation (BLAKE2b-256)
- Database ID → topic mapping
- Peer authentication (future)
- Rate limiting (future)

### Future Enhancements
- Per-database encryption keys
- Access control lists
- Audit logging
- Peer reputation system

## Known Limitations

### Current
1. Hyperswarm transport is stubbed (waiting for crate)
2. Relay transport is stubbed (needs WebSocket client)
3. No automatic fallback between transports
4. No connection pooling or reuse

### Design Limitations
1. UDP may be blocked in strict corporate networks
2. DHT bootstrap nodes are hard-coded
3. Topic collisions possible (though unlikely with BLAKE2b-256)
4. No built-in rate limiting

## References

- [hyperswarm-rs README](https://github.com/plures/hyperswarm/tree/main/crates/hyperswarm-rs)
- [PluresDB Sync Transport Docs](./SYNC_TRANSPORT.md)
- [PluresDB Design Architecture](./DESIGN.md)
- [PARES Transport Stack Design](https://github.com/plures/development-guide/blob/main/design/PARES-OASIS-GTM.md)
- PluresDB Issue #70: P2P CRDT Sync
- PluresDB Issue #71: Pluggable Transport Architecture

## Changelog

### 2026-02-17
- ✅ Created Transport trait and abstractions
- ✅ Implemented topic derivation with BLAKE2b-256
- ✅ Added DisabledTransport for local-only mode
- ✅ Created stubs for HyperswarmTransport and RelayTransport
- ✅ Added integration tests
- ✅ Documented architecture and usage

### Next
- 🚧 Add hyperswarm-rs dependency
- 🚧 Implement HyperswarmTransport methods
- 🚧 Wire to CRDT sync engine
- 🚧 Add two-node integration test
