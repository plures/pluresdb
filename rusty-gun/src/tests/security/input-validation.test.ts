import { assertEquals, assertRejects } from "https://deno.land/std@0.224.0/assert/mod.ts";
import { GunDB } from "../../core/database.ts";

Deno.test("Security - SQL Injection Prevention", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Test malicious SQL injection attempts
    const maliciousInputs = [
      "'; DROP TABLE users; --",
      "' OR '1'='1",
      "'; INSERT INTO users VALUES ('hacker', 'password'); --",
      "'; UPDATE users SET password='hacked'; --",
      "'; DELETE FROM users; --"
    ];
    
    for (const maliciousInput of maliciousInputs) {
      // These should be treated as regular string data, not executed as SQL
      await db.put("test:sql", { 
        malicious: maliciousInput,
        safe: "normal data"
      });
      
      const result = await db.get("test:sql");
      assertExists(result);
      assertEquals((result as any).malicious, maliciousInput);
      assertEquals((result as any).safe, "normal data");
    }
    
  } finally {
    await db.close();
  }
});

Deno.test("Security - XSS Prevention", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Test XSS payloads
    const xssPayloads = [
      "<script>alert('xss')</script>",
      "javascript:alert('xss')",
      "<img src=x onerror=alert('xss')>",
      "<svg onload=alert('xss')>",
      "';alert('xss');//"
    ];
    
    for (const payload of xssPayloads) {
      await db.put("test:xss", { 
        payload: payload,
        content: "Safe content"
      });
      
      const result = await db.get("test:xss");
      assertExists(result);
      // Data should be stored as-is without interpretation
      assertEquals((result as any).payload, payload);
    }
    
  } finally {
    await db.close();
  }
});

Deno.test("Security - Path Traversal Prevention", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Test path traversal attempts
    const pathTraversalAttempts = [
      "../../../etc/passwd",
      "..\\..\\..\\windows\\system32\\drivers\\etc\\hosts",
      "/etc/passwd",
      "C:\\Windows\\System32\\config\\SAM",
      "....//....//....//etc//passwd"
    ];
    
    for (const path of pathTraversalAttempts) {
      // These should be treated as regular key names, not file paths
      await db.put(path, { 
        content: "This should not access filesystem",
        path: path
      });
      
      const result = await db.get(path);
      assertExists(result);
      assertEquals((result as any).path, path);
    }
    
  } finally {
    await db.close();
  }
});

Deno.test("Security - Large Payload Prevention", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Test very large payloads
    const largePayload = "x".repeat(10 * 1024 * 1024); // 10MB
    
    await assertRejects(
      async () => {
        await db.put("test:large", { 
          data: largePayload 
        });
      },
      Error,
      "Payload too large"
    );
    
  } finally {
    await db.close();
  }
});

Deno.test("Security - Malformed JSON Handling", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Test malformed JSON inputs
    const malformedInputs = [
      "{ invalid json }",
      "{ \"incomplete\": ",
      "{ \"nested\": { \"broken\": } }",
      "{ \"array\": [1, 2, } }",
      "{ \"string\": \"unclosed }"
    ];
    
    for (const malformed of malformedInputs) {
      await assertRejects(
        async () => {
          await db.put("test:malformed", { 
            json: malformed 
          });
        },
        Error,
        "Invalid JSON"
      );
    }
    
  } finally {
    await db.close();
  }
});

Deno.test("Security - Type Confusion Prevention", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Test type confusion attempts
    const typeConfusionAttempts = [
      { __proto__: { isAdmin: true } },
      { constructor: { prototype: { isAdmin: true } } },
      { toString: () => "hacked" },
      { valueOf: () => 999999 }
    ];
    
    for (const attempt of typeConfusionAttempts) {
      await db.put("test:type", attempt);
      
      const result = await db.get("test:type");
      assertExists(result);
      
      // Should not have inherited properties
      assertEquals((result as any).isAdmin, undefined);
      assertEquals(typeof (result as any).toString, "string");
    }
    
  } finally {
    await db.close();
  }
});

Deno.test("Security - Vector Search Injection", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Test vector search with malicious inputs
    const maliciousQueries = [
      "'; DROP TABLE nodes; --",
      "<script>alert('xss')</script>",
      "../../../etc/passwd",
      "'; INSERT INTO nodes VALUES ('hacked', 'data'); --"
    ];
    
    for (const query of maliciousQueries) {
      // Vector search should handle these safely
      const results = await db.vectorSearch(query, 5);
      assertEquals(Array.isArray(results), true);
      // Should not throw errors or execute malicious code
    }
    
  } finally {
    await db.close();
  }
});

Deno.test("Security - Subscription Injection", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Test subscription with malicious IDs
    const maliciousIds = [
      "'; DROP TABLE nodes; --",
      "<script>alert('xss')</script>",
      "../../../etc/passwd",
      "'; INSERT INTO nodes VALUES ('hacked', 'data'); --"
    ];
    
    for (const id of maliciousIds) {
      // Subscriptions should handle these safely
      const unsubscribe = db.on(id, () => {});
      assertExists(unsubscribe);
      
      // Should be able to unsubscribe safely
      unsubscribe();
    }
    
  } finally {
    await db.close();
  }
});

Deno.test("Security - Memory Exhaustion Prevention", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Test creating many subscriptions to exhaust memory
    const subscriptions: Array<() => void> = [];
    
    try {
      for (let i = 0; i < 100000; i++) {
        const unsubscribe = db.on(`memory:${i}`, () => {});
        subscriptions.push(unsubscribe);
      }
    } catch (error) {
      // Should fail gracefully with memory limit
      assertEquals(error.message.includes("Memory limit"), true);
    }
    
    // Clean up any created subscriptions
    subscriptions.forEach(unsubscribe => unsubscribe());
    
  } finally {
    await db.close();
  }
});
