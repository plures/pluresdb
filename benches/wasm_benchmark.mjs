/**
 * WASM benchmark suite for PluresDB regression tracking.
 *
 * Measures core operations through the WebAssembly binding layer.
 * Runs in Node.js with the WASM module loaded.
 *
 * Usage:
 *   node benches/wasm_benchmark.mjs
 *
 * Requires: pluresdb-wasm built (wasm-pack build crates/pluresdb-wasm --target nodejs).
 */

import { performance } from "node:perf_hooks";

let wasm;
try {
  wasm = await import("../crates/pluresdb-wasm/pkg/pluresdb_wasm.js");
} catch {
  try {
    wasm = await import("../web/pkg/pluresdb_wasm.js");
  } catch {
    console.error(
      "Could not load PluresDB WASM module. Build it first with:\n" +
        "  wasm-pack build crates/pluresdb-wasm --target nodejs",
    );
    process.exit(1);
  }
}
const initWasm = wasm.default ?? wasm.init;
if (typeof initWasm === "function") {
  await initWasm();
}
// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function median(arr) {
  const sorted = [...arr].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

function bench(name, iterations, fn) {
  // Warmup
  for (let i = 0; i < Math.min(5, iterations); i++) {
    fn();
  }

  const times = [];
  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    fn();
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

const PluresDBBrowser = wasm.PluresDBBrowser || wasm.default?.PluresDBBrowser;
if (!PluresDBBrowser) {
  console.error("Could not find PluresDBBrowser export in WASM module.");
  process.exit(1);
}

// WasmCrdtStore doesn't expose put/get, so benchmark via PluresDBBrowser for now.
const useCrdtStore = false;
if (useCrdtStore) {
  // CrdtStore-based benchmarks
  {
    const store = new CrdtStore();
    const r = bench("wasm_crdt_put_1000", ITERATIONS, () => {
      for (let i = 0; i < 1000; i++) {
        store.put(`bench:${i}`, JSON.stringify({ value: i, data: "benchmark" }));
      }
    });
    results.push(r);
  }

  {
    const store = new CrdtStore();
    for (let i = 0; i < 1000; i++) {
      store.put(`bench:${i}`, JSON.stringify({ value: i }));
    }
    const r = bench("wasm_crdt_get_1000", ITERATIONS, () => {
      for (let i = 0; i < 1000; i++) {
        store.get(`bench:${i}`);
      }
    });
    results.push(r);
  }
} else {
  // PluresDB-class based benchmarks
  {
    const db = new PluresDB();
    const r = bench("wasm_crdt_put_1000", ITERATIONS, () => {
      for (let i = 0; i < 1000; i++) {
        db.put(`bench:${i}`, JSON.stringify({ value: i, data: "benchmark" }));
      }
    });
    results.push(r);
  }

  {
    const db = new PluresDB();
    for (let i = 0; i < 1000; i++) {
      db.put(`bench:${i}`, JSON.stringify({ value: i }));
    }
    const r = bench("wasm_crdt_get_1000", ITERATIONS, () => {
      for (let i = 0; i < 1000; i++) {
        db.get(`bench:${i}`);
      }
    });
    results.push(r);
  }
}

// ---------------------------------------------------------------------------
// Output results (JSON for CI consumption)
// ---------------------------------------------------------------------------

console.log(JSON.stringify({ target: "wasm", results }, null, 2));
