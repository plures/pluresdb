# PluresDB Roadmap

## Role in OASIS
PluresDB is the data backbone for OASIS: a CRDT graph database with vector search and P2P sync that powers local‑first commerce state, agent memory, and cross‑device replication. Every OASIS subsystem depends on PluresDB for privacy‑preserving storage and synchronization.

## Current State
v3.0.1 (workspace) is a Rust‑first implementation with CRDT storage, vector search, SQLite compatibility, a procedures engine, and Hyperswarm-based P2P sync. Recent work landed timer-based procedure triggers, CLI health diagnostics, memory‑efficient vector storage, and multiple CI fixes. One open issue remains for bounded memory usage in sled cache configuration.

## Milestones

### Phase 1 — Stability + Resource Control
- Cap sled cache capacity and validate RSS limits under vector workloads (open issue #371).
- Add benchmark suite for regression tracking across N-API/WASM/native targets.
- Improve WAL and sync recovery paths for corrupted CRDT entries.
- Harden `pluresdb doctor` output coverage (network, storage, vector index, procedures).

### Phase 2 — Procedures Engine v2
- Conditional branching and parallel execution steps.
- Procedure versioning + migration support.
- Debugger UI/CLI with step‑level state inspection.
- Event‑driven triggers on data changes (not just cron/interval).

### Phase 3 — P2P & Selective Sync
- Selective sync by graph/collection with bandwidth throttling.
- Sync progress events and conflict hooks for app‑level resolution UX.
- Relay/transport improvements for restricted networks.

### Phase 4 — Query & Indexing
- Graph path queries with filters and traversal constraints.
- Full‑text search index alongside vector search.
- Materialized views and cost‑based query planner.

### Phase 5 — Ecosystem Expansion
- Stable HTTP/GraphQL gateway for non‑Rust/JS clients.
- Python bindings (PyO3) and React bindings parity.
- Plugin system for custom storage/index engines.
