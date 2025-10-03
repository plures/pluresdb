// @ts-nocheck
import { assertEquals, assertExists } from "jsr:@std/assert@1.0.14";
import { GunDB } from "../../core/database.ts";

interface PerformanceMetrics {
  operation: string;
  count: number;
  totalTime: number;
  averageTime: number;
  operationsPerSecond: number;
}

function measureOperation(
  operation: () => Promise<void>,
  count: number,
): Promise<PerformanceMetrics> {
  return new Promise(async (resolve) => {
    const startTime = performance.now();

    for (let i = 0; i < count; i++) {
      await operation();
    }

    const endTime = performance.now();
    const totalTime = endTime - startTime;
    const averageTime = totalTime / count;
    const operationsPerSecond = (count / totalTime) * 1000;

    resolve({
      operation: "unknown",
      count,
      totalTime,
      averageTime,
      operationsPerSecond,
    });
  });
}

Deno.test("Performance - Bulk Insert Operations", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const count = 1000;
    let currentCount = 0;

    const metrics = await measureOperation(async () => {
      await db.put(`perf:${currentCount++}`, {
        id: currentCount,
        data: `Performance test data ${currentCount}`,
        timestamp: Date.now(),
      });
    }, count);

    console.log(`Bulk Insert Performance:`, metrics);

    // Verify all data was inserted
    const allNodes = await db.getAll();
    assertEquals(allNodes.length, count);

    // Performance assertions
    assertEquals(metrics.operationsPerSecond > 100, true); // At least 100 ops/sec
    assertEquals(metrics.averageTime < 10, true); // Less than 10ms per operation
  } finally {
    await db.close();
  }
});

Deno.test("Performance - Bulk Read Operations", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    // Pre-populate with data
    const count = 1000;
    for (let i = 0; i < count; i++) {
      await db.put(`read:${i}`, {
        id: i,
        data: `Read test data ${i}`,
        timestamp: Date.now(),
      });
    }

    let currentCount = 0;
    const metrics = await measureOperation(async () => {
      const data = await db.get(`read:${currentCount++}`);
      assertExists(data);
    }, count);

    console.log(`Bulk Read Performance:`, metrics);

    // Performance assertions
    assertEquals(metrics.operationsPerSecond > 500, true); // At least 500 ops/sec
    assertEquals(metrics.averageTime < 2, true); // Less than 2ms per operation
  } finally {
    await db.close();
  }
});

Deno.test("Performance - Vector Search Operations", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    // Pre-populate with documents for vector search
    const count = 500;
    for (let i = 0; i < count; i++) {
      await db.put(`doc:${i}`, {
        text: `Document ${i} about machine learning and artificial intelligence`,
        content: `This is document number ${i} containing information about AI and ML algorithms`,
      });
    }

    const searchQueries = [
      "machine learning algorithms",
      "artificial intelligence",
      "neural networks",
      "deep learning",
      "data science",
    ];

    let queryCount = 0;
    const metrics = await measureOperation(async () => {
      const query = searchQueries[queryCount % searchQueries.length];
      const results = await db.vectorSearch(query, 10);
      assertExists(results);
      queryCount++;
    }, 100);

    console.log(`Vector Search Performance:`, metrics);

    // Performance assertions
    assertEquals(metrics.operationsPerSecond > 50, true); // At least 50 ops/sec
    assertEquals(metrics.averageTime < 20, true); // Less than 20ms per operation
  } finally {
    await db.close();
  }
});

Deno.test("Performance - Concurrent Operations", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const concurrentCount = 100;
    const operationsPerWorker = 10;

    const startTime = performance.now();

    // Run concurrent operations
    const promises = Array.from({ length: concurrentCount }, async (_, i) => {
      for (let j = 0; j < operationsPerWorker; j++) {
        await db.put(`concurrent:${i}:${j}`, {
          worker: i,
          operation: j,
          timestamp: Date.now(),
        });
      }
    });

    await Promise.all(promises);

    const endTime = performance.now();
    const totalTime = endTime - startTime;
    const totalOperations = concurrentCount * operationsPerWorker;
    const operationsPerSecond = (totalOperations / totalTime) * 1000;

    console.log(`Concurrent Operations Performance:`, {
      totalOperations,
      totalTime,
      operationsPerSecond,
    });

    // Verify all operations completed
    const allNodes = await db.getAll();
    assertEquals(allNodes.length, totalOperations);

    // Performance assertions
    assertEquals(operationsPerSecond > 200, true); // At least 200 ops/sec
  } finally {
    await db.close();
  }
});

Deno.test("Performance - Memory Usage", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const initialMemory = (performance as any).memory?.usedJSHeapSize || 0;

    // Insert large amount of data
    const count = 10000;
    for (let i = 0; i < count; i++) {
      await db.put(`memory:${i}`, {
        id: i,
        largeData: "x".repeat(1000), // 1KB per record
        timestamp: Date.now(),
      });
    }

    const afterInsertMemory = (performance as any).memory?.usedJSHeapSize || 0;
    const memoryIncrease = afterInsertMemory - initialMemory;

    console.log(`Memory Usage:`, {
      initialMemory,
      afterInsertMemory,
      memoryIncrease,
      memoryPerRecord: memoryIncrease / count,
    });

    // Memory efficiency assertions
    assertEquals(memoryIncrease < 50 * 1024 * 1024, true); // Less than 50MB increase
    assertEquals(memoryIncrease / count < 5000, true); // Less than 5KB per record
  } finally {
    await db.close();
  }
});

Deno.test("Performance - Subscription Performance", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const subscriptionCount = 1000;
    const updateCount = 100;

    // Set up many subscriptions
    const subscriptions: Array<() => void> = [];
    for (let i = 0; i < subscriptionCount; i++) {
      const unsubscribe = db.on(`sub:${i}`, () => {});
      subscriptions.push(unsubscribe);
    }

    const startTime = performance.now();

    // Trigger updates
    for (let i = 0; i < updateCount; i++) {
      await db.put(`sub:${i % subscriptionCount}`, {
        update: i,
        timestamp: Date.now(),
      });
    }

    const endTime = performance.now();
    const totalTime = endTime - startTime;
    const updatesPerSecond = (updateCount / totalTime) * 1000;

    console.log(`Subscription Performance:`, {
      subscriptionCount,
      updateCount,
      totalTime,
      updatesPerSecond,
    });

    // Clean up subscriptions
    subscriptions.forEach((unsubscribe) => unsubscribe());

    // Performance assertions
    assertEquals(updatesPerSecond > 50, true); // At least 50 updates/sec
  } finally {
    await db.close();
  }
});
