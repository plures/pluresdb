# PluresDB IPC

Inter-process communication layer for PluresDB, enabling high-performance local-first integration for native desktop applications.

## Features

- 🚧 Shared memory for zero-copy data transfer (planned)
- 🚧 Message-based protocol (planned)
- ✅ Process isolation
- ✅ No network exposure

## Status

This crate is in early development as part of Phase 3 of the local-first integration roadmap.

Current status:
- [ ] Shared memory implementation
- [ ] Message protocol
- [ ] Server implementation
- [ ] Client library
- [x] API design
- [x] Documentation

## Planned Usage

### Server Process

```rust
use pluresdb_ipc::IPCServer;
use pluresdb_core::CrdtStore;

fn main() -> anyhow::Result<()> {
    // Create database
    let db = CrdtStore::default();
    
    // Start IPC server
    let server = IPCServer::new("my-app-channel")?;
    server.start()?;
    
    Ok(())
}
```

### Client Process

```rust
use pluresdb_ipc::IPCClient;

fn main() -> anyhow::Result<()> {
    // Connect to IPC server
    let client = IPCClient::new("my-app-channel")?;
    
    // Use database operations
    client.put("user:1", serde_json::json!({
        "name": "Alice",
        "email": "alice@example.com"
    }))?;
    
    let user = client.get("user:1")?;
    println!("User: {:?}", user);
    
    Ok(())
}
```

## Architecture

```
┌─────────────────────────────────────┐
│   Application Process               │
│   (Electron, NW.js, etc.)           │
│                                     │
│   IPCClient                         │
└───────────┬─────────────────────────┘
            │ Shared Memory
            │ (message passing)
┌───────────▼─────────────────────────┐
│   PluresDB Server Process           │
│                                     │
│   IPCServer                         │
│   CrdtStore                         │
│   Storage (filesystem)              │
└─────────────────────────────────────┘
```

## Performance Goals

Compared to HTTP REST:

| Metric | HTTP | IPC | Target |
|--------|------|-----|--------|
| **Latency** | 5-10ms | 0.5ms | **10-20x faster** |
| **Throughput** | 1k ops/s | 50k ops/s | **50x faster** |
| **Security** | Port exposure | Process isolation | **No network** |

## Implementation Plan

### Phase 3.1: Shared Memory Foundation
- [ ] Implement shared memory allocation
- [ ] Message queue for request/response
- [ ] Synchronization primitives

### Phase 3.2: Protocol Implementation
- [ ] Define message protocol
- [ ] Implement serialization/deserialization
- [ ] Add message validation

### Phase 3.3: Server Implementation
- [ ] Request handler
- [ ] Database integration
- [ ] Process lifecycle management

### Phase 3.4: Client Library
- [ ] Client connection handling
- [ ] Request/response matching
- [ ] Error handling

### Phase 3.5: Testing & Polish
- [ ] Unit tests
- [ ] Integration tests
- [ ] Performance benchmarks
- [ ] Documentation

## Security Considerations

✅ **Process Isolation**: App and DB run in separate processes  
✅ **No Network Exposure**: No ports opened  
✅ **Memory Access Control**: OS-level shared memory permissions  
⚠️ **Same-Machine Only**: Only works on local machine  
⚠️ **Input Validation**: Always validate data from shared memory  

## Contributing

This crate is in active development. Contributions are welcome!

See:
- [LOCAL_FIRST_INTEGRATION.md](../../docs/LOCAL_FIRST_INTEGRATION.md)
- [CONTRIBUTING.md](../../CONTRIBUTING.md)

## License

AGPL-3.0 - see [LICENSE](../../LICENSE) for details.
