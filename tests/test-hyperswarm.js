#!/usr/bin/env node
/**
 * Simple test runner for Hyperswarm sync functionality
 * Run with: node tests/test-hyperswarm.js
 */

const { GunDB } = require("../dist/core/database.js");
const { generateSyncKey } = require("../dist/network/hyperswarm-sync.js");
const fs = require("fs").promises;
const path = require("path");

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

async function createTempFile() {
  const tmpDir = path.join(__dirname, ".tmp");
  await fs.mkdir(tmpDir, { recursive: true });
  const filename = `test_${Date.now()}_${Math.random().toString(36).slice(2)}.sqlite`;
  return path.join(tmpDir, filename);
}

async function cleanupTempFile(filepath) {
  try {
    await fs.unlink(filepath);
  } catch {
    /* ignore */
  }
}

async function runTests() {
  console.log("\n🧪 Hyperswarm P2P Sync Tests\n");

  await test("generateSyncKey generates valid 64-char hex", async () => {
    const key = generateSyncKey();
    if (typeof key !== "string") throw new Error("Key is not a string");
    if (key.length !== 64) throw new Error(`Expected length 64, got ${key.length}`);
    if (!/^[0-9a-f]{64}$/i.test(key)) throw new Error("Key is not valid hex");
  });

  await test("generateSyncKey generates unique keys", async () => {
    const key1 = generateSyncKey();
    const key2 = generateSyncKey();
    if (key1 === key2) throw new Error("Keys should be unique");
  });

  await test("GunDB.generateSyncKey static method works", async () => {
    const key = GunDB.generateSyncKey();
    if (typeof key !== "string") throw new Error("Key is not a string");
    if (key.length !== 64) throw new Error(`Expected length 64, got ${key.length}`);
  });

  await test("GunDB has sync methods", async () => {
    const db = new GunDB();
    const kvPath = await createTempFile();

    try {
      await db.ready(kvPath);

      if (typeof db.enableSync !== "function") {
        throw new Error("enableSync method missing");
      }
      if (typeof db.disableSync !== "function") {
        throw new Error("disableSync method missing");
      }
      if (typeof db.getSyncStats !== "function") {
        throw new Error("getSyncStats method missing");
      }
      if (typeof db.isSyncEnabled !== "function") {
        throw new Error("isSyncEnabled method missing");
      }

      if (db.isSyncEnabled() !== false) {
        throw new Error("Sync should be disabled initially");
      }
    } finally {
      await db.close();
      await cleanupTempFile(kvPath);
    }
  });

  await test("enableSync rejects invalid keys", async () => {
    const db = new GunDB();
    const kvPath = await createTempFile();

    try {
      await db.ready(kvPath);

      const invalidKeys = ["short", "not-hex", "0".repeat(63), "0".repeat(65)];

      for (const invalidKey of invalidKeys) {
        let errorThrown = false;
        try {
          await db.enableSync({ key: invalidKey });
        } catch (error) {
          errorThrown = true;
          if (!/Sync key must be a 64-character hex string/i.test(error.message)) {
            throw new Error(`Wrong error message for invalid key: ${error.message}`);
          }
        }

        if (!errorThrown) {
          throw new Error(`Should have thrown error for invalid key: ${invalidKey}`);
        }
      }
    } finally {
      await db.close();
      await cleanupTempFile(kvPath);
    }
  });

  await test("enableSync works with valid key", async () => {
    const db = new GunDB();
    const kvPath = await createTempFile();

    try {
      await db.ready(kvPath);
      const key = generateSyncKey();

      await db.enableSync({ key });

      if (!db.isSyncEnabled()) throw new Error("Sync should be enabled");
      if (db.getSyncKey() !== key) throw new Error("Sync key mismatch");

      const stats = db.getSyncStats();
      if (!stats) throw new Error("Stats should not be null");
      if (stats.peersConnected !== 0) throw new Error("Should have no peers initially");

      await db.disableSync();
      if (db.isSyncEnabled()) throw new Error("Sync should be disabled after disableSync");
    } finally {
      await db.close();
      await cleanupTempFile(kvPath);
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

// Run tests
runTests().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
