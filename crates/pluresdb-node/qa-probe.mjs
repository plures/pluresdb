// H-QA adversarial JS probe against the REBUILT native .node surface.
// NAPI signature (from index.d.ts): compressText(content: string, contentType?: string) -> string
//                                   countTokens(text: string) -> number
//                                   detectContentType(content: string) -> string
// Bounded output: prints one line per FAIL + a final SUMMARY token.
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const native = require("./index.js");
const { compressText, countTokens, detectContentType } = native;

const KNOWN_TYPES = new Set(["json", "log", "code", "error", "prose"]);
let fail = 0;
let pass = 0;
const fails = [];
function check(name, cond, detail = "") {
  if (!cond) { fail++; fails.push(`FAIL ${name} :: ${detail}`.slice(0, 220)); }
  else { pass++; }
}

// compressText takes a bare string contentType (or undefined for auto-detect).
function comp(input, contentType) {
  return contentType === undefined ? compressText(input) : compressText(input, contentType);
}

// Safety wrapper: compressText must never throw, never grow tokens, type valid.
function safe(label, input, contentType) {
  let out, threw = null;
  try { out = comp(input, contentType); }
  catch (e) { threw = String((e && e.message) || e); }
  check(`${label}:no-panic`, threw === null, `threw=${threw}`);
  if (threw !== null) return { out: null };
  check(`${label}:returns-string`, typeof out === "string", `typeof=${typeof out}`);
  let bt, ct;
  try { bt = countTokens(input); ct = countTokens(out); }
  catch (e) { check(`${label}:counttok-no-panic`, false, String(e)); return { out }; }
  check(`${label}:tok-finite`, Number.isFinite(bt) && Number.isFinite(ct) && bt >= 0 && ct >= 0, `bt=${bt} ct=${ct}`);
  check(`${label}:no-token-growth`, ct <= bt, `base=${bt} comp=${ct}`);
  let dt, dthrew = null;
  try { dt = detectContentType(input); } catch (e) { dthrew = String(e); }
  check(`${label}:detect-no-panic`, dthrew === null, `threw=${dthrew}`);
  if (dthrew === null) check(`${label}:detect-known-label`, KNOWN_TYPES.has(dt), `got=${dt}`);
  return { out, bt, ct, dt };
}

// ── 1. BOUNDARY / DEGENERATE ──────────────────────────────────────────
safe("empty", "");
safe("single-char", "x");
safe("single-line", "just one single line of text with no terminator");
safe("all-whitespace", "   \t   \n   \t  \n    ");
safe("only-newlines", "\n\n\n\n\n\n\n\n");
safe("long-single-line", "word ".repeat(4000));                       // ~20k chars, no sentence break
safe("unicode-emoji-cjk", "日本語のテキスト 🚀🔥💯 émojis café résumé 中文字符 한국어 ".repeat(40));
safe("token-dense-blob", "a1B2c3D4e5F6g7H8".repeat(200));
// also exercise each explicit type on a degenerate empty/tiny input
for (const t of KNOWN_TYPES) { safe(`empty-as-${t}`, "", t); safe(`tiny-as-${t}`, "x\nx\nx\n", t); }
// extremely long single line forced through prose/code/log explicitly
safe("long-line-as-prose", "alpha beta gamma ".repeat(3000), "prose");
safe("long-line-as-code", "let x".repeat(3000), "code");
safe("long-line-as-log", "z".repeat(20000), "log");

// ── 2. MIXED / ADVERSARIAL ────────────────────────────────────────────
const proseWithFences = "Here is some explanatory prose about the system. " +
  "It describes a problem in plain words.\n```rust\nfn handler() {\n    let x = 1;\n    x + 1\n}\n```\n" +
  "And then the prose continues after the fence with more discussion of the matter at hand. ".repeat(3);
safe("prose-with-code-fence", proseWithFences);

const logWithProse = "INFO starting service alpha\nINFO starting service alpha\n" +
  "This is an interjected prose sentence the operator left in the log file by hand.\n" +
  "ERROR connection refused to db\nERROR connection refused to db\nERROR connection refused to db\n";
safe("log-with-prose-lines", logWithProse);

const jsonEscapedNl = JSON.stringify({ note: "line one\nline two\nline three", tags: ["a","b","c"], padding: "x".repeat(220) });
safe("json-escaped-newlines", jsonEscapedNl);

// "log" where EVERY line is unique (no runs) — safe, must NOT claim a collapse.
let uniqueLog = "";
for (let i = 0; i < 40; i++) uniqueLog += `2026-06-29 12:00:${String(i).padStart(2,"0")} ERROR unique event number ${i} occurred\n`;
const ul = safe("log-all-unique-pinned", uniqueLog, "log");
check("log-all-unique:no-false-collapse", ul.out !== null && !ul.out.includes("[×"), `claimed collapse: ${(ul.out||"").slice(0,80)}`);

// prose with exactly 1 / 2 / 3 sentences (<=6 -> whitespace-collapse path; must not dup/drop).
for (const n of [1, 2, 3]) {
  let body = "";
  for (let i = 0; i < n; i++) body += `Sentence ${i} carries a uniqueZ${i} distinct marker. `;
  while (body.length < 260) body += `Extra filler clause uniqueZ${n} keeps the body over the floor. `; // pad WITHOUT adding the watched markers twice
  const r = safe(`prose-${n}-sentence`, body, "prose");
  if (r.out !== null) {
    for (let i = 0; i < n; i++) {
      const count = (r.out.match(new RegExp(`uniqueZ${i}\\b`, "g")) || []).length;
      check(`prose-${n}-sent:marker-${i}-not-duplicated`, count === 1, `uniqueZ${i} appears ${count}x`);
    }
    check(`prose-${n}-sent:no-elision`, !r.out.includes("elided"), `unexpected elision: ${r.out.slice(0,80)}`);
  }
}

// ── 3. IDEMPOTENCY / DETERMINISM ──────────────────────────────────────
const idemInputs = {
  prose: "Distinct prose sentence number A about subsystems. ".repeat(40),
  code: "pub fn one(x: i32) -> i32 {\n    let y = x + 1;\n    y\n}\npub fn two(z: i32) -> i32 {\n    let w = z - 1;\n    w\n}\nstruct S { a: i32, b: i32 }\nimpl S { pub fn m(&self) -> i32 { self.a } }\n".repeat(2),
  log: (() => { let s=""; for(let i=0;i<30;i++) s+="ERROR upstream connection refused mid run\n"; s+="INFO recovered ok\n"; for(let i=0;i<10;i++) s+="WARN retry backoff\n"; return s; })(),
};
for (const [t, inp] of Object.entries(idemInputs)) {
  const a  = comp(inp, t);
  const a2 = comp(inp, t);
  check(`determinism-${t}`, a === a2, "two runs differ");
  const b = comp(a, t); // second pass
  check(`idempotency-${t}:no-token-growth`, countTokens(b) <= countTokens(a), `pass2 grew ${countTokens(a)}->${countTokens(b)}`);
  const c = comp(b, t); // third pass -> fixpoint expected
  check(`idempotency-${t}:fixpoint`, b === c, `no fixpoint by pass3`);
}

// ── 4. STRUCTURE INTEGRITY (exact counts on crafted inputs) ───────────
// 4a. CODE: every signature line present in input present in output.
const codeSigs = `pub fn sig_alpha(a: i32) -> i32 {
    let body_alpha = a * 2;
    body_alpha + 1
}
pub fn sig_beta(b: i32) -> i32 {
    let body_beta = b - 3;
    body_beta
}
struct SigGamma {
    f1: i32,
    f2: i32,
}
impl SigGamma {
    pub fn sig_method_delta(&self) -> i32 {
        self.f1 + self.f2
    }
}
`;
const codeOut = comp(codeSigs, "code");
for (const s of ["sig_alpha", "sig_beta", "SigGamma", "sig_method_delta"])
  check(`code-sig-present:${s}`, codeOut.includes(s), `dropped signature ${s}`);
check("code-body-dropped", !codeOut.includes("body_alpha = a") && !codeOut.includes("body_beta = b"), "body leaked");

// 4b. LOG: [×N] counts ACCURATE — known run of exactly 7 then 4.
let craftedLog = "";
for (let i = 0; i < 7; i++) craftedLog += "ERROR exact run of seven identical lines here\n";
for (let i = 0; i < 4; i++) craftedLog += "WARN exact run of four identical lines here\n";
craftedLog += "INFO one distinct trailing line\n";
const logOut = comp(craftedLog, "log");
check("log-count-7-accurate", logOut.includes("[×7]"), `out=${logOut.replace(/\n/g,"\\n").slice(0,160)}`);
check("log-count-4-accurate", logOut.includes("[×4]"), `out=${logOut.replace(/\n/g,"\\n").slice(0,160)}`);
check("log-distinct-not-marked", logOut.includes("INFO one distinct trailing line") && !logOut.includes("distinct trailing line  [×"), "distinct wrongly collapsed");
check("log-no-spurious-count", !logOut.includes("[×8]") && !logOut.includes("[×5]") && !logOut.includes("[×3]") && !logOut.includes("[×6]"), "spurious count present");

// 4c. PROSE: elision marker count reflects real elided count.
// 40 sentences -> head 3 + tail 3 -> 34 elided -> "[… 34 sentences elided …]".
let proseExact = "";
for (let i = 0; i < 40; i++) proseExact += `Distinct prose unit ${i} with numeric tokenZ ${i}. `;
const proseOut = comp(proseExact, "prose");
const elideTxt = (proseOut.match(/\[… \d+ sentences elided …\]/) || ["<none>"])[0];
check("prose-elide-count-exact", proseOut.includes("34 sentences elided"), `got: ${elideTxt}`);
check("prose-elide-marker-once", (proseOut.match(/sentences elided/g) || []).length === 1, `count=${(proseOut.match(/sentences elided/g)||[]).length}`);
for (const i of [0,1,2,37,38,39]) check(`prose-keep-${i}`, proseOut.includes(`tokenZ ${i}.`), `lost sentence ${i}`);
check("prose-drop-middle-20", !proseOut.includes("tokenZ 20."), "middle sentence 20 leaked");

// ── 5. TOKEN-COUNT FIDELITY (vs known cl100k expectations) ────────────
check("tok:empty", countTokens("") === 0, `got ${countTokens("")}`);
check("tok:hello-world", countTokens("hello world") === 2, `got ${countTokens("hello world")}`);
check("tok:pangram", countTokens("The quick brown fox jumps over the lazy dog") === 9, `got ${countTokens("The quick brown fox jumps over the lazy dog")}`);
check("tok:newline", countTokens("\n") === 1, `got ${countTokens("\n")}`);

if (fails.length) console.log(fails.join("\n"));
console.log(`SUMMARY pass=${pass} fail=${fail} STATUS=${fail === 0 ? "PASS" : "FAIL"}`);
process.exitCode = fail === 0 ? 0 : 1;
