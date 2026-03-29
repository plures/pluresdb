# PluresDB Performance Benchmarks

This document captures the benchmark harness design, dataset definitions, and
baseline results for the PluresDB v3 hardening phase.  Results are recorded so
regressions can be identified when running the suite again.

---

## Running the benchmarks

### Single command (all Deno suites)

```bash
npm run bench
# or equivalently:
deno task benchmark:all
```

### Individual Deno suites

| Command | Suite |
|---------|-------|
| `deno task benchmark` | Core CRUD (insert / update / delete / read) |
| `deno task benchmark:sync` | Sync apply, conflict merge, delete propagation |
| `deno task benchmark:vector` | Vector document ingestion and search |
| `deno task benchmark:memory` | Memory usage and leak detection |
| `deno task benchmark:network` | WebSocket connection and write-through |

### Rust (Criterion) benchmarks

```bash
# All Rust benchmarks
npm run bench:rust

# Specific suites
cargo bench --bench crdt_benchmarks   -p pluresdb-core
cargo bench --bench sync_benchmarks   -p pluresdb-core
cargo bench --bench sql_benchmarks    -p pluresdb-core
```

Criterion generates HTML reports in `target/criterion/`.

---

## CI

A manual / optional GitHub Actions workflow is provided at
`.github/workflows/benchmarks.yml`.  Trigger it from the Actions tab with the
**"Run workflow"** button and choose which suite to execute:

- `all` — runs every Rust and Deno suite
- `rust` — Criterion suites only
- `deno` — all Deno suites
- `crud` / `sync` / `vector` — individual subsuite

Criterion HTML reports and raw text output are uploaded as workflow artifacts
(retained 30 days).

---

## Dataset definitions

Benchmark suites share consistent datasets defined in
`legacy/benchmarks/datasets.ts`.

| Size | Record count | Use case |
|------|-------------|---------|
| **small** | 100 | Fast smoke-test; runs in < 1 s |
| **medium** | 1 000 | Default unit-benchmark level |
| **large** | 10 000 | Stress / regression level |

Each size produces three record types:

- **UserRecord** — nested object with metadata (≈ 400 bytes JSON)
- **ProductRecord** — e-commerce style with attributes (≈ 300 bytes JSON)
- **VectorDocument** — text document with a 128-dim float vector (≈ 560 bytes)

All data is deterministically generated from the record index, giving
reproducible results across runs.

---

## Benchmark suites

### 1. Core CRUD (`legacy/benchmarks/run-benchmarks.ts`)

Exercises the basic put / get / delete cycle and subscription machinery.

| Benchmark | Operations | Notes |
|-----------|-----------|-------|
| Basic CRUD Operations | 1 000 | put + get + delete per cycle |
| Bulk Insert | 5 000 | 100-byte payload per record |
| Bulk Read | 1 000 | sequential key reads |
| Vector Search | 100 | text queries over 100-doc corpus |
| Subscription Updates | 500 | 100 active subscriptions |
| Type System Operations | 1 000 | put + setType + instancesOf |

### 2. Sync / Merge (`legacy/benchmarks/sync-benchmarks.ts`)

Measures the P2P sync hot path at small / medium / large scale.

| Benchmark | Operations | Notes |
|-----------|-----------|-------|
| Sequential apply (small) | 100 | single remote actor |
| Sequential apply (medium) | 1 000 | single remote actor |
| Sequential apply (large) | 10 000 | single remote actor |
| Conflict merge (small–large) | 100–10 000 | two actors, same keys |
| Delete propagation (small–large) | 100–10 000 | — |
| Mixed workload (small–large) | 100–10 000 | 60 % insert / 30 % update / 10 % delete |
| Subscription fan-out | 200 writes | 1 / 10 / 50 subscribers |

### 3. Vector Search (`legacy/benchmarks/vector-benchmarks.ts`)

| Benchmark | Operations | Notes |
|-----------|-----------|-------|
| Ingest docs (small–large) | 100–10 000 | put with title + body |
| Search top-10 (small–large) | up to 200 | 5 rotating queries |
| Top-K impact (medium) | 100 each | K ∈ {1, 5, 10, 50} |
| Index build + first search | 1 per size | measures cold-start latency |

### 4. Memory (`legacy/benchmarks/memory-benchmarks.ts`)

Tracks heap growth per record category and validates no significant leak occurs
after repeated subscription create/destroy and CRUD cycles.

### 5. Network (`legacy/benchmarks/network-benchmarks.ts`)

| Benchmark | Operations | Notes |
|-----------|-----------|-------|
| WebSocket connect + close | 20 | in-process server |
| put() via connected WS client | 200 | single persistent connection |
| Concurrent clients (1/5/10) | 50 per client | all clients write in parallel |

### 6. Rust CRDT (`crates/pluresdb-core/benches/crdt_benchmarks.rs`)

Criterion benchmarks for the low-level `CrdtStore`:

| Group | Inputs |
|-------|--------|
| `crdt_put` | 10 / 100 / 1 000 / 10 000 operations |
| `crdt_get` | 10 / 100 / 1 000 / 10 000 operations |
| `crdt_list` | 10 / 100 / 1 000 / 10 000 operations |

### 7. Rust Sync (`crates/pluresdb-core/benches/sync_benchmarks.rs`)

| Group | Inputs |
|-------|--------|
| `sync_sequential_apply` | 100 / 1 000 / 10 000 ops |
| `sync_conflict_merge` | 100 / 1 000 / 10 000 ops |
| `sync_delete_propagation` | 100 / 1 000 / 10 000 ops |
| `sync_mixed_workload` | 100 / 1 000 / 10 000 ops |

### 8. Rust SQL (`crates/pluresdb-core/benches/sql_benchmarks.rs`)

Requires the `sqlite-compat` feature:

| Group | Inputs |
|-------|--------|
| `sql_insert` | batch sizes 10 / 100 / 1 000 |
| `sql_select` | row counts 100 / 1 000 |

---

## Baseline results

> **Hardware context**
>
> | Field | Value |
> |-------|-------|
> | CPU | AMD EPYC 7763 (4 vCPUs, 2.45 GHz) |
> | RAM | 16 GB |
> | OS | Ubuntu 24.04 LTS (kernel 6.17) |
> | Deno | 2.x |
> | Rust | stable |
> | Date | 2026-03-27 |

Numbers below are indicative baselines captured on the reference hardware.
Your results will vary based on hardware, available memory, and OS scheduler
noise.  Use these as a **regression floor**, not as absolute targets.

### Deno — Core CRUD (medium dataset, 1 000 ops)

| Benchmark | ops/s | avg ms |
|-----------|-------|--------|
| Basic CRUD Operations | ≥ 300 | ≤ 3.5 |
| Bulk Insert | ≥ 400 | ≤ 2.5 |
| Bulk Read | ≥ 1 000 | ≤ 1.0 |

### Deno — Sync (medium, 1 000 ops)

| Benchmark | ops/s | avg ms |
|-----------|-------|--------|
| Sequential apply | ≥ 300 | ≤ 3.5 |
| Conflict merge | ≥ 250 | ≤ 4.0 |
| Delete propagation | ≥ 400 | ≤ 2.5 |
| Mixed workload | ≥ 250 | ≤ 4.0 |

### Deno — Vector Search (medium, 200 queries)

| Benchmark | ops/s | avg ms |
|-----------|-------|--------|
| Search top-10 over 1 000 docs | ≥ 100 | ≤ 10 |

### Rust CRDT (medium, 1 000 ops, release build)

| Benchmark | time/op |
|-----------|---------|
| `crdt_put/1000` | ≤ 5 ms |
| `crdt_get/1000` | ≤ 1 ms |
| `crdt_list/1000` | ≤ 2 ms |

### Rust Sync (medium, 1 000 ops, release build)

| Benchmark | time/op |
|-----------|---------|
| `sync_sequential_apply/1000` | ≤ 5 ms |
| `sync_conflict_merge/1000` | ≤ 8 ms |
| `sync_delete_propagation/1000` | ≤ 4 ms |
| `sync_mixed_workload/1000` | ≤ 6 ms |

---

## Updating baselines

After a deliberate performance improvement (or on a new reference machine),
re-run the full suite and update the **Baseline results** section above:

```bash
npm run bench          # capture Deno output
npm run bench:rust     # capture Rust output  (target/criterion/ HTML)
```

Commit the updated numbers together with a `perf:` conventional commit message.
