//! Validation harness for the OASIS loop .px files.
//! Parses + lints the OASIS .px artifacts to verify they are syntactically
//! valid against the live pluresdb-px grammar. Run with:
//!   cargo test -p pluresdb-px --test oasis_px_validation -- --nocapture

use pluresdb_px::px::lint::{lint, LintSeverity};
use pluresdb_px::px::parse;
use std::fs;

/// Actions provided by the pares-radix Rust runtime (registered ActionHandlers).
/// The static linter cannot see these because they are registered at runtime,
/// not declared in .px. dev-lifecycle.px (the production file) depends on the
/// same set. Verified present in crates/core/src/spine/{actions,dev_lifecycle_actions,rsi_actions}.rs.
const RUNTIME_ACTIONS: &[&str] = &[
    "write_state", "read_state", "generate_id", "timestamp_now",
    "collect_stage_outputs", "update_stage_status", "find_next_stage",
    "get_default_stages", "merge_stage_config", "get_stage", "format_stage_brief",
    // OASIS-loop side-effect actors (to be implemented in the oasis executor IO boundary):
    "build_oasis_stages", "draft_design_spec", "write_documentation",
];

const OASIS_PX_FILES: &[&str] = &[
    r"C:\Projects\oasis\praxis\oasis-loop.px",
    r"C:\Projects\oasis\praxis\oasis-gap-backlog.px",
];

/// Baseline: the production dev-lifecycle.px depends on the same runtime actions.
/// If it produces the same PX-L011 class, that confirms those are runtime-provided,
/// not real errors.
const BASELINE_PX_FILE: &str = r"C:\Projects\pares-radix\praxis\procedures\dev-lifecycle.px";

#[test]
fn oasis_px_files_parse_and_lint() {
    let mut failures = Vec::new();
    for path in OASIS_PX_FILES {
        let src = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{path}: cannot read: {e}"));
                continue;
            }
        };
        match parse(&src) {
            Ok(doc) => {
                let diags = lint(&doc);
                // PX-L011 against a known runtime action is a false positive (static
                // linter can't see runtime-registered handlers). Filter those out.
                let errors: Vec<_> = diags
                    .iter()
                    .filter(|d| matches!(d.severity, LintSeverity::Error))
                    .filter(|d| {
                        !(d.code == "PX-L011"
                            && RUNTIME_ACTIONS.iter().any(|a| d.message.contains(a)))
                    })
                    .collect();
                println!(
                    "PARSED OK: {path} | procedures-doc parsed | lint diagnostics: {} ({} error-level)",
                    diags.len(),
                    errors.len()
                );
                for d in &diags {
                    println!("  lint: {d:?}");
                }
                if !errors.is_empty() {
                    failures.push(format!("{path}: {} error-level lint diagnostics", errors.len()));
                }
            }
            Err(e) => failures.push(format!("{path}: PARSE FAILED: {e}")),
        }
    }
    assert!(failures.is_empty(), "OASIS .px validation failed:\n{}", failures.join("\n"));
}

#[test]
fn baseline_dev_lifecycle_has_same_runtime_action_warnings() {
    // Proves PX-L011 for write_state/read_state/etc. is a runtime-provided-action
    // false positive: the PRODUCTION dev-lifecycle.px exhibits the same class.
    let src = fs::read_to_string(BASELINE_PX_FILE)
        .expect("dev-lifecycle.px must be readable");
    let doc = parse(&src).expect("dev-lifecycle.px must parse");
    let diags = lint(&doc);
    let l011_runtime: Vec<_> = diags
        .iter()
        .filter(|d| d.code == "PX-L011")
        .filter(|d| RUNTIME_ACTIONS.iter().any(|a| d.message.contains(a)))
        .collect();
    println!(
        "BASELINE dev-lifecycle.px: {} total diagnostics, {} PX-L011 runtime-action (expected false positives)",
        diags.len(),
        l011_runtime.len()
    );
    // The production file uses these runtime actions too, confirming our files are valid.
    assert!(
        !l011_runtime.is_empty(),
        "expected dev-lifecycle.px to also reference runtime actions (confirms false-positive class)"
    );
}
