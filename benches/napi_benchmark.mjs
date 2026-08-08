/**
 * N-API benchmark suite for PluresDB regression tracking.
 *
 * Measures core operations through the Node.js binding layer to detect
 * performance regressions in the N-API target.
 *
 * Usage:
 *   node benches/napi_benchmark.mjs
 *
 * Requires: pluresdb native module built (npm run build or napi build).
 */

import { performance } from "node:perf_hooks";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// Attempt to load the native module from standard build locations.
let PluresDatabase;
try {
  const mod = await import("../crates/pluresdb-node/index.js");
  PluresDatabase = mod.PluresDatabase || mod.default?.PluresDatabase;
} catch {
  console.error(
    "Could not load PluresDatabase native module. Build it first with `npm run build` (or `cd crates/pluresdb-node && npm run build`).",
  );
  process.exit(1);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeTempDir() {
  return mkdtempSync(join(tmpdir(), "pluresdb-bench-"));
}

function median(arr) {
  const sorted = [...arr].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

async function bench(name, iterations, fn) {
  // Warmup
  for (let i = 0; i < Math.min(5, iterations); i++) {
    await fn();
  }

  const times = [];
  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    await fn();
    times.push(performance.now() - start);
  }

  const med = median(times);
  const min = Math.min(...times);
  const max = Math.max(...times);

  return { name, iterations, median_ms: med, min_ms: min, max_ms: max };
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

const results = [];
const ITERATIONS = 20;

// CRDT put
{
  const dir = makeTempDir();
  try {
    const db = new PluresDatabase(undefined, dir);
    const r = await bench("crdt_put_1000", ITERATIONS, () => {
      for (let i = 0; i < 1000; i++) {
        db.put(`bench:${i}`, { value: i, data: "benchmark payload" });
      }
    });
    results.push(r);
    db.close?.();
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// CRDT get
{
  const dir = makeTempDir();
  try {
    const db = new PluresDatabase(undefined, dir);
    for (let i = 0; i < 1000; i++) {
      db.put(`bench:${i}`, { value: i });
    }
    const r = await bench("crdt_get_1000", ITERATIONS, () => {
      for (let i = 0; i < 1000; i++) {
        db.get(`bench:${i}`);
      }
    });
    results.push(r);
    db.close?.();
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// CRDT list
{
  const dir = makeTempDir();
  try {
    const db = new PluresDatabase(undefined, dir);
    for (let i = 0; i < 1000; i++) {
      db.put(`bench:${i}`, { value: i });
    }
    const r = await bench("crdt_list_1000", ITERATIONS, () => {
      db.list();
    });
    results.push(r);
    db.close?.();
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// ---------------------------------------------------------------------------
// Output results (JSON for CI consumption)
// ---------------------------------------------------------------------------

console.log(JSON.stringify({ target: "napi", results }, null, 2));
