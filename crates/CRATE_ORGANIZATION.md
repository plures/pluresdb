# PluresDB Crate Organization

## Overview

PluresDB is organized into multiple crates for modularity and flexibility. This document explains the crate structure and when to use each crate.

## Main Library Crate

### `pluresdb` (Recommended for most users)

The main unified crate that re-exports all core functionality. This is the recommended entry point for most users.

**Use this crate when:**
- You want a simple, unified API
- You need multiple PluresDB features (core + storage + sync)
- You're building a new application

**Installation:**
```toml
[dependencies]
pluresdb = "1.4.2"
```

**Example:**
```rust
use pluresdb::{CrdtStore, MemoryStorage, StorageEngine};

let (store, storage) = pluresdb::new_memory_database();
```

## Individual Crates

### `pluresdb-core`

Core CRDTs, data structures, and query primitives. The foundation of PluresDB.

**Use this crate when:**
- You only need CRDT functionality
- You're building a custom storage backend
- You want minimal dependencies

**Installation:**
```toml
[dependencies]
pluresdb-core = "1.4.2"
```

### `pluresdb-storage`

Storage abstraction layer with multiple backends (Memory, Sled).

**Use this crate when:**
- You need storage functionality
- You're building a custom application
- You want to use specific storage backends

**Installation:**
```toml
[dependencies]
pluresdb-storage = "1.4.2"
```

### `pluresdb-sync`

Sync orchestration primitives for P2P peers.

**Use this crate when:**
- You need P2P synchronization
- You're building distributed applications
- You need event broadcasting

**Installation:**
```toml
[dependencies]
pluresdb-sync = "1.4.2"
```

## Application Crates

### `pluresdb-cli`

Command-line interface for managing PluresDB nodes. This is a binary crate, not a library.

**Use this when:**
- You want to use the PluresDB CLI tool
- You're installing the command-line interface

**Installation:**
```bash
cargo install pluresdb-cli
```

## Binding Crates

### `pluresdb-node`

Node.js bindings using N-API. For use in Node.js applications.

**Use this when:**
- You're building a Node.js application
- You need native performance in Node.js

**Installation:**
```bash
npm install @plures/pluresdb-native
```

### `pluresdb-deno`

Deno bindings using deno_bindgen FFI. For use in Deno applications.

**Use this when:**
- You're building a Deno application
- You need native performance in Deno

**Installation:**
```bash
deno add jsr:@plures/pluresdb
```

## Crate Dependencies

```
pluresdb (main crate)
├── pluresdb-core
├── pluresdb-storage
│   └── pluresdb-core (indirect)
└── pluresdb-sync

pluresdb-cli
├── pluresdb-core
├── pluresdb-storage
└── pluresdb-sync

pluresdb-node
├── pluresdb-core
└── pluresdb-sync

pluresdb-deno
├── pluresdb-core
└── pluresdb-sync
```

## Choosing the Right Crate

### For Rust Applications

**Most users:** Use `pluresdb` (unified crate)
```toml
[dependencies]
pluresdb = "1.4.2"
```

**Minimal dependencies:** Use individual crates
```toml
[dependencies]
pluresdb-core = "1.4.2"
pluresdb-storage = "1.4.2"  # Only if needed
pluresdb-sync = "1.4.2"     # Only if needed
```

### For Node.js Applications

```bash
npm install @plures/pluresdb-native
```

### For Deno Applications

```bash
deno add jsr:@plures/pluresdb
```

### For CLI Usage

```bash
cargo install pluresdb-cli
```

## Version Compatibility

All crates share the same version number (currently 1.4.2) and are designed to work together. When updating, update all crates to the same version.

## Migration Guide

### From Individual Crates to Unified Crate

If you're currently using individual crates:

**Before:**
```toml
[dependencies]
pluresdb-core = "1.4.2"
pluresdb-storage = "1.4.2"
pluresdb-sync = "1.4.2"
```

```rust
use pluresdb_core::CrdtStore;
use pluresdb_storage::MemoryStorage;
use pluresdb_sync::SyncBroadcaster;
```

**After:**
```toml
[dependencies]
pluresdb = "1.4.2"
```

```rust
use pluresdb::{CrdtStore, MemoryStorage, SyncBroadcaster};
```

The unified crate provides the same functionality with a cleaner import path.

## Benefits of the Unified Crate

1. **Simpler API**: One import path instead of multiple
2. **Better Discoverability**: All types in one place
3. **Consistent Versioning**: One version to manage
4. **Easier Onboarding**: New users don't need to understand crate structure
5. **Flexibility**: Still allows using individual crates if needed

## When to Use Individual Crates

Use individual crates when:
- You only need specific functionality (e.g., just CRDTs)
- You want to minimize dependencies
- You're building a custom integration
- You need fine-grained control over features

## Summary

- **Most users**: Use `pluresdb` (unified crate)
- **Minimal dependencies**: Use `pluresdb-core` (and others as needed)
- **Node.js**: Use `@plures/pluresdb-native`
- **Deno**: Use `jsr:@plures/pluresdb`
- **CLI**: Install `pluresdb-cli`

