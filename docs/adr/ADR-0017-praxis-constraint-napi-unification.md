# ADR-0017: Praxis Constraint Engine NAPI Unification - Collapse the TS Reimplementation onto Rust

- **Status:** Accepted (store strategy decided 2026-06-25 — Option C)
- **Date:** 2026-06-25
- **Deciders:** kbristol (Paradox), mswork
- **Related:** ADR-0015 (queue-driven dataflow), ADR-0016 (hardware-adaptive compute); pares-agens ADR-0014 (Full Plures Stack - "Praxis for all logic", no Node reimpl of core); PLURES-FOUNDATION.md ("TS -> Rust -> NAPI"; "Rust is the runtime"); workspace constraints C-DRIFT-001, C-PLURES-001..004

> Authoring note: drafted via a design subagent (multi-repo source read) + parent verification of every evidence row against file:line. Supersedes an earlier short draft; this is the canonical ADR. Originally mis-filed to `.praxis/decisions/` before being moved here to match pluresdb's `docs/adr/ADR-NNNN` convention.
## 1. Context

Plures has a documented, binding architecture principle: **write the engine once in Rust, expose it
everywhere through NAPI; the TypeScript package is the *API surface*, the Rust crate is the *runtime*.**
(`repos/plures/development-guide/design/PLURES-FOUNDATION.md` L52-68: the "TS->Rust->NAPI Pattern" -- "The
Rust crate is always the production runtime... Applications never call Rust directly -- they use the
published TS package which loads the native addon... TS types remain the API surface. Rust is the runtime.")

The praxis layer has **two engines**, in **different states of compliance** with that principle:

1. **Procedure / reactive engine -- COMPLIANT.** The `.px` procedure executor and the Agens reactive runtime
   are authored once in Rust (`pluresdb-procedures`) and exposed through NAPI in `pluresdb-node`
   (`exec_dsl` L611, `exec_ir` L635, `agens_emit` L702, `agens_emit_praxis` L720, `agens_state_get` L773,
   `agens_state_set` L784). No parallel TS reimplementation exists. This is exactly the pattern we want.

2. **Constraint engine -- DUPLICATED / DRIFTED.** A complete Rust constraint engine exists in `pluresdb-px`
   (`evaluate`, `on_action`, `compile_nl`, `apply_correction`, `undo_correction`, `query_gaps`, plus
   `Violation` / `ActionBlocked` / `Constraint` / `AgentContext` types). **It is not exposed through NAPI** --
   `pluresdb-node` does not even depend on the `pluresdb-px` crate. Meanwhile `@plures/praxis` (v2.6.1,
   `github.com/plures/praxis`) ships a **second, independent constraint engine in pure TypeScript**
   (`PraxisRegistry` + `LogicEngine` + `defineConstraint` + `ConstraintFn` / `ConstraintDescriptor`,
   re-exported from the `@plures/praxis-core` workspace package). Two engines, two languages, one job.

This violates the write-once-Rust principle (workspace constraint **C-DRIFT-001**: do not re-fork an engine
that already exists in Rust behind NAPI). It also contradicts **ADR-0014**, which forbids "Node.js anything"
for core capabilities and mandates Praxis-as-the-single-logic-engine.

**Why this blocks the upcoming orchestration refactor.** The orchestration work (pre-action interception:
`action -> evaluate constraints -> block or proceed`, previously tracked as pares-agens #314/#315/#316) needs
a single canonical constraint API to build on. If we build orchestration on the **TS** engine, every new
feature deepens the fork -- the Rust and TS engines diverge further, and the "queryable / reactive /
portable" PluresDB-native-runtime vision (ADR-0014; the "PluresDB as praxis runtime" WIP) becomes
unreachable because the authoritative logic stays trapped in TS. We must collapse the duplication **before**
orchestration, not after.

The direction is already settled (collapse toward Rust+NAPI). This ADR (a) draws the exact capability
boundary so we cut in the right place, (b) records airtight evidence, (c) specifies the surgical fix.

---

## 2. Evidence table

Each assertion was confirmed by reading the cited file:line on 2026-06-25. Confidence: **Tested** = saw the
exact symbol via grep/read; **Observed** = read structurally; **UNKNOWN** = not yet verified / needs a spike.

| # | Claim | Source (file:line) | Method | Confidence |
|---|-------|--------------------|--------|------------|
| E1 | Procedure engine is Rust->NAPI (compliant): `exec_dsl`/`exec_ir` are `#[napi]` on `PluresDatabase`, call `ProcedureEngine`. | `crates/pluresdb-node/src/lib.rs` L610-613 (`exec_dsl`), L634-637 (`exec_ir`); import L11 `use pluresdb_procedures::engine::ProcedureEngine`. | grep+read | Tested |
| E2 | Agens reactive runtime is Rust->NAPI (compliant): `agens_emit`/`agens_state_*` are `#[napi]`, call `AgensRuntime`. | `lib.rs` L701-706 (`agens_emit`+`AgensRuntime::new`), L772-784 (`agens_state_get`/`set`); import L10 `use pluresdb_procedures::agens::{AgensEvent, AgensRuntime}`. | grep+read | Tested |
| E3 | A full Rust **constraint** engine exists in `pluresdb-px`. Public API: `evaluate(&PraxisStore,&AgentContext)->Vec<Violation>`, `on_action(...)->Result<Vec<Violation>,ActionBlocked>`, `compile_nl(&str,id)->Constraint`, `query_gaps(&PraxisStore)->Vec<&Evidence>`, `apply_correction`, `undo_correction`. | `crates/pluresdb-px/src/db/procedures.rs` L86,L131,L169,L231,L277,L312; types `Violation` L23, `ActionBlocked` L36, `CorrectionApplied` L244. | grep+read | Tested |
| E4 | `pluresdb-px` is **constraints**, not procedures: its `db` module is the constraint store (`PraxisStore`, `Constraint`, `Adr`, `Evidence`, `AgentContext`, `GuidanceStore`); its `px` module is the *separate* `.px` language compiler/executor. | `pluresdb-px/src/lib.rs` L101 `pub mod px;`, L104 `pub mod db;`; `db/mod.rs` L1-46 doc table + re-exports. | read | Observed |
| E5 | `pluresdb-px` constraint API is **NOT** exposed through NAPI. `pluresdb-node` has no `px`/`Violation`/`compile_nl`/`Guidance`/`PraxisStore` symbol (only doc-comment mentions of `praxis_guidance_updated` *events*). | `lib.rs` grep `pluresdb_px|Violation|compile_nl|Guidance|PraxisStore` -> only L688,L716 (doc comments). | grep | Tested |
| E6 | Stronger form of E5: `pluresdb-node`'s manifest does **not depend on `pluresdb-px` at all** -- only `pluresdb-core`, `pluresdb-procedures`, `pluresdb-storage`, `pluresdb-sync`. Unreachable by construction. | `crates/pluresdb-node/Cargo.toml` deps (no `pluresdb-px` line). | grep | Tested |
| E7 | `@plures/praxis` reimplements a constraint/rules engine in **pure TS** (the duplication): `PraxisRegistry`, `LogicEngine`, `createPraxisEngine`, `defineConstraint`, `ConstraintFn`/`ConstraintDescriptor`/`ConstraintId`. | `praxis/src/index.ts` L62-86; `praxis/src/core/rules.ts` L1 re-exports those types from `@plures/praxis-core`; `praxis/src/core/engine.ts` L1 likewise. | grep+read | Tested |
| E8 | `@plures/praxis` is published as pure TS (no napi); self-describes as "The Full Plures Application Framework - declarative schemas, logic engine, component generation, and local-first data." | `praxis/package.json` `version:2.6.1`, `description`, no native fields; `engines.node ">=24"`. | read | Tested |
| E9 | `@plures/praxis` has large genuinely-JS-only framework areas with **no Rust equivalent** in `pluresdb-px`: schema loader/normalizer + logic codegen, component generation, lifecycle engine (versioning/QA/review/release/docs), git hooks, decision-ledger analyzers, chronos chronicle, integration adapters (PluresDB adapter, Unum, CodeCanvas, Tauri, State-Docs, MCP). | `praxis/src/index.ts` schema L196-243, codegen L246-251, decision-ledger L120-195, integrations L260-390, chronos ~L390-430, hooks ~L440-465, lifecycle ~L470-650; `praxis/src/` dirs (factory, project, experiments, research, conversations, analysis, unified). | read | Observed |
| E10 | The canonical architecture **already mandates** the target: praxis's constraint engine is supposed to be Rust-behind-NAPI. | `PLURES-FOUNDATION.md` L52-68 (lifecycle, "Rust is the runtime"), L87 ("praxis ... constraint engine, ... NAPI bindings"); `repo-routing-validation.md` L29 ("praxis ... NAPI bindings. ALL logic expression flows through Praxis."). | grep+read | Tested |
| E11 | Constraint API I/O types are `serde` round-trippable (clean NAPI marshaling via `serde_json::Value`): `Constraint`, `Adr`, `Evidence`, `AgentContext`, `Condition`, `Severity`, `SessionType`, `EvidenceResult` all derive `Serialize, Deserialize`. `Violation`/`ActionBlocked` are at least `Serialize` (confirm `Deserialize` on `Violation` for the test round-trip -> R5). | `pluresdb-px/src/db/schema.rs` L135-290 (derives); `db/procedures.rs` L23 (`Violation`), L36 (`ActionBlocked`). | read | Observed |
| E12 | **KEY RISK (boundary mismatch).** The px engine operates over `PraxisStore` -- a *separate, in-memory* constraint store (`db/store.rs` L38) seeded via `default_store()` (`db/seed.rs` L431) -- **not** the `CrdtStore` (`Arc<Mutex<CrdtStore>>`) that `PluresDatabase` wraps (`lib.rs` L35). So unlike `ProcedureEngine::new(&crdtStore, actor)`, the constraint fns cannot simply borrow `self.store`. Constructing `PraxisStore`/`AgentContext` across the NAPI boundary is **non-trivial**. | `db/store.rs` L38,L46,L54-162; `db/seed.rs` L431; contrast `lib.rs` L613. | read | Observed -> drives Sec.4 + R1 |
| E13 | px persistence is currently *file/seed* based, not CRDT: constraints seeded from built-ins migrated from `.praxis/` (`seed_constraints` L27, `seed_adrs` L257, `seed_evidence` L308). No evidence `PraxisStore` reads/writes through `CrdtStore`. Whether it *should* be CRDT-backed (so it is queryable/portable per ADR-0014) is an **open design question**. | `db/seed.rs` L27/L257/L308/L431; `db/store.rs` (in-memory collections, no CRDT import). | read | UNKNOWN (design) |

---

## 3. Capability Venn - where to cut

The core deliverable: it determines whether the fix is a "thin shell" or "replace one module, keep the
framework." Every `@plures/praxis` export area is bucketed.

| ONLY in pluresdb-px (Rust) - authoritative, needs a NAPI door | BOTH - the duplication to collapse (TS -> thin NAPI binding) | ONLY in @plures/praxis (TS) - KEEP (genuine app-framework, no Rust equivalent) |
|---|---|---|
| `compile_nl` (NL -> Constraint record) [procedures L169] | Constraint **evaluation** semantics: px `evaluate` (L86) vs TS `LogicEngine`/`PraxisRegistry` constraint checking (`core/rules.ts`,`core/engine.ts` via praxis-core) | **(b) Schema:** `PraxisSchema`, loader (JSON/YAML/TS), `normalize`, `validateSchema` [index.ts L196-243] |
| `on_action` gate (block/proceed via `ActionBlocked`) [procedures L131] | Constraint **definition** model: px `Constraint`/`Condition`/`Severity` (schema L151/L30/L135) vs TS `defineConstraint`/`ConstraintDescriptor` (core/rules.ts L1) | **(c) Component / codegen:** `LogicGenerator`, `PluresDBGenerator`, factory rule modules [index.ts L246, factory] |
| `apply_correction` / `undo_correction` [procedures L277/L312] | "Does this action violate the rules?" -> `Violation[]` (px) ~ TS constraint failures | **(d) Svelte / UI:** `@plures/praxis-svelte` (already extracted), `ui-rules`, CodeCanvas editor |
| `query_gaps` (evidence-gap query over ADR graph) [procedures L231] | (definition + evaluation + violation production = the whole "constraint core") | **(e) Local-first / data:** PluresDB *adapter*, `InMemoryPraxisDB`, Unum, Chronicle [integrations L260-390] |
| Constraint<->ADR<->Evidence graph store (`PraxisStore` traversal: `constraint_adrs` L162, `adr_evidence` L176) | | **(f) Lifecycle / event-bus:** `createEventBus`, versioning, QA, review, release, docs, git hooks, project gates [index.ts L440-650] |
| `GuidanceStore` (Facts/Rules/Constraints/Decisions/Risks categories) [guidance.rs L56/L124] | | **(g) MCP server + CLI + chronos chronicle + decision-ledger *analyzers* (dead-rule, shadowed, contradiction, gap)** [mcp/*, cli/*, index.ts L120-195] |

**Reading of the Venn:** the duplication is **narrow** -- the *constraint-checking core* (define a
constraint, evaluate context, produce violations / block actions). Everything else in `@plures/praxis` is
**genuine application framework** (schema -> component generation -> lifecycle automation -> integration
adapters -> MCP/CLI tooling) with **no Rust counterpart** in `pluresdb-px`, correctly TS-only.

**Therefore this is NOT a "thin shell over NAPI" of all of praxis.** It is **"replace ONE module (the
constraint/rules engine core), keep the framework."** `@plures/praxis` stays a large, valuable TS
app-framework; only its `core/rules` + `core/engine` constraint-evaluation internals get demoted to a thin
binding over the Rust NAPI surface.

> Caveat (honest): some "BOTH" surfaces may have *behavioral* divergence the file-level read cannot rule out
> -- e.g. TS `LogicEngine` may implement constraint behaviors (priority ordering, fact-derivation feedback,
> reactive re-eval) that pluresdb-px does not yet have. Any such gap is a **port-to-Rust-first** task, not a
> reason to keep the TS engine. Tracked as R2.

---

## 4. Decision

1. **Expose `pluresdb-px`'s constraint API through `pluresdb-node` via NAPI**, mirroring the existing
   `exec_ir` / `agens_emit` pattern. New `#[napi]` methods: `evaluate`, `on_action`, `compile_nl`,
   `apply_correction`, `undo_correction`, `query_gaps` (JS names in Sec.5). The Rust constraint engine
   becomes the single authoritative runtime, reachable from Node.

2. **Demote `@plures/praxis`'s constraint/rules engine core to a thin binding** over that NAPI surface: the
   evaluation/definition internals behind `PraxisRegistry` constraint-checking + `LogicEngine` constraint
   dispatch + `defineConstraint` are reimplemented to call the Rust NAPI (via `@plures/pluresdb`), preserving
   the existing TS *types/signatures* as the API surface (FOUNDATION L67: "TS types remain the API surface").

3. **Keep the genuinely-JS-only framework** intact and TS-native: schema (b), component/codegen (c),
   Svelte/UI (d), local-first adapters (e), lifecycle/event-bus (f), MCP/CLI/decision-ledger analyzers (g).
   These are not engine duplication; they are the framework that *uses* the engine.

**Shape statement (explicit, per contract):** **"Replace one module, keep the framework."** Most of TS
`@plures/praxis` is app-framework (Venn right column); the fix removes/replaces only the constraint-engine
core (Venn middle column) and leaves the framework standing. This is *not* a whole-package "thin shell".

> **Scope honesty:** one genuine prerequisite is unresolved -- the px engine runs over an in-memory
> `PraxisStore`, **not** the CRDT store the Node binding holds (E12). The decision *direction* is firm, but
> the binding **cannot** be a trivial `&self.store` borrow. Sec.5 Step 1 + Risk R1 define the
> store-construction work this implies. This ADR authorizes the direction and the surgical plan and flags
> the store-bridge as the one real engineering unknown to resolve first (a short spike).

---

## 5. Implementation plan (concrete, no code)

### Step 1 - `pluresdb-node`: add `#[napi]` constraint methods (mirror `exec_ir`)
First add `pluresdb-px = { path = "../pluresdb-px" }` to `crates/pluresdb-node/Cargo.toml` (absent today, E6).
Then add these `#[napi]` methods on `PluresDatabase`, each taking/returning `serde_json::Value` and mapping
errors with `map_node_error` exactly like `exec_ir` (lib.rs L634-644):

| JS method (camelCase, napi convention) | Rust call (pluresdb-px) | Params (serde_json::Value) | Returns (serde_json::Value) |
|---|---|---|---|
| `pxEvaluate(ctx)` | `db::procedures::evaluate(&pstore, &ctx)` | `ctx`: AgentContext | `Vec<Violation>` |
| `pxOnAction(ctx)` | `db::procedures::on_action(&pstore, &ctx)` | `ctx`: AgentContext | `{ violations: [...] }` on Ok / **throw** mapped `ActionBlocked` on Err |
| `pxCompileNl(text, id)` | `db::procedures::compile_nl(text, id)` | `text:String`, `id:String` | `Constraint` |
| `pxApplyCorrection(args)` | `db::procedures::apply_correction(&mut pstore, ...)` | correction args | `CorrectionApplied` |
| `pxUndoCorrection(constraintId)` | `db::procedures::undo_correction(&mut pstore, id)` | `id:String` | `Constraint?` (nullable) |
| `pxQueryGaps()` | `db::procedures::query_gaps(&pstore)` | - | `Vec<Evidence>` |

Marshaling pattern (per method): deserialize params via `serde_json::from_value::<AgentContext>(ctx)`
(map err -> `CoreErrorCode::InvalidInput`), call the px fn, then `serde_json::to_value(&result)`
(map err -> `CoreErrorCode::SerializationError`). `pxOnAction` maps the `Err(ActionBlocked)` arm to a thrown
JS error so the Node caller sees a real exception for blocked actions.

**Store construction (the non-trivial part - UNKNOWN, see R1).** Unlike `ProcedureEngine::new(&crdtStore,..)`,
these fns need a `PraxisStore` (`pstore` above), which is a *different* store than `self.store: CrdtStore`
(E12). Three candidate strategies; pick one in a short spike before coding:
  - **(1a) Lazy seeded singleton.** Hold `praxis_store: Arc<Mutex<PraxisStore>>` on `PluresDatabase`,
    initialized from `db::seed::default_store()` (`db/seed.rs` L431); mutating fns
    (`compile_nl` insert, `apply_correction`, `undo_correction`) lock+mutate it. *Simplest.* **Downside:** a
    second store not synced/persisted via CRDT -- contradicts the ADR-0014 "PluresDB is the runtime" vision;
    constraints added at runtime would not survive restart or sync to peers.
  - **(1b) CRDT-backed projection.** Build `PraxisStore` by reading `Constraint`/`Adr`/`Evidence` records out
    of `self.store` (the seed already migrates `.praxis/` -> records), evaluate, and write mutations back as
    CRDT records. *Correct long-term shape* (queryable, reactive, portable per ADR-0014). **Downside:**
    requires a `PraxisStore::from_crdt(&store)` + `to_crdt` adapter that **does not exist yet** -> a porting
    task in `pluresdb-px`, not just a binding. This is the recommended target; (1a) is acceptable only as an
    explicitly-temporary stepping stone.
  - **(1c) Hybrid.** Ship (1a) behind the NAPI surface now to unblock orchestration, with a tracked
    follow-up to swap the backing store to (1b) without changing the JS method signatures (the whole point of
    keeping the API surface stable). Recommended sequencing.

`AgentContext` construction is *not* a blocker: it has an empty-metadata constructor (`schema.rs` L274) and
derives `Deserialize`, so it round-trips cleanly from a JS object `{ action_type, target, session_type,
metadata }`.

### Step 2 - publish surface (`@plures/pluresdb`)
The new methods land on the existing `PluresDatabase` napi class, so they ship in `@plures/pluresdb`
automatically once the `.d.ts` is regenerated (napi build emits the types). Expose them under a clear
constraint namespace in the TS wrapper -- e.g. a `db.constraints` facade (`evaluate`, `onAction`, `compileNl`,
`applyCorrection`, `undoCorrection`, `queryGaps`) so consumers do not call the raw `pxEvaluate` names.
Bump `@plures/pluresdb` minor; document the new surface in its README. (No new top-level package needed.)

### Step 3 - `@plures/praxis`: demote the engine, keep the framework
- **Replace** the constraint-evaluation internals of `core/rules.ts` + `core/engine.ts` (the
  `PraxisRegistry` constraint-check path + `LogicEngine` constraint dispatch) with a thin adapter that calls
  `@plures/pluresdb` `db.constraints.*`. Keep the **exported TS types and function signatures identical**
  (`defineConstraint`, `ConstraintDescriptor`, `PraxisRegistry`, `LogicEngine`) so downstream code does not
  change -- only the implementation moves to Rust (FOUNDATION L67).
- **Keep unchanged** (Venn right column, no Rust equivalent): `core/schema/*`, `core/logic/generator`,
  factory, project, lifecycle, hooks, chronos, analysis, experiments, research, conversations, unified,
  integrations (pluresdb adapter, unum, code-canvas, tauri, state-docs), mcp, cli, decision-ledger
  *analyzers*. (Note: decision-ledger *analyzers* like dead-rule/contradiction detection are static analysis
  over the registry, distinct from runtime constraint evaluation -- they stay TS for now; porting them is
  out of scope and tracked as future work, not part of this ADR.)
- **`@plures/praxis-core`**: this is where `ConstraintFn`/`ConstraintDescriptor` actually live (E7). The
  demotion edits land here for the engine internals; the type *definitions* stay (they are the API surface).
- Bump `@plures/praxis` minor (additive-internal; no public type change) once it depends on the new
  `@plures/pluresdb` constraint surface.

### Step 4 - C-DRIFT-001 enforcement (so the TS engine cannot re-fork)
Add a guard that fails CI if a *second* constraint-evaluation implementation reappears in TS:
  - A repo-level expectation / praxis constraint asserting "`@plures/praxis` constraint evaluation MUST
    delegate to `@plures/pluresdb` (no local violation-computation loop)". Implementable as a lint/grep gate:
    fail if `core/engine.ts` or `core/rules.ts` contains a hand-rolled violation-evaluation loop instead of a
    `db.constraints.*` call.
  - Encode the rule itself as a `Constraint` record once the NAPI surface exists (dogfood: the drift rule is
    enforced by the very engine it protects). Tie it to this ADR's id in the `evidence` edge.

---

## 6. Test strategy (channel-agnostic, C-TEST-002)

A pure Node smoke test, no adapter/channel dependency, that exercises the new NAPI surface end-to-end:

- **Entry point:** `@plures/pluresdb` (the published package that loads the native addon). New test file
  `crates/pluresdb-node/__tests__/constraints.smoke.mjs` (or the package's existing node-test dir).
- **Flow:**
  1. `const { PluresDatabase } = require('@plures/pluresdb')` (or the napi factory used in existing tests).
  2. `const c = db.constraints.compileNl('always set labels on issues', 'C-TEST-LABELS')` -> assert it
     returns a `Constraint` object with `id === 'C-TEST-LABELS'`, a `when`, a `require`, a `severity`.
  3. Register it (insert into the constraint store via the same surface), then
     `const violations = db.constraints.evaluate({ action_type: 'create_issue', target: 'repo#1',
     session_type: 'main', metadata: {} })` -> assert `violations` is an array and (for a context that
     should trip the rule) contains a `Violation` with the expected `constraint_id`.
  4. **Round-trip assertion (the key one):** assert the returned `Violation` deserializes to a JS object with
     the expected fields -- proving the Rust `Violation` -> `serde_json` -> JS marshaling works.
  5. `db.constraints.onAction({...})` for a blocking context -> assert it **throws** (ActionBlocked path).
- **Gate:** this test must pass in CI before the praxis demotion (Step 3) merges. Build the binary, run the
  binary (AGENTS.md test-first: "build the binary, run the binary" -- `cargo test` alone is insufficient).
- **Pre-req for the round-trip:** confirm `Violation` derives `Deserialize` (only `Serialize` is certain from
  the read) -- if not, the test asserts on the JS object shape directly, which still proves marshaling. (R5.)

---

## 7. Risks / open questions (need kbristol)

- **R1 (the real one) - PraxisStore vs CrdtStore boundary.** px constraints live in an in-memory `PraxisStore`,
  not the `CrdtStore` the Node binding holds (E12, E13). **RESOLVED 2026-06-25 (kbristol): Option C.** Build
  toward **1b (CRDT-backed projection) as the TARGET persistence path**, with **1a (seeded snapshot-bridge) as
  the FALLBACK** that ships first to unblock orchestration. JS method signatures stay identical across the
  swap (the API-surface invariant), so moving from fallback to target is invisible to consumers. See Decision
  Record R1-C below.
- **R2 - TS engine behaviors with no Rust equivalent.** `LogicEngine`/`PraxisRegistry` may implement
  constraint semantics pluresdb-px lacks (priority/ordering, fact-derivation feedback into constraints,
  reactive re-evaluation on state change). A file-level read cannot rule this out. If found, those behaviors
  must be **ported to `pluresdb-px` first** (engine-in-Rust), then bound -- they are not a license to keep the
  TS engine. Needs a focused behavioral diff before Step 3.
- **R3 - Versioning / release coordination across repos.** Two repos move together: `pluresdb` (Rust NAPI +
  republish `@plures/pluresdb`) and `praxis` (depends on the new surface, demotes its engine). `@plures/praxis`
  already pins `@plures/pluresdb ^3.9.1`; the NAPI surface must land + publish **before** the praxis demotion
  can build. Sequence: pluresdb minor -> publish -> praxis bumps dep -> praxis demotion. **-> Needs kbristol**
  for release ordering / whether to gate praxis behind a published pluresdb or use a workspace link during dev.
- **R5 - `Violation`/`ActionBlocked` serde. RESOLVED + DONE (verified via git diff 2026-06-25).** Committed
  baseline (`d4fd5fc`) had `#[derive(Debug, Clone)]` ONLY on `Violation` (L22), `ActionBlocked` (L35), and
  `CorrectionApplied` (L243). Stage B1 added `serde::Serialize, serde::Deserialize` to all three (working tree
  `d3a8fff`). This was REAL, required work — without it the `#[napi]` methods could not `serde_json::to_value`
  their returns. **Status: implemented and built.** (An intermediate parent note briefly claimed the derives
  pre-existed; the git diff is authoritative — they were added this session.) Verified live: `pxEvaluate`
  round-tripped a full `Violation` (nested `constraint` + `message`) Rust→serde→JS in the B2 smoke test.
- **R4 - `apply_correction` signature. RESOLVED (read 2026-06-25):** `apply_correction(store: &mut PraxisStore,
  correction_text: &str, id: impl Into<String>) -> CorrectionApplied` (procedures.rs L277). NAPI shape:
  `pxApplyCorrection(correctionText: String, id: String) -> CorrectionApplied`. Not an opaque args blob.
- **R6 - mutation persistence semantics.** **RESOLVED by R1-C.** Under Option C, durable runtime constraints
  are the TARGET (1b CRDT-backed): `compile_nl`+insert / `apply_correction` / `undo_correction` write through
  to CRDT records and survive restart + sync to peers. The 1a snapshot-bridge fallback is explicitly
  *temporary*; while it is the active backing, runtime mutations are process-local and the seed set is
  canonical. The fallback exists only to ship the seam — it is not the resting state.

---

## 8. Explicit repo routing

Per `repo-routing-validation.md` "Golden rule" (L77: code changes belong in the repo that owns the code) and
the foundation ownership map (`PLURES-FOUNDATION.md` L86-87):

- **`pluresdb`** (owns the CRDT engine, reactive procedures, **and** -- per the documented ownership -- the
  NAPI bindings for the engine): add `pluresdb-px` as a dependency of `pluresdb-node`, implement the `#[napi]`
  constraint methods, regenerate `.d.ts`, republish `@plures/pluresdb`. Any `PraxisStore::from_crdt` adapter
  (1b) is also Rust work in `pluresdb-px`, i.e. this repo.
- **`praxis`** (owns the Praxis app-framework + the constraint engine *as an API surface*): demote the TS
  constraint-evaluation internals (`@plures/praxis-core` `core/rules` + `core/engine`) to a thin binding over
  `@plures/pluresdb`; keep the framework. Bump + republish `@plures/praxis`.
- **This ADR** lives in `pluresdb`'s `.praxis/decisions/` because the primary, blocking code change (NAPI
  exposure) is in `pluresdb`. *Routing note:* per `repo-routing-validation.md` L122, Praxis-era ADRs often
  live in `praxis-business`; a companion governance pointer there is reasonable, but the engineering decision
  is colocated with the code it changes (pluresdb), consistent with the Golden rule. **-> Optional: kbristol
  may want a cross-link ADR stub in `praxis-business` for governance visibility.**
- **No change** to `pares-agens`/`pares-radix`/other consumers: they keep importing `@plures/praxis`; the
  engine moving under them is transparent (API surface preserved). This is the payoff -- the orchestration
  refactor then builds on the unified Rust engine without touching consumer code.

---

## 9. Consequences

- **Positive:** single authoritative constraint engine (Rust); orchestration builds on one surface;
  C-DRIFT-001 + ADR-0014 satisfied; path opened to the "PluresDB as praxis runtime" vision (queryable,
  reactive, portable constraints) once (1b) lands.
- **Negative / cost:** two-repo release coordination (R3); a real porting task if R2 finds TS-only behaviors;
  the store-bridge spike (R1) before any binding code.
- **If we do nothing:** orchestration deepens the TS fork, the Rust engine bit-rots unused, and the
  duplication becomes load-bearing -- exactly the drift this ADR exists to stop.

---

## 10. Status / next action

**Accepted** (store strategy resolved 2026-06-25). Remaining open items are tactical (R2 behavioral diff, R3
release ordering, R4 signature read, R5 Deserialize confirm) and are handled inside the staged build, not
blocking. The work is a clean staged lifecycle: NAPI methods (pluresdb, 1a-backed) -> smoke test (gate) ->
publish -> praxis demotion -> C-DRIFT-001 guard -> verify; with 1b CRDT-backed projection as a tracked
follow-up that does not change JS signatures.

---

## 11. Decision Record - R1-C (store strategy, kbristol 2026-06-25)

**Chosen: Option C = persistence refactor (1b CRDT-backed) as the target, snapshot-bridge (1a) as the
fallback that ships first.** kbristol: *"persistence refactor with snapshot bridge fall back."*

**Why this over plain 1a or plain 1b:**
- Plain 1a (seeded singleton) ships fast but strands constraints in a non-CRDT store forever — violates the
  ADR-0014 / C-PLURES-003 "PluresDB is the runtime" vision; runtime constraints would never persist or sync.
- Plain 1b (CRDT-native) is the correct resting state but requires a `PraxisStore::from_crdt` / `to_crdt`
  adapter in `pluresdb-px` that **does not exist yet** (E12/E13) — doing it up-front blocks the orchestration
  unblock on a porting task.
- **C gets both:** ship the NAPI seam now on the 1a bridge so orchestration is unblocked, while building the
  1b adapter as the target. Because the JS method signatures are fixed (Step 2 `db.constraints.*`), the
  backing store swaps underneath with **zero consumer change**.

**Build order this implies (drives the staged subagents):**
1. **Stage B1 — NAPI seam (1a-backed).** Add `pluresdb-px` dep to `pluresdb-node/Cargo.toml`; add the six
   `#[napi]` methods on `PluresDatabase` over an `Arc<Mutex<PraxisStore>>` seeded from `db::seed::default_store()`.
   Gate: crate builds, `.d.ts` regenerates.
2. **Stage B2 — channel-agnostic smoke test (gate).** `constraints.smoke.mjs` per Sec.6: `compileNl` -> insert
   -> `evaluate` -> assert `Violation[]` round-trips; `onAction` throws on block. Must pass before anything
   downstream merges.
3. **Stage B3 — publish.** `db.constraints` facade in the TS wrapper; minor-bump + republish `@plures/pluresdb`
   with prebuilt platform packages (resolve U2/R4 here).
4. **Stage B4 — praxis demotion.** Replace `@plures/praxis-core` `core/rules`+`core/engine` constraint internals
   with bindings to `db.constraints.*`; keep framework (Venn right column); preserve exported TS types.
5. **Stage B5 — C-DRIFT-001 guard.** CI gate failing on a re-introduced TS violation-evaluation loop; encode
   the rule as a `Constraint` record (dogfood).
6. **Stage B6 — 1b CRDT-backed projection (follow-up, non-blocking).** `PraxisStore::from_crdt`/`to_crdt` in
   `pluresdb-px`; flip the NAPI backing from the 1a singleton to the CRDT projection. **No JS signature change**
   — B2's smoke test must still pass unmodified, which is the proof the swap was invisible.

**Resolves:** R1 (store boundary) and R6 (durability). R2/R3/R4/R5 remain tactical, handled within the stages
above (R2 diff before B4; R3 ordering is B3-before-B4; R4 signature read at B1; R5 confirm at B2).
