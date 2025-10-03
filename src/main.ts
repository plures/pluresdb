import { GunDB } from "./core/database.ts";
import { debugLog } from "./util/debug.ts";
import { startApiServer } from "./http/api-server.ts";
import { loadConfig, saveConfig } from "./config.ts";

function printUsage() {
  console.log(
    [
      "Usage:",
      "  deno run -A src/main.ts serve [--port <port>] [--kv <path>] [ws://peer ...]",
      "  deno run -A src/main.ts put <id> <json> [--kv <path>]",
      "  deno run -A src/main.ts get <id> [--kv <path>]",
      "  deno run -A src/main.ts delete <id> [--kv <path>]",
      "  deno run -A src/main.ts vsearch <query> <k> [--kv <path>]",
      "  deno run -A src/main.ts type <id> <TypeName> [--kv <path>]",
      "  deno run -A src/main.ts instances <TypeName> [--kv <path>]",
      "  deno run -A src/main.ts list [--kv <path>]",
      "",
    ].join("\n"),
  );
}

if (import.meta.main) {
  const [cmd, ...rest] = Deno.args;
  switch (cmd) {
    case "serve": {
      const cfg = await loadConfig();
      let port = cfg.port ?? 8080;
      let kvPath: string | undefined = cfg.kvPath;
      const pi = rest.indexOf("--port");
      if (pi >= 0 && rest[pi + 1]) {
        const n = Number(rest[pi + 1]);
        if (Number.isFinite(n)) port = n;
      }
      const ki = rest.indexOf("--kv");
      if (ki >= 0 && rest[ki + 1]) kvPath = rest[ki + 1];
      const peers = (cfg.peers ?? []).concat(
        rest.filter((v) => v.startsWith("ws://") || v.startsWith("wss://")),
      );

      const db = new GunDB();
      await db.ready(kvPath);
      db.serve({ port });
      for (const p of peers) db.connect(p);

      const api = startApiServer({ port: port + (cfg.apiPortOffset ?? 1), db });
      console.log(`PluresDB node serving on ws://localhost:${port}`);
      console.log(`HTTP API/UI on ${api.url}`);
      if (peers.length) console.log("Connected to peers:", peers.join(", "));

      // Keep process alive
      await new Promise(() => {});
      break;
    }
    case "put": {
      const [id, json, ...flags] = rest;
      if (!id || !json) {
        printUsage();
        Deno.exit(1);
      }
      let kvPath: string | undefined;
      const ki = flags.indexOf("--kv");
      if (ki >= 0 && flags[ki + 1]) kvPath = flags[ki + 1];
      const db = new GunDB();
      await db.ready(kvPath);
      const obj = JSON.parse(json);
      await db.put(id, obj);
      console.log("ok");
      await db.close();
      break;
    }
    case "get": {
      const [id, ...flags] = rest;
      if (!id) {
        printUsage();
        Deno.exit(1);
      }
      let kvPath: string | undefined;
      const ki = flags.indexOf("--kv");
      if (ki >= 0 && flags[ki + 1]) kvPath = flags[ki + 1];
      const db = new GunDB();
      await db.ready(kvPath);
      const val = await db.get<Record<string, unknown>>(id);
      console.log(JSON.stringify(val));
      await db.close();
      break;
    }
    case "delete": {
      const [id, ...flags] = rest;
      if (!id) {
        printUsage();
        Deno.exit(1);
      }
      let kvPath: string | undefined;
      const ki = flags.indexOf("--kv");
      if (ki >= 0 && flags[ki + 1]) kvPath = flags[ki + 1];
      const db = new GunDB();
      await db.ready(kvPath);
      await db.delete(id);
      console.log("ok");
      await db.close();
      break;
    }
    case "vsearch": {
      const [query, kRaw, ...flags] = rest;
      if (!query || !kRaw) {
        printUsage();
        Deno.exit(1);
      }
      const k = Number(kRaw);
      if (!Number.isFinite(k)) {
        printUsage();
        Deno.exit(1);
      }
      let kvPath: string | undefined;
      const ki = flags.indexOf("--kv");
      if (ki >= 0 && flags[ki + 1]) kvPath = flags[ki + 1];
      const db = new GunDB();
      await db.ready(kvPath);
      const results = await db.vectorSearch(query, k);
      console.log(JSON.stringify(results.map((n) => ({ id: n.id, data: n.data }))));
      await db.close();
      break;
    }
    case "type": {
      const [id, typeName, ...flags] = rest;
      if (!id || !typeName) {
        printUsage();
        Deno.exit(1);
      }
      let kvPath: string | undefined;
      const ki = flags.indexOf("--kv");
      if (ki >= 0 && flags[ki + 1]) kvPath = flags[ki + 1];
      const db = new GunDB();
      await db.ready(kvPath);
      await db.setType(id, typeName);
      console.log("ok");
      await db.close();
      break;
    }
    case "instances": {
      const [typeName, ...flags] = rest;
      if (!typeName) {
        printUsage();
        Deno.exit(1);
      }
      let kvPath: string | undefined;
      const ki = flags.indexOf("--kv");
      if (ki >= 0 && flags[ki + 1]) kvPath = flags[ki + 1];
      const db = new GunDB();
      await db.ready(kvPath);
      const rows = await db.instancesOf(typeName);
      console.log(JSON.stringify(rows.map((n) => ({ id: n.id, data: n.data }))));
      await db.close();
      break;
    }
    case "list": {
      const flags = rest;
      let kvPath: string | undefined;
      const ki = flags.indexOf("--kv");
      if (ki >= 0 && flags[ki + 1]) kvPath = flags[ki + 1];
      const db = new GunDB();
      await db.ready(kvPath);
      const nodes = await db.getAll();
      const out = nodes.map((node) => ({ id: node.id, data: node.data as Record<string, unknown> }));
      console.log(JSON.stringify(out));
      await db.close();
      break;
    }
    case "config": {
      const cfg = await loadConfig();
      console.log(JSON.stringify(cfg, null, 2));
      break;
    }
    case "config:set": {
      const [key, value] = rest;
      if (!key || value === undefined) {
        printUsage();
        Deno.exit(1);
      }
      const cfg = await loadConfig();
      (cfg as any)[key] = /^[0-9]+$/.test(value) ? Number(value) : value;
      await saveConfig(cfg);
      console.log("ok");
      break;
    }
    default:
      printUsage();
  }
}
