# LIST-BUG-DIAGNOSIS — `test-node.js` "Test 1: Basic CRUD" `list()` mismatch

**Status:** Root-caused. Diagnosis only — no fix committed.
**Date:** 2026-07-01
**Repo/branch:** `C:\Projects\pluresdb` on local `main` (build produced `pluresdb-node.win32-x64-msvc.node`, release, 3m31s, exit 0).\n\n---\n\n## 1. Reproduced failure + exact `list()` value

Instrumented `test-node.js` locally (reverted after) with a `LIST RESULT` log immediately before the `all.length === 1` assertion, rebuilt (`npm run build` → `napi build --platform --release`), and ran `node test-node.js`:

```
✓ Put node: node-1
✓ Get node: {"age":30,"name":"Alice","type":"Person"}      <-- get() SUCCEEDS
LIST RESULT COUNT: 11                                        <-- list() returns 11, not 1
LIST RESULT IDS: ["C-0001","C-0009","C-0008","C-0006","C-0003","node-1",
                  "C-0007","C-0010","C-0004","C-0005","C-0002"]
Test failed: Error: List failed: expected 1 node            <-- test-node.js:40
```

**`list()` returns 11.** The extra 10 nodes are `C-0001` … `C-0010`. The user's `node-1` is present. So this is **NOT** "put not visible to list" (that would be 0) and **NOT** a tombstone/persistence/stale-actor issue — it is **seed nodes counted by `list()`** (candidate cause **(e)** / **(d)**: a fresh `PluresDatabase('test-actor')` is *not* empty — it is pre-seeded with 10 praxis-constraint nodes).

`get('node-1')` works because it is an exact-id lookup; `list()` returns *all* nodes, and the 10 seeded constraints are real nodes.

---

## 2. Root cause (file:line evidence)

The `PluresDatabase` constructor **seeds 10 built-in praxis constraints into the CrdtStore as ordinary nodes**, then `list()` (correctly) returns them alongside the user node.

### 2a. Constructor seeds constraint nodes
`crates/pluresdb-node/src/lib.rs`

- **L216** (`fn new`): `seed_praxis_into_crdt(&core_store, &actor_id);` — runs on *every* `new PluresDatabase(...)` (and again at L288 in `new_with_embeddings`).
- **L156–167** (`fn seed_praxis_into_crdt`): reads the canonical default set and writes each not-yet-present constraint as a CrdtStore node:
  ```rust
  let defaults = px_default_store();          // L157
  for constraint in defaults.constraints() {  // L160
      if store.get(&constraint.id).is_none() {
          if let Ok(data) = constraint_node_data(constraint) {
              store.put(constraint.id.clone(), actor_id.to_string(), data); // L163
          }
      }
  }
  ```
  `constraint_node_data` (L57) shapes each as a normal node: `{ "type": "praxis_constraint", "constraint": <json> }`. These are **full nodes**, not a side table.

### 2b. The default set contains exactly 10 constraints
`crates/pluresdb-px/src/db/seed.rs`

- **L431 `default_store()`** → calls `seed_constraints()`, `seed_adrs()`, `seed_evidence()`.
- `seed_constraints` performs exactly **10** `store.upsert_constraint(...)` calls: `C-0001` (L30) … `C-0010` (L223). (`grep -c upsert_constraint` = 10.)
- ADRs / Evidence go into *separate* collections (`upsert_adr`, `upsert_evidence`) and are **not** iterated by `seed_praxis_into_crdt` (which only loops `defaults.constraints()`), so they are **not** added to the CrdtStore — consistent with the 10 observed extra ids being exactly the `C-00xx` constraints.

`crates/pluresdb-px/src/db/store.rs`
- **L74 `constraints()`** returns `self.constraints.values()` — all 10.

### 2c. `list()` is behaving correctly by design
`crates/pluresdb-core/src/lib.rs`

- **L944 `pub fn list(&self)`**: returns *every* node — from persistence when attached (L945-960) else all in-memory `self.nodes` (L967-970). No type filter, no tombstone exclusion. It is a faithful "list all nodes."
- **L933 `pub fn get(&self, id)`**: exact-id lookup → finds `node-1` regardless of how many other nodes exist. This is why get and list "disagree": they are answering different questions (one id vs. all nodes), and there genuinely are 11 nodes.

The NAPI passthrough is thin and correct: `crates/pluresdb-node/src/lib.rs` `list()` (L400-419) just maps `store.list()` → `{id,data,timestamp}`; `get()` (L346-363) maps `store.get(id)`. Neither introduces the discrepancy.

### 2d. Provenance — why it went red ~2 months ago
`git log -S seed_praxis_into_crdt` → introduced by commit **`00024d6`** *"feat(pluresdb-node): unify px constraints onto CrdtStore single source of truth … (TASK-PX-CANON Stage 2)"* (see also ADR-0017 Stage B6, referenced in `lib.rs` L40-52). **Before** that commit, praxis constraints lived only in a side in-memory `PraxisStore` and did **not** appear in `list()`, so `all.length === 1` held. **After** Stage 2 made constraints first-class CrdtStore nodes, `list()` legitimately returns them → the hard-coded `=== 1` assertion broke. This matches the "red on CI for ~2 months" window.

**Conclusion:** `list()` / `get()` / `put()` are all correct. The **test assertion encodes stale pre-Stage-2 semantics** ("a fresh DB has exactly the nodes I put"), which stopped being true when constraint-as-node seeding was added by design.

---

## 3. Minimal correct fix

**Which side is wrong:** the **test** (`crates/pluresdb-node/test-node.js`), not the store. `list()` returning the seeded constraint nodes is *correct-by-design* per TASK-PX-CANON Stage 2 / ADR-0017 (constraints are real, queryable nodes — that is the whole point of the unification). Weakening `list()` (e.g. hiding `type == "praxis_constraint"` nodes, or hardcoding a count) would **reintroduce a hidden side-table** and violate the single-source-of-truth invariant — and is a banned stub-style "fix." **Do not touch `list()`.**

### Recommended minimal fix (test-only, no stub, semantically honest)

Make Test 1 assert against a *baseline*, i.e. "exactly one **more** node than the fresh DB started with," so it verifies the real behavior (put adds exactly one node) without pretending seeds don't exist. Two acceptable equivalent forms:

**Option A (preferred — baseline delta):**
```js
const db = new PluresDatabase('test-actor');
const baseline = db.list().length;          // seeded praxis constraints (currently 10)
// ... put node-1 ...
const all = db.list();
if (all.length !== baseline + 1) {
  throw new Error(`List failed: expected baseline+1 (${baseline + 1}) nodes, got ${all.length}`);
}
// and assert node-1 is present:
if (!all.some(n => n && n.id === 'node-1')) {
  throw new Error('List failed: node-1 not in list()');
}
```
This is robust even if the seeded-constraint count changes later (10 → N).

**Option B (assert membership only):** drop the count assertion entirely and assert `all.some(n => n.id === 'node-1')` — but Option A is strictly better because it still proves "put added *exactly one* node."

Apply the same baseline-delta pattern to the later Test-1 delete check and to any other test that assumes an empty fresh DB (Test 5 `stats().totalNodes < 5` is a `<` lower-bound and is unaffected; Test 2 `listByType` filters by type and is unaffected since `praxis_constraint` ≠ `Person`/`Item`; Test 7 uses a *separate* `query-actor` DB and only counts its own puts via DSL filters, unaffected).

> Note: the assertion currently fires at **`test-node.js:40`** ("List failed: expected 1 node"), driven by the `all.length !== 1` check at line 39. Fix is contained to that block (plus mirror any identical empty-DB assumption elsewhere).

### Does the fix need the pluresdb-core / pluresdb-storage mutation-gate?
**No.** The fix touches **only** `crates/pluresdb-node/test-node.js` (a test script, node crate). It does **not** modify `pluresdb-core`, `pluresdb-storage`, or `pluresdb-px` source, so the core/storage mutation-gate is **not** triggered. No ADR change is required (the current behavior already matches ADR-0017 Stage B6 / TASK-PX-CANON Stage 2 — the test simply hadn't caught up).

---

## 4. Summary table

| Question | Answer |
|---|---|
| `list()` reproduced value | **11** (ids `C-0001…C-0010` + `node-1`) |
| Why get≠list | `get('node-1')` = exact-id hit; `list()` = all 11 nodes; there really are 11 |
| Which candidate cause | **(e)/(d)** — fresh DB seeds 10 praxis-constraint nodes; none of (a) type-filter-drop, (b) stale-persistence, (c) split view/generation |
| Root cause file:line | `pluresdb-node/src/lib.rs:216` → `:156-167 seed_praxis_into_crdt` → `pluresdb-px/src/db/seed.rs:431 default_store` (10× `upsert_constraint`, C-0001..C-0010) |
| Is `list()` buggy? | **No** — correct by design (constraints are first-class nodes since commit `00024d6`, ADR-0017 Stage B6) |
| Is the test stale? | **Yes** — `all.length === 1` assumes an empty fresh DB, false since Stage 2 (~2 months ago) |
| Minimal fix | Test-only: assert `baseline + 1` and `node-1 ∈ list()` (Option A). Do NOT weaken `list()`. No stubs. |
| Needs core/storage mutation-gate? | **No** — change is confined to `pluresdb-node/test-node.js` |

---

## SECOND BUG (found by parent via full 
ode test-node.js run — subagent stopped at Test 1)

**Test 4 (Vector search) FAILS:** ectorSearch([1,0,0,0]) against stored identical [1,0,0,0] returns **score 0.84**, test expects >=0.99.

### Root cause — NOT a vector/HNSW bug; a public-API CONTRACT/DOC defect
- HNSW cosine is correct: identical vector -> raw similarity = 1.0. mb-rust IS the top result (ranking correct).
- But ector_search (pluresdb-core lib.rs:1063) returns **blended_search_score**, NOT raw cosine:
  lended = 0.7*similarity + 0.2*quality + 0.1*recency (lib.rs:519).
- For fresh node {title:'Rust'}: compute_quality_score (lib.rs:436) = 0.2 (only the +0.2 recency bonus; no content/category/tags/source). recency=1.0.
  => 0.7*1.0 + 0.2*0.2 + 0.1*1.0 = **0.84**. Deterministic, fully explained. Intended blend (test ector_search_blends_quality_into_ranking asserts blending is by design).
- **The DEFECT:** NAPI ector_search doc (pluresdb-node lib.rs:598) says *"Results are ordered by **cosine similarity** (highest first) and filtered by 	hreshold (0-1)"* and the 	hreshold/min_score param is documented (pluresdb-cli main.rs:1850, procedures/ir.rs:439, training.rs:57) as *"Minimum **cosine similarity** score in [0,1]"*. **But it actually sorts/filters by the BLENDED score.** A caller filtering min_score=0.95 for near-identical vectors silently gets nothing (identical vector = 0.84). The API lies about what score/	hreshold mean.

### Fix options (STRATEGIC FORK — affects memory-epic recall scoring; put to kbristol)
- **(A) Honest-API, no behavior change:** return BOTH fields — similarity (raw cosine) + score (blended) — and fix docs; rename 	hreshold->minScore/minRelevance filtering the blended score, OR add a separate minSimilarity. Test checks similarity ~1.0. Smallest change, makes the surface truthful.
- **(B) Move blending OUT of low-level vector_search:** low-level returns pure cosine; blending becomes an explicit higher-level recall concern. Bigger; changes recall semantics for the memory epic (P1/H recall path).

**NOT a stale-test-only fix** (unlike the seed-count bug): here the API genuinely misrepresents its score. Do NOT just bump the test to accept 0.84 — that ratifies a lying API. mutation-gate: option A/B touch pluresdb-core (score fields) -> WILL trip mutation-gate(pluresdb-core) — correct.
