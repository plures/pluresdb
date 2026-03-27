#!/usr/bin/env -S deno run -A --unstable-kv

/**
 * Vector search benchmarks for PluresDB.
 *
 * Measures:
 *  - Document ingestion throughput (with and without pre-built vectors)
 *  - Vector search latency across small / medium / large corpora
 *  - Top-K result count impact on search latency
 */

import { PluresDB } from "../core/database.ts";
import { DATASET_SIZES, generateVectorDocuments } from "./datasets.ts";

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
    `  ${r.name.padEnd(48)} ` +
      `${r.opsPerSec.toFixed(1).padStart(9)} ops/s  ` +
      `avg ${r.avgMs.toFixed(3)}ms`,
  );
}

async function benchmarkIngestion(): Promise<void> {
  console.log("\n── Vector Document Ingestion ──────────────────────────────");

  for (const [sizeLabel, count] of Object.entries(DATASET_SIZES) as [
    string,
    number,
  ][]) {
    const db = new PluresDB();
    const kvPath = await Deno.makeTempFile({ prefix: "vec_bench_", suffix: ".sqlite" });
    try {
      await db.ready(kvPath);
      const docs = generateVectorDocuments(count);

      let idx = 0;
      const result = await time(count, async () => {
        const doc = docs[idx++];
        await db.put(`doc:${doc.id}`, { title: doc.title, body: doc.body, tags: doc.tags });
      });

      printResult({
        name: `Ingest ${count.toLocaleString()} docs (${sizeLabel})`,
        operations: count,
        ...result,
      });
    } finally {
      await db.close();
      await Deno.remove(kvPath).catch(() => {});
    }
  }
}

async function benchmarkSearch(): Promise<void> {
  console.log("\n── Vector Search (by text query) ──────────────────────────");

  const searchQueries = [
    "machine learning",
    "graph database",
    "offline first",
    "sync protocol",
    "vector similarity",
  ];

  for (const [sizeLabel, count] of Object.entries(DATASET_SIZES) as [
    string,
    number,
  ][]) {
    const db = new PluresDB();
    const kvPath = await Deno.makeTempFile({ prefix: "vec_search_", suffix: ".sqlite" });
    try {
      await db.ready(kvPath);

      // Pre-populate corpus
      const docs = generateVectorDocuments(count);
      for (const doc of docs) {
        await db.put(`doc:${doc.id}`, { title: doc.title, body: doc.body });
      }

      // Bench repeated searches
      const searchOps = Math.min(count, 200);
      let qi = 0;
      const result = await time(searchOps, async () => {
        await db.vectorSearch(searchQueries[qi++ % searchQueries.length], 10);
      });

      printResult({
        name: `Search corpus=${count.toLocaleString()} top-10 (${sizeLabel})`,
        operations: searchOps,
        ...result,
      });

      // Top-K impact (medium only to keep suite fast)
      if (sizeLabel === "medium") {
        for (const k of [1, 5, 10, 50]) {
          let qi2 = 0;
          const r2 = await time(100, async () => {
            await db.vectorSearch(searchQueries[qi2++ % searchQueries.length], k);
          });
          printResult({
            name: `Search corpus=medium top-${k}`,
            operations: 100,
            ...r2,
          });
        }
      }
    } finally {
      await db.close();
      await Deno.remove(kvPath).catch(() => {});
    }
  }
}

async function benchmarkBuildIndex(): Promise<void> {
  console.log("\n── Vector Index Build ─────────────────────────────────────");

  for (const [sizeLabel, count] of Object.entries(DATASET_SIZES) as [
    string,
    number,
  ][]) {
    const db = new PluresDB();
    const kvPath = await Deno.makeTempFile({ prefix: "vec_idx_", suffix: ".sqlite" });
    try {
      await db.ready(kvPath);

      const docs = generateVectorDocuments(count);
      for (const doc of docs) {
        await db.put(`doc:${doc.id}`, { title: doc.title, body: doc.body });
      }

      const start = performance.now();
      // Trigger a search to force index build
      await db.vectorSearch("machine learning", 1);
      const elapsed = performance.now() - start;

      console.log(
        `  Index build + first search ${count.toLocaleString()} docs (${sizeLabel}): ${elapsed.toFixed(1)}ms`,
      );
    } finally {
      await db.close();
      await Deno.remove(kvPath).catch(() => {});
    }
  }
}

async function main(): Promise<void> {
  console.log("PluresDB Vector Search Benchmark Suite");
  console.log("======================================\n");

  try {
    await benchmarkIngestion();
    await benchmarkSearch();
    await benchmarkBuildIndex();
    console.log("\n✓ Vector benchmarks complete.\n");
  } catch (err) {
    console.error("Benchmark failed:", err);
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}
