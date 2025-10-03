import { assertEquals, assertExists, assertThrows } from "https://deno.land/std@0.224.0/assert/mod.ts";
import { GunDB } from "../../core/database.ts";
import { mergeNodes } from "../../core/crdt.ts";
import type { NodeRecord } from "../../types/index.ts";

Deno.test("Core Database - Basic CRUD Operations", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Test put and get
    const user = { name: "Alice", age: 30, email: "alice@example.com" };
    await db.put("user:alice", user as unknown as Record<string, unknown>);
    const got = await db.get<typeof user>("user:alice");
    
    assertEquals(got?.name, "Alice");
    assertEquals(got?.age, 30);
    assertEquals(got?.email, "alice@example.com");
    
    // Test update
    await db.put("user:alice", { ...user, age: 31 });
    const updated = await db.get<typeof user>("user:alice");
    assertEquals(updated?.age, 31);
    
    // Test delete
    await db.delete("user:alice");
    const deleted = await db.get<typeof user>("user:alice");
    assertEquals(deleted, null);
  } finally {
    await db.close();
  }
});

Deno.test("Core Database - Vector Clock Increments", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    const id = "vc:test";
    await db.put(id, { value: 1 });
    await db.put(id, { value: 2 });
    await db.put(id, { value: 3 });
    
    // Inspect underlying record
    const { KvStorage } = await import("../../storage/kv-storage.ts");
    const kv = new KvStorage();
    await kv.open(kvPath);
    const node = await kv.getNode(id);
    await kv.close();
    
    assertExists(node);
    const clockValues = Object.values(node.vectorClock);
    assertEquals(clockValues.length, 1);
    assertEquals(clockValues[0], 3);
  } finally {
    await db.close();
  }
});

Deno.test("Core Database - Type System", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Test setType and instancesOf
    await db.put("person:1", { name: "Alice" });
    await db.setType("person:1", "Person");
    
    await db.put("company:1", { name: "Acme Corp" });
    await db.setType("company:1", "Company");
    
    const people = await db.instancesOf("Person");
    assertEquals(people.length, 1);
    assertEquals(people[0].id, "person:1");
    
    const companies = await db.instancesOf("Company");
    assertEquals(companies.length, 1);
    assertEquals(companies[0].id, "company:1");
  } finally {
    await db.close();
  }
});

Deno.test("Core Database - Vector Search", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    
    // Add documents with text content
    await db.put("doc:1", { text: "Machine learning and artificial intelligence" });
    await db.put("doc:2", { text: "Cooking recipes and food preparation" });
    await db.put("doc:3", { text: "Deep learning neural networks" });
    
    // Test vector search
    const results = await db.vectorSearch("machine learning", 2);
    assertExists(results);
    assertEquals(results.length, 2);
    
    // Should find the most relevant documents
    const docIds = results.map(r => r.id);
    assertExists(docIds.includes("doc:1"));
    assertExists(docIds.includes("doc:3"));
  } finally {
    await db.close();
  }
});

Deno.test("CRDT Merge - Equal Timestamps", () => {
  const timestamp = Date.now();
  const local: NodeRecord = {
    id: "test:1",
    data: { a: 1, shared: 1, nested: { x: 1, y: 1 } },
    vector: [0.1, 0.2],
    type: "TestType",
    timestamp,
    vectorClock: { peerA: 2 },
  };
  
  const incoming: NodeRecord = {
    id: "test:1",
    data: { b: 2, shared: 2, nested: { y: 2, z: 3 } },
    timestamp,
    vectorClock: { peerB: 3 },
  } as unknown as NodeRecord;
  
  const merged = mergeNodes(local, incoming);
  
  assertEquals(merged.id, "test:1");
  assertEquals(merged.timestamp, timestamp);
  assertEquals(merged.data, { 
    a: 1, 
    shared: 2, 
    b: 2, 
    nested: { x: 1, y: 2, z: 3 } 
  });
  assertEquals(merged.type, "TestType");
  assertEquals(merged.vector, [0.1, 0.2]);
  assertEquals(merged.vectorClock.peerA, 2);
  assertEquals(merged.vectorClock.peerB, 3);
});

Deno.test("CRDT Merge - LWW on Different Timestamps", () => {
  const t1 = 1000;
  const t2 = 2000;
  
  const older: NodeRecord = {
    id: "test:2",
    data: { a: 1, b: 1 },
    timestamp: t1,
    vectorClock: { p1: 1 },
  } as unknown as NodeRecord;
  
  const newer: NodeRecord = {
    id: "test:2",
    data: { a: 999, b: 2, c: 3 },
    timestamp: t2,
    vectorClock: { p2: 1 },
  } as unknown as NodeRecord;
  
  const merged = mergeNodes(older, newer);
  
  assertEquals(merged.data, { a: 999, b: 2, c: 3 });
  assertEquals(merged.timestamp, t2);
});

Deno.test("Core Database - Error Handling", async () => {
  const db = new GunDB();
  
  // Test operations before ready
  await assertThrows(
    async () => await db.put("test", { value: 1 }),
    Error,
    "Database not ready"
  );
  
  await assertThrows(
    async () => await db.get("test"),
    Error,
    "Database not ready"
  );
  
  await assertThrows(
    async () => await db.delete("test"),
    Error,
    "Database not ready"
  );
});

Deno.test("Core Database - Persistence Across Restarts", async () => {
  const kvPath = await Deno.makeTempFile({ 
    prefix: "kv_", 
    suffix: ".sqlite" 
  });
  const id = "persist:test";
  
  // First session
  const db1 = new GunDB();
  await db1.ready(kvPath);
  await db1.put(id, { value: 123, text: "persistent data" });
  await db1.close();
  
  // Second session
  const db2 = new GunDB();
  await db2.ready(kvPath);
  const got = await db2.get<{ value: number; text: string }>(id);
  await db2.close();
  
  assertExists(got);
  assertEquals(got.value, 123);
  assertEquals(got.text, "persistent data");
});
