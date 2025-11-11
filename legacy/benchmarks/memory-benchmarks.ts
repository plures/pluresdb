#!/usr/bin/env -S deno run -A

import { GunDB } from "../core/database.ts";

interface MemoryMetrics {
  operation: string;
  initialMemory: number;
  finalMemory: number;
  memoryIncrease: number;
  recordCount: number;
  memoryPerRecord: number;
}

class MemoryBenchmark {
  private results: MemoryMetrics[] = [];

  private getMemoryUsage(): number {
    return (performance as any).memory?.usedJSHeapSize || 0;
  }

  async measureMemoryUsage(
    operation: string,
    recordCount: number,
    operationFn: () => Promise<void>,
  ): Promise<MemoryMetrics> {
    console.log(`Measuring memory usage for: ${operation}`);

    // Force garbage collection if available
    if ((globalThis as any).gc) {
      (globalThis as any).gc();
    }

    const initialMemory = this.getMemoryUsage();

    await operationFn();

    // Force garbage collection if available
    if ((globalThis as any).gc) {
      (globalThis as any).gc();
    }

    const finalMemory = this.getMemoryUsage();
    const memoryIncrease = finalMemory - initialMemory;
    const memoryPerRecord = memoryIncrease / recordCount;

    const metrics: MemoryMetrics = {
      operation,
      initialMemory,
      finalMemory,
      memoryIncrease,
      recordCount,
      memoryPerRecord,
    };

    this.results.push(metrics);

    console.log(
      `  Initial Memory: ${(initialMemory / 1024 / 1024).toFixed(2)}MB`,
    );
    console.log(`  Final Memory: ${(finalMemory / 1024 / 1024).toFixed(2)}MB`);
    console.log(
      `  Memory Increase: ${(memoryIncrease / 1024 / 1024).toFixed(2)}MB`,
    );
    console.log(`  Memory per Record: ${memoryPerRecord.toFixed(2)} bytes`);
    console.log();

    return metrics;
  }

  printSummary() {
    console.log("\n" + "=".repeat(80));
    console.log("MEMORY USAGE SUMMARY");
    console.log("=".repeat(80));

    this.results.forEach((result) => {
      console.log(`${result.operation}:`);
      console.log(`  Records: ${result.recordCount.toLocaleString()}`);
      console.log(
        `  Memory Increase: ${
          (result.memoryIncrease / 1024 / 1024).toFixed(2)
        }MB`,
      );
      console.log(
        `  Memory per Record: ${result.memoryPerRecord.toFixed(2)} bytes`,
      );
      console.log();
    });
  }
}

async function runMemoryBenchmarks() {
  const benchmark = new MemoryBenchmark();
  const db = new GunDB();

  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    console.log("Starting Memory Benchmarks...\n");

    // Benchmark 1: Small Records
    await benchmark.measureMemoryUsage(
      "Small Records (100 bytes)",
      1000,
      async () => {
        for (let i = 0; i < 1000; i++) {
          await db.put(`small:${i}`, {
            id: i,
            data: "x".repeat(100),
            timestamp: Date.now(),
          });
        }
      },
    );

    // Benchmark 2: Medium Records
    await benchmark.measureMemoryUsage(
      "Medium Records (1KB)",
      1000,
      async () => {
        for (let i = 0; i < 1000; i++) {
          await db.put(`medium:${i}`, {
            id: i,
            data: "x".repeat(1024),
            timestamp: Date.now(),
            metadata: { size: "medium", type: "test" },
          });
        }
      },
    );

    // Benchmark 3: Large Records
    await benchmark.measureMemoryUsage(
      "Large Records (10KB)",
      100,
      async () => {
        for (let i = 0; i < 100; i++) {
          await db.put(`large:${i}`, {
            id: i,
            data: "x".repeat(10 * 1024),
            timestamp: Date.now(),
            metadata: { size: "large", type: "test" },
            additional: "y".repeat(1024),
          });
        }
      },
    );

    // Benchmark 4: Vector Data
    await benchmark.measureMemoryUsage(
      "Vector Data (100 dimensions)",
      500,
      async () => {
        for (let i = 0; i < 500; i++) {
          const vector = Array.from({ length: 100 }, () => Math.random());
          await db.put(`vector:${i}`, {
            id: i,
            text: `Document ${i} with vector data`,
            vector: vector,
            timestamp: Date.now(),
          });
        }
      },
    );

    // Benchmark 5: Nested Objects
    await benchmark.measureMemoryUsage("Nested Objects", 500, async () => {
      for (let i = 0; i < 500; i++) {
        await db.put(`nested:${i}`, {
          id: i,
          user: {
            name: `User ${i}`,
            email: `user${i}@example.com`,
            profile: {
              age: 20 + (i % 50),
              location: `City ${i}`,
              preferences: {
                theme: i % 2 === 0 ? "dark" : "light",
                notifications: true,
                privacy: "public",
              },
            },
          },
          metadata: {
            created: Date.now(),
            updated: Date.now(),
            version: 1,
          },
        });
      }
    });

    // Benchmark 6: Subscriptions Memory Usage
    await benchmark.measureMemoryUsage("Subscriptions", 1000, async () => {
      const subscriptions: Array<() => void> = [];
      for (let i = 0; i < 1000; i++) {
        const unsubscribe = db.on(`sub:${i}`, () => {});
        subscriptions.push(unsubscribe);
      }

      // Store subscriptions for cleanup
      (globalThis as any).subscriptions = subscriptions;
    });

    // Clean up subscriptions
    if ((globalThis as any).subscriptions) {
      ((globalThis as any).subscriptions as Array<() => void>).forEach((
        unsubscribe,
      ) => unsubscribe());
      delete (globalThis as any).subscriptions;
    }

    // Benchmark 7: Type System Memory Usage
    await benchmark.measureMemoryUsage("Type System", 1000, async () => {
      for (let i = 0; i < 1000; i++) {
        await db.put(`type:${i}`, { name: `Item ${i}` });
        await db.setType(`type:${i}`, "TestItem");
      }
    });

    // Benchmark 8: CRDT Operations Memory Usage
    await benchmark.measureMemoryUsage("CRDT Operations", 1000, async () => {
      for (let i = 0; i < 1000; i++) {
        await db.put(`crdt:${i}`, {
          value: i,
          timestamp: Date.now(),
          vectorClock: { peer1: i, peer2: i * 2 },
        });
      }
    });

    benchmark.printSummary();
  } finally {
    await db.close();
  }
}

async function runMemoryLeakTests() {
  console.log("\n" + "=".repeat(80));
  console.log("MEMORY LEAK TESTS");
  console.log("=".repeat(80));

  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    // Test 1: Subscription Memory Leaks
    console.log("Testing subscription memory leaks...");

    const initialMemory = (performance as any).memory?.usedJSHeapSize || 0;

    // Create and destroy many subscriptions
    for (let cycle = 0; cycle < 10; cycle++) {
      const subscriptions: Array<() => void> = [];

      for (let i = 0; i < 100; i++) {
        subscriptions.push(db.on(`leak:${cycle}:${i}`, () => {}));
      }

      // Destroy all subscriptions
      subscriptions.forEach((unsubscribe) => unsubscribe());

      // Force garbage collection if available
      if ((globalThis as any).gc) {
        (globalThis as any).gc();
      }
    }

    const finalMemory = (performance as any).memory?.usedJSHeapSize || 0;
    const memoryIncrease = finalMemory - initialMemory;

    console.log(
      `Memory increase after subscription cycles: ${
        (memoryIncrease / 1024).toFixed(2)
      }KB`,
    );

    if (memoryIncrease > 1024 * 1024) {
      // More than 1MB
      console.log("⚠️  Potential memory leak detected in subscriptions");
    } else {
      console.log("✓ No significant memory leak in subscriptions");
    }

    // Test 2: CRUD Operations Memory Leaks
    console.log("\nTesting CRUD operations memory leaks...");

    const crudInitialMemory = (performance as any).memory?.usedJSHeapSize || 0;

    // Perform many CRUD operations
    for (let cycle = 0; cycle < 100; cycle++) {
      for (let i = 0; i < 100; i++) {
        await db.put(`leak:${cycle}:${i}`, {
          data: `Cycle ${cycle} Item ${i}`,
        });
        await db.get(`leak:${cycle}:${i}`);
        await db.delete(`leak:${cycle}:${i}`);
      }

      // Force garbage collection if available
      if ((globalThis as any).gc) {
        (globalThis as any).gc();
      }
    }

    const crudFinalMemory = (performance as any).memory?.usedJSHeapSize || 0;
    const crudMemoryIncrease = crudFinalMemory - crudInitialMemory;

    console.log(
      `Memory increase after CRUD cycles: ${
        (crudMemoryIncrease / 1024).toFixed(2)
      }KB`,
    );

    if (crudMemoryIncrease > 1024 * 1024) {
      // More than 1MB
      console.log("⚠️  Potential memory leak detected in CRUD operations");
    } else {
      console.log("✓ No significant memory leak in CRUD operations");
    }
  } finally {
    await db.close();
  }
}

async function main() {
  console.log("PluresDB Memory Benchmarks");
  console.log("===========================\n");

  try {
    await runMemoryBenchmarks();
    await runMemoryLeakTests();

    console.log("\nMemory benchmarks completed successfully!");
  } catch (error) {
    console.error("Memory benchmark failed:", error);
    Deno.exit(1);
  }
}

if (import.meta.main) {
  await main();
}
