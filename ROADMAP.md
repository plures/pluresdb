# PluresDB Roadmap

## Current: v3.4.0

## Phase 1: Stability & Performance (v3.5)
- [ ] Benchmark suite — automated regression benchmarks on CI (NAPI, WASM, native)
- [ ] WASM bundle size optimization — tree-shake unused features for browser builds
- [ ] Connection pool for multi-tenant NAPI usage
- [ ] SQLite WAL mode tuning for concurrent read/write workloads
- [ ] Error recovery — graceful handling of corrupted CRDT entries

## Phase 2: Procedures Engine v2 (v3.6)
- [ ] Conditional step type — if/else branching in procedure pipelines
- [ ] Parallel step type — concurrent execution of independent steps
- [ ] Procedure versioning — migrate running procedures across schema changes
- [ ] Procedure debugging — step-by-step execution with state inspection
- [ ] Event-driven triggers — fire procedures on data changes (not just manual/cron)

## Phase 3: P2P & Sync (v3.7)
- [ ] Hyperswarm relay transport — NAT traversal for restricted networks
- [ ] Selective sync — sync specific graphs/collections instead of full DB
- [ ] Sync conflict resolution UI hooks — expose merge decisions to applications
- [ ] Bandwidth-aware sync — throttle replication on metered connections
- [ ] Sync progress events — observable progress for large initial syncs

## Phase 4: Query & Index (v4.0)
- [ ] Full-text search index — inverted index alongside vector search
- [ ] Graph path queries — shortest path, traversal with filters
- [ ] Materialized views — pre-computed query results updated on write
- [ ] Index advisor — suggest indexes based on query patterns
- [ ] Query planner — cost-based optimization for complex joins

## Phase 5: Ecosystem (v4.1+)
- [ ] Deno Deploy adapter — run PluresDB on edge functions
- [ ] React bindings — equivalent of @plures/unum for React
- [ ] Python bindings — PyO3 bridge for data science workflows
- [ ] REST/GraphQL gateway — HTTP API for non-JS/Rust consumers
- [ ] Plugin system — user-defined storage engines and index types

