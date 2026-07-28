// PX-TIMER GATE: PxTimerDispatcher Node/embedded FFI wiring (ADR-0017).
// Run from the crate dir AFTER build: node __tests__/px-timer.gate.mjs
// Asserts the real napi binding, not just Rust unit tests:
//   1. px_timer_tick() before px_timer_configure() rejects (loud failure,
//      not a silent no-op) per ADR-0017 explicit-config-required design.
//   2. A real timer registered via agens_timer_schedule (CRDT-backed
//      TimerTable, interval_secs=1) becomes due, and pxTimerTick() actually
//      dispatches it through the compiled px "steps" procedure, invoking
//      StoreActionHandler's crdt.put against the real embedded CrdtStore
//      (verified via a follow-up db.get() read) -- proving the whole
//      timer -> executor -> ActionHandler -> store chain is wired end to
//      end, not stubbed.
//   3. TickReport JSON shape includes a fired count > 0.
//   4. px_timer_recover() runs and returns a RecoveryReport JSON shape.
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
  const db = new PluresDatabase("px-timer-gate-actor");

  // --- 1: tick before configure must reject loudly, not silently no-op ---
  let rejectedBeforeConfigure = false;
  try {
    db.pxTimerTick();
  } catch (e) {
    rejectedBeforeConfigure = true;
  }
  check("px_timer_tick before configure rejects loudly", rejectedBeforeConfigure);

  // --- 2: register a REAL timer via the CRDT-backed TimerTable, let it ---
  //        become due, configure a real "steps" procedure that calls
  //        crdt.put, tick, and verify the action landed in the store. -----
  const targetNodeId = "px-timer-gate-node";
  const timerId = db.agensTimerSchedule("gate-timer", 1, { seeded: true });
  check("agensTimerSchedule returns a timer id", typeof timerId === "string" && timerId.length > 0);

  // interval_secs=1: wait past that so due_timers() picks it up on tick.
  await sleep(1200);

  const procedure = {
    type: "procedure",
    name: "gate-procedure",
    steps: [
      {
        kind: "call",
        name: "crdt.put",
        params: { id: targetNodeId, data: { hit: true, from: "px-timer-gate" } },
      },
    ],
  };
  db.pxTimerConfigure(procedure);

  const tick1 = db.pxTimerTick();
  check("tick returns an object", tick1 != null && typeof tick1 === "object");
  check("tick report shows at least one fired timer", (tick1.fired ?? 0) > 0);
  check("tick report shows no errors", Array.isArray(tick1.errors) && tick1.errors.length === 0);

  // Confirm the action was really applied against the embedded store, not
  // just reported as dispatched. Read it back via the standard db.get().
  let applied = null;
  try {
    applied = db.get(targetNodeId);
  } catch (e) {
    applied = null;
  }
  check(
    "crdt.put action from the fired timer actually landed in the store",
    applied != null && applied.hit === true,
  );

  // --- 3: recover() runs against the same configured procedure ------------
  let recoverReport = null;
  let recoverThrew = false;
  try {
    recoverReport = db.pxTimerRecover(60);
  } catch (e) {
    recoverThrew = true;
  }
  check("px_timer_recover does not throw once configured", !recoverThrew);
  check("recover returns an object", recoverReport != null && typeof recoverReport === "object");

  console.log(`\nPX_TIMER_GATE: ${failures === 0 ? "PASS" : "FAIL"} (${failures} failures)`);
  process.exit(failures === 0 ? 0 : 1);
}

main().catch((e) => {
  console.log("PX_TIMER_GATE_ERROR:", e && e.message ? e.message.split("\n")[0] : String(e));
  console.log("PX_TIMER_GATE: FAIL");
  process.exit(1);
});
