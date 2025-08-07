import { GunDB } from "./core/database.ts";

function printUsage() {
  console.log("Usage:\n  deno run -A src/main.ts serve [--port <port>] [ws://peer1 ws://peer2 ...]\n");
}

if (import.meta.main) {
  const [cmd, ...rest] = Deno.args;
  switch (cmd) {
    case "serve": {
      let port = 8080;
      const pi = rest.indexOf("--port");
      if (pi >= 0 && rest[pi + 1]) {
        const n = Number(rest[pi + 1]);
        if (Number.isFinite(n)) port = n;
      }
      const peers = rest.filter((v) => v.startsWith("ws://") || v.startsWith("wss://"));

      const db = new GunDB();
      await db.ready();
      await db.serve({ port });
      for (const p of peers) db.connect(p);

      console.log(`Rusty Gun node serving on ws://localhost:${port}`);
      if (peers.length) console.log("Connected to peers:", peers.join(", "));

      // Keep process alive
      await new Promise(() => {});
      break;
    }
    default:
      printUsage();
  }
}
