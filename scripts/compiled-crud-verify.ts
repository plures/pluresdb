import { GunDB } from "../src/core/database.ts";

const serverUrl = Deno.env.get("SERVER_URL") ?? "ws://localhost:34567";

const clientA = new GunDB();
const clientB = new GunDB();

const kvA = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });
const kvB = await Deno.makeTempFile({ prefix: "kv_", suffix: ".sqlite" });

await clientA.ready(kvA);
await clientB.ready(kvB);

clientA.connect(serverUrl);
clientB.connect(serverUrl);

const id = `bin:crud:${crypto.randomUUID()}`;

const receivedOnB = new Promise<void>((resolve) =>
  clientB.on(id, (n) => n && resolve())
);

await clientA.put(id, { text: "compiled works" } as Record<string, unknown>);

await receivedOnB;

await clientA.close();
await clientB.close();

console.log("COMPILED-CRUD-OK", id);
