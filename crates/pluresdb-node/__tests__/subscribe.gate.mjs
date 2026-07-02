// S2 GATE: reactive native subscribe() — push delivery, no polling.
// Run from the crate dir AFTER build: node __tests__/subscribe.gate.mjs
// Asserts:
//   1. subscribe(cb) returns a numeric id.
//   2. put() -> cb fires with { kind:'upsert', id } WITHOUT any polling
//      (we await a Promise the callback resolves; no setInterval/loop).
//   3. delete() -> cb fires with { kind:'delete', id }.
//   4. unsubscribe(id) -> subsequent put() does NOT fire the callback.
//   5. No leaked behavior: a second subscription works independently.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { PluresDatabase } = require("../index.js");

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let failures = 0;
function check(name, cond) {
  console.log(`  ${cond ? "PASS" : "FAIL"}  ${name}`);
  if (!cond) failures++;
}

async function main() {
  const db = new PluresDatabase("s2-gate-actor");

  // --- 2/3: events are pushed to the callback -----------------------------
  const received = [];
  let resolveNext = null;
  const nextEvent = () => new Promise((res) => { resolveNext = res; });

  const subId = db.subscribe((ev) => {
    // napi ThreadsafeFunction (non-fatal) delivers a single SyncEventJs arg.
    received.push(ev);
    if (resolveNext) { const r = resolveNext; resolveNext = null; r(ev); }
  });
  check("subscribe returns numeric id", typeof subId === "number");

  const p1 = nextEvent();
  db.put("s2-a", { name: "Alice" });
  const ev1 = await Promise.race([p1, sleep(3000).then(() => null)]);
  check("put pushed an event (no polling)", ev1 != null);
  check("event kind == upsert", ev1 && ev1.kind === "upsert");
  check("event id present", ev1 && typeof ev1.id === "string" && ev1.id.length > 0);

  const p2 = nextEvent();
  db.delete("s2-a");
  const ev2 = await Promise.race([p2, sleep(3000).then(() => null)]);
  check("delete pushed an event", ev2 != null);
  check("delete kind == delete", ev2 && ev2.kind === "delete");

  // --- 4: unsubscribe stops delivery --------------------------------------
  const countBefore = received.length;
  db.unsubscribe(subId);
  await sleep(100); // let the cancel flag settle
  db.put("s2-b", { name: "Bob" });
  await sleep(500); // give any (wrongly) live thread a chance to fire
  check("no callback after unsubscribe", received.length === countBefore);

  // --- 5: a fresh subscription works independently ------------------------
  let resolveNext2 = null;
  const nextEvent2 = () => new Promise((res) => { resolveNext2 = res; });
  const sub2 = db.subscribe((ev) => {
    if (resolveNext2) { const r = resolveNext2; resolveNext2 = null; r(ev); }
  });
  const p3 = nextEvent2();
  db.put("s2-c", { name: "Carol" });
  const ev3 = await Promise.race([p3, sleep(3000).then(() => null)]);
  check("second subscription receives events", ev3 != null && ev3.kind === "upsert");
  db.unsubscribe(sub2);

  console.log(`\nS2_GATE: ${failures === 0 ? "PASS" : "FAIL"} (${failures} failures)`);
  process.exit(failures === 0 ? 0 : 1);
}

main().catch((e) => {
  console.log("S2_GATE_ERROR:", e && e.message ? e.message.split("\n")[0] : String(e));
  console.log("S2_GATE: FAIL");
  process.exit(1);
});
