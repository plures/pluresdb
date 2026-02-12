/**
 * Azure Relay Integration Tests
 * 
 * Tests the Azure relay transport with a local relay server
 */

import { assertEquals, assertExists } from "https://deno.land/std@0.208.0/assert/mod.ts";
import { AzureRelayTransport } from "../../sync/transports/azure-relay.ts";
import type { SyncConnection } from "../../sync/transport.ts";

const shouldRunRelayTests =
  (Deno.env.get("RUN_RELAY_TESTS") ?? "").toLowerCase() === "true";

if (!shouldRunRelayTests) {
  console.warn(
    "[relay-integration.test] Skipping relay integration tests. Set RUN_RELAY_TESTS=true and start the relay server to enable.",
  );
}

function relayTest(name: string, fn: () => Promise<void>) {
  Deno.test({
    name,
    ignore: !shouldRunRelayTests,
    sanitizeOps: false,
    sanitizeResources: false,
    fn,
  });
}

relayTest("Azure Relay - connect and communicate", async () => {
  const relayUrl = Deno.env.get("RELAY_URL") || "ws://localhost:8080";
  const topic = `test-topic-${Date.now()}`;

  const transportA = new AzureRelayTransport({ relayUrl, topic });
  const transportB = new AzureRelayTransport({ relayUrl, topic });

  try {
    // Start listening on transport B
    let receivedConnection = false;
    await transportB.listen((conn: SyncConnection) => {
      receivedConnection = true;
    });

    // Give server time to start
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Connect from transport A
    const connA = await transportA.connect("peer-a");

    // Wait for connection
    await new Promise((resolve) => setTimeout(resolve, 200));

    assertEquals(receivedConnection, true, "Should receive connection on transport B");

    // Test sending data
    const testData = new TextEncoder().encode("Hello from A");
    await connA.send(testData);

    // Wait for data to relay
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Cleanup
    await connA.close();
    await transportA.close();
    await transportB.close();
  } catch (error) {
    console.error("Test failed:", error);
    throw error;
  }
});

relayTest("Azure Relay - multiple peers in same topic", async () => {
  const relayUrl = Deno.env.get("RELAY_URL") || "ws://localhost:8080";
  const topic = `multi-peer-topic-${Date.now()}`;

  const transport1 = new AzureRelayTransport({ relayUrl, topic });
  const transport2 = new AzureRelayTransport({ relayUrl, topic });
  const transport3 = new AzureRelayTransport({ relayUrl, topic });

  try {
    // All peers listen
    const connections: SyncConnection[] = [];
    
    await transport1.listen((conn) => connections.push(conn));
    await transport2.listen((conn) => connections.push(conn));
    await transport3.listen((conn) => connections.push(conn));

    // Give time for setup
    await new Promise((resolve) => setTimeout(resolve, 500));

    // Connect from first peer
    const conn1 = await transport1.connect("peer-1");
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Verify we have connections
    assertExists(conn1, "Connection from peer 1 should exist");

    // Cleanup
    await conn1.close();
    await transport1.close();
    await transport2.close();
    await transport3.close();
  } catch (error) {
    console.error("Test failed:", error);
    throw error;
  }
});

relayTest("Azure Relay - topic isolation", async () => {
  const relayUrl = Deno.env.get("RELAY_URL") || "ws://localhost:8080";
  const topic1 = `topic-1-${Date.now()}`;
  const topic2 = `topic-2-${Date.now()}`;

  const transportA1 = new AzureRelayTransport({ relayUrl, topic: topic1 });
  const transportA2 = new AzureRelayTransport({ relayUrl, topic: topic1 });
  const transportB1 = new AzureRelayTransport({ relayUrl, topic: topic2 });

  try {
    // Listen on different topics
    let connectionsA = 0;
    let connectionsB = 0;

    await transportA1.listen(() => connectionsA++);
    await transportA2.listen(() => connectionsA++);
    await transportB1.listen(() => connectionsB++);

    await new Promise((resolve) => setTimeout(resolve, 200));

    // Connect to topic A
    const connA = await transportA1.connect("peer-a1");
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Only topic A should have connections
    assertEquals(connectionsB, 0, "Topic B should have no connections");

    // Cleanup
    await connA.close();
    await transportA1.close();
    await transportA2.close();
    await transportB1.close();
  } catch (error) {
    console.error("Test failed:", error);
    throw error;
  }
});
