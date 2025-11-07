// @ts-nocheck
import { assertEquals, assertThrows } from "jsr:@std/assert@1.0.14";
import { GunDB } from "../../core/database.ts";

Deno.test("Subscriptions - Basic Update Events", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const id = "user:test";
    let updateCount = 0;
    let lastData: any = null;

    const unsubscribe = db.on(id, (node) => {
      updateCount++;
      lastData = node?.data;
    });

    // Test initial put
    await db.put(id, { name: "Alice", age: 30 });
    await new Promise((resolve) => setTimeout(resolve, 100));
    assertEquals(updateCount, 1);
    assertEquals(lastData?.name, "Alice");

    // Test update
    await db.put(id, { name: "Alice", age: 31 });
    await new Promise((resolve) => setTimeout(resolve, 100));
    assertEquals(updateCount, 2);
    assertEquals(lastData?.age, 31);

    // Test unsubscribe
    unsubscribe();
    await db.put(id, { name: "Alice", age: 32 });
    await new Promise((resolve) => setTimeout(resolve, 100));
    assertEquals(updateCount, 2); // Should not increment
  } finally {
    await db.close();
  }
});

Deno.test("Subscriptions - Delete Events", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const id = "user:delete";
    let deleteReceived = false;

    const unsubscribe = db.on(id, (node) => {
      if (node === null) {
        deleteReceived = true;
      }
    });

    await db.put(id, { name: "Bob" });
    await db.delete(id);
    await new Promise((resolve) => setTimeout(resolve, 100));

    assertEquals(deleteReceived, true);
    unsubscribe();
  } finally {
    await db.close();
  }
});

Deno.test("Subscriptions - Multiple Subscribers", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const id = "user:multi";
    let subscriber1Count = 0;
    let subscriber2Count = 0;

    const unsubscribe1 = db.on(id, () => subscriber1Count++);
    const unsubscribe2 = db.on(id, () => subscriber2Count++);

    await db.put(id, { name: "Charlie" });
    await new Promise((resolve) => setTimeout(resolve, 100));

    assertEquals(subscriber1Count, 1);
    assertEquals(subscriber2Count, 1);

    unsubscribe1();
    unsubscribe2();
  } finally {
    await db.close();
  }
});

Deno.test("Subscriptions - Error Handling", () => {
  const db = new GunDB();

  // Test subscription before ready
  assertThrows(() => db.on("test", () => {}), Error, "Database not ready");
});

Deno.test("Subscriptions - Off Method", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    const id = "user:off";
    let called = false;

    const callback = () => {
      called = true;
    };
    db.on(id, callback);
    db.off(id, callback);

    await db.put(id, { name: "Dave" });
    await new Promise((resolve) => setTimeout(resolve, 100));

    assertEquals(called, false);
  } finally {
    await db.close();
  }
});

