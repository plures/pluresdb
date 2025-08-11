import { assertEquals } from "https://deno.land/std@0.224.0/assert/mod.ts";
import { GunDB } from "../core/database.ts";
import { mergeNodes } from "../core/crdt.ts";
import type { NodeRecord } from "../types/index.ts";

Deno.test("put and get returns stored data", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    const user = { name: "Alice", age: 30 } as const;
    await db.put("user:alice", user as unknown as Record<string, unknown>);
    const got = await db.get<typeof user>("user:alice");
    assertEquals(got?.name, "Alice");
    assertEquals(got?.age, 30);
  } finally {
    await db.close();
  }
});

Deno.test({ name: "subscription receives updates", sanitizeOps: false, sanitizeResources: false }, async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
     const updated = new Promise((resolve) =>
      db.on(
        "user:bob",
        (n) =>
          n &&
          (n.data as Record<string, unknown>).age === 42 &&
          resolve(true),
      )
    );
     await db.put("user:bob", { name: "Bob", age: 41 });
     await db.put("user:bob", { name: "Bob", age: 42 });
     const timeout = new Promise((_, rej) => setTimeout(() => rej(new Error("timeout: subscription")), 2000));
     await Promise.race([updated, timeout]);
  } finally {
    await db.close();
  }
});

Deno.test("vector search returns relevant notes", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    await db.put("note:london1", { text: "Museums and galleries in London" });
    await db.put("note:newyork1", { text: "Pizza places in New York" });
    const results = await db.vectorSearch("London", 1);
    if (results.length === 0) throw new Error("No vector results");
    if (results[0].id !== "note:london1") {
      throw new Error(`Expected note:london1 got ${results[0].id}`);
    }
  } finally {
    await db.close();
  }
});

Deno.test({ name: "delete emits subscription with null", sanitizeOps: false, sanitizeResources: false }, async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    await db.put("user:carol", { name: "Carol" });
     const deleted = new Promise((resolve) =>
      db.on("user:carol", (n) => n === null && resolve(true))
    );
     await db.delete("user:carol");
     const timeout = new Promise((_, rej) => setTimeout(() => rej(new Error("timeout: delete")), 2000));
     await Promise.race([deleted, timeout]);
  } finally {
    await db.close();
  }
});

Deno.test({
  name: "mesh snapshot sync and propagation",
  sanitizeOps: false,
  sanitizeResources: false,
}, async () => {
  function randomPort() {
    return 18000 + Math.floor(Math.random() * 10000);
  }
  const port = randomPort();
  const serverUrl = `ws://localhost:${port}`;

  const dbA = new GunDB();
  const dbB = new GunDB();
  try {
    const kvA = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
    const kvB = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
    await dbA.ready(kvA);
    await dbB.ready(kvB);
    await dbA.serve({ port });

    await dbA.put("mesh:one", { text: "hello from A" });

    const receivedSnapshot = new Promise((resolve) =>
      dbB.on("mesh:one", (n) => n && resolve(true))
    );
    dbB.connect(serverUrl);
    await receivedSnapshot;

    const receivedOnA = new Promise((resolve) =>
      dbA.on(
        "mesh:fromB",
        (n) =>
          n && (n.data as Record<string, unknown>).who === "B" && resolve(true),
      )
    );
    await dbB.put("mesh:fromB", { who: "B", text: "hi A" });
    await receivedOnA;
  } finally {
    await dbB.close();
    await dbA.close();
  }
});

// --- Additional tests to cover remaining checklist items ---

Deno.test("persists across restarts", async () => {
  const kvPath = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
  const id = "persist:one";

  const db1 = new GunDB();
  await db1.ready(kvPath);
  await db1.put(id, { value: 123 });
  await db1.close();

  const db2 = new GunDB();
  await db2.ready(kvPath);
  const got = await db2.get<{ value: number }>(id);
  await db2.close();

  if (!got) throw new Error("Expected value after restart");
  assertEquals(got.value, 123);
});

Deno.test("vector clock increments on local puts", async () => {
  const kvPath = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
  const id = "vc:counter";
  const db = new GunDB();
  await db.ready(kvPath);
  await db.put(id, { n: 1 });
  await db.put(id, { n: 2 });
  await db.put(id, { n: 3 });
  await db.close();

  // Inspect underlying record
  const { KvStorage } = await import("../storage/kv-storage.ts");
  const kv = new KvStorage();
  await kv.open(kvPath);
  const node = await kv.getNode(id);
  await kv.close();
  if (!node) throw new Error("Missing node for vector clock check");
  const clockValues = Object.values(node.vectorClock);
  assertEquals(clockValues.length, 1);
  assertEquals(clockValues[0], 3);
});

Deno.test("CRDT merge: equal timestamps deterministic merge", () => {
  const t = Date.now();
  const local: NodeRecord = {
    id: "n1",
    data: { a: 1, shared: 1 },
    vector: [0.1, 0.2],
    type: "TypeA",
    timestamp: t,
    vectorClock: { peerA: 2 },
  };
  const incoming: NodeRecord = {
    id: "n1",
    data: { b: 2, shared: 2 },
    // vector and type intentionally undefined to test fallback
    timestamp: t,
    vectorClock: { peerB: 3 },
  } as unknown as NodeRecord;

  const merged = mergeNodes(local, incoming);
  assertEquals(merged.id, "n1");
  assertEquals(merged.timestamp, t);
  assertEquals(merged.data, { a: 1, shared: 2, b: 2 });
  assertEquals(merged.type, "TypeA");
  assertEquals(merged.vector, [0.1, 0.2]);
  assertEquals(merged.vectorClock.peerA, 2);
  assertEquals(merged.vectorClock.peerB, 3);
});

Deno.test("CRDT merge: LWW on differing timestamps", () => {
  const t1 = 1000;
  const t2 = 2000;
  const base: NodeRecord = {
    id: "n2",
    data: { a: 1 },
    timestamp: t1,
    vectorClock: { p1: 1 },
  } as unknown as NodeRecord;
  const newer: NodeRecord = {
    id: "n2",
    data: { a: 999, b: 2 },
    timestamp: t2,
    vectorClock: { p2: 1 },
  } as unknown as NodeRecord;

  const up = mergeNodes(base, newer);
  assertEquals(up.data, { a: 999, b: 2 });
  assertEquals(up.timestamp, t2);

  const down = mergeNodes(newer, base);
  assertEquals(down.data, { a: 999, b: 2 });
  assertEquals(down.timestamp, t2);
});

Deno.test({ name: "off stops receiving events", sanitizeOps: false, sanitizeResources: false }, async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);
    let called = false;
    const id = "user:dave";
    const cb = () => {
      called = true;
    };
    db.on(id, cb);
    db.off(id, cb);
    await db.put(id, { name: "Dave" });
    await new Promise((r) => setTimeout(r, 200));
    assertEquals(called, false);
  } finally {
    await db.close();
  }
});
