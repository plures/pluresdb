# PluresDB Rust-First Rebuild Plan (GUN-compatible + Extended Capabilities)

Status: draft v0.1  
Branch: `feature/pluresdb-rust-first-review`

## 1) Target State
A modular, Rust-first system that provides:
- GUN.js ecosystem functional parity (core graph/CRDT semantics, SEA security model, RAD-style storage abstraction, relay/wire compatibility)
- Hyperswarm-native sync transport
- PluresDB extensions (vector search, procedures runtime hooks, Praxis capabilities)
- High performance, reliability, and deterministic behavior under load

## 2) Architecture Modules (clean boundaries)
1. `pluresdb-core`
   - canonical data model, graph operations, conflict/merge semantics
2. `pluresdb-crdt`
   - HAM-compatible merge engine + deterministic conflict tests
3. `pluresdb-sea`
   - auth, signatures, encryption, key management
4. `pluresdb-storage` (RAD-equivalent)
   - storage trait + adapters (sqlite, rocksdb, memory)
5. `pluresdb-wire-gun`
   - GUN-compatible wire codec (`get/put/ack`) + compatibility shim
6. `pluresdb-relay`
   - websocket relay endpoint + peer fanout rules
7. `pluresdb-sync`
   - hyperswarm transport + replication orchestration
8. `pluresdb-procedures`
   - procedure execution engine integration points
9. `pluresdb-vector`
   - vector index/search and embedding-backed retrieval
10. `pluresdb-observability`
   - metrics/tracing/health and SLO indicators

## 3) Phased Delivery

### Phase 0 — Baseline & Stabilization (now)
- Inventory current crates and ownership boundaries
- Define invariants and compatibility contract
- Build golden test corpus from current behavior + legacy TS references

### Phase 1 — Compatibility Core
- Implement/validate GUN wire compatibility layer
- Implement HAM-compatible deterministic merge module
- Deliver relay compatibility endpoint

### Phase 2 — Security + Storage
- SEA module with explicit crypto policy
- RAD-style storage adapters and migration harness
- key lifecycle + topic permissions

### Phase 3 — Sync + Transport
- hyperswarm transport productionization
- replication protocol with backpressure and recovery
- encrypted-at-rest/transport merge behavior

### Phase 4 — Extensions (Plures-native)
- procedures runtime integration
- praxis capability hooks
- vector/search optimization path

### Phase 5 — Hardening
- fuzz + property tests for CRDT/codec
- perf benchmarks and regression gates
- failure injection and soak testing

## 4) Quality Gates
- No module merges without contract tests
- Cross-version compatibility tests mandatory for wire/shim
- Performance budgets tracked in CI
- Security review required for SEA/key changes

## 5) Immediate Next Steps
1. Publish crate-level maturity map (implemented/partial/stub)
2. Freeze protocol/merge contracts into formal spec docs
3. Build first compatibility test harness for `get/put/ack`
4. Define ownership per module and sequencing dependencies

