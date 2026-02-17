/**
 * Unit tests for Hyperswarm P2P sync functionality
 *
 * Tests key generation, sync enable/disable, and basic operations
 *
 * NOTE: Tests that require actual Hyperswarm connections are skipped in CI
 * environments (when CI=true) because:
 * 1. Hyperswarm requires Node.js native modules (udx-native) that fail in Deno
 * 2. Network-dependent tests create CI instability
 * 3. Full integration tests run locally where Hyperswarm is available
 *
 * To run tests locally: deno test -A --unstable-kv ...
 * (CI is unset by default locally; network test still skips in Deno due to udx-native)
 */

import { assertEquals, assertExists, assertMatch } from "jsr:@std/assert@1.0.14";
import { generateSyncKey } from "../../network/hyperswarm-sync.ts";
import { GunDB } from "../../core/database.ts";

// Detect CI environment - skip network-dependent tests in CI
const isCI = Deno.env.get("CI") === "true";

Deno.test("generateSyncKey - generates valid 64-char hex string", () => {
  const key = generateSyncKey();
  assertExists(key);
  assertEquals(typeof key, "string");
  assertEquals(key.length, 64);
  assertMatch(key, /^[0-9a-f]{64}$/i);
});

Deno.test("generateSyncKey - generates unique keys", () => {
  const key1 = generateSyncKey();
  const key2 = generateSyncKey();
  assertExists(key1);
  assertExists(key2);
  assertEquals(key1.length, 64);
  assertEquals(key2.length, 64);
  // Different keys should be different
  if (key1 === key2) {
    throw new Error("Generated keys should be unique (collision detected)");
  }
});

Deno.test("GunDB.generateSyncKey - static method works", () => {
  const key = GunDB.generateSyncKey();
  assertExists(key);
  assertEquals(key.length, 64);
  assertMatch(key, /^[0-9a-f]{64}$/i);
});

Deno.test("GunDB sync methods - available on instance", async () => {
  const db = new GunDB();
  const kvPath = await Deno.makeTempFile({ prefix: "sync_test_", suffix: ".sqlite" });

  try {
    await db.ready(kvPath);

    // Check that sync methods exist
    assertEquals(typeof db.enableSync, "function");
    assertEquals(typeof db.disableSync, "function");
    assertEquals(typeof db.getSyncStats, "function");
    assertEquals(typeof db.getSyncPeers, "function");
    assertEquals(typeof db.isSyncEnabled, "function");
    assertEquals(typeof db.getSyncKey, "function");

    // Initially sync should be disabled
    assertEquals(db.isSyncEnabled(), false);
    assertEquals(db.getSyncKey(), null);
    assertEquals(db.getSyncStats(), null);
    assertEquals(db.getSyncPeers().length, 0);
  } finally {
    await db.close();
    try {
      await Deno.remove(kvPath);
    } catch {
      /* ignore */
    }
  }
});

Deno.test("GunDB.enableSync - rejects invalid keys", async () => {
  const db = new GunDB();
  const kvPath = await Deno.makeTempFile({ prefix: "sync_test_", suffix: ".sqlite" });

  try {
    await db.ready(kvPath);

    // Test with invalid key formats
    const invalidKeys = [
      "short",
      "not-hex-characters-here!@#$%^&*()",
      "0".repeat(63), // 63 chars instead of 64
      "0".repeat(65), // 65 chars instead of 64
      "",
    ];

    for (const invalidKey of invalidKeys) {
      let errorThrown = false;
      try {
        await db.enableSync({ key: invalidKey });
      } catch (error) {
        errorThrown = true;
        assertExists(error);
        assertMatch(
          (error as Error).message,
          /Sync key must be a 64-character hex string/i,
        );
      }

      if (!errorThrown) {
        throw new Error(`Should have thrown error for invalid key: ${invalidKey}`);
      }
    }

    // Sync should still be disabled
    assertEquals(db.isSyncEnabled(), false);
  } finally {
    await db.close();
    try {
      await Deno.remove(kvPath);
    } catch {
      /* ignore */
    }
  }
});

Deno.test("GunDB.enableSync - requires database to be ready", async () => {
  const db = new GunDB();
  const key = generateSyncKey();

  let errorThrown = false;
  try {
    await db.enableSync({ key });
  } catch (error) {
    errorThrown = true;
    assertExists(error);
    assertMatch((error as Error).message, /Database not ready/i);
  }

  if (!errorThrown) {
    throw new Error("Should have thrown error when database not ready");
  }

  assertEquals(db.isSyncEnabled(), false);
});

// Note: The following test is skipped in CI because Hyperswarm requires Node.js
// This test verifies Deno incompatibility but requires importing hyperswarm which fails in CI
// In CI: Skipped to avoid udx-native native module errors
// Locally: Can run to verify the error message (though will still fail in Deno runtime)
Deno.test({
  name: "GunDB.enableSync - throws error in Deno environment",
  ignore: isCI, // Skip in CI to avoid udx-native native module errors
  async fn() {
    const db = new GunDB();
    const kvPath = await Deno.makeTempFile({ prefix: "sync_test_", suffix: ".sqlite" });

    try {
      await db.ready(kvPath);
      const key = generateSyncKey();

      let errorThrown = false;
      try {
        await db.enableSync({ key });
      } catch (error) {
        errorThrown = true;
        assertExists(error);
        // Should fail because Hyperswarm is Node.js only
        assertMatch(
          (error as Error).message,
          /Hyperswarm is only available in Node.js/i,
        );
      }

      if (!errorThrown) {
        throw new Error("Should have thrown error in Deno environment");
      }

      assertEquals(db.isSyncEnabled(), false);
    } finally {
      await db.close();
      try {
        await Deno.remove(kvPath);
      } catch {
        /* ignore */
      }
    }
  },
});
