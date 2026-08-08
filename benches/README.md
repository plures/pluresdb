# PluresDB Benchmark Suite

Regression tracking benchmarks across all PluresDB compilation targets.

## Targets

| Target | Tool | Location |
|--------|------|----------|
| **Native** (Rust) | Criterion | `crates/pluresdb-core/benches/` |
| **N-API** (Node.js) | Custom harness | `benches/napi_benchmark.mjs` |
| **WASM** | Custom harness | `benches/wasm_benchmark.mjs` |

## Running locally

### Native benchmarks

```bash
# All native benchmarks
cargo bench -p pluresdb-core --features native

# Individual suites
cargo bench --bench crdt_benchmarks -p pluresdb-core --features native
cargo bench --bench sync_benchmarks -p pluresdb-core --features native
cargo bench --bench vector_benchmarks -p pluresdb-core --features native
```

### N-API benchmarks

```bash
npm run build        # Build the native Node.js module
node benches/napi_benchmark.mjs
```

### WASM benchmarks

```bash
wasm-pack build crates/pluresdb-wasm --target nodejs
node benches/wasm_benchmark.mjs
```

## CI

The `.github/workflows/benchmarks.yml` workflow runs all three target suites on
every push to `main` and on pull requests. Results are uploaded as artifacts for
comparison.

## Adding new benchmarks

- **Native:** Add a new function to an existing `*_benchmarks.rs` file or create
  a new file in `crates/pluresdb-core/benches/` with a corresponding `[[bench]]`
  entry in `Cargo.toml`.
- **N-API / WASM:** Add new benchmark functions to the respective `.mjs` file.
