// @ts-nocheck
import { assertEquals, assertExists } from "jsr:@std/assert@1.0.14";
import { GunDB } from "../../core/database.ts";

Deno.test("Vector Search - Basic Functionality", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    // Add documents with different content
    await db.put("doc:1", {
      text: "Machine learning and artificial intelligence algorithms",
      content: "Deep learning neural networks for pattern recognition",
    });
    await db.put("doc:2", {
      text: "Cooking recipes and food preparation techniques",
      content: "Italian pasta recipes and cooking methods",
    });
    await db.put("doc:3", {
      text: "Web development and JavaScript programming",
      content: "React and TypeScript for modern web applications",
    });

    // Test search with text field
    const results1 = await db.vectorSearch("machine learning", 2);
    assertExists(results1);
    assertEquals(results1.length, 2);

    // Test search with content field
    const results2 = await db.vectorSearch("neural networks", 1);
    assertExists(results2);
    assertEquals(results2.length, 1);
    assertEquals(results2[0].id, "doc:1");

    // Test search with different query
    const results3 = await db.vectorSearch("cooking food", 1);
    assertExists(results3);
    assertEquals(results3.length, 1);
    assertEquals(results3[0].id, "doc:2");
  } finally {
    await db.close();
  }
});

Deno.test("Vector Search - Similarity Scoring", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    await db.put("doc:1", { text: "Machine learning algorithms" });
    await db.put("doc:2", { text: "Machine learning and AI" });
    await db.put("doc:3", { text: "Cooking recipes" });

    const results = await db.vectorSearch("machine learning", 3);
    assertExists(results);
    assertEquals(results.length, 3);

    // Results should be ordered by similarity (highest first)
    assertExists(results[0].similarity);
    assertExists(results[1].similarity);
    assertExists(results[2].similarity);

    // First result should have highest similarity
    assertEquals(results[0].similarity >= results[1].similarity, true);
    assertEquals(results[1].similarity >= results[2].similarity, true);
  } finally {
    await db.close();
  }
});

Deno.test("Vector Search - Limit Parameter", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    // Add multiple documents
    for (let i = 1; i <= 10; i++) {
      await db.put(`doc:${i}`, {
        text: `Document ${i} about machine learning and AI`,
      });
    }

    // Test different limits
    const results1 = await db.vectorSearch("machine learning", 3);
    assertEquals(results1.length, 3);

    const results2 = await db.vectorSearch("machine learning", 5);
    assertEquals(results2.length, 5);

    const results3 = await db.vectorSearch("machine learning", 1);
    assertEquals(results3.length, 1);
  } finally {
    await db.close();
  }
});

Deno.test("Vector Search - Empty Results", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    // Search in empty database
    const results = await db.vectorSearch("anything", 5);
    assertEquals(results.length, 0);
  } finally {
    await db.close();
  }
});

Deno.test("Vector Search - Custom Vector Input", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    // Add document with custom vector
    const customVector = [0.1, 0.2, 0.3, 0.4, 0.5];
    await db.put("doc:custom", {
      text: "Custom vector document",
      vector: customVector,
    });

    // Search with custom vector
    const results = await db.vectorSearch(customVector, 1);
    assertExists(results);
    assertEquals(results.length, 1);
    assertEquals(results[0].id, "doc:custom");
  } finally {
    await db.close();
  }
});

Deno.test("Vector Search - No Text Content", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({
      prefix: "kv_",
      suffix: ".sqlite",
    });
    await db.ready(kvPath);

    // Add document without text or content
    await db.put("doc:no-text", {
      name: "Document without text",
      value: 123,
    });

    // Search should return empty results
    const results = await db.vectorSearch("anything", 5);
    assertEquals(results.length, 0);
  } finally {
    await db.close();
  }
});
