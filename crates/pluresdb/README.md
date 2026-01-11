# pluresdb

P2P Graph Database with SQLite Compatibility - Local-first, offline-first database for modern applications.

## Overview

This is the main PluresDB crate that provides a unified API for all core functionality. It re-exports types and functions from:

- **pluresdb-core**: Core CRDTs, data structures, and query primitives
- **pluresdb-storage**: Storage abstraction layer with multiple backends
- **pluresdb-sync**: Sync orchestration primitives for P2P peers

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
pluresdb = "1.4.2"
```

## Quick Start

```rust
use pluresdb::{CrdtStore, MemoryStorage, StorageEngine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an in-memory database
    let storage = MemoryStorage::default();
    let store = CrdtStore::default();
    
    // Insert a node
    let node_id = store.put("node-1", "actor-1", serde_json::json!({
        "name": "Alice",
        "age": 30
    }));
    
    // Store it
    storage.put(pluresdb::StoredNode {
        id: node_id.clone(),
        payload: serde_json::json!({"name": "Alice", "age": 30}),
    }).await?;
    
    // Retrieve it
    let node = storage.get(&node_id).await?;
    println!("Retrieved: {:?}", node);
    
    Ok(())
}
```

## Using Individual Crates

If you prefer to depend on individual crates directly, you can:

```toml
[dependencies]
pluresdb-core = "1.4.2"
pluresdb-storage = "1.4.2"
pluresdb-sync = "1.4.2"
```

This gives you more control over dependencies and can result in smaller binary sizes if you only need specific functionality.

## Features

- `default`: Includes tokio for async support
- `async`: Enables async/await support (included in default)

## Documentation

- [API Documentation](https://docs.rs/pluresdb)
- [Core Crate](https://docs.rs/pluresdb-core)
- [Storage Crate](https://docs.rs/pluresdb-storage)
- [Sync Crate](https://docs.rs/pluresdb-sync)

## License

AGPL-3.0

