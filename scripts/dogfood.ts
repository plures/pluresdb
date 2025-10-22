#!/usr/bin/env -S deno run -A --unstable-kv

import { GunDB } from "../legacy/core/database.ts";
import { startApiServer } from "../legacy/http/api-server.ts";

declare const Deno: any;

interface StepResult {
  name: string;
  ok: boolean;
  details?: string;
}

async function main() {
  const results: StepResult[] = [];
  const kvDir = await Deno.makeTempDir({ prefix: "pluresdb-dogfood-" });
  const kvPath = `${kvDir}/kv`;
  const wsPort = 4600 + Math.floor(Math.random() * 5000);
  const apiPort = wsPort + 1;
  const apiUrl = `http://localhost:${apiPort}`;

  const db = new GunDB();
  await db.ready(kvPath);
  db.serve({ port: wsPort });
  const api = startApiServer({ port: apiPort, db });

  const cleanupTasks: Array<() => Promise<void> | void> = [
    async () => {
      try {
        api.close();
      } catch (_) {
        /* ignore */
      }
    },
    async () => {
      try {
        await db.close();
      } catch (_) {
        /* ignore */
      }
    },
    async () => {
      try {
        await Deno.remove(kvDir, { recursive: true });
      } catch (_) {
        /* ignore */
      }
    },
  ];

  const finalize = async () => {
    for (const task of cleanupTasks) {
      await Promise.resolve(task());
    }
  };

  const assert = (cond: boolean, message: string) => {
    if (!cond) {
      throw new Error(message);
    }
  };

  const record = (name: string, ok: boolean, details?: string) => {
    const entry: StepResult = { name, ok, details };
    results.push(entry);
    const icon = ok ? "✅" : "❌";
    console.log(`${icon} ${name}${details ? ` — ${details}` : ""}`);
  };

  try {
    console.log("🚀 Starting PluresDB dogfooding run");
    console.log(`🗂️  KV path: ${kvPath}`);
    console.log(`🔌 Mesh port: ${wsPort}`);
    console.log(`🌐 API port: ${apiPort}`);

    await waitFor(
      async () => {
        try {
          const res = await fetch(`${apiUrl}/api/list`);
          return res.ok;
        } catch (_) {
          return false;
        }
      },
      { timeoutMs: 10_000, intervalMs: 250 },
    );
    record("Server readiness", true);

    const nodeId = "dogfood:api";
    const initialPayload = {
      type: "DogfoodTest",
      text: "Hello from the dogfooding script",
      vector: Array.from({ length: 4 }, (_, i) => i + 1),
    };
    const putRes = await fetch(`${apiUrl}/api/put`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ id: nodeId, data: initialPayload }),
    });
    assert(putRes.ok, `API put failed (${putRes.status})`);
    record("API put", true);

    const getRes = await fetch(`${apiUrl}/api/get?id=${encodeURIComponent(nodeId)}`);
    assert(getRes.ok, "API get failed");
    const getJson = await getRes.json();
    assert(getJson.text === initialPayload.text, "Unexpected API get payload");
    record("API get", true);

    const listRes = await fetch(`${apiUrl}/api/list`);
    assert(listRes.ok, "API list failed");
    const listJson = await listRes.json();
    assert(
      Array.isArray(listJson) && listJson.some((n: any) => n.id === nodeId),
      "Node missing from API list",
    );
    record("API list", true);

    const searchRes = await fetch(
      `${apiUrl}/api/search?q=${encodeURIComponent("dogfood script test")}&k=5`,
    );
    assert(searchRes.ok, "API search failed");
    const searchJson = await searchRes.json();
    assert(Array.isArray(searchJson) && searchJson.length > 0, "Vector search returned no results");
    record("API vector search", true);

    const updatePayload = {
      type: "DogfoodTest",
      text: "Hello from the updated dogfooding script",
    };
    const putUpdate = await fetch(`${apiUrl}/api/put`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ id: nodeId, data: updatePayload }),
    });
    assert(putUpdate.ok, "API put update failed");
    record("API update", true);

    const historyRes = await fetch(`${apiUrl}/api/history?id=${encodeURIComponent(nodeId)}`);
    assert(historyRes.ok, "API history failed");
    const historyJson = await historyRes.json();
    assert(Array.isArray(historyJson) && historyJson.length >= 2, "API history missing versions");
    record("API history", true, `${historyJson.length} versions`);

    const restoreTimestamp =
      historyJson.at(-1)?.timestamp ?? historyJson[historyJson.length - 1]?.timestamp;
    assert(typeof restoreTimestamp === "number", "Failed to locate restore timestamp");
    const restoreRes = await fetch(
      `${apiUrl}/api/restore?id=${encodeURIComponent(nodeId)}&timestamp=${restoreTimestamp}`,
    );
    assert(restoreRes.ok, "API restore failed");
    record("API restore", true);

    const restored = await (
      await fetch(`${apiUrl}/api/get?id=${encodeURIComponent(nodeId)}`)
    ).json();
    assert(restored.text === initialPayload.text, "Restore did not revert payload");
    record("API post-restore verification", true);

    const instancesRes = await fetch(
      `${apiUrl}/api/instances?type=${encodeURIComponent("DogfoodTest")}`,
    );
    assert(instancesRes.ok, "API instances failed");
    const instancesJson = await instancesRes.json();
    assert(
      Array.isArray(instancesJson) && instancesJson.some((n: any) => n.id === nodeId),
      "Instances endpoint missing node",
    );
    record("API type instances", true);

    const webRes = await fetch(`${apiUrl}/`);
    assert(webRes.ok, "Web UI endpoint failed");
    const webHtml = await webRes.text();
    record("Web UI fetch", true, `${webHtml.length} chars`);

    const cliId = "dogfood:cli";
    const cliPut = await runCli([
      "put",
      cliId,
      JSON.stringify({ type: "DogfoodTest", text: "CLI write" }),
      "--kv",
      kvPath,
    ]);
    assert(cliPut.code === 0, `CLI put failed: ${cliPut.stderr}`);
    record("CLI put", true);

    const cliGet = await runCli(["get", cliId, "--kv", kvPath]);
    assert(cliGet.code === 0, `CLI get failed: ${cliGet.stderr}`);
    const cliGetJson = JSON.parse(cliGet.stdout.trim() || "null");
    assert(cliGetJson?.text === "CLI write", "CLI get returned wrong payload");
    record("CLI get", true);

    const cliList = await runCli(["list", "--kv", kvPath]);
    assert(cliList.code === 0, `CLI list failed: ${cliList.stderr}`);
    const cliListJson = JSON.parse(cliList.stdout.trim() || "[]");
    assert(Array.isArray(cliListJson) && cliListJson.length >= 2, "CLI list missing entries");
    record("CLI list", true);

    const cliSearch = await runCli(["vsearch", "dogfooding", "5", "--kv", kvPath]);
    assert(cliSearch.code === 0, `CLI vsearch failed: ${cliSearch.stderr}`);
    const cliSearchJson = JSON.parse(cliSearch.stdout.trim() || "[]");
    assert(
      Array.isArray(cliSearchJson) && cliSearchJson.length > 0,
      "CLI vsearch returned no results",
    );
    record("CLI vector search", true);

    console.log("\n🎉 Dogfooding run succeeded!");
  } catch (error) {
    record("Dogfooding run", false, error instanceof Error ? error.message : String(error));
    await finalize();
    console.log("\n❌ Dogfooding run failed");
    console.log(error);
    Deno.exit(1);
  }

  await finalize();
}

async function runCli(args: string[]) {
  const command = new Deno.Command("deno", {
    args: ["run", "-A", "--unstable-kv", "--no-lock", "legacy/main.ts", ...args],
    stdout: "piped",
    stderr: "piped",
  });
  const { code, stdout, stderr } = await command.output();
  return {
    code,
    stdout: new TextDecoder().decode(stdout),
    stderr: new TextDecoder().decode(stderr),
  };
}

async function waitFor(
  fn: () => Promise<boolean>,
  opts: { timeoutMs: number; intervalMs: number },
) {
  const deadline = Date.now() + opts.timeoutMs;
  while (Date.now() < deadline) {
    if (await fn()) return;
    await sleep(opts.intervalMs);
  }
  throw new Error("Timed out waiting for condition");
}

function sleep(ms: number): Promise<void>;
function sleep<T>(ms: number, value: T): Promise<T>;
function sleep(ms: number, value?: unknown) {
  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      clearTimeout(timer);
      resolve(value);
    }, ms);
  });
}

if ((import.meta as any).main) {
  await main();
}
