// S3 smoke test — reactive .px evaluation on write (EPIC-MEMORY Effort 3).
//
// Proves the native `subscribePx(cb)` path REALLY evaluates the persisted
// praxis policy reactively off each write (no polling), delivering the actual
// violated constraints for a write that trips a rule, staying SILENT for a
// clean write, and going silent after `unsubscribe`.
//
// This is POST-write / observe-and-report by design (the value is already
// persisted by the time the change broadcast fires) — see the Rust doc-comment
// on `subscribe_px`. So we assert on the reactively-DELIVERED violations, not on
// any (non-existent) block of the write.
//
// Exit 0 on PASS, non-zero on FAIL.

import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const native = require('./index.js');
const { PluresDatabase } = native;

const delay = (ms) => new Promise((r) => setTimeout(r, ms));
// Window we allow for an async TSFN delivery from the Rust subscription thread
// to land on the Node event loop. Generous but bounded; this is waiting for a
// PUSH, not polling the store.
const DELIVERY_MS = 400;

let failures = 0;
const check = (name, cond, detail = '') => {
  if (cond) {
    console.log(`  PASS  ${name}${detail ? ' — ' + detail : ''}`);
  } else {
    failures += 1;
    console.error(`  FAIL  ${name}${detail ? ' — ' + detail : ''}`);
  }
};

async function main() {
  console.log('S3 reactive .px on write — smoke test');

  const db = new PluresDatabase('s3-test-actor');

  // 1) Persist a constraint that WILL fire on a specific write. Compiles via the
  //    canonical grammar to `require: !(amount > 100)` (i.e. metadata.amount must
  //    be <= 100), which is violated exactly when metadata.amount > 100.
  const compiled = db.pxCompileNl('metadata.amount <= 100', 'C-AMOUNT-CAP');
  check(
    'constraint compiled + persisted with a real enforcing require',
    !!compiled &&
      compiled.id === 'C-AMOUNT-CAP' &&
      !!compiled.require &&
      compiled.require.op === 'not' &&
      compiled.require.condition &&
      compiled.require.condition.op === 'field_gt' &&
      compiled.require.condition.field === 'amount' &&
      Number(compiled.require.condition.threshold) === 100,
    `require=${JSON.stringify(compiled?.require)}`,
  );

  // Sanity: the same policy, evaluated synchronously, must fire for amount=500
  // and be clean for amount=50 — establishes ground truth independent of the
  // reactive path.
  const syncBad = db.pxEvaluate({
    action_type: 'upsert',
    target: 'sanity',
    session_type: 'main',
    metadata: { amount: 500 },
  });
  const syncGood = db.pxEvaluate({
    action_type: 'upsert',
    target: 'sanity',
    session_type: 'main',
    metadata: { amount: 50 },
  });
  check('sync pxEvaluate ground truth: amount=500 violates', Array.isArray(syncBad) && syncBad.length >= 1);
  check('sync pxEvaluate ground truth: amount=50 clean', Array.isArray(syncGood) && syncGood.length === 0);

  // 2) Subscribe to reactive px. Collect every delivered violation event.
  const events = [];
  const subId = db.subscribePx((evt) => {
    // evt = { kind, id, violationCount, violationsJson }
    let violations = [];
    try {
      violations = JSON.parse(evt.violationsJson);
    } catch {
      violations = null; // signal a marshaling problem loudly
    }
    events.push({ ...evt, violations });
  });
  check('subscribePx returned a numeric subscription id', typeof subId === 'number', `id=${subId}`);

  // 3) Violating write — must be reactively evaluated and its violation DELIVERED
  //    without any polling on our side.
  const beforeBad = events.length;
  db.put('n-bad', { amount: 500, note: 'over the cap' });
  await delay(DELIVERY_MS);
  const badEvents = events.slice(beforeBad).filter((e) => e.id === 'n-bad');
  const badEvt = badEvents[0];
  check(
    'violating write delivered a reactive px violation (no polling)',
    badEvent(badEvt),
    badEvt
      ? `kind=${badEvt.kind} count=${badEvt.violationCount} constraint=${badEvt.violations?.[0]?.constraint?.id}`
      : 'no event delivered',
  );
  check(
    'delivered violation names the real constraint C-AMOUNT-CAP',
    !!badEvt && Array.isArray(badEvt.violations) && badEvt.violations.some((v) => v?.constraint?.id === 'C-AMOUNT-CAP'),
  );

  // 4) Clean write — must NOT deliver a violation.
  const beforeGood = events.length;
  db.put('n-good', { amount: 50, note: 'under the cap' });
  await delay(DELIVERY_MS);
  const goodEvents = events.slice(beforeGood).filter((e) => e.id === 'n-good');
  check(
    'clean write is silent (no violation delivered)',
    goodEvents.length === 0,
    `unexpected events=${JSON.stringify(goodEvents)}`,
  );

  // 5) Unsubscribe, then violate again — must be silent.
  db.unsubscribe(subId);
  await delay(50); // let the cancel flag settle
  const beforeSilent = events.length;
  db.put('n-bad2', { amount: 999, note: 'way over, but after unsubscribe' });
  await delay(DELIVERY_MS);
  const afterSilent = events.slice(beforeSilent);
  check(
    'after unsubscribe, a violating write delivers nothing',
    afterSilent.length === 0,
    `leaked events=${JSON.stringify(afterSilent)}`,
  );

  // Summary
  if (failures === 0) {
    console.log('\nS3_SMOKE: PASS — reactive .px fires on violating write, silent on clean write, silent after unsubscribe.');
    process.exit(0);
  } else {
    console.error(`\nS3_SMOKE: FAIL — ${failures} check(s) failed.`);
    process.exit(1);
  }
}

// small helper so the assertion reads cleanly
function badEvent(e) {
  return !!e && e.kind === 'upsert' && e.violationCount >= 1 && Array.isArray(e.violations) && e.violations.length >= 1;
}

main().catch((err) => {
  console.error('S3_SMOKE: FAIL — unexpected error:', err);
  process.exit(2);
});
