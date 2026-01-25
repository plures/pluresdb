/**
 * Tests for Local-First Unified API
 */

import { assertEquals, assertExists } from "https://deno.land/std@0.208.0/assert/mod.ts";
import { PluresDBLocalFirst } from "../legacy/local-first/unified-api.ts";

Deno.test("PluresDBLocalFirst - Runtime detection", () => {
  // Should detect "network" mode in Deno test environment
  const db = new PluresDBLocalFirst({ mode: "auto" });
  const mode = db.getMode();
  
  assertEquals(mode, "network", "Should default to network mode in Deno");
});

Deno.test("PluresDBLocalFirst - Manual mode selection", () => {
  // Should allow manual mode selection
  const db = new PluresDBLocalFirst({ mode: "network", port: 34567 });
  const mode = db.getMode();
  
  assertEquals(mode, "network", "Should use network mode when explicitly set");
});

Deno.test("PluresDBLocalFirst - WASM mode throws not implemented", async () => {
  const db = new PluresDBLocalFirst({ mode: "wasm", dbName: "test-db" });
  
  try {
    await db.put("test:1", { value: "test" });
    throw new Error("Should have thrown not implemented error");
  } catch (error) {
    assertEquals(
      error instanceof Error && error.message.includes("not yet implemented"),
      true,
      "Should throw not implemented error for WASM mode"
    );
  }
});

Deno.test("PluresDBLocalFirst - IPC mode throws not implemented", async () => {
  const db = new PluresDBLocalFirst({ mode: "ipc", channelName: "test-channel" });
  
  try {
    await db.put("test:1", { value: "test" });
    throw new Error("Should have thrown not implemented error");
  } catch (error) {
    assertEquals(
      error instanceof Error && error.message.includes("not yet implemented"),
      true,
      "Should throw not implemented error for IPC mode"
    );
  }
});

Deno.test("PluresDBLocalFirst - Network mode API surface", () => {
  const db = new PluresDBLocalFirst({ mode: "network", port: 34567 });
  
  // Check that all required methods exist
  assertExists(db.put, "Should have put method");
  assertExists(db.get, "Should have get method");
  assertExists(db.delete, "Should have delete method");
  assertExists(db.list, "Should have list method");
  assertExists(db.vectorSearch, "Should have vectorSearch method");
  assertExists(db.close, "Should have close method");
  assertExists(db.getMode, "Should have getMode method");
});
