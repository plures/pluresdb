# Rusty Gun (Deno)

A lightweight, Deno-first reimagining of GunDB with CRDT-based sync, optional
vectors for semantic search, and a simple mesh over WebSockets.

## Quick start

- Prerequisite: Deno v1.44+ with `--unstable-kv` support

- Run a node

```sh
deno run -A src/main.ts serve --port 8080
```

- Connect two nodes

```sh
deno run -A src/main.ts serve --port 8081 ws://localhost:8080
```

- Run tests / lint / fmt / type-check

```sh
deno task test
deno task lint
deno fmt --check
deno task check
```

- Compile a single binary

```sh
deno task compile
```

## Minimal usage (programmatic)

```ts
import { GunDB } from "./src/core/database.ts";

const db = new GunDB();
await db.ready();

await db.put("user:alice", { name: "Alice", age: 30 });
const alice = await db.get<{ name: string; age: number }>("user:alice");

db.on("user:alice", (node) => console.log("Updated:", node));
await db.put("user:alice", { name: "Alice", age: 31 });

// Semantic search (auto-embeds vectors for text fields)
await db.put("note:1", { text: "Museums in London" });
const hits = await db.vectorSearch("London", 3);

await db.close();
```

## API outline

- `constructor(options?)`
  - `options.kvPath?: string`
  - `options.peerId?: string`
- `ready(kvPath?)`: Open Deno.Kv (optionally overriding path)
- `put(id: string, data: Record<string, unknown>)`: Upsert data. Auto-embeds
  `vector` when `text` or `content` fields are present
- `get<T>(id: string): Promise<(T & { id: string }) | null>`
- `delete(id: string)`
- `on(id: string, cb: (node: NodeRecord | null) => void)`,
  `off(id: string, cb?)`
- `vectorSearch(query: string | number[], limit: number)`
- `serve({ port }?)`: Start a WebSocket mesh server
- `connect(url: string)`: Connect to a peer (`ws://`)
- `close()`: Close sockets and storage

### Data conventions

- Optional `type: string` may be stored on any node (e.g.,
  `{ type: "Person", name: "Alice" }`)
- Vectors: When `text` or `content` exists, a simple local embedding is
  generated. You can also provide an explicit `vector: number[]`.

## License

MIT


