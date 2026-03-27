#!/usr/bin/env -S deno run -A --unstable-kv

/**
 * Sync apply / merge throughput benchmarks for PluresDB.
 *
 * Measures the speed of applying incoming CRDT operations from a remote peer,
 * which is the hot path during P2P synchronisation.  Tests are purely in-
 * process so no network I/O is required.
 *
 * Scenarios:
 *  - Sequential apply (single-peer inserts in order)
 *  - Concurrent apply (multiple peer actors writing the same keys → merge)
 *  - Delete propagation throughput
 *  - Mixed workload (insert + update + delete)
 */

import { PluresDB } from "../core/database.ts";
import { DATASET_SIZES, generateUsers } from "./datasets.ts";

interface BenchmarkResult {
  name: string;
  operations: number;
  totalMs: number;
  avgMs: number;
  opsPerSec: number;
}

async function time(
  ops: number,
  fn: () => Promise<void>,
): Promise<Pick<BenchmarkResult, "totalMs" | "avgMs" | "opsPerSec">> {
  const start = performance.now();
  for (let i = 0; i < ops; i++) await fn();
  const totalMs = performance.now() - start;
  return { totalMs, avgMs: totalMs / ops, opsPerSec: (ops / totalMs) * 1_000 };
}

function printResult(r: BenchmarkResult) {
  console.log(
    `  ${r.name.padEnd(52)} ` +
      `${r.opsPerSec.toFixed(1).padStart(9)} ops/s  ` +
      `avg ${r.avgMs.toFixed(3)}ms`,
  );
}

// ---------------------------------------------------------------------------
// Sequential apply — remote peer pushes N distinct inserts
// ---------------------------------------------------------------------------
async function benchmarkSequentialApply(): Promise<void> {
  console.log("\n── Sequential Apply (single remote actor) ─────────────────");

  for (const [sizeLabel, count] of Object.entries(DATASET_SIZES) as [string, number][]) {
    const db = new PluresDB();
    const kvPath = await Deno.makeTempFile({ prefix: "sync_seq_", suffix: ".sqlite" });
    try {
      await db.ready(kvPath);
      const users = generateUsers(count);

      let idx = 0;
      const result = await time(count, async () => {
        const u = users[idx++];
        // Simulate applying an op received from "peer-alice"
        await db.put(`user:${u.id}`, { ...u, _actor: "peer-alice" });
      });

      printResult({
        name: `Sequential apply ${count.toLocaleString()} ops (${sizeLabel})`,
        operations: count,
        ...result,
      });
    } finally {
      await db.close();
      await Deno.remove(kvPath).catch(() => {});
    }
  }
}

// ---------------------------------------------------------------------------
// Concurrent merge — two actors write the same keys → last-write-wins
// ---------------------------------------------------------------------------
async function benchmarkConcurrentMerge(): Promise<void> {
  console.log("\n── Concurrent Merge (two actors, same keys) ───────────────");

  for (const [sizeLabel, count] of Object.entries(DATASET_SIZES) as [string, number][]) {
    const db = new PluresDB();
    const kvPath = await Deno.makeTempFile({ prefix: "sync_merge_", suffix: ".sqlite" });
    try {
      await db.ready(kvPath);
      const users = generateUsers(count);

      // Pre-populate from actor-A
      for (const u of users) {
        await db.put(`user:${u.id}`, { ...u, _actor: "peer-alice" });
      }

      // Measure applying conflicting updates from actor-B (all keys overlap)
      let idx = 0;
      const result = await time(count, async () => {
        const u = users[idx++];
        await db.put(`user:${u.id}`, {
          ...u,
          age: u.age + 1, // slightly different value → merge conflict
          _actor: "peer-bob",
        });
      });

      printResult({
        name: `Conflict merge ${count.toLocaleString()} ops (${sizeLabel})`,
        operations: count,
        ...result,
      });
    } finally {
      await db.close();
      await Deno.remove(kvPath).catch(() => {});
    }
  }
}

// ---------------------------------------------------------------------------
// Delete propagation
// ---------------------------------------------------------------------------
async function benchmarkDeletePropagation(): Promise<void> {
  console.log("\n── Delete Propagation ─────────────────────────────────────");

  for (const [sizeLabel, count] of Object.entries(DATASET_SIZES) as [string, number][]) {
    const db = new PluresDB();
    const kvPath = await Deno.makeTempFile({ prefix: "sync_del_", suffix: ".sqlite" });
    try {
      await db.ready(kvPath);
      const users = generateUsers(count);

      // Pre-populate
      for (const u of users) {
        await db.put(`user:${u.id}`, u);
      }

      // Measure delete throughput
      let idx = 0;
      const result = await time(count, async () => {
        await db.delete(`user:${users[idx++].id}`);
      });

      printResult({
        name: `Delete propagation ${count.toLocaleString()} ops (${sizeLabel})`,
        operations: count,
        ...result,
      });
    } finally {
      await db.close();
      await Deno.remove(kvPath).catch(() => {});
    }
  }
}

// ---------------------------------------------------------------------------
// Mixed workload: 60 % insert, 30 % update, 10 % delete
// ---------------------------------------------------------------------------
async function benchmarkMixedWorkload(): Promise<void> {
  console.log("\n── Mixed Workload (60% insert, 30% update, 10% delete) ────");

  for (const [sizeLabel, count] of Object.entries(DATASET_SIZES) as [string, number][]) {
    const db = new PluresDB();
    const kvPath = await Deno.makeTempFile({ prefix: "sync_mixed_", suffix: ".sqlite" });
    try {
      await db.ready(kvPath);
      const users = generateUsers(count);

      // Seed half the dataset so updates and deletes have targets
      for (let i = 0; i < Math.floor(count / 2); i++) {
        await db.put(`user:${users[i].id}`, users[i]);
      }

      let opIdx = 0;
      const result = await time(count, async () => {
        const r = opIdx % 10;
        const u = users[opIdx % count];
        opIdx++;

        if (r < 6) {
          // 60 % insert new key
          await db.put(`user:new:${opIdx}`, u);
        } else if (r < 9) {
          // 30 % update existing key
          await db.put(`user:${u.id}`, { ...u, age: u.age + 1 });
        } else {
          // 10 % delete
          await db.delete(`user:${u.id}`);
        }
      });

      printResult({
        name: `Mixed workload ${count.toLocaleString()} ops (${sizeLabel})`,
        operations: count,
        ...result,
      });
    } finally {
      await db.close();
      await Deno.remove(kvPath).catch(() => {});
    }
  }
}

// ---------------------------------------------------------------------------
// Subscription fan-out — measure how fast subscribers receive sync updates
// ---------------------------------------------------------------------------
async function benchmarkSubscriptionFanout(): Promise<void> {
  console.log("\n── Subscription Fan-out ───────────────────────────────────");

  const db = new PluresDB();
  const kvPath = await Deno.makeTempFile({ prefix: "sync_fanout_", suffix: ".sqlite" });
  try {
    await db.ready(kvPath);

    for (const subCount of [1, 10, 50]) {
      const unsubs: Array<() => void> = [];
      let received = 0;

      for (let s = 0; s < subCount; s++) {
        unsubs.push(db.on("fanout:key", () => { received++; }));
      }

      const writeOps = 200;
      const t0 = performance.now();

      for (let i = 0; i < writeOps; i++) {
        await db.put("fanout:key", { seq: i });
      }

      const totalMs = performance.now() - t0;
      console.log(
        `  ${subCount} subscriber(s) × ${writeOps} updates: ` +
          `${totalMs.toFixed(1)}ms total  ` +
          `${((writeOps / totalMs) * 1_000).toFixed(1)} writes/s  ` +
          `${received} callbacks fired`,
      );

      unsubs.forEach((u) => u());
    }
  } finally {
    await db.close();
  }
}

async function main(): Promise<void> {
  console.log("PluresDB Sync / Merge Benchmark Suite");
  console.log("=====================================\n");

  try {
    await benchmarkSequentialApply();
    await benchmarkConcurrentMerge();
    await benchmarkDeletePropagation();
    await benchmarkMixedWorkload();
    await benchmarkSubscriptionFanout();
    console.log("\n✓ Sync benchmarks complete.\n");
  } catch (err) {
    console.error("Benchmark failed:", err);
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}
