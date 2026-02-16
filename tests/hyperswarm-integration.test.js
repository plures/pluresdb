/**
 * Integration tests for Hyperswarm P2P sync
 *
 * These tests require Node.js and real P2P connections.
 * Run with: node tests/hyperswarm-integration.test.js
 *
 * NOTE: These tests are SKIPPED in CI environments because:
 * 1. Hyperswarm requires real network connections which are unstable in CI
 * 2. P2P discovery can be slow/unreliable in CI network environments
 * 3. These are manual integration tests for local validation
 *
 * To run these tests locally: CI=false node tests/hyperswarm-integration.test.js
 */

const assert = require("assert");
const fs = require("fs").promises;
const path = require("path");

// Check if running in CI environment
const isCI = process.env.CI === "true";

if (isCI) {
  console.log("\n⏭️  Skipping Hyperswarm integration tests in CI environment");
  console.log(
    "   These tests require real P2P network connections which are unstable in CI.",
  );
  console.log("   Run locally with: CI=false node tests/hyperswarm-integration.test.js\n");
  process.exit(0);
}

// Import from compiled dist
const { GunDB } = require("../dist/core/database.js");

let testsPassed = 0;
let testsFailed = 0;

async function test(name, fn) {
  try {
    process.stdout.write(`  ${name} ... `);
    await fn();
    console.log("✓ PASS");
    testsPassed++;
  } catch (error) {
    console.log("✗ FAIL");
    console.error(`    Error: ${error.message}`);
    if (error.stack) {
      console.error(`    ${error.stack.split("\n").slice(1, 3).join("\n    ")}`);
    }
    testsFailed++;
  }
}

// Helper to create temporary file
async function createTempFile() {
  const tmpDir = path.join(__dirname, "..", ".tmp");
  await fs.mkdir(tmpDir, { recursive: true });
  const filename = `test_${Date.now()}_${Math.random().toString(36).slice(2)}.sqlite`;
  return path.join(tmpDir, filename);
}

// Helper to wait for condition
function waitFor(condition, timeout = 5000, checkInterval = 100) {
  return new Promise((resolve, reject) => {
    const startTime = Date.now();
    const interval = setInterval(async () => {
      try {
        if (await condition()) {
          clearInterval(interval);
          resolve(true);
        } else if (Date.now() - startTime > timeout) {
          clearInterval(interval);
          reject(new Error("Timeout waiting for condition"));
        }
      } catch (error) {
        clearInterval(interval);
        reject(error);
      }
    }, checkInterval);
  });
}

async function runTests() {
  console.log("\n🧪 Hyperswarm P2P Sync Integration Tests\n");
  console.log(
    "Note: These tests require network access and may take several seconds.\n",
  );

  await test("should enable sync with a generated key", async () => {
    const db = new GunDB();
    const kvPath = await createTempFile();

    try {
      await db.ready(kvPath);

      const key = GunDB.generateSyncKey();
      assert.strictEqual(typeof key, "string");
      assert.strictEqual(key.length, 64);

      await db.enableSync({ key });

      assert.strictEqual(db.isSyncEnabled(), true);
      assert.strictEqual(db.getSyncKey(), key);

      const stats = db.getSyncStats();
      assert(stats !== null);
      assert.strictEqual(stats.peersConnected, 0); // No peers yet

      await db.disableSync();
      assert.strictEqual(db.isSyncEnabled(), false);
    } finally {
      await db.close();
      try {
        await fs.unlink(kvPath);
      } catch (e) {
        /* ignore */
      }
    }
  });

  await test("should discover and connect two peers with same sync key", async () => {
    const dbA = new GunDB({ peerId: "peer-A" });
    const dbB = new GunDB({ peerId: "peer-B" });

    const kvPathA = await createTempFile();
    const kvPathB = await createTempFile();

    try {
      await dbA.ready(kvPathA);
      await dbB.ready(kvPathB);

      const sharedKey = GunDB.generateSyncKey();

      // Track peer connection events
      let peerAConnected = false;
      let peerBConnected = false;

      dbA.on("peer:connected", (info) => {
        console.log("Peer A detected connection:", info.peerId.slice(0, 16));
        peerAConnected = true;
      });

      dbB.on("peer:connected", (info) => {
        console.log("Peer B detected connection:", info.peerId.slice(0, 16));
        peerBConnected = true;
      });

      // Enable sync on both databases with the same key
      await dbA.enableSync({ key: sharedKey });
      await dbB.enableSync({ key: sharedKey });

      // Wait for peers to discover and connect to each other
      await waitFor(() => peerAConnected && peerBConnected, 10000);

      // Both should have 1 peer connected
      assert.strictEqual(dbA.getSyncPeers().length, 1);
      assert.strictEqual(dbB.getSyncPeers().length, 1);

      await dbA.disableSync();
      await dbB.disableSync();
    } finally {
      await dbA.close();
      await dbB.close();
      try {
        await fs.unlink(kvPathA);
        await fs.unlink(kvPathB);
      } catch (e) {
        /* ignore */
      }
    }
  });

  await test("should sync data between two peers", async () => {
    const dbA = new GunDB({ peerId: "peer-A" });
    const dbB = new GunDB({ peerId: "peer-B" });

    const kvPathA = await createTempFile();
    const kvPathB = await createTempFile();

    try {
      await dbA.ready(kvPathA);
      await dbB.ready(kvPathB);

      const sharedKey = GunDB.generateSyncKey();

      let dataReceivedOnB = false;

      // Listen for data on B
      dbB.on("test:data", (node) => {
        if (node && node.data.message === "Hello from A") {
          console.log("Peer B received data from A");
          dataReceivedOnB = true;
        }
      });

      // Enable sync
      await dbA.enableSync({ key: sharedKey });
      await dbB.enableSync({ key: sharedKey });

      // Wait for connection
      await waitFor(() => dbA.getSyncPeers().length > 0, 10000);

      // Put data on A
      await dbA.put("test:data", { message: "Hello from A", timestamp: Date.now() });

      // Wait for B to receive the data
      await waitFor(() => dataReceivedOnB, 10000);

      // Verify data on B
      const dataOnB = await dbB.get("test:data");
      assert(dataOnB !== null);
      assert.strictEqual(dataOnB.message, "Hello from A");

      await dbA.disableSync();
      await dbB.disableSync();
    } finally {
      await dbA.close();
      await dbB.close();
      try {
        await fs.unlink(kvPathA);
        await fs.unlink(kvPathB);
      } catch (e) {
        /* ignore */
      }
    }
  });

  await test("should handle bidirectional sync", async () => {
    const dbA = new GunDB({ peerId: "peer-A" });
    const dbB = new GunDB({ peerId: "peer-B" });

    const kvPathA = await createTempFile();
    const kvPathB = await createTempFile();

    try {
      await dbA.ready(kvPathA);
      await dbB.ready(kvPathB);

      const sharedKey = GunDB.generateSyncKey();

      let dataFromBReceivedOnA = false;
      let dataFromAReceivedOnB = false;

      dbA.on("from:B", (node) => {
        if (node && node.data.source === "B") {
          console.log("Peer A received data from B");
          dataFromBReceivedOnA = true;
        }
      });

      dbB.on("from:A", (node) => {
        if (node && node.data.source === "A") {
          console.log("Peer B received data from A");
          dataFromAReceivedOnB = true;
        }
      });

      // Enable sync
      await dbA.enableSync({ key: sharedKey });
      await dbB.enableSync({ key: sharedKey });

      // Wait for connection
      await waitFor(() => dbA.getSyncPeers().length > 0, 10000);

      // Both send data
      await dbA.put("from:A", { source: "A", message: "Hello from A" });
      await dbB.put("from:B", { source: "B", message: "Hello from B" });

      // Wait for both to receive
      await waitFor(() => dataFromAReceivedOnB && dataFromBReceivedOnA, 10000);

      // Verify data
      const dataOnA = await dbA.get("from:B");
      const dataOnB = await dbB.get("from:A");

      assert(dataOnA !== null);
      assert(dataOnB !== null);
      assert.strictEqual(dataOnA.source, "B");
      assert.strictEqual(dataOnB.source, "A");

      await dbA.disableSync();
      await dbB.disableSync();
    } finally {
      await dbA.close();
      await dbB.close();
      try {
        await fs.unlink(kvPathA);
        await fs.unlink(kvPathB);
      } catch (e) {
        /* ignore */
      }
    }
  });

  await test("should not connect peers with different keys", async () => {
    const dbA = new GunDB({ peerId: "peer-A" });
    const dbB = new GunDB({ peerId: "peer-B" });

    const kvPathA = await createTempFile();
    const kvPathB = await createTempFile();

    try {
      await dbA.ready(kvPathA);
      await dbB.ready(kvPathB);

      const keyA = GunDB.generateSyncKey();
      const keyB = GunDB.generateSyncKey();

      // Enable sync with different keys
      await dbA.enableSync({ key: keyA });
      await dbB.enableSync({ key: keyB });

      // Wait a bit to ensure no connection happens
      await new Promise((resolve) => setTimeout(resolve, 3000));

      // Should have no peers
      assert.strictEqual(dbA.getSyncPeers().length, 0);
      assert.strictEqual(dbB.getSyncPeers().length, 0);

      await dbA.disableSync();
      await dbB.disableSync();
    } finally {
      await dbA.close();
      await dbB.close();
      try {
        await fs.unlink(kvPathA);
        await fs.unlink(kvPathB);
      } catch (e) {
        /* ignore */
      }
    }
  });

  console.log("\n" + "=".repeat(50));
  console.log(
    `Results: ${testsPassed} passed, ${testsFailed} failed, ${testsPassed + testsFailed} total`,
  );
  console.log("=".repeat(50) + "\n");

  if (testsFailed > 0) {
    process.exit(1);
  }
}

// Only run if executed directly
if (require.main === module) {
  runTests().catch((error) => {
    console.error("Fatal error:", error);
    process.exit(1);
  });
}
