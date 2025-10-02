import { assertEquals, assertExists } from "https://deno.land/std@0.224.0/assert/mod.ts";
import { GunDB } from "../../core/database.ts";

Deno.test("API Server - HTTP Endpoints", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Start server
    const port = 18000 + Math.floor(Math.random() * 10000);
    await db.serve({ port });
    
    const baseUrl = `http://localhost:${port}`;
    
    // Test PUT endpoint
    const putResponse = await fetch(`${baseUrl}/api/nodes/test:1`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: "Test Node", value: 123 })
    });
    assertEquals(putResponse.status, 200);
    
    // Test GET endpoint
    const getResponse = await fetch(`${baseUrl}/api/nodes/test:1`);
    assertEquals(getResponse.status, 200);
    const data = await getResponse.json();
    assertEquals(data.name, "Test Node");
    assertEquals(data.value, 123);
    
    // Test DELETE endpoint
    const deleteResponse = await fetch(`${baseUrl}/api/nodes/test:1`, {
      method: "DELETE"
    });
    assertEquals(deleteResponse.status, 200);
    
    // Verify deletion
    const getAfterDelete = await fetch(`${baseUrl}/api/nodes/test:1`);
    assertEquals(getAfterDelete.status, 404);
    
  } finally {
    await db.close();
  }
});

Deno.test("API Server - Vector Search Endpoint", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Add test data
    await db.put("doc:1", { text: "Machine learning algorithms" });
    await db.put("doc:2", { text: "Cooking recipes and food" });
    
    const port = 18000 + Math.floor(Math.random() * 10000);
    await db.serve({ port });
    
    const baseUrl = `http://localhost:${port}`;
    
    // Test vector search endpoint
    const searchResponse = await fetch(`${baseUrl}/api/search`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ 
        query: "machine learning", 
        limit: 5 
      })
    });
    
    assertEquals(searchResponse.status, 200);
    const results = await searchResponse.json();
    assertExists(results);
    assertEquals(Array.isArray(results), true);
    
  } finally {
    await db.close();
  }
});

Deno.test("API Server - WebSocket Connection", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    const port = 18000 + Math.floor(Math.random() * 10000);
    await db.serve({ port });
    
    const wsUrl = `ws://localhost:${port}/ws`;
    
    // Test WebSocket connection
    const ws = new WebSocket(wsUrl);
    
    const connectionPromise = new Promise((resolve, reject) => {
      ws.onopen = () => resolve(true);
      ws.onerror = (error) => reject(error);
      setTimeout(() => reject(new Error("Connection timeout")), 5000);
    });
    
    await connectionPromise;
    
    // Test sending data via WebSocket
    const messagePromise = new Promise((resolve) => {
      ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        if (data.type === "node_update") {
          resolve(data);
        }
      };
    });
    
    // Send a message
    ws.send(JSON.stringify({
      type: "put",
      id: "ws:test",
      data: { message: "Hello WebSocket" }
    }));
    
    await messagePromise;
    ws.close();
    
  } finally {
    await db.close();
  }
});

Deno.test("API Server - Error Handling", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    const port = 18000 + Math.floor(Math.random() * 10000);
    await db.serve({ port });
    
    const baseUrl = `http://localhost:${port}`;
    
    // Test 404 for non-existent node
    const notFoundResponse = await fetch(`${baseUrl}/api/nodes/nonexistent`);
    assertEquals(notFoundResponse.status, 404);
    
    // Test 400 for invalid JSON
    const invalidJsonResponse = await fetch(`${baseUrl}/api/nodes/test`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: "invalid json"
    });
    assertEquals(invalidJsonResponse.status, 400);
    
  } finally {
    await db.close();
  }
});

Deno.test("API Server - CORS Headers", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    const port = 18000 + Math.floor(Math.random() * 10000);
    await db.serve({ port });
    
    const baseUrl = `http://localhost:${port}`;
    
    // Test OPTIONS request for CORS
    const optionsResponse = await fetch(`${baseUrl}/api/nodes/test`, {
      method: "OPTIONS",
      headers: {
        "Origin": "http://localhost:3000",
        "Access-Control-Request-Method": "PUT"
      }
    });
    
    assertEquals(optionsResponse.status, 200);
    assertExists(optionsResponse.headers.get("Access-Control-Allow-Origin"));
    assertExists(optionsResponse.headers.get("Access-Control-Allow-Methods"));
    
  } finally {
    await db.close();
  }
});
