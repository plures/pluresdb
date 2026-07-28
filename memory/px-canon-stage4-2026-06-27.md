# TASK-PX-CANON Stage 4 — ONE GRAMMAR + CI CONFORMANCE (2026-06-27)

## Scope completed
Implemented hermetic `.px` grammar conformance corpus + test harness + CI wiring in `pluresdb-px` to enforce C-DRIFT-001.

## Canonical API symbols used (verified from crate)
- `pluresdb_px::px::parse`
- `pluresdb_px::px::compiler::compile`
- `pluresdb_px::px::lint::lint`
- `pluresdb_px::px::lint::LintSeverity`

## Files added
- `.github/workflows/pluresdb-px-conformance.yml`
- `crates/pluresdb-px/tests/conformance.rs`
- `crates/pluresdb-px/tests/conformance_corpus/01_import_fact_rule_constraint.px`
- `crates/pluresdb-px/tests/conformance_corpus/02_entity_contract_trigger.px`
- `crates/pluresdb-px/tests/conformance_corpus/03_dataflow_procedure_code_block.px`
- `crates/pluresdb-px/tests/conformance_corpus/04_legacy_procedure_steps_and_runtime_actions.px`
- `crates/pluresdb-px/tests/conformance_corpus/05_scenario_expectation.px`
- `crates/pluresdb-px/tests/conformance_corpus/06_function_and_config.px`
- `crates/pluresdb-px/tests/conformance_corpus/07_dataflow_plain_steps.px`
- `crates/pluresdb-px/tests/conformance_corpus/08_procedure_composition.px`
- `crates/pluresdb-px/tests/conformance_corpus/invalid/01_missing_colon_in_constraint.px`
- `crates/pluresdb-px/tests/conformance_corpus/invalid/02_invalid_procedure_header.px`
- `crates/pluresdb-px/tests/conformance_corpus/invalid/03_unknown_severity_value.px`
- `crates/pluresdb-px/tests/conformance_corpus/invalid/04_entity_fields_shape_error.px`

## Conformance harness behavior
- Valid corpus (`tests/conformance_corpus/*.px`, excluding `invalid/`):
  - `parse()` must succeed
  - `compile()` must return non-empty `Vec<CompiledRecord>`
  - lint errors fail test, except PX-L011 diagnostics for known runtime-provided actions via shared `RUNTIME_ACTIONS` filter
- Invalid corpus (`tests/conformance_corpus/invalid/*.px`):
  - `parse()` must fail
- Paths are deterministic and hermetic via `env!("CARGO_MANIFEST_DIR")`

## CI wiring
Workflow: `.github/workflows/pluresdb-px-conformance.yml`
- Triggers scoped to crate/workflow paths only:
  - `crates/pluresdb-px/**`
  - `.github/workflows/pluresdb-px-conformance.yml`
- Runs: `cargo test -p pluresdb-px --test conformance --locked`
- Includes workflow concurrency group `pluresdb-px-conformance-${{ github.ref }}`.

## Verification gate results
1. `cargo test -p pluresdb-px --test conformance --quiet`
   - Pass: `2 passed; 0 failed`
2. `cargo test -p pluresdb-px --quiet`
   - Unit/integration summary observed:
     - `running 516 tests` -> `516 passed; 0 failed`
     - plus integration groups: `2 passed`, `2 passed`, and `9 tests` with `4 passed; 5 ignored; 0 failed`
3. `cargo clippy -p pluresdb-px -- -D warnings`
   - Pass: clean, no warnings/errors

## Drift-proof before/after demonstration
- Mutated valid fixture `01_import_fact_rule_constraint.px` by removing `:` from `rule auto_merge:` header.
- Ran conformance test; it FAILED with parse error at that fixture (`expected ... rule_decl ...`).
- Reverted mutation to `rule auto_merge:`.
- Re-ran conformance test; returned GREEN (`2 passed; 0 failed`).
- No mutation left behind.

## Commit
- SHA: `2d8d7868ffa3e7a590e8d26f6575debf85c0194b`
- Message:
  - `test(pluresdb-px): hermetic .px conformance corpus + CI gate so grammar drift fails the build (TASK-PX-CANON Stage 4)`

## Staging hygiene check
Confirmed only requested work was committed. Pre-existing unrelated untracked paths remained unstaged:
- `.px-consolidation/`
- `docs/adr/ADR-0017-praxis-constraint-napi-unification.md`
- (also observed `crates/pluresdb-px/tests/oasis_px_validation.rs` as untracked in this repo state)
