import { GunDB } from "../core/database.ts";
import type { Rule } from "../logic/rules.ts";

Deno.test("rule engine classification: Person.age >= 18 -> adult = true", async () => {
  const db = new GunDB();
  try {
    const kvPath = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
    await db.ready(kvPath);

    const rule: Rule = {
      name: "adultClassifier",
      whenType: "Person",
      predicate: (node) =>
        typeof (node.data as any).age === "number" && (node.data as any).age >= 18,
      action: async (ctx, node) => {
        const data = { ...(node.data as Record<string, unknown>), adult: true };
        await ctx.db.put(node.id, data);
      },
    };
    db.addRule(rule);

    await db.put("p:alice", { name: "Alice", age: 20, type: "Person" });
    const got = await db.get<{ adult?: boolean }>("p:alice");
    if (!got || got.adult !== true) {
      throw new Error("Expected adult flag set by rule");
    }
  } finally {
    await db.close();
  }
});
