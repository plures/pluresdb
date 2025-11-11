// @ts-nocheck
import { assertEquals, assertExists } from "jsr:@std/assert@1.0.14";
import { GunDB } from "../../core/database.ts";

const shouldRunMeshTests =
  (Deno.env.get("RUN_MESH_TESTS") ?? "").toLowerCase() === "true";
const defaultTimeoutMs = Number(
  Deno.env.get("RUN_MESH_TEST_TIMEOUT_MS") ?? "10000",
);

if (!shouldRunMeshTests) {
  console.warn(
    "[mesh-network.test] Skipping mesh networking integration tests. Set RUN_MESH_TESTS=true to enable.",
  );
}

function withTimeout<T>(
  promise: Promise<T>,
  message: string,
  timeoutMs = defaultTimeoutMs,
): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(message));
    }, timeoutMs);

    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        clearTimeout(timer);
        reject(error);
      },
    );
  });
}

function meshTest(name: string, fn: () => Promise<void>) {
  Deno.test({
    name,
    ignore: !shouldRunMeshTests,
    sanitizeOps: false,
    sanitizeResources: false,
    fn,
  });
}

function randomPort(): number {
  return 18000 + Math.floor(Math.random() * 10000);
}

meshTest("Mesh Network - Basic Connection and Sync", async () => {
  const port = randomPort();
  const serverUrl = `ws://localhost:${port}`;

  const dbA = new GunDB();
  const dbB = new GunDB();

  try {
    const kvA = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
    const kvB = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });

    await dbA.ready(kvA);
    await dbB.ready(kvB);

    // Start server
    await dbA.serve({ port });

    // Add data to server
    await dbA.put("mesh:test", {
      text: "Hello from server A",
      timestamp: Date.now(),
    });

    // Connect client and wait for sync
    const receivedData = new Promise((resolve) => {
      dbB.on("mesh:test", (node) => {
        if (node && (node.data as any).text === "Hello from server A") {
          resolve(true);
        }
      });
    });

    dbB.connect(serverUrl);
    await withTimeout(
      receivedData as Promise<unknown>,
      "Timed out waiting for initial mesh sync",
    );

    // Verify data was received
    const syncedData = await dbB.get("mesh:test");
    assertExists(syncedData);
    assertEquals((syncedData as any).text, "Hello from server A");
  } finally {
    await dbB.close();
    await dbA.close();
  }
});

meshTest("Mesh Network - Bidirectional Sync", async () => {
  const port = randomPort();
  const serverUrl = `ws://localhost:${port}`;

  const dbA = new GunDB();
  const dbB = new GunDB();

  try {
    const kvA = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
    const kvB = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });

    await dbA.ready(kvA);
    await dbB.ready(kvB);
    await dbA.serve({ port });

    // Connect B to A
    dbB.connect(serverUrl);
    await withTimeout(
      new Promise((resolve) => setTimeout(resolve, 100)),
      "Timed out waiting for mesh connection",
    );

    // A sends data to B
    await dbA.put("from:A", { message: "Hello from A" });

    const receivedFromA = new Promise((resolve) => {
      dbB.on("from:A", (node) => {
        if (node && (node.data as any).message === "Hello from A") {
          resolve(true);
        }
      });
    });
    await withTimeout(
      receivedFromA as Promise<unknown>,
      "Timed out waiting for data from node A",
    );

    // B sends data to A
    await dbB.put("from:B", { message: "Hello from B" });

    const receivedFromB = new Promise((resolve) => {
      dbA.on("from:B", (node) => {
        if (node && (node.data as any).message === "Hello from B") {
          resolve(true);
        }
      });
    });
    await withTimeout(
      receivedFromB as Promise<unknown>,
      "Timed out waiting for data from node B",
    );

    // Verify both received data
    const dataFromA = await dbB.get("from:A");
    const dataFromB = await dbA.get("from:B");

    assertExists(dataFromA);
    assertExists(dataFromB);
    assertEquals((dataFromA as any).message, "Hello from A");
    assertEquals((dataFromB as any).message, "Hello from B");
  } finally {
    await dbB.close();
    await dbA.close();
  }
});

meshTest("Mesh Network - Conflict Resolution", async () => {
  const port = randomPort();
  const serverUrl = `ws://localhost:${port}`;

  const dbA = new GunDB();
  const dbB = new GunDB();

  try {
    const kvA = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
    const kvB = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });

    await dbA.ready(kvA);
    await dbB.ready(kvB);
    await dbA.serve({ port });

    // Connect B to A
    dbB.connect(serverUrl);
    await new Promise((resolve) => setTimeout(resolve, 100));

    // Both modify the same key simultaneously
    const timestamp = Date.now();
    await dbA.put("conflict:test", {
      value: "from A",
      timestamp: timestamp + 1000,
    });
    await dbB.put("conflict:test", {
      value: "from B",
      timestamp: timestamp + 2000,
    });

    // Wait for sync
    await withTimeout(
      new Promise((resolve) => setTimeout(resolve, 500)),
      "Timed out waiting for conflict resolution",
    );

    // Check that both databases have the same final state
    const finalA = await dbA.get("conflict:test");
    const finalB = await dbB.get("conflict:test");

    assertExists(finalA);
    assertExists(finalB);

    // Should have the newer timestamp (from B)
    assertEquals((finalA as any).value, "from B");
    assertEquals((finalB as any).value, "from B");
  } finally {
    await dbB.close();
    await dbA.close();
  }
});

meshTest("Mesh Network - Multiple Clients", async () => {
  const port = randomPort();
  const serverUrl = `ws://localhost:${port}`;

  const dbA = new GunDB(); // Server
  const dbB = new GunDB(); // Client 1
  const dbC = new GunDB(); // Client 2

  try {
    const kvA = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
    const kvB = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
    const kvC = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });

    await dbA.ready(kvA);
    await dbB.ready(kvB);
    await dbC.ready(kvC);
    await dbA.serve({ port });

    // Connect both clients
    dbB.connect(serverUrl);
    dbC.connect(serverUrl);
    await new Promise((resolve) => setTimeout(resolve, 200));

    // B sends data
    await dbB.put("multi:test", { from: "B", message: "Hello from B" });

    // Wait for both A and C to receive
    const receivedOnA = new Promise((resolve) => {
      dbA.on("multi:test", (node) => {
        if (node && (node.data as any).from === "B") resolve(true);
      });
    });

    const receivedOnC = new Promise((resolve) => {
      dbC.on("multi:test", (node) => {
        if (node && (node.data as any).from === "B") resolve(true);
      });
    });

    await withTimeout(
      Promise.all([receivedOnA, receivedOnC]),
      "Timed out waiting for multi-client propagation",
    );

    // Verify all have the data
    const dataA = await dbA.get("multi:test");
    const dataB = await dbB.get("multi:test");
    const dataC = await dbC.get("multi:test");

    assertExists(dataA);
    assertExists(dataB);
    assertExists(dataC);
    assertEquals((dataA as any).from, "B");
    assertEquals((dataB as any).from, "B");
    assertEquals((dataC as any).from, "B");
  } finally {
    await dbC.close();
    await dbB.close();
    await dbA.close();
  }
});

meshTest("Mesh Network - Connection Error Handling", async () => {
  const db = new GunDB();

  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    // Try to connect to non-existent server
    db.connect("ws://localhost:99999");

    // Should not throw error, but connection should fail gracefully
    await withTimeout(
      new Promise((resolve) => setTimeout(resolve, 1000)),
      "Timed out waiting for graceful connection failure",
    );
  } finally {
    await db.close();
  }
});
