# PluresDB Rust Crates

This directory contains all Rust crates for the PluresDB project.

## Crates Overview

### Main Library Crate (Recommended)

- **pluresdb** - Unified main crate that re-exports all core functionality

### Core Crates

- **pluresdb-core** - Core CRDTs, data structures, and query primitives
- **pluresdb-sync** - Sync orchestration primitives for PluresDB peers
- **pluresdb-storage** - Storage abstraction layer with multiple backends

### Application Crates

- **pluresdb-cli** - Command-line interface for managing PluresDB nodes

### Bindings

- **pluresdb-node** - Node.js bindings using N-API
- **pluresdb-deno** - Deno bindings using deno_bindgen FFI

## Status

| Crate | Status | Registry |
|-------|--------|----------|
| pluresdb | ✅ Ready | crates.io |
| pluresdb-core | ✅ Published | crates.io |
| pluresdb-sync | ✅ Published | crates.io |
| pluresdb-storage | ✅ Ready | crates.io |
| pluresdb-cli | ✅ Ready | crates.io |
| pluresdb-node | ✅ Ready | npm |
| pluresdb-deno | ✅ Ready | JSR |

## Quick Start

### Installing Rust Crates

```bash
# Install CLI
cargo install pluresdb-cli

# Add main crate to your project (recommended)
cargo add pluresdb

# Or add individual crates if you prefer
cargo add pluresdb-core
cargo add pluresdb-storage
cargo add pluresdb-sync
```

### Installing Node.js Bindings

```bash
npm install @plures/pluresdb-native
```

### Installing Deno Bindings

```bash
deno add jsr:@plures/pluresdb
```

## Building from Source

```bash
# Build all crates
cargo build --workspace --release

# Build specific crate
cargo build -p pluresdb-cli --release

# Run tests
cargo test --workspace
```

## Publishing

See [PUBLISHING_GUIDE.md](./PUBLISHING_GUIDE.md) for detailed publishing instructions.

## Documentation

- [Crate Organization](./CRATE_ORGANIZATION.md) - Guide to choosing the right crate
- [Implementation Status](./IMPLEMENTATION_STATUS.md)
- [Completion Summary](./COMPLETION_SUMMARY.md)
- [Publishing Guide](./PUBLISHING_GUIDE.md)
- [Next Steps](./NEXT_STEPS.md)

## Version

All crates use the workspace version: **1.4.2**

## License

AGPL-3.0

