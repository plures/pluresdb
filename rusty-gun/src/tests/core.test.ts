import { assertEquals } from "https://deno.land/std@0.224.0/assert/mod.ts";
import { GunDB } from "../core/database.ts";

Deno.test("put and get returns stored data", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
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

Deno.test("subscription receives updates", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
    await db.ready(kvPath);
    const updated = new Promise((resolve) => db.on("user:bob", (n) => n && (n.data as any).age === 42 && resolve(true)));
    await db.put("user:bob", { name: "Bob", age: 41 });
    await db.put("user:bob", { name: "Bob", age: 42 });
    await updated;
  } finally {
    await db.close();
  }
});

Deno.test("vector search returns relevant notes", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
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

Deno.test("delete emits subscription with null", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
    await db.ready(kvPath);
    await db.put("user:carol", { name: "Carol" });
    const deleted = new Promise((resolve) => db.on("user:carol", (n) => n === null && resolve(true)));
    await db.delete("user:carol");
    await deleted;
  } finally {
    await db.close();
  }
});

Deno.test({ name: "mesh snapshot sync and propagation", sanitizeOps: false, sanitizeResources: false }, async () => {
  function randomPort() { return 18000 + Math.floor(Math.random() * 10000); }
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

    const receivedSnapshot = new Promise((resolve) => dbB.on("mesh:one", (n) => n && resolve(true)));
    dbB.connect(serverUrl);
    await receivedSnapshot;

    const receivedOnA = new Promise((resolve) => dbA.on("mesh:fromB", (n) => n && (n.data as any).who === "B" && resolve(true)));
    await dbB.put("mesh:fromB", { who: "B", text: "hi A" });
    await receivedOnA;
  } finally {
    await dbB.close();
    await dbA.close();
  }
});
