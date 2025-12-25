# Rust Migration Strategy: TypeScript → Rust with Node.js & Deno Bindings

**Date:** January 2025  
**Status:** Implementation Guide  
**Goal:** Complete migration from TypeScript to Rust while maintaining npm and Deno package compatibility

---

## Executive Summary

This document provides a **detailed, step-by-step strategy** for migrating PluresDB from TypeScript to Rust while ensuring npm and Deno packages continue to work. The approach uses **native bindings** (N-API for Node.js, FFI for Deno) to expose Rust functionality to JavaScript/TypeScript runtimes.

### Migration Phases

1. **Phase 1:** Create Rust bindings for Node.js (N-API)
2. **Phase 2:** Create Rust bindings for Deno (FFI)
3. **Phase 3:** Create compatibility layer (TypeScript wrappers)
4. **Phase 4:** Migrate consumers to Rust bindings
5. **Phase 5:** Remove TypeScript implementation

---

## Phase 1: Node.js Bindings (N-API)

### Overview

Use **N-API** (Node-API) to create native Node.js addons from Rust. N-API is the recommended approach for Node.js bindings as it provides ABI stability.

### Technology Stack

- **`napi-rs`**: High-level Rust bindings for N-API
- **`napi-derive`**: Macros for easier N-API usage
- **Build tool**: `napi-build` for build configuration

### Implementation Steps

#### Step 1.1: Update `pluresdb-node` Crate

```toml
# crates/pluresdb-node/Cargo.toml
[package]
name = "pluresdb-node"
version = "1.2.10"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
napi = { version = "2.15", features = ["napi6", "serde-json"] }
napi-derive = "2.15"
pluresdb-core = { path = "../pluresdb-core" }
pluresdb-storage = { path = "../pluresdb-storage" }
pluresdb-sync = { path = "../pluresdb-sync" }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

[build-dependencies]
napi-build = "2.15"
```

#### Step 1.2: Create Build Script

```rust
// crates/pluresdb-node/build.rs
fn main() {
    napi_build::setup();
}
```

#### Step 1.3: Implement N-API Bindings

```rust
// crates/pluresdb-node/src/lib.rs
use napi::bindgen_prelude::*;
use napi_derive::napi;
use pluresdb_core::{Database, DatabaseOptions, NodeId, NodeData};
use pluresdb_storage::{StorageEngine, SledStorage};
use std::sync::Arc;
use tokio::runtime::Runtime;

// Database wrapper for N-API
#[napi]
pub struct PluresDatabase {
    inner: Arc<Database>,
    rt: Runtime,
}

#[napi]
impl PluresDatabase {
    #[napi(constructor)]
    pub fn new(options: Option<DatabaseOptions>) -> Result<Self> {
        let rt = Runtime::new().map_err(|e| Error::from_reason(e.to_string()))?;
        
        let storage = rt.block_on(async {
            SledStorage::new("./data").await
                .map_err(|e| Error::from_reason(e.to_string()))?
        })?;
        
        let db = Database::new(storage, options.unwrap_or_default())
            .map_err(|e| Error::from_reason(e.to_string()))?;
        
        Ok(Self {
            inner: Arc::new(db),
            rt,
        })
    }

    #[napi]
    pub async fn put(&self, id: String, data: serde_json::Value) -> Result<()> {
        let db = self.inner.clone();
        self.rt.spawn(async move {
            db.put(id, data).await
                .map_err(|e| Error::from_reason(e.to_string()))
        }).await
        .map_err(|e| Error::from_reason(e.to_string()))?
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<serde_json::Value>> {
        let db = self.inner.clone();
        self.rt.spawn(async move {
            db.get(id).await
                .map_err(|e| Error::from_reason(e.to_string()))
        }).await
        .map_err(|e| Error::from_reason(e.to_string()))?
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let db = self.inner.clone();
        self.rt.spawn(async move {
            db.delete(id).await
                .map_err(|e| Error::from_reason(e.to_string()))
        }).await
        .map_err(|e| Error::from_reason(e.to_string()))?
    }

    #[napi]
    pub async fn vector_search(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<serde_json::Value>> {
        let db = self.inner.clone();
        self.rt.spawn(async move {
            db.vector_search(query, limit as usize).await
                .map_err(|e| Error::from_reason(e.to_string()))
        }).await
        .map_err(|e| Error::from_reason(e.to_string()))?
    }
}

// Export module
#[napi]
pub fn init() -> Result<()> {
    Ok(())
}
```

#### Step 1.4: Create TypeScript Definitions

```typescript
// crates/pluresdb-node/index.d.ts
export interface DatabaseOptions {
  dataDir?: string;
  port?: number;
}

export class PluresDatabase {
  constructor(options?: DatabaseOptions);
  put(id: string, data: any): Promise<void>;
  get(id: string): Promise<any | null>;
  delete(id: string): Promise<void>;
  vectorSearch(query: string, limit: number): Promise<any[]>;
}

export function init(): void;
```

#### Step 1.5: Build Configuration

```json
// crates/pluresdb-node/package.json
{
  "name": "@plures/pluresdb-native",
  "version": "1.2.10",
  "main": "index.js",
  "types": "index.d.ts",
  "napi": {
    "name": "pluresdb-node",
    "triples": {
      "defaults": true,
      "additional": [
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc"
      ]
    }
  },
  "scripts": {
    "build": "napi build --platform --release",
    "build:debug": "napi build --platform"
  }
}
```

#### Step 1.6: Build Commands

```bash
# Build for all platforms
cd crates/pluresdb-node
npm run build

# This generates:
# - pluresdb-node.linux-x64-gnu.node
# - pluresdb-node.darwin-x64.node
# - pluresdb-node.win32-x64-msvc.node
# - etc.
```

---

## Phase 2: Deno Bindings (FFI)

### Overview

Use **Deno FFI** (Foreign Function Interface) to call Rust functions from Deno. This is Deno's native way to interface with native code.

### Technology Stack

- **`deno_bindgen`**: Tool to generate Deno FFI bindings from Rust
- **`deno_core`**: Deno's core runtime (for advanced use cases)

### Implementation Steps

#### Step 2.1: Update `pluresdb-deno` Crate

```toml
# crates/pluresdb-deno/Cargo.toml
[package]
name = "pluresdb-deno"
version = "1.2.10"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
deno_bindgen = "0.10"
pluresdb-core = { path = "../pluresdb-core" }
pluresdb-storage = { path = "../pluresdb-storage" }
pluresdb-sync = { path = "../pluresdb-sync" }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

[build-dependencies]
deno_bindgen = "0.10"
```

#### Step 2.2: Implement FFI Bindings

```rust
// crates/pluresdb-deno/src/lib.rs
use deno_bindgen::deno_bindgen;
use pluresdb_core::{Database, DatabaseOptions};
use pluresdb_storage::SledStorage;
use std::sync::Arc;
use tokio::runtime::Runtime;

#[deno_bindgen]
pub struct DatabaseHandle {
    inner: Arc<Database>,
    rt: Runtime,
}

#[deno_bindgen]
impl DatabaseHandle {
    #[deno_bindgen(constructor)]
    pub fn new(data_dir: Option<String>) -> Result<Self, String> {
        let rt = Runtime::new().map_err(|e| e.to_string())?;
        
        let storage = rt.block_on(async {
            let path = data_dir.unwrap_or_else(|| "./data".to_string());
            SledStorage::new(&path).await
                .map_err(|e| e.to_string())?
        })?;
        
        let db = Database::new(storage, DatabaseOptions::default())
            .map_err(|e| e.to_string())?;
        
        Ok(Self {
            inner: Arc::new(db),
            rt,
        })
    }

    #[deno_bindgen]
    pub async fn put(&self, id: String, data: serde_json::Value) -> Result<(), String> {
        let db = self.inner.clone();
        self.rt.spawn(async move {
            db.put(id, data).await
                .map_err(|e| e.to_string())
        }).await
        .map_err(|e| e.to_string())?
    }

    #[deno_bindgen]
    pub async fn get(&self, id: String) -> Result<Option<serde_json::Value>, String> {
        let db = self.inner.clone();
        self.rt.spawn(async move {
            db.get(id).await
                .map_err(|e| e.to_string())
        }).await
        .map_err(|e| e.to_string())?
    }

    #[deno_bindgen]
    pub async fn delete(&self, id: String) -> Result<(), String> {
        let db = self.inner.clone();
        self.rt.spawn(async move {
            db.delete(id).await
                .map_err(|e| e.to_string())
        }).await
        .map_err(|e| e.to_string())?
    }

    #[deno_bindgen]
    pub async fn vector_search(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<serde_json::Value>, String> {
        let db = self.inner.clone();
        self.rt.spawn(async move {
            db.vector_search(query, limit as usize).await
                .map_err(|e| e.to_string())
        }).await
        .map_err(|e| e.to_string())?
    }
}
```

#### Step 2.3: Generate Deno Bindings

```bash
# Install deno_bindgen CLI
cargo install deno_bindgen_cli

# Generate bindings
cd crates/pluresdb-deno
deno_bindgen

# This generates:
# - bindings/bindings.ts (TypeScript definitions)
# - bindings/bindings.js (JavaScript bindings)
# - target/release/libpluresdb_deno.so (Linux)
# - target/release/libpluresdb_deno.dylib (macOS)
# - target/release/pluresdb_deno.dll (Windows)
```

#### Step 2.4: Create Deno Module Wrapper

```typescript
// crates/pluresdb-deno/mod.ts
import { load } from "https://deno.land/x/plug@1.0.0/plug.ts";
import type { DatabaseHandle } from "./bindings/bindings.ts";

// Load native library based on platform
const lib = await load({
  name: "pluresdb_deno",
  url: "https://github.com/plures/pluresdb/releases/download/v1.2.10",
  // Platform-specific library paths
  paths: {
    darwin: {
      x86_64: "./libpluresdb_deno.dylib",
      aarch64: "./libpluresdb_deno.dylib",
    },
    linux: {
      x86_64: "./libpluresdb_deno.so",
      aarch64: "./libpluresdb_deno.so",
    },
    windows: {
      x86_64: "./pluresdb_deno.dll",
      aarch64: "./pluresdb_deno.dll",
    },
  },
});

export class PluresDB {
  private handle: DatabaseHandle;

  constructor(dataDir?: string) {
    this.handle = new lib.DatabaseHandle(dataDir);
  }

  async put(id: string, data: any): Promise<void> {
    await this.handle.put(id, data);
  }

  async get(id: string): Promise<any | null> {
    return await this.handle.get(id);
  }

  async delete(id: string): Promise<void> {
    await this.handle.delete(id);
  }

  async vectorSearch(query: string, limit: number): Promise<any[]> {
    return await this.handle.vector_search(query, limit);
  }
}

export default PluresDB;
```

---

## Phase 3: Compatibility Layer (TypeScript Wrappers)

### Overview

Create TypeScript wrappers that maintain the **exact same API** as the current TypeScript implementation, but delegate to Rust bindings under the hood.

### Implementation Strategy

#### Step 3.1: Node.js Compatibility Layer

```typescript
// legacy/node-index.ts (updated to use Rust)
import { PluresDatabase } from "@plures/pluresdb-native";

// Maintain exact same API as before
export class GunDB {
  private db: PluresDatabase;

  constructor(options?: { dataDir?: string; port?: number }) {
    this.db = new PluresDatabase({
      dataDir: options?.dataDir,
      port: options?.port,
    });
  }

  async put(id: string, data: any): Promise<void> {
    await this.db.put(id, data);
  }

  async get(id: string): Promise<any | null> {
    return await this.db.get(id);
  }

  async delete(id: string): Promise<void> {
    await this.db.delete(id);
  }

  async vectorSearch(query: string, limit: number): Promise<any[]> {
    return await this.db.vectorSearch(query, limit);
  }

  // ... other methods delegate to Rust
}
```

#### Step 3.2: Deno Compatibility Layer

```typescript
// mod.ts (updated to use Rust)
import { PluresDB } from "./crates/pluresdb-deno/mod.ts";

// Maintain exact same API
export class GunDB {
  private db: PluresDB;

  constructor(options?: { dataDir?: string }) {
    this.db = new PluresDB(options?.dataDir);
  }

  async put(id: string, data: any): Promise<void> {
    await this.db.put(id, data);
  }

  // ... same pattern
}

export { GunDB };
```

---

## Phase 4: Migration Plan

### Step-by-Step Migration

#### Step 4.1: Feature Parity Checklist

Create a checklist of all TypeScript features that need Rust equivalents:

```markdown
## Feature Parity Checklist

### Core Features
- [x] CRUD operations (put, get, delete)
- [x] Vector search
- [ ] Subscriptions (on/off)
- [ ] Mesh networking
- [ ] CRDT conflict resolution
- [ ] Type system
- [ ] Rules engine

### API Features
- [ ] HTTP API server
- [ ] WebSocket server
- [ ] REST endpoints
- [ ] SSE streaming

### Storage Features
- [x] SQLite backend
- [x] RocksDB backend
- [x] Sled backend
- [ ] Migration system
- [ ] Backup/restore
```

#### Step 4.2: Incremental Migration

1. **Week 1-2:** Implement core CRUD bindings
2. **Week 3-4:** Implement vector search bindings
3. **Week 5-6:** Implement subscriptions bindings
4. **Week 7-8:** Implement mesh networking bindings
5. **Week 9-10:** Implement API server bindings
6. **Week 11-12:** Testing and bug fixes

#### Step 4.3: Testing Strategy

```typescript
// tests/migration.test.ts
import { describe, it, assertEquals } from "@std/testing";
import { GunDB } from "../mod.ts";

describe("Rust Migration Compatibility", () => {
  it("should maintain API compatibility", async () => {
    const db = new GunDB();
    await db.put("test", { name: "test" });
    const result = await db.get("test");
    assertEquals(result?.name, "test");
  });

  it("should handle vector search", async () => {
    const db = new GunDB();
    await db.put("doc1", { text: "machine learning" });
    const results = await db.vectorSearch("AI", 10);
    assertEquals(results.length, 1);
  });
});
```

---

## Phase 5: TypeScript Removal

### Prerequisites

Before removing TypeScript code:

1. ✅ All features implemented in Rust
2. ✅ All tests passing with Rust bindings
3. ✅ Performance benchmarks show Rust is faster
4. ✅ No breaking API changes
5. ✅ Documentation updated

### Removal Steps

#### Step 5.1: Update Package Exports

```json
// package.json (Node.js)
{
  "main": "./crates/pluresdb-node/index.js",
  "types": "./crates/pluresdb-node/index.d.ts",
  "exports": {
    ".": {
      "types": "./crates/pluresdb-node/index.d.ts",
      "require": "./crates/pluresdb-node/index.js"
    }
  }
}
```

```json
// deno.json
{
  "exports": {
    ".": "./crates/pluresdb-deno/mod.ts"
  }
}
```

#### Step 5.2: Archive Legacy Code

```bash
# Move legacy code to archive
git mv legacy/ archive/legacy-typescript-v1.2.10/
git commit -m "chore: archive TypeScript implementation"
```

#### Step 5.3: Update Documentation

- Update README to reflect Rust-only implementation
- Update API documentation
- Update migration guides

---

## Build & Distribution

### Node.js Package Build

```json
// package.json scripts
{
  "scripts": {
    "build:rust": "cd crates/pluresdb-node && npm run build",
    "build:types": "tsc --declaration --emitDeclarationOnly",
    "build": "npm run build:rust && npm run build:types"
  }
}
```

### Deno Package Build

```bash
# Build Rust library
cd crates/pluresdb-deno
cargo build --release

# Generate bindings
deno_bindgen

# Package for distribution
tar -czf pluresdb-deno-v1.2.10.tar.gz \
  mod.ts \
  bindings/ \
  target/release/libpluresdb_deno.*
```

---

## Troubleshooting

### Common Issues

#### Issue 1: Async Runtime Conflicts

**Problem:** Tokio runtime conflicts between Rust and Node.js/Deno

**Solution:** Use separate runtime or use async-friendly bindings

```rust
// Use tokio-compat or separate runtime
use tokio::runtime::Handle;

#[napi]
pub async fn put(&self, id: String, data: serde_json::Value) -> Result<()> {
    Handle::current().spawn(async move {
        // async work here
    }).await
}
```

#### Issue 2: Memory Management

**Problem:** Memory leaks or double-free errors

**Solution:** Use Arc/Rc for shared ownership, proper cleanup

```rust
impl Drop for PluresDatabase {
    fn drop(&mut self) {
        // Cleanup resources
    }
}
```

#### Issue 3: Type Conversion

**Problem:** JSON serialization/deserialization issues

**Solution:** Use serde_json consistently

```rust
use serde_json::Value;

#[napi]
pub fn put(&self, id: String, data: Value) -> Result<()> {
    // Value is automatically converted
}
```

---

## Performance Considerations

### Expected Performance Improvements

- **CRUD Operations:** 10x faster (Rust vs TypeScript)
- **Vector Search:** 5-10x faster (HNSW in Rust)
- **Memory Usage:** 4x lower (zero-cost abstractions)
- **Startup Time:** Similar (native bindings add minimal overhead)

### Benchmarking

```rust
// crates/pluresdb-node/benches/performance.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_put(c: &mut Criterion) {
    c.bench_function("put", |b| {
        b.iter(|| {
            // benchmark put operation
        });
    });
}

criterion_group!(benches, bench_put);
criterion_main!(benches);
```

---

## Timeline Estimate

### Conservative Timeline

- **Phase 1 (Node.js):** 4-6 weeks
- **Phase 2 (Deno):** 3-4 weeks
- **Phase 3 (Compatibility):** 2-3 weeks
- **Phase 4 (Migration):** 8-12 weeks
- **Phase 5 (Removal):** 2-3 weeks

**Total:** 19-28 weeks (~5-7 months)

### Aggressive Timeline

- **Phase 1-2 (Both bindings):** 6-8 weeks (parallel work)
- **Phase 3 (Compatibility):** 1-2 weeks
- **Phase 4 (Migration):** 6-8 weeks
- **Phase 5 (Removal):** 1-2 weeks

**Total:** 14-20 weeks (~3.5-5 months)

---

## Next Steps

1. **Immediate:** Start Phase 1 (Node.js bindings)
2. **Week 2:** Begin Phase 2 (Deno bindings) in parallel
3. **Week 4:** Start Phase 3 (compatibility layer)
4. **Week 6:** Begin Phase 4 (incremental migration)
5. **Week 14:** Start Phase 5 (TypeScript removal)

---

## Resources

- [napi-rs Documentation](https://napi.rs/)
- [Deno FFI Documentation](https://deno.land/manual/runtime/ffi_api)
- [deno_bindgen Guide](https://github.com/denoland/deno_bindgen)
- [Rust FFI Best Practices](https://michael-f-bryan.github.io/rust-ffi-guide/)

---

**Last Updated:** January 2025  
**Status:** Ready for Implementation

