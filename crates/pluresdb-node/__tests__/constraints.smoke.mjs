// TASK-PX-CANON Stage 2 / ADR-0017 Stage B6 — praxis constraints unified onto
// the CrdtStore single source of truth. Channel-agnostic NAPI smoke test
// against the BUILT native addon (C-TEST-001/C-VERIFY-001, C-TEST-002).
//
// Proves the Stage-2 mandate:
//   * a constraint authored via the Node binding PERSISTS as a queryable
//     CrdtStore node (listByType / get) — NOT a side in-memory store;
//   * that persistence is REAL: it survives closing and reopening a sled-backed
//     DB at the same path;
//   * the SAME store is what evaluate/onAction read: a numeric `amount <= 100`
//     rule BLOCKS amount=500 and PASSES amount=50;
//   * Violation marshals Rust -> serde_json -> JS; onAction throws on block.
//
// Run AFTER `napi build --release`:  node __tests__/constraints.smoke.mjs

import { createRequire } from 'node:module';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const require = createRequire(import.meta.url);
const { PluresDatabase } = require('../index.js');
const INDEX_JS = fileURLToPath(new URL('../index.js', import.meta.url));

let failures = 0;
function check(name, cond, detail) {
  if (cond) {
    console.log(`  \u2713 ${name}`);
  } else {
    failures += 1;
    console.error(`  \u2717 ${name}${detail ? ' \u2014 ' + detail : ''}`);
  }
}

const PRAXIS_TYPE = 'praxis_constraint';

function main() {
  console.log('=== Stage 2: Praxis constraints on CrdtStore (single SoT) ===\n');

  const dbDir = mkdtempSync(join(tmpdir(), 'px-stage2-'));
  const dbPath = join(dbDir, 'store');

  try {
    // ===================================================================
    // PART A — REAL PERSISTENCE: author a numeric constraint in a SEPARATE
    // process, then prove it is a queryable CrdtStore node that SURVIVED the
    // authoring process exiting (true restart-durability, not in-memory).
    // ===================================================================
    console.log('A. (child process) pxLoadPxSource persists `amount <= 100`, then exits');
    {
      // Authoring runs in a child node process. When it exits, sled flushes and
      // releases its file lock — so the parent reopening the same path proves
      // the constraint is durably on disk, not living in a shared in-memory store.
      const authorScript = `
        const { createRequire } = require('node:module');
        const req = createRequire(${JSON.stringify(import.meta.url)});
        const { PluresDatabase } = req(${JSON.stringify(INDEX_JS)});
        const db = new PluresDatabase('px-stage2-author', ${JSON.stringify(dbPath)});
        const out = db.pxLoadPxSource('constraint amount_cap:\\n  require: amount <= 100\\n  severity: error\\n');
        if (!out.constraints.includes('amount_cap')) { console.error('author: missing id'); process.exit(2); }
        console.log('child-authored:' + JSON.stringify(out.constraints));
      `;
      const childOut = execFileSync(process.execPath, ['-e', authorScript], { encoding: 'utf8' });
      check('child process authored amount_cap and exited cleanly',
        childOut.includes('child-authored:') && childOut.includes('amount_cap'),
        `child stdout: ${childOut.trim()}`);
    }

    console.log('');
    console.log('B. Parent reopens the SAME db_path — constraint persisted to disk');
    let reopened;
    {
      const db2 = new PluresDatabase('px-stage2-actor', dbPath);
      reopened = db2.get('amount_cap');
      check('amount_cap present after a SEPARATE authoring process exited',
        reopened && reopened.constraint && reopened.constraint.id === 'amount_cap',
        `got ${JSON.stringify(reopened)}`);

      // PROVE it is a real, queryable PluresDB node (not in-memory only):
      // it must appear in listByType(PRAXIS_TYPE) AND get(id).
      const nodes = db2.listByType(PRAXIS_TYPE);
      const found = nodes.find((n) => n.id === 'amount_cap');
      check('constraint is queryable via listByType("praxis_constraint")',
        !!found, `got ${nodes.length} praxis_constraint nodes`);
      check('the node carries the structured Constraint payload',
        found && found.data && found.data.constraint && found.data.constraint.id === 'amount_cap'
          && found.data.type === PRAXIS_TYPE,
        `got ${JSON.stringify(found && found.data)}`);

      // The require AST must be a REAL enforcing condition (`<= 100` maps to
      // Not(FieldGt threshold:100)) — not a no-op Always.
      const requireAst = reopened && reopened.constraint && reopened.constraint.require;
      check('require compiled to a real enforcing Condition (not Always)',
        requireAst && requireAst.op && requireAst.op !== 'always',
        `got ${JSON.stringify(requireAst)}`);
      console.log(`    persisted require AST: ${JSON.stringify(requireAst)}`);

      // ===================================================================
      // PART C — ENFORCEMENT from that SAME persisted store.
      // ===================================================================
      console.log('');
      console.log('C. Enforcement reads the SAME CrdtStore: 500 blocks, 50 passes');

      const ctx = (amount) => ({
        action_type: 'transfer',
        target: 'account#1',
        session_type: 'main',
        metadata: { amount },
      });

      // amount = 500 -> require (amount <= 100) is false -> violation -> BLOCK.
      const vBlocked = db2.pxEvaluate(ctx(500));
      check('pxEvaluate(amount=500) returns a violation',
        Array.isArray(vBlocked) && vBlocked.some((v) => v.constraint && v.constraint.id === 'amount_cap'),
        `got ${JSON.stringify(vBlocked)}`);

      let threw = false; let thrownMsg = '';
      try { db2.pxOnAction(ctx(500)); } catch (err) {
        threw = true; thrownMsg = String(err && err.message ? err.message : err);
      }
      check('pxOnAction(amount=500) THROWS (ActionBlocked)', threw, 'expected a throw');
      if (threw) console.log(`    thrown: ${thrownMsg.split('\n')[0]}`);

      // amount = 50 -> require holds -> no violation -> PASS.
      const vPass = db2.pxEvaluate(ctx(50));
      check('pxEvaluate(amount=50) returns NO amount_cap violation',
        Array.isArray(vPass) && !vPass.some((v) => v.constraint && v.constraint.id === 'amount_cap'),
        `got ${JSON.stringify(vPass)}`);

      let passThrew = false;
      let passResult;
      try { passResult = db2.pxOnAction(ctx(50)); } catch (_e) { passThrew = true; }
      check('pxOnAction(amount=50) does NOT throw', !passThrew);
      check('pxOnAction(amount=50) returns { violations: [...] }',
        passResult && Array.isArray(passResult.violations),
        `got ${JSON.stringify(passResult)}`);

      // ===================================================================
      // PART D — Violation marshaling round-trip + structured insert + seeded.
      // ===================================================================
      console.log('');
      console.log('D. Violation round-trip, pxInsertConstraint, seeded constraints');

      const v = vBlocked.find((x) => x.constraint && x.constraint.id === 'amount_cap');
      check('violation.message is a string', v && typeof v.message === 'string');
      check('violation.constraint.severity is a string',
        v && v.constraint && typeof v.constraint.severity === 'string',
        `got ${v && v.constraint && v.constraint.severity}`);

      // pxInsertConstraint via { id, text } compile path, then enforce it.
      const inserted = db2.pxInsertConstraint({ id: 'C-RISK', text: 'risk_score < 0.9' });
      check('pxInsertConstraint returns the persisted constraint',
        inserted && inserted.id === 'C-RISK', `got ${JSON.stringify(inserted)}`);
      const riskNode = db2.get('C-RISK');
      check('pxInsertConstraint persisted C-RISK to the CrdtStore',
        riskNode && riskNode.constraint && riskNode.constraint.id === 'C-RISK',
        `got ${JSON.stringify(riskNode)}`);
      const riskViol = db2.pxEvaluate({
        action_type: 'deploy', target: 'prod', session_type: 'main',
        metadata: { risk_score: 0.95 },
      });
      check('C-RISK enforces (risk_score=0.95 violates)',
        Array.isArray(riskViol) && riskViol.some((x) => x.constraint && x.constraint.id === 'C-RISK'),
        `got ${JSON.stringify(riskViol)}`);

      // Seeded built-ins must be present as real CrdtStore nodes too.
      const allConstraintNodes = db2.listByType(PRAXIS_TYPE);
      const ids = allConstraintNodes.map((n) => n.id);
      check('seeded constraints (e.g. C-0002) present as CrdtStore nodes',
        ids.includes('C-0002'), `got ids ${JSON.stringify(ids)}`);
      console.log(`    total praxis_constraint nodes in store: ${ids.length}`);
    }

    console.log('');
    if (failures > 0) {
      console.error(`=== ${failures} check(s) FAILED ===`);
      process.exit(1);
    }
    console.log('=== All Stage-2 CrdtStore-unification checks passed! ===');
  } finally {
    try { rmSync(dbDir, { recursive: true, force: true }); } catch (_e) { /* best effort */ }
  }
}

main();
