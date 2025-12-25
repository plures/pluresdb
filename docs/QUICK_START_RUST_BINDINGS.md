# Quick Start: Implementing Rust Bindings

This guide provides **immediate, actionable steps** to start implementing Rust bindings for Node.js and Deno.

---

## Prerequisites

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Node.js build tools
npm install -g node-gyp

# Install Deno
curl -fsSL https://deno.land/install.sh | sh

# Install deno_bindgen CLI
cargo install deno_bindgen_cli
```

---

## Step 1: Node.js Bindings (N-API) - 30 Minutes

### 1.1 Update Cargo.toml

```toml
# crates/pluresdb-node/Cargo.toml
[package]
name = "pluresdb-node"
version = "1.2.10"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
napi = { version = "2.15", features = ["napi6", "serde-json", "tokio_rt"] }
napi-derive = "2.15"
pluresdb-core = { path = "../pluresdb-core" }
pluresdb-storage = { path = "../pluresdb-storage" }
tokio = { workspace = true, features = ["rt-multi-thread"] }
serde = { workspace = true }
serde_json = { workspace = true }

[build-dependencies]
napi-build = "2.15"
```

### 1.2 Create Build Script

```rust
// crates/pluresdb-node/build.rs
fn main() {
    napi_build::setup();
}
```

### 1.3 Implement Basic Bindings

```rust
// crates/pluresdb-node/src/lib.rs
use napi::bindgen_prelude::*;
use napi_derive::napi;
use pluresdb_core::{Database, DatabaseOptions};
use pluresdb_storage::SledStorage;
use std::sync::Arc;
use tokio::runtime::Runtime;

#[napi]
pub struct PluresDatabase {
    inner: Arc<Database>,
    rt: Arc<Runtime>,
}

#[napi]
impl PluresDatabase {
    #[napi(constructor)]
    pub fn new(options: Option<DatabaseOptions>) -> Result<Self> {
        let rt = Arc::new(
            Runtime::new().map_err(|e| Error::from_reason(format!("Failed to create runtime: {}", e)))?
        );
        
        let storage = rt.block_on(async {
            SledStorage::new("./data").await
                .map_err(|e| Error::from_reason(format!("Storage error: {}", e)))?
        })?;
        
        let db = Database::new(storage, options.unwrap_or_default())
            .map_err(|e| Error::from_reason(format!("Database error: {}", e)))?;
        
        Ok(Self {
            inner: Arc::new(db),
            rt,
        })
    }

    #[napi]
    pub async fn put(&self, id: String, data: serde_json::Value) -> Result<()> {
        let db = self.inner.clone();
        db.put(id, data).await
            .map_err(|e| Error::from_reason(format!("Put error: {}", e)))
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<serde_json::Value>> {
        let db = self.inner.clone();
        db.get(id).await
            .map_err(|e| Error::from_reason(format!("Get error: {}", e)))
    }

    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let db = self.inner.clone();
        db.delete(id).await
            .map_err(|e| Error::from_reason(format!("Delete error: {}", e)))
    }
}

#[napi]
pub fn init() -> Result<()> {
    Ok(())
}
```

### 1.4 Create Package Configuration

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
      "defaults": true
    }
  },
  "scripts": {
    "build": "napi build --platform --release",
    "build:debug": "napi build --platform"
  }
}
```

### 1.5 Build and Test

```bash
cd crates/pluresdb-node
npm install
npm run build

# Test in Node.js
node -e "const db = require('./index.js'); console.log(db);"
```

---

## Step 2: Deno Bindings (FFI) - 30 Minutes

### 2.1 Update Cargo.toml

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
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
```

### 2.2 Implement FFI Bindings

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
    rt: Arc<Runtime>,
}

#[deno_bindgen]
impl DatabaseHandle {
    #[deno_bindgen(constructor)]
    pub fn new(data_dir: Option<String>) -> Result<Self, String> {
        let rt = Arc::new(
            Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?
        );
        
        let storage = rt.block_on(async {
            let path = data_dir.unwrap_or_else(|| "./data".to_string());
            SledStorage::new(&path).await
                .map_err(|e| format!("Storage error: {}", e))?
        })?;
        
        let db = Database::new(storage, DatabaseOptions::default())
            .map_err(|e| format!("Database error: {}", e))?;
        
        Ok(Self {
            inner: Arc::new(db),
            rt,
        })
    }

    #[deno_bindgen]
    pub async fn put(&self, id: String, data: serde_json::Value) -> Result<(), String> {
        let db = self.inner.clone();
        db.put(id, data).await
            .map_err(|e| format!("Put error: {}", e))
    }

    #[deno_bindgen]
    pub async fn get(&self, id: String) -> Result<Option<serde_json::Value>, String> {
        let db = self.inner.clone();
        db.get(id).await
            .map_err(|e| format!("Get error: {}", e))
    }

    #[deno_bindgen]
    pub async fn delete(&self, id: String) -> Result<(), String> {
        let db = self.inner.clone();
        db.delete(id).await
            .map_err(|e| format!("Delete error: {}", e))
    }
}
```

### 2.3 Generate Bindings

```bash
cd crates/pluresdb-deno
cargo build --release
deno_bindgen

# This creates:
# - bindings/bindings.ts
# - bindings/bindings.js
```

### 2.4 Create Deno Module

```typescript
// crates/pluresdb-deno/mod.ts
import { load } from "https://deno.land/x/plug@1.0.0/plug.ts";
import type { DatabaseHandle } from "./bindings/bindings.ts";

const lib = await load({
  name: "pluresdb_deno",
  url: "file:///path/to/crates/pluresdb-deno/target/release",
  // Adjust paths based on your platform
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
}

export default PluresDB;
```

### 2.5 Test in Deno

```bash
deno run --allow-ffi --allow-read --allow-write test.ts
```

---

## Step 3: Integration Testing

### 3.1 Node.js Test

```javascript
// test-node.js
const { PluresDatabase } = require('./crates/pluresdb-node/index.js');

async function test() {
  const db = new PluresDatabase();
  await db.put('test', { name: 'test' });
  const result = await db.get('test');
  console.log('Result:', result);
}

test().catch(console.error);
```

### 3.2 Deno Test

```typescript
// test-deno.ts
import { PluresDB } from './crates/pluresdb-deno/mod.ts';

async function test() {
  const db = new PluresDB();
  await db.put('test', { name: 'test' });
  const result = await db.get('test');
  console.log('Result:', result);
}

test().catch(console.error);
```

---

## Common Issues & Solutions

### Issue: "Cannot find native module"

**Solution:** Ensure the `.node` file is in the correct location and has correct permissions.

```bash
# Check file exists
ls -la crates/pluresdb-node/*.node

# Fix permissions if needed
chmod +x crates/pluresdb-node/*.node
```

### Issue: "Runtime error: Tokio runtime already initialized"

**Solution:** Use a single shared runtime or use `Handle::current()`.

```rust
// Instead of creating new runtime, use existing one
use tokio::runtime::Handle;

#[napi]
pub async fn put(&self, id: String, data: serde_json::Value) -> Result<()> {
    Handle::current().spawn(async move {
        // async work
    }).await
}
```

### Issue: "FFI symbol not found"

**Solution:** Ensure the library is built and exported correctly.

```bash
# Rebuild
cargo clean
cargo build --release

# Regenerate bindings
deno_bindgen
```

---

## Next Steps

1. ✅ Implement basic CRUD operations
2. ✅ Add error handling
3. ✅ Add vector search
4. ✅ Add subscriptions
5. ✅ Add mesh networking
6. ✅ Create compatibility layer
7. ✅ Migrate consumers
8. ✅ Remove TypeScript

---

**Estimated Time:** 1-2 hours for basic implementation  
**Difficulty:** Medium  
**Prerequisites:** Rust, Node.js, Deno knowledge

