import { GunDB } from "../src/core/database.ts";

const db = new GunDB();
await db.ready();

await db.put("user:alice", {
  name: "Alice",
  age: 30,
  city: "London",
  vector: undefined,
});

const user = await db.get("user:alice");
console.log("User:", user);

db.on("user:alice", (node) => console.log("Updated user:alice:", node));

await db.put("note:1", { text: "I love visiting museums in London" });
await db.put("note:2", { text: "Best pizza in New York" });
await db.put("note:3", { text: "Parks and galleries around London are great" });

const results = await db.vectorSearch("Find things about London", 5);
console.log(
  "Semantic search results:",
  results.map((n) => ({
    id: n.id,
    text: (n.data as Record<string, unknown>).text,
  })),
);
