# PXLANG-M6.1-3 Milestones — pluresdb-px → praxis-lang

Worker updates this after EACH gate. Main session verifies independently on disk.
Branch: `m6-pluresdb-px-praxis-lang` (base dd62623). praxis-lang pinned rev: bbc306c.

## M6.1 — add dep + re-export shim (no deletes)
- [x] 3 git deps (px-ast/px-compiler/px-eval @ bbc306c) added to pluresdb-px Cargo.toml
- [x] names-only re-export shim added to src/px/mod.rs (both engines compile side-by-side) — as `pub mod pxlang` (namespaced to avoid colliding with the local flat `Px*` AST during migration; becomes the top-level re-export hub in M6.3)
- [x] GATE M6.1: `cargo build -p pluresdb-px` GREEN — result: cold build w/ git fetch of plures/praxis-lang@bbc306c; px-grammar+px-ast+px-compiler+px-eval compiled, pluresdb-px compiled with BOTH engines. Finished dev profile in 1m 08s.
- [x] committed: c72037d (M6.1)

## M6.2 — rewrite AST-facing files against px-ast
- [x] compiler.rs rewritten (PxDocument→CompiledRecord; §2d adapters; ProcedureBody::Code handled honestly, not dropped — serialized into the record under `code` with `body_kind=code`, proven by test `code_block_body_is_preserved_not_dropped`). Authored bespoke `expr_to_string` / `value_to_json` / `var_ref_to_string` renderers (px-ast has NO Display for Expr/Value) so every executor JSON field stays byte-form strings. Public API preserved: `compile`, `compile_with_stats`→`CompileResult`, `compile_with_lint`→`CompileWithLintResult`, `compile_step`, `CompileStats`.
- [x] dataflow.rs::ast_to_node rewritten to `px_ast::DataflowProcedureDecl` (§2d: `source`→`source_queue:StringLiteral`, `type_expr`→`param_type:TypeExpr` via Display, `destination`→`dest_queue`); a Code-body → empty v1 step list (record compiler still preserves the code; documented follow-up).
- [x] lint.rs retyped: public `lint(&px_ast::PxDocument)` lowers each procedure to a lint-local string-form view via the compiler's `step_to_json` (single source of truth), then runs all L001–L012 rules unchanged. View types (LintDoc/LintProc/LintStep/…) are lint-owned (they outlive the deleted flat AST). resolver.rs retyped to the px-ast statement list (clone non-import `statements`, prefix decl names by `Statement::*`, filter imports; `import.path:Vec<Ident>`→`module::sub` string); tests count facts via `Statement::Fact` helpers.
- [x] executor/async_executor/scenario_runner/compose LEFT UNTOUCHED. **watcher.rs: 2-line necessary bridge only** — `load_and_compile` now parses via `px_compiler::parse` (was local `parse`) to feed the px-ast-typed `compile`; NO behavioral/JSON change. 4 in-crate end-to-end TESTS (mod.rs ×4, executor.rs ×1, conformance.rs) had their parse switched to `pxlang::parse` for the same type-bridge reason.
- [x] GATE M6.2a: `cargo build -p pluresdb-px` GREEN — result: **EXIT 0, 0 errors, 9.78s** (warm; both engines coexist).
- [x] GATE M6.2b: `cargo test -p pluresdb-px` GREEN — result: **517 lib + 2 + 4 passed, 0 failed (5 pre-existing ignored)**. Includes byte-fidelity e2e: full_pipeline_loop_emit_try, full_pipeline_loop_key_as, parse_try_retry_compiles_to_json, parse_parallel_branch_retry_compiles_to_json, end_to_end_parse_compile_execute, conformance corpus.
- [x] GATE M6.2 build-the-binary: ran real .px end-to-end via **`cargo run -p pluresdb-px --example run_px -- crates/pluresdb-px/examples/pipeline.px`** (real binary `run_px.exe` + real DemoHandler) — output: **compiled 3 records (fact/rule/procedure); executed `pipeline` success=true, 4 steps, $results=["ALPHA","BETA","GAMMA"], emit=[{count:3,type:complete}]**.
- [x] committed: ____ (sha filled in after commit below)

## M6.3 — delete duplicate engine
- [ ] deleted: grammar.pest, builder.rs, PxParser+Px* AST+local parse in mod.rs
- [ ] pest/pest_derive removed from Cargo.toml (or noted why kept): ____
- [ ] GATE M6.3 build+test GREEN with one grammar — result: ____
- [ ] verify `git grep grammar.pest crates/pluresdb-px` → 0 hits: ____
- [ ] GATE M6.3 clippy `-D warnings` (or documented Defender-FP-blocked): ____
- [ ] committed: ____

## Branch HEAD after M6.3: ____
## Honest gaps / deferrals: ____
