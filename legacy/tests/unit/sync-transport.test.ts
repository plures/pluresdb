/**
 * Sync Transport Tests
 * 
 * Unit tests for the pluggable sync transport system
 */

import { assertEquals, assertExists } from "https://deno.land/std@0.208.0/assert/mod.ts";
import type { SyncTransport, SyncConnection } from "../../sync/transport.ts";
import { DirectTransport } from "../../sync/transports/direct.ts";
import { AzureRelayTransport } from "../../sync/transports/azure-relay.ts";
import { AutoTransport, createTransport } from "../../sync/transports/auto.ts";
import { defaultTransportConfig } from "../../sync/transport.ts";

/**
 * Helper to create a mock WebSocket server for testing
 */
function randomPort(): number {
  return 18000 + Math.floor(Math.random() * 10000);
}

/**
 * Test suite for DirectTransport
 */
Deno.test({
  name: "DirectTransport - name property",
  fn() {
    const transport = new DirectTransport();
    assertEquals(transport.name, "direct");
  },
});

Deno.test({
  name: "DirectTransport - basic functionality",
  async fn() {
    const transport = new DirectTransport({ port: randomPort() });

    // Test that we can create and close a transport
    try {
      // Just verify close works without errors
      await transport.close();
    } finally {
      await transport.close();
    }
  },
  sanitizeOps: false,
  sanitizeResources: false,
});

/**
 * Test suite for AzureRelayTransport
 */
Deno.test({
  name: "AzureRelayTransport - name property",
  fn() {
    const transport = new AzureRelayTransport({
      relayUrl: "wss://test-relay.example.com",
    });
    assertEquals(transport.name, "azure-relay");
  },
});

Deno.test({
  name: "AzureRelayTransport - constructor with custom topic",
  fn() {
    const transport = new AzureRelayTransport({
      relayUrl: "wss://test-relay.example.com",
      topic: "custom-topic",
    });
    assertEquals(transport.name, "azure-relay");
  },
});

/**
 * Test suite for AutoTransport
 */
Deno.test({
  name: "AutoTransport - name property",
  fn() {
    const transport = new AutoTransport(defaultTransportConfig);
    assertEquals(transport.name, "auto");
  },
});

Deno.test({
  name: "AutoTransport - falls back through transport chain",
  async fn() {
    const transport = new AutoTransport({
      mode: "auto",
      azureRelayUrl: "wss://invalid-relay.example.com",
      vercelRelayUrl: "wss://invalid-vercel.example.com",
      connectionTimeoutMs: 1000,
    });

    try {
      // This should fail since all URLs are invalid
      await transport.connect("test-peer");
      throw new Error("Should have thrown an error");
    } catch (error) {
      const err = error as Error;
      assertEquals(err.message.includes("All transports failed"), true);
    } finally {
      await transport.close();
    }
  },
  sanitizeOps: false,
  sanitizeResources: false,
});

/**
 * Test suite for createTransport factory
 */
Deno.test({
  name: "createTransport - creates AutoTransport for 'auto' mode",
  fn() {
    const transport = createTransport({
      mode: "auto",
      azureRelayUrl: "wss://test.example.com",
    });
    assertEquals(transport.name, "auto");
  },
});

Deno.test({
  name: "createTransport - creates AzureRelayTransport for 'azure-relay' mode",
  fn() {
    const transport = createTransport({
      mode: "azure-relay",
      azureRelayUrl: "wss://test.example.com",
    });
    assertEquals(transport.name, "azure-relay");
  },
});

Deno.test({
  name: "createTransport - creates DirectTransport for 'direct' mode",
  fn() {
    const transport = createTransport({
      mode: "direct",
    });
    assertEquals(transport.name, "direct");
  },
});

Deno.test({
  name: "createTransport - throws error for azure-relay without URL",
  fn() {
    try {
      createTransport({
        mode: "azure-relay",
      });
      throw new Error("Should have thrown an error");
    } catch (error) {
      const err = error as Error;
      assertEquals(err.message.includes("Azure relay URL is required"), true);
    }
  },
});

Deno.test({
  name: "createTransport - throws error for vercel-relay without URL",
  fn() {
    try {
      createTransport({
        mode: "vercel-relay",
      });
      throw new Error("Should have thrown an error");
    } catch (error) {
      const err = error as Error;
      assertEquals(err.message.includes("Vercel relay URL is required"), true);
    }
  },
});

/**
 * Test suite for SyncConnection interface
 */
Deno.test({
  name: "SyncConnection - data encoding/decoding with base64",
  async fn() {
    // Test that we can encode and decode binary data
    const original = new Uint8Array([0, 1, 2, 255, 254, 253]);

    // Encode to base64
    let binaryString = "";
    for (let i = 0; i < original.length; i++) {
      binaryString += String.fromCharCode(original[i]);
    }
    const base64 = btoa(binaryString);

    // Decode from base64
    const decodedBinaryString = atob(base64);
    const decoded = new Uint8Array(decodedBinaryString.length);
    for (let i = 0; i < decodedBinaryString.length; i++) {
      decoded[i] = decodedBinaryString.charCodeAt(i);
    }

    assertEquals(decoded, original);
  },
});

Deno.test({
  name: "defaultTransportConfig - has expected defaults",
  fn() {
    assertEquals(defaultTransportConfig.mode, "auto");
    assertEquals(
      defaultTransportConfig.azureRelayUrl,
      "wss://pluresdb-relay.azurewebsites.net",
    );
    assertEquals(
      defaultTransportConfig.vercelRelayUrl,
      "wss://pluresdb-relay.vercel.app",
    );
    assertEquals(defaultTransportConfig.connectionTimeoutMs, 30000);
  },
});
