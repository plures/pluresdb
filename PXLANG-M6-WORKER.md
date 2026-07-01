# PXLANG-M6.1-3 Worker Spec — pluresdb-px → import praxis-lang

**You are the pluresdb-px rewire worker for M6 of the praxis-lang epic.** Your job: make `pluresdb-px` STOP duplicating the `.px` language engine and instead IMPORT it from `praxis-lang`, keeping ONLY the pluresdb-specific runtime (executor/dataflow/scenario/watcher/compose/store). This closes the language/grammar duplication (NOT ADR-0017 — that's a separate constraint-engine concern in `src/db/`, leave it untouched).

## AUTHORITATIVE SPEC (read this FIRST, in full)
`C:\Users\kbristol\.openclaw\workspace\epic-praxis-lang\PXLANG-M6-ANALYSIS.md` — a disk-verified, file-level rewire plan. It classifies every file (DELETE / STAY / REWRITE), gives the exact API delta, the field-shape adapters (§2d), and the gated stage sequence (§4). Follow §4 stages M6.1→M6.3 exactly. Do NOT re-derive; it's already verified.

## Working dir
`C:\Projects\pluresdb-m6` (worktree, branch `m6-pluresdb-px-praxis-lang`, base `dd62623` = origin/main). All work + commits here.\n\n## praxis-lang dependency (pin by git rev — C-NIX-002, praxis-lang is NOT published to a registry)
In `crates/pluresdb-px/Cargo.toml` add:
```
px-ast = { git = "https://github.com/plures/praxis-lang.git", rev = "bbc306ca4377cb437e7366d07dabb50392a0e659" }
px-compiler = { git = "https://github.com/plures/praxis-lang.git", rev = "bbc306ca4377cb437e7366d07dabb50392a0e659" }
px-eval = { git = "https://github.com/plures/praxis-lang.git", rev = "bbc306ca4377cb437e7366d07dabb50392a0e659" }
```
(rev bbc306c = current praxis-lang main HEAD. Verify with `git ls-remote` if unsure. Cargo.lock MUST end up committed + consistent.)

## THE STAGES (each GATE must pass before the next — build-the-binary-run-the-binary, not just cargo test)

### M6.1 — add dep + re-export shim (NO deletes yet)
- Add the 3 git deps above.
- In `src/px/mod.rs`, ADD (alongside the existing engine, don't delete anything yet) a names-only re-export shim so downstream names resolve to px-ast:
  `pub use px_compiler::{parse, parse_statement};`
  `pub use px_ast::{DataflowProcedureDecl as PxDataflowProcedure, DataflowParam as PxDataflowParam, DataflowReturn as PxDataflowReturn};`
  (If the old engine already defines those names, gate the shim behind a temporary `mod praxis_shim { ... }` or a cfg so BOTH compile side-by-side for differential testing.)
- **GATE M6.1:** `cargo build -p pluresdb-px` green with both engines present. Confirms praxis-lang git deps resolve + fetch.

### M6.2 — rewrite the AST-facing kept files against px-ast
- **`compiler.rs`** (the main labor): rewrite its body to lower `px_ast::PxDocument` → the existing `CompiledRecord` JSON. Apply the §2d field adapters: `Ident`→`.as_str()`, `TypeExpr`→render to string (use px-ast `Display`/`to_string`; if none, a tiny local formatter), `StringLiteral`→`.value`, **note the field renames** `source→source_queue`, `destination→dest_queue`, and **match on `ProcedureBody`**: `Steps(_)` walk as today (via px-ast `Step`), and **`Code(_)` MUST be handled honestly** — emit the code-block into the record (parsing/storage) OR return a real `unsupported code-block body` error. **NEVER silently drop it (C-NOSTUB-001).**
- **`dataflow.rs::ast_to_node`**: rewrite to consume `px_ast::DataflowProcedureDecl`.
- **`lint.rs`**, **`resolver.rs`**: retype to px-ast (`Step` not `PxStep`, `PxDocument` from px-ast). Re-map `PxStep::*` match arms to `px_ast::Step::*` (same 13 variants, different payload sub-structs — see §2d).
- Leave `executor.rs` / `async_executor.rs` / `scenario_runner.rs` / `watcher.rs` / `compose.rs` UNTOUCHED (they read JSON records, not the AST — proven 0 AST refs). Do NOT collapse `executor.rs::default_evaluate_condition` onto px_eval — DEFER that (file it as a follow-up note in the result file); keep M6 blast radius tight.
- **GATE M6.2:** `cargo build -p pluresdb-px` green AND `cargo test -p pluresdb-px` (existing px parse/compile/execute/scenario tests pass against px-compiler+px-ast). Then **build-the-binary-run-the-binary**: find + run a pluresdb-px example/bin/CLI that parses+compiles (+executes if available) a REAL `.px` file end-to-end. Record the command + output.

### M6.3 — delete the duplicate engine
- DELETE `src/px/grammar.pest`, `src/px/builder.rs`, and the `PxParser` + `Px*` AST definitions + local `parse` fn inside `mod.rs`. `mod.rs` becomes the re-export hub (praxis-lang parse/AST + local runtime mods).
- Remove now-unused `pest`/`pest_derive` deps from Cargo.toml **IFF** nothing else in the crate uses `Rule` (grep first). If other code uses `Rule`, leave pest and note it.
- **GATE M6.3:** `cargo build -p pluresdb-px` + `cargo test -p pluresdb-px` green with ONLY ONE grammar in the crate. Verify: `git grep -n "grammar.pest" crates/pluresdb-px` → 0 hits; the ONLY `grammar.pest` reachable is praxis-lang's. Then `cargo clippy -p pluresdb-px -- -D warnings` (NOTE: if clippy is quarantined by Windows Defender as a FALSE-POSITIVE `Trojan:Win32/Wacatac.B!ml` on clippy-driver.exe — same as M5 — do NOT fight it; document it as blocked-locally and note CI will run clippy on Linux. Everything ELSE must be green locally.)

## COMMIT DISCIPLINE
- Commit per stage, milestone-coded: `M6: <stage> pluresdb-px onto praxis-lang [praxis-lang epic]`.
- Commit Cargo.toml + Cargo.lock together.
- Do NOT push, do NOT open a PR, do NOT touch main — the main session handles the external side-effect boundary. Commit to the branch ONLY.

## ANTI-HANG BUILD DISCIPLINE (MANDATORY — a prior epic worker hung 3h)
- Run EVERY build/test/clippy as a BOUNDED, FOREGROUND, time-limited exec. NEVER fire-and-forget a background `cargo build`. Use a timeout (e.g. 900s for a cold build with git-dep fetch, 300s for warm). Keep tool output bounded (`2>&1 | Select-Object -Last 40`), no walls of text, no node_modules/target recursion.

## PROGRESS TRACKING
Update `C:\Projects\pluresdb-m6\PXLANG-M6-MILESTONES.md` after EACH gate (tick the box + paste the 1-line gate result). The main session reads this file to verify — so it must be truthful. Never tick a box whose gate you didn't actually run green.

## HONESTY (C-NOSTUB-001)
Real implementations or honest documented absence. No `todo!()`/placeholder/canned returns. If you cannot make a gate green, STOP, write exactly what's red + why in the milestones file, and report — do NOT fake it green or claim done. The main session VERIFIES every gate independently on disk; a false "green" will be caught and is worse than an honest "blocked here."

## When done (all 3 gates green)
Report a 8-12 line summary: what you deleted, what you rewrote, the field adapters used, how `ProcedureBody::Code` was handled, the build-the-binary run command+result, each gate's verdict, the branch HEAD sha, and any honest gaps/deferrals (e.g. the deferred executor eval-collapse).
