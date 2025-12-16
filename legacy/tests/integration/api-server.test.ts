// @ts-nocheck
import { assertEquals, assertExists } from "jsr:@std/assert@1.0.14";
import { GunDB } from "../../core/database.ts";
import { type ApiServerHandle, startApiServer } from "../../http/api-server.ts";

function randomPort(): number {
  return 18000 + Math.floor(Math.random() * 10000);
}

Deno.test("API Server - HTTP Endpoints", async () => {
  const db = new GunDB();
  let api: ApiServerHandle | null = null;
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const port = randomPort();
    const apiPort = port + 1;
    db.serve({ port });
    api = startApiServer({ port: apiPort, db });

    const baseUrl = `http://localhost:${apiPort}`;

    // Test PUT endpoint
    const putResponse = await fetch(`${baseUrl}/api/nodes/test:1`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "Test Node", value: 123 }),
    });
    assertEquals(putResponse.status, 200);
    await putResponse.body?.cancel();

    // Test GET endpoint
    const getResponse = await fetch(`${baseUrl}/api/nodes/test:1`);
    assertEquals(getResponse.status, 200);
    const data = await getResponse.json();
    assertEquals(data.name, "Test Node");
    assertEquals(data.value, 123);

    // Test DELETE endpoint
    const deleteResponse = await fetch(`${baseUrl}/api/nodes/test:1`, {
      method: "DELETE",
    });
    assertEquals(deleteResponse.status, 200);
    await deleteResponse.body?.cancel();

    // Verify deletion
    const getAfterDelete = await fetch(`${baseUrl}/api/nodes/test:1`);
    assertEquals(getAfterDelete.status, 404);
    await getAfterDelete.body?.cancel();
  } finally {
    api?.close();
    await db.close();
  }
});

Deno.test("API Server - Vector Search Endpoint", async () => {
  const db = new GunDB();
  let api: ApiServerHandle | null = null;
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    // Add test data
    await db.put("doc:1", { text: "Machine learning algorithms" });
    await db.put("doc:2", { text: "Cooking recipes and food" });

    const port = randomPort();
    const apiPort = port + 1;
    db.serve({ port });
    api = startApiServer({ port: apiPort, db });

    const baseUrl = `http://localhost:${apiPort}`;

    // Test vector search endpoint
    const searchResponse = await fetch(`${baseUrl}/api/search`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        query: "machine learning",
        limit: 5,
      }),
    });

    assertEquals(searchResponse.status, 200);
    const results = await searchResponse.json();
    assertExists(results);
    assertEquals(Array.isArray(results), true);
  } finally {
    api?.close();
    await db.close();
  }
});

Deno.test("API Server - WebSocket Connection", async () => {
  const db = new GunDB();
  let api: ApiServerHandle | null = null;
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const port = randomPort();
    const apiPort = port + 1;
    db.serve({ port });
    api = startApiServer({ port: apiPort, db });

    const wsUrl = `ws://localhost:${port}/ws`;

    // Test WebSocket connection
    const ws = new WebSocket(wsUrl);

    const connectionPromise = new Promise((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error("Connection timeout")),
        5000,
      );
      ws.onopen = () => {
        clearTimeout(timer);
        resolve(true);
      };
      ws.onerror = (error) => {
        clearTimeout(timer);
        reject(error);
      };
    });

    await connectionPromise;

    // Test sending data via WebSocket
    const messagePromise = new Promise((resolve) => {
      ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        if (data.type === "put") {
          resolve(data);
        }
      };
    });

    // Send a message
    ws.send(
      JSON.stringify({
        type: "put",
        id: "ws:test",
        data: { message: "Hello WebSocket" },
      }),
    );

    await messagePromise;
    ws.close();
  } finally {
    api?.close();
    await db.close();
  }
});

Deno.test("API Server - Error Handling", async () => {
  const db = new GunDB();
  let api: ApiServerHandle | null = null;
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const port = randomPort();
    const apiPort = port + 1;
    db.serve({ port });
    api = startApiServer({ port: apiPort, db });

    const baseUrl = `http://localhost:${apiPort}`;

    // Test 404 for non-existent node
    const notFoundResponse = await fetch(`${baseUrl}/api/nodes/nonexistent`);
    assertEquals(notFoundResponse.status, 404);
    await notFoundResponse.body?.cancel();

    // Test 400 for invalid JSON
    const invalidJsonResponse = await fetch(`${baseUrl}/api/nodes/test`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: "invalid json",
    });
    assertEquals(invalidJsonResponse.status, 400);
    await invalidJsonResponse.body?.cancel();
  } finally {
    api?.close();
    await db.close();
  }
});

Deno.test("API Server - CORS Headers", async () => {
  const db = new GunDB();
  let api: ApiServerHandle | null = null;
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const port = randomPort();
    const apiPort = port + 1;
    db.serve({ port });
    api = startApiServer({ port: apiPort, db });

    const baseUrl = `http://localhost:${apiPort}`;

    // Test OPTIONS request for CORS
    const optionsResponse = await fetch(`${baseUrl}/api/nodes/test`, {
      method: "OPTIONS",
      headers: {
        Origin: "http://localhost:3000",
        "Access-Control-Request-Method": "PUT",
      },
    });

    assertEquals(optionsResponse.status, 200);
    assertExists(optionsResponse.headers.get("Access-Control-Allow-Origin"));
    assertExists(optionsResponse.headers.get("Access-Control-Allow-Methods"));
    await optionsResponse.body?.cancel();
  } finally {
    api?.close();
    await db.close();
  }
});

Deno.test("API Server - P2P API Endpoints", async () => {
  const db = new GunDB();
  let api: ApiServerHandle | null = null;
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const port = randomPort();
    const apiPort = port + 1;
    db.serve({ port });
    api = startApiServer({ port: apiPort, db });

    const baseUrl = `http://localhost:${apiPort}`;

    // Test createIdentity endpoint
    const identityResponse = await fetch(`${baseUrl}/api/identity`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "John Doe", email: "john@example.com" }),
    });
    assertEquals(identityResponse.status, 200);
    const identity = await identityResponse.json();
    assertExists(identity.id);
    assertExists(identity.publicKey);
    assertEquals(identity.name, "John Doe");
    assertEquals(identity.email, "john@example.com");

    // Test searchPeers endpoint
    const peersResponse = await fetch(`${baseUrl}/api/peers/search?q=developer`);
    assertEquals(peersResponse.status, 200);
    const peers = await peersResponse.json();
    assertEquals(Array.isArray(peers), true);

    // Test shareNode endpoint
    const shareResponse = await fetch(`${baseUrl}/api/share`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        nodeId: "node:123",
        targetPeerId: "peer:456",
        accessLevel: "read-only",
      }),
    });
    assertEquals(shareResponse.status, 200);
    const shareData = await shareResponse.json();
    assertExists(shareData.sharedNodeId);
    assertEquals(shareData.nodeId, "node:123");
    assertEquals(shareData.targetPeerId, "peer:456");
    assertEquals(shareData.accessLevel, "read-only");

    // Test acceptSharedNode endpoint
    const acceptResponse = await fetch(`${baseUrl}/api/share/accept`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ sharedNodeId: "shared:789" }),
    });
    assertEquals(acceptResponse.status, 200);
    const acceptData = await acceptResponse.json();
    assertEquals(acceptData.success, true);
    assertEquals(acceptData.sharedNodeId, "shared:789");

    // Test addDevice endpoint
    const deviceResponse = await fetch(`${baseUrl}/api/devices`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "My Laptop", type: "laptop" }),
    });
    assertEquals(deviceResponse.status, 200);
    const device = await deviceResponse.json();
    assertExists(device.id);
    assertEquals(device.name, "My Laptop");
    assertEquals(device.type, "laptop");
    assertEquals(device.status, "online");

    // Test syncWithDevice endpoint
    const syncResponse = await fetch(`${baseUrl}/api/devices/sync`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ deviceId: "device:123" }),
    });
    assertEquals(syncResponse.status, 200);
    const syncData = await syncResponse.json();
    assertEquals(syncData.success, true);
    assertEquals(syncData.deviceId, "device:123");

    // Test GET devices list
    const devicesListResponse = await fetch(`${baseUrl}/api/devices`);
    assertEquals(devicesListResponse.status, 200);
    const devicesList = await devicesListResponse.json();
    assertEquals(Array.isArray(devicesList), true);
  } finally {
    api?.close();
    await db.close();
  }
});
