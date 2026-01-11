# pluresdb-storage

Storage abstraction layer for PluresDB, including in-memory and sled-backed implementations.

## Features

- **Multiple Storage Backends**
  - `MemoryStorage` - In-memory storage for testing and ephemeral use
  - `SledStorage` - Persistent storage using the sled embedded database

- **Encryption Support**
  - AES-256-GCM encryption
  - Configurable encryption metadata
  - Secure key management

- **Write-Ahead Logging (WAL)**
  - Transaction logging
  - Durability levels (None, Sync, Full)
  - WAL validation and replay

- **Replay System**
  - Rebuild state from WAL
  - Metadata pruning
  - Replay statistics

## Usage

```rust
use pluresdb_storage::{MemoryStorage, SledStorage, StorageEngine, StoredNode};

// In-memory storage
let storage = MemoryStorage::default();
let node = StoredNode {
    id: "node-1".to_string(),
    payload: serde_json::json!({"name": "Alice"}),
};
storage.put(node).await?;

// Persistent storage
let storage = SledStorage::open("./data")?;
let node = StoredNode {
    id: "node-2".to_string(),
    payload: serde_json::json!({"name": "Bob"}),
};
storage.put(node).await?;
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
pluresdb-storage = "1.4.2"
```

## Documentation

Full documentation available at: https://docs.rs/pluresdb-storage

## License

AGPL-3.0

