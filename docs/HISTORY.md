# PluresDB Development History

A high-level summary of major milestones in PluresDB's development.

## v2.1 — WASM & Rename (2025–2026)

- **Renamed** GunDB → PluresDB across the entire codebase (#95)
- **pluresdb-wasm**: resolved IndexedDB / web-sys API compatibility issues for browser targets (#103)

## v2.0 — Rust-Native Rewrite (2024–2025)

- **Core engine rewritten in Rust** (`crates/pluresdb-core`) using RocksDB/Sled storage with CRDT conflict resolution via vector clocks
- **N-API bindings** for Node.js (`crates/pluresdb-node`) providing a synchronous better-sqlite3-compatible API with 10x+ performance improvement over the TypeScript prototype
- **Deno bindings** (`crates/pluresdb-deno`) published to JSR as `@plures/pluresdb`
- **P2P sync layer** (`crates/pluresdb-sync`) with Hyperswarm DHT transport, relay WebSocket fallback, and BLAKE2b topic derivation
- **Vector search** integrated into the core store using HNSW indexing (hnsw_rs) with cosine similarity
- **Auto-embedding on insert** via pluggable `EmbedText` backend (FastEmbedder with fastembed/ONNX, feature-gated) (#93)
- **Web UI** (Svelte, 24-tab management interface) bundled into the CLI server
- **CLI** (`crates/pluresdb-cli`) with `serve`, `import`, `export`, and `migrate` subcommands
- **Windows packaging**: MSI installer and winget manifest published to the Windows Package Manager Community Repository
- **Local-first architecture**: all data stored on-device; P2P sync is opt-in

## v1.x — TypeScript Prototype (2023–2024)

- Initial TypeScript/Deno implementation of the CRDT graph database (`legacy/`)
- SQLite-compatible API surface (`legacy/sqlite-compat.ts`)
- Hyperswarm-based P2P discovery and sync (`legacy/network/hyperswarm-sync.ts`)
- Express REST API and WebSocket server for browser/extension access
- Published to npm as `pluresdb` and to the Deno registry

## See Also

- [CHANGELOG.md](../CHANGELOG.md) — detailed per-release change log
- [ROADMAP.md](../ROADMAP.md) — planned future work
- [CONTRIBUTING.md](../CONTRIBUTING.md) — how to contribute
