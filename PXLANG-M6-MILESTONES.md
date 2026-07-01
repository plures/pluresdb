# PXLANG-M6.1-3 Milestones — pluresdb-px → praxis-lang

Worker updates this after EACH gate. Main session verifies independently on disk.
Branch: `m6-pluresdb-px-praxis-lang` (base dd62623). praxis-lang pinned rev: bbc306c.

## M6.1 — add dep + re-export shim (no deletes)
- [x] 3 git deps (px-ast/px-compiler/px-eval @ bbc306c) added to pluresdb-px Cargo.toml
- [x] names-only re-export shim added to src/px/mod.rs (both engines compile side-by-side) — as `pub mod pxlang` (namespaced to avoid colliding with the local flat `Px*` AST during migration; becomes the top-level re-export hub in M6.3)
- [x] GATE M6.1: `cargo build -p pluresdb-px` GREEN — result: cold build w/ git fetch of plures/praxis-lang@bbc306c; px-grammar+px-ast+px-compiler+px-eval compiled, pluresdb-px compiled with BOTH engines. Finished dev profile in 1m 08s.
- [x] committed: (see M6.1 commit sha below)

## M6.2 — rewrite AST-facing files against px-ast
- [ ] compiler.rs rewritten (PxDocument→CompiledRecord; §2d adapters; ProcedureBody::Code handled honestly, not dropped)
- [ ] dataflow.rs::ast_to_node rewritten to DataflowProcedureDecl
- [ ] lint.rs + resolver.rs retyped to px-ast (PxStep→px_ast::Step)
- [ ] executor/async_executor/scenario_runner/watcher/compose LEFT UNTOUCHED (verified)
- [ ] GATE M6.2a: `cargo build -p pluresdb-px` GREEN — result: ____
- [ ] GATE M6.2b: `cargo test -p pluresdb-px` GREEN — result: ____
- [ ] GATE M6.2 build-the-binary: ran real .px end-to-end via ____ — output: ____
- [ ] committed: ____

## M6.3 — delete duplicate engine
- [ ] deleted: grammar.pest, builder.rs, PxParser+Px* AST+local parse in mod.rs
- [ ] pest/pest_derive removed from Cargo.toml (or noted why kept): ____
- [ ] GATE M6.3 build+test GREEN with one grammar — result: ____
- [ ] verify `git grep grammar.pest crates/pluresdb-px` → 0 hits: ____
- [ ] GATE M6.3 clippy `-D warnings` (or documented Defender-FP-blocked): ____
- [ ] committed: ____

## Branch HEAD after M6.3: ____
## Honest gaps / deferrals: ____
