#!/usr/bin/env -S deno run -A --unstable-kv

/**
 * Network / WebSocket benchmarks for PluresDB.
 *
 * Measures:
 *  - WebSocket connection establishment latency
 *  - Message round-trip latency over a local server
 *  - Concurrent client throughput
 *
 * Requires no external services — a temporary in-process server is started on
 * a random high port and torn down at the end.
 */

import { PluresDB } from "../core/database.ts";

interface BenchmarkResult {
  name: string;
  operations: number;
  totalMs: number;
  avgMs: number;
  opsPerSec: number;
}

function printResult(r: BenchmarkResult) {
  console.log(
    `  ${r.name.padEnd(48)} ` +
      `${r.opsPerSec.toFixed(1).padStart(9)} ops/s  ` +
      `avg ${r.avgMs.toFixed(3)}ms`,
  );
}

async function waitForServer(url: string, retries = 20, delayMs = 50): Promise<void> {
  for (let i = 0; i < retries; i++) {
    try {
      const ws = new WebSocket(url);
      await new Promise<void>((resolve, reject) => {
        ws.onopen = () => { ws.close(); resolve(); };
        ws.onerror = reject;
        setTimeout(() => reject(new Error("timeout")), 1_000);
      });
      return;
    } catch {
      await new Promise((r) => setTimeout(r, delayMs));
    }
  }
  throw new Error(`Server at ${url} never became ready`);
}

async function benchmarkConnections(): Promise<void> {
  console.log("\n── WebSocket Connection Latency ───────────────────────────");

  const db = new PluresDB();
  const kvPath = await Deno.makeTempFile({ prefix: "net_bench_", suffix: ".sqlite" });
  const port = 18_000 + Math.floor(Math.random() * 10_000);

  try {
    await db.ready(kvPath);
    await db.serve({ port });
    const serverUrl = `ws://localhost:${port}`;
    await waitForServer(serverUrl);

    const connectOps = 20;
    const times: number[] = [];

    for (let i = 0; i < connectOps; i++) {
      const t0 = performance.now();
      await new Promise<void>((resolve, reject) => {
        const ws = new WebSocket(serverUrl);
        ws.onopen = () => { ws.close(); resolve(); };
        ws.onerror = reject;
        setTimeout(() => reject(new Error("Connection timeout")), 5_000);
      });
      times.push(performance.now() - t0);
    }

    const totalMs = times.reduce((a, b) => a + b, 0);
    printResult({
      name: "WebSocket connect + close",
      operations: connectOps,
      totalMs,
      avgMs: totalMs / connectOps,
      opsPerSec: (connectOps / totalMs) * 1_000,
    });
  } finally {
    await db.close();
    await Deno.remove(kvPath).catch(() => {});
  }
}

async function benchmarkWriteThroughWs(): Promise<void> {
  console.log("\n── Write-Through WebSocket (put via WS client) ────────────");

  const serverDb = new PluresDB();
  const kvPath = await Deno.makeTempFile({ prefix: "net_write_", suffix: ".sqlite" });
  const port = 19_000 + Math.floor(Math.random() * 5_000);

  try {
    await serverDb.ready(kvPath);
    await serverDb.serve({ port });
    const serverUrl = `ws://localhost:${port}`;
    await waitForServer(serverUrl);

    // Open a single persistent client connection
    const clientDb = new PluresDB();
    const clientKvPath = await Deno.makeTempFile({ prefix: "net_client_", suffix: ".sqlite" });
    await clientDb.ready(clientKvPath);
    await clientDb.connect(serverUrl);

    const writeOps = 200;
    let idx = 0;
    const t0 = performance.now();

    for (let i = 0; i < writeOps; i++) {
      await clientDb.put(`ws:${idx++}`, { value: idx, ts: Date.now() });
    }

    const totalMs = performance.now() - t0;
    printResult({
      name: "put() via connected WS client",
      operations: writeOps,
      totalMs,
      avgMs: totalMs / writeOps,
      opsPerSec: (writeOps / totalMs) * 1_000,
    });

    await clientDb.close();
    await Deno.remove(clientKvPath).catch(() => {});
  } finally {
    await serverDb.close();
    await Deno.remove(kvPath).catch(() => {});
  }
}

async function benchmarkConcurrentClients(): Promise<void> {
  console.log("\n── Concurrent Clients ─────────────────────────────────────");

  const serverDb = new PluresDB();
  const kvPath = await Deno.makeTempFile({ prefix: "net_conc_", suffix: ".sqlite" });
  const port = 20_000 + Math.floor(Math.random() * 5_000);

  try {
    await serverDb.ready(kvPath);
    await serverDb.serve({ port });
    const serverUrl = `ws://localhost:${port}`;
    await waitForServer(serverUrl);

    for (const concurrency of [1, 5, 10]) {
      const writesPerClient = 50;
      const t0 = performance.now();

      await Promise.all(
        Array.from({ length: concurrency }, async (_, ci) => {
          const clientDb = new PluresDB();
          const kv = await Deno.makeTempFile({ prefix: `nc_c${ci}_`, suffix: ".sqlite" });
          await clientDb.ready(kv);
          await clientDb.connect(serverUrl);

          for (let i = 0; i < writesPerClient; i++) {
            await clientDb.put(`c${ci}:${i}`, { client: ci, seq: i });
          }
          await clientDb.close();
        }),
      );

      const totalMs = performance.now() - t0;
      const totalOps = concurrency * writesPerClient;
      printResult({
        name: `${concurrency} concurrent clients × ${writesPerClient} puts`,
        operations: totalOps,
        totalMs,
        avgMs: totalMs / totalOps,
        opsPerSec: (totalOps / totalMs) * 1_000,
      });
    }
  } finally {
    await serverDb.close();
    await Deno.remove(kvPath).catch(() => {});
  }
}

async function main(): Promise<void> {
  console.log("PluresDB Network Benchmark Suite");
  console.log("================================\n");

  try {
    await benchmarkConnections();
    await benchmarkWriteThroughWs();
    await benchmarkConcurrentClients();
    console.log("\n✓ Network benchmarks complete.\n");
  } catch (err) {
    console.error("Benchmark failed:", err);
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}
