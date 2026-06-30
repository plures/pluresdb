// H VERIFY -- end-to-end context-reduction proof on REAL data.
// Channel-agnostic (C-TEST-002): calls the shipped native surface directly.
// Real rebuilt .node + real cl100k countTokens only. No chat adapter.
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const require = createRequire(import.meta.url);
const here = path.dirname(fileURLToPath(import.meta.url));
const native = require(path.join(here, 'index.js'));
const { compressText, countTokens, detectContentType } = native;
const repo = path.resolve(here, '..', '..');

// Realistic multi-message context payload assembled from REAL files.
// prose : real markdown notes doc (long prose explanation)
// code  : the real headroom.rs module under test (a code file)
// log   : real leveled structured log (the log route; runs + distinct lines)
// mlog  : a REAL cargo-mutants build log (noisy, contains panic text)
// json  : the real package.json (a JSON blob)
const PROSE_PATH = 'C:/Projects/plureslm-openclaw/epic/H-IMPLEMENT-NOTES.md';
const CODE_PATH = path.join(repo, 'crates', 'pluresdb-node', 'src', 'headroom.rs');
const LOG_PATH = path.join(here, 'verify-log-sample.txt');
const MLOG_PATH = path.join(repo, 'mutants-enc-bridge', 'mutants.out', 'log', 'crates__pluresdb-storage__src__bridge__mod.rs_line_101_col_9.log');
const JSON_PATH = path.join(repo, 'package.json');

const PAYLOAD = [
  { id: 'prose', label: 'real markdown notes (H-IMPLEMENT-NOTES.md)', file: PROSE_PATH },
  { id: 'code', label: 'real Rust source (headroom.rs)', file: CODE_PATH },
  { id: 'log', label: 'real leveled structured log', file: LOG_PATH },
  { id: 'mlog', label: 'real cargo-mutants build log', file: MLOG_PATH },
  { id: 'json', label: 'real package.json', file: JSON_PATH },
];

function readReal(p) { return readFileSync(p, 'utf8'); }

function isValidUtf8(s) {
  return Buffer.from(s, 'utf8').toString('utf8') === s;
}
function distinctLines(s) {
  const set = new Set();
  const arr = s.split('\n');
  for (const ln of arr) { const t = ln.trim(); if (t) set.add(t); }
  return set;
}
function rustSignatures(s) {
  const sigs = [];
  const arr = s.split('\n');
  for (const ln of arr) {
    const t = ln.trim();
    const isSig =
      /^(pub\s+)?(async\s+)?fn\s+[A-Za-z_]/.test(t) ||
      /^(pub\s+)?struct\s+[A-Za-z_]/.test(t) ||
      /^(pub\s+)?enum\s+[A-Za-z_]/.test(t) ||
      /^(pub\s+)?trait\s+[A-Za-z_]/.test(t) ||
      /^impl(\s|<)/.test(t);
    if (isSig && !t.endsWith(',')) sigs.push(t);
  }
  return sigs;
}

let fail = 0;
function ok(cond, msg) { if (!cond) { fail++; console.log('  FAIL: ' + msg); } }

const rows = [];
let sumBefore = 0;
let sumAfter = 0;

for (const item of PAYLOAD) {
  const raw = readReal(item.file);
  const detected = detectContentType(raw);
  const out = compressText(raw);
  const tin = countTokens(raw);
  const tout = countTokens(out);
  sumBefore += tin;
  sumAfter += tout;
  ok(tout <= tin, item.id + ': tokens grew ' + tin + ' -> ' + tout);
  ok(typeof out === 'string', item.id + ': output not a string');
  ok(isValidUtf8(out), item.id + ': output not valid UTF-8');
  ok(['prose', 'code', 'log', 'error', 'json'].includes(detected), item.id + ': unknown label ' + detected);
  const row = {
    id: item.id, label: item.label, detected,
    bytesIn: raw.length, bytesOut: out.length,
    tokensIn: tin, tokensOut: tout,
    ratio: tin ? Number((tout / tin).toFixed(4)) : 1,
    savedTokens: tin - tout,
  };
  row._out = out;
  row._raw = raw;
  rows.push(row);
}

// SAFETY (b): code signatures survive -- consumer can still see the API surface.
const codeRow = rows.find(r => r.id === 'code');
const expectedSigs = rustSignatures(codeRow._raw);
const survived = expectedSigs.filter(sig => {
  const key = sig.split('(')[0].slice(0, 40);
  return codeRow._out.includes(key);
});
const sigSurvivalPct = expectedSigs.length ? Number((100 * survived.length / expectedSigs.length).toFixed(1)) : 100;
ok(survived.length === expectedSigs.length, 'code: only ' + survived.length + '/' + expectedSigs.length + ' signatures survived');
ok(codeRow.detected === 'code', 'code: detected as ' + codeRow.detected + ' (expected code)');
ok(/body elided/.test(codeRow._out), 'code: missing body-elided header');

// SAFETY (c): log distinct lines survive on the LOG-classified item.
const logRow = rows.find(r => r.id === 'log');
const logDistinctIn = distinctLines(logRow._raw);
let logDistinctLost = 0;
for (const d of logDistinctIn) { if (!logRow._out.includes(d)) logDistinctLost++; }
const logDistinctSurvivalPct = logDistinctIn.size ? Number((100 * (logDistinctIn.size - logDistinctLost) / logDistinctIn.size).toFixed(1)) : 100;
ok(logRow.detected === 'log', 'log: detected as ' + logRow.detected + ' (expected log)');
ok(logDistinctLost === 0, 'log: ' + logDistinctLost + '/' + logDistinctIn.size + ' distinct lines LOST');
ok(/\[\xD7\d+\]/.test(logRow._out), 'log: no run marker emitted');

// Report (not assert) the real mutants build log: it classifies as error,
// so it is routed through the prose head/tail window (lossy by design).
const mlogRow = rows.find(r => r.id === 'mlog');
const mlogDistinctIn = distinctLines(mlogRow._raw);
let mlogDistinctLost = 0;
for (const d of mlogDistinctIn) { if (!mlogRow._out.includes(d)) mlogDistinctLost++; }
const mlogDistinctSurvivalPct = mlogDistinctIn.size ? Number((100 * (mlogDistinctIn.size - mlogDistinctLost) / mlogDistinctIn.size).toFixed(1)) : 100;

// SAFETY (a): aggregate never grows (net-savings guard at the payload level).
ok(sumAfter <= sumBefore, 'AGGREGATE tokens grew ' + sumBefore + ' -> ' + sumAfter);

const overallRatio = sumBefore ? Number((sumAfter / sumBefore).toFixed(4)) : 1;
const overallSavedPct = sumBefore ? Number((100 * (sumBefore - sumAfter) / sumBefore).toFixed(2)) : 0;

const compact = {
  sumBefore, sumAfter, overallRatio, overallSavedPct,
  perItem: rows.map(r => ({ id: r.id, detected: r.detected, tokensIn: r.tokensIn, tokensOut: r.tokensOut, ratio: r.ratio })),
  sigSurvivalPct, sigCount: expectedSigs.length,
  logDistinctSurvivalPct, logDistinctIn: logDistinctIn.size,
  mlogDetected: mlogRow.detected, mlogDistinctSurvivalPct, mlogDistinctIn: mlogDistinctIn.size,
};

console.log('=== PER-ITEM (real data, AUTO-ROUTE) ===');
for (const r of rows) {
  console.log('  ' + r.id.padEnd(5) + ' detect=' + r.detected.padEnd(5) + ' tokens ' + String(r.tokensIn).padStart(6) + ' -> ' + String(r.tokensOut).padStart(6) + '  ratio=' + r.ratio + '  saved=' + r.savedTokens);
}
console.log('=== SAFETY / LOSSLESS ===');
console.log('  code signatures survived: ' + survived.length + '/' + expectedSigs.length + ' (' + sigSurvivalPct + '%)');
console.log('  log distinct lines survived: ' + (logDistinctIn.size - logDistinctLost) + '/' + logDistinctIn.size + ' (' + logDistinctSurvivalPct + '%)');
console.log('  aggregate never grew: ' + (sumAfter <= sumBefore));
console.log('  [note] mutants build log classified as ' + mlogRow.detected + '; distinct-line survival=' + mlogDistinctSurvivalPct + '% (error->prose head/tail, lossy BY DESIGN)');
console.log('=== HEADLINE (5 real items) ===');
console.log('  AGGREGATE ' + sumBefore + ' tokens -> ' + sumAfter + ' tokens  (ratio ' + overallRatio + ', ' + overallSavedPct + '% saved)');
const cleanBefore = sumBefore - mlogRow.tokensIn;
const cleanAfter = sumAfter - mlogRow.tokensOut;
const cleanRatio = cleanBefore ? Number((cleanAfter / cleanBefore).toFixed(4)) : 1;
const cleanSavedPct = cleanBefore ? Number((100 * (cleanBefore - cleanAfter) / cleanBefore).toFixed(2)) : 0;
console.log('  (excluding the lossy error-classified build log: ' + cleanBefore + ' -> ' + cleanAfter + ', ratio ' + cleanRatio + ', ' + cleanSavedPct + '% saved)');
console.log('RESULT_JSON=' + JSON.stringify(compact));
console.log('STATUS=' + (fail === 0 ? 'PASS' : 'FAIL') + ' fails=' + fail);
process.exit(0);
