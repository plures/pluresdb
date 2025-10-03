#!/usr/bin/env -S deno run -A

import { GunDB } from "../core/database.ts";

interface BenchmarkResult {
  name: string;
  operations: number;
  totalTime: number;
  averageTime: number;
  operationsPerSecond: number;
  memoryUsage?: number;
}

class BenchmarkRunner {
  private results: BenchmarkResult[] = [];

  async runBenchmark(
    name: string,
    operations: number,
    operation: () => Promise<void>,
  ): Promise<BenchmarkResult> {
    console.log(`Running benchmark: ${name} (${operations} operations)`);

    const startTime = performance.now();
    const startMemory = (performance as any).memory?.usedJSHeapSize || 0;

    for (let i = 0; i < operations; i++) {
      await operation();
    }

    const endTime = performance.now();
    const endMemory = (performance as any).memory?.usedJSHeapSize || 0;

    const totalTime = endTime - startTime;
    const averageTime = totalTime / operations;
    const operationsPerSecond = (operations / totalTime) * 1000;
    const memoryUsage = endMemory - startMemory;

    const result: BenchmarkResult = {
      name,
      operations,
      totalTime,
      averageTime,
      operationsPerSecond,
      memoryUsage,
    };

    this.results.push(result);
    console.log(`  ✓ ${operationsPerSecond.toFixed(2)} ops/sec (${averageTime.toFixed(2)}ms avg)`);

    return result;
  }

  printSummary() {
    console.log("\n" + "=".repeat(80));
    console.log("BENCHMARK SUMMARY");
    console.log("=".repeat(80));

    this.results.forEach((result) => {
      console.log(`${result.name}:`);
      console.log(`  Operations: ${result.operations.toLocaleString()}`);
      console.log(`  Total Time: ${result.totalTime.toFixed(2)}ms`);
      console.log(`  Average Time: ${result.averageTime.toFixed(2)}ms`);
      console.log(`  Operations/sec: ${result.operationsPerSecond.toFixed(2)}`);
      if (result.memoryUsage) {
        console.log(`  Memory Usage: ${(result.memoryUsage / 1024 / 1024).toFixed(2)}MB`);
      }
      console.log();
    });
  }
}

async function runCoreBenchmarks() {
  const runner = new BenchmarkRunner();
  const db = new GunDB();

  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    console.log("Starting Core Database Benchmarks...\n");

    // Benchmark 1: Basic CRUD Operations
    await runner.runBenchmark("Basic CRUD Operations", 1000, async () => {
      const id = `crud:${Math.random()}`;
      await db.put(id, { value: Math.random(), timestamp: Date.now() });
      await db.get(id);
      await db.delete(id);
    });

    // Benchmark 2: Bulk Insert
    await runner.runBenchmark("Bulk Insert", 5000, async () => {
      const id = `bulk:${Math.random()}`;
      await db.put(id, {
        data: "x".repeat(100),
        timestamp: Date.now(),
        random: Math.random(),
      });
    });

    // Benchmark 3: Bulk Read
    // First populate with data
    for (let i = 0; i < 1000; i++) {
      await db.put(`read:${i}`, { value: i, data: `Data ${i}` });
    }

    let readCount = 0;
    await runner.runBenchmark("Bulk Read", 1000, async () => {
      await db.get(`read:${readCount % 1000}`);
      readCount++;
    });

    // Benchmark 4: Vector Search
    // First populate with documents
    for (let i = 0; i < 100; i++) {
      await db.put(`doc:${i}`, {
        text: `Document ${i} about machine learning and artificial intelligence`,
        content: `This is document number ${i} containing information about AI and ML algorithms`,
      });
    }

    const searchQueries = [
      "machine learning",
      "artificial intelligence",
      "neural networks",
      "deep learning",
      "data science",
    ];

    let queryCount = 0;
    await runner.runBenchmark("Vector Search", 100, async () => {
      const query = searchQueries[queryCount % searchQueries.length];
      await db.vectorSearch(query, 10);
      queryCount++;
    });

    // Benchmark 5: Subscription Performance
    const subscriptions: Array<() => void> = [];
    for (let i = 0; i < 100; i++) {
      subscriptions.push(db.on(`sub:${i}`, () => {}));
    }

    let updateCount = 0;
    await runner.runBenchmark("Subscription Updates", 500, async () => {
      await db.put(`sub:${updateCount % 100}`, {
        update: updateCount,
        timestamp: Date.now(),
      });
      updateCount++;
    });

    // Clean up subscriptions
    subscriptions.forEach((unsubscribe) => unsubscribe());

    // Benchmark 6: Type System Operations
    await runner.runBenchmark("Type System Operations", 1000, async () => {
      const id = `type:${Math.random()}`;
      await db.put(id, { name: `Item ${Math.random()}` });
      await db.setType(id, "TestItem");
      await db.instancesOf("TestItem");
    });

    runner.printSummary();
  } finally {
    await db.close();
  }
}

async function runNetworkBenchmarks() {
  const runner = new BenchmarkRunner();

  console.log("Starting Network Benchmarks...\n");

  // Benchmark 1: WebSocket Connection Performance
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const port = 18000 + Math.floor(Math.random() * 10000);
    await db.serve({ port });

    const serverUrl = `ws://localhost:${port}`;

    await runner.runBenchmark("WebSocket Connections", 10, async () => {
      const clientDb = new GunDB();
      const clientKv = await Deno.makeTempFile({
        prefix: "kv_client_",
        suffix: ".sqlite",
      });
      await clientDb.ready(clientKv);

      const connectionPromise = new Promise<void>((resolve, reject) => {
        const ws = new WebSocket(serverUrl);
        ws.onopen = () => {
          ws.close();
          resolve();
        };
        ws.onerror = reject;
        setTimeout(() => reject(new Error("Connection timeout")), 5000);
      });

      await connectionPromise;
      await clientDb.close();
    });
  } finally {
    await db.close();
  }

  runner.printSummary();
}

async function runMemoryBenchmarks() {
  const runner = new BenchmarkRunner();
  const db = new GunDB();

  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    console.log("Starting Memory Benchmarks...\n");

    // Benchmark 1: Memory Usage with Large Datasets
    const initialMemory = (performance as any).memory?.usedJSHeapSize || 0;

    await runner.runBenchmark("Large Dataset Memory Usage", 1000, async () => {
      const id = `memory:${Math.random()}`;
      await db.put(id, {
        data: "x".repeat(1024), // 1KB per record
        timestamp: Date.now(),
        random: Math.random(),
      });
    });

    const finalMemory = (performance as any).memory?.usedJSHeapSize || 0;
    const memoryIncrease = finalMemory - initialMemory;

    console.log(`Memory Usage: ${(memoryIncrease / 1024 / 1024).toFixed(2)}MB for 1000 records`);
    console.log(`Memory per record: ${(memoryIncrease / 1000).toFixed(2)} bytes`);

    // Benchmark 2: Subscription Memory Usage
    const subscriptionMemory = (performance as any).memory?.usedJSHeapSize || 0;

    const subscriptions: Array<() => void> = [];
    await runner.runBenchmark("Subscription Memory Usage", 1000, async () => {
      const id = `sub:${Math.random()}`;
      const unsubscribe = db.on(id, () => {});
      subscriptions.push(unsubscribe);
    });

    const afterSubscriptionMemory = (performance as any).memory?.usedJSHeapSize || 0;
    const subscriptionMemoryIncrease = afterSubscriptionMemory - subscriptionMemory;

    console.log(
      `Subscription Memory: ${(subscriptionMemoryIncrease / 1024).toFixed(2)}KB for 1000 subscriptions`,
    );
    console.log(`Memory per subscription: ${(subscriptionMemoryIncrease / 1000).toFixed(2)} bytes`);

    // Clean up subscriptions
    subscriptions.forEach((unsubscribe) => unsubscribe());
  } finally {
    await db.close();
  }
}

async function main() {
  console.log("PluresDB Benchmark Suite");
  console.log("========================\n");

  try {
    await runCoreBenchmarks();
    await runNetworkBenchmarks();
    await runMemoryBenchmarks();

    console.log("All benchmarks completed successfully!");
  } catch (error) {
    console.error("Benchmark failed:", error);
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}
