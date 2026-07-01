use pluresdb_px::px::compiler::compile;
use pluresdb_px::px::lint::{lint, LintSeverity};
// M6: parse via px-compiler (SSOT) so `compile`/`lint` receive a px_ast doc.
use pluresdb_px::px::pxlang::parse;
use std::fs;
use std::path::{Path, PathBuf};

/// Actions provided by runtime handlers (not statically visible to the linter).
/// Reused from oasis_px_validation.rs to suppress known PX-L011 false positives.
const RUNTIME_ACTIONS: &[&str] = &[
    "write_state",
    "read_state",
    "generate_id",
    "timestamp_now",
    "collect_stage_outputs",
    "update_stage_status",
    "find_next_stage",
    "get_default_stages",
    "merge_stage_config",
    "get_stage",
    "format_stage_brief",
    "build_oasis_stages",
    "draft_design_spec",
    "write_documentation",
];

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("conformance_corpus")
}

fn list_px_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .expect("failed to read corpus directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("px"))
        .collect();
    files.sort();
    files
}

#[test]
fn conformance_valid_px_parse_compile_and_lint() {
    let root = corpus_root();
    let valid_files = list_px_files(&root);
    assert!(
        !valid_files.is_empty(),
        "expected at least one valid conformance fixture"
    );

    for file in valid_files {
        let src = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("{}: failed to read fixture: {e}", file.display()));

        let doc =
            parse(&src).unwrap_or_else(|e| panic!("{}: parse should succeed: {e}", file.display()));

        let records = compile(&doc);
        assert!(
            !records.is_empty(),
            "{}: compile() produced no records",
            file.display()
        );

        let diags = lint(&doc);
        let errors: Vec<_> = diags
            .iter()
            .filter(|d| matches!(d.severity, LintSeverity::Error))
            .filter(|d| {
                !(d.code == "PX-L011"
                    && RUNTIME_ACTIONS
                        .iter()
                        .any(|action| d.message.contains(action)))
            })
            .collect();

        assert!(
            errors.is_empty(),
            "{}: lint error diagnostics:\n{}",
            file.display(),
            errors
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

#[test]
fn conformance_invalid_px_must_fail_parse() {
    let invalid_dir = corpus_root().join("invalid");
    let invalid_files = list_px_files(&invalid_dir);
    assert!(
        !invalid_files.is_empty(),
        "expected at least one invalid conformance fixture"
    );

    for file in invalid_files {
        let src = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("{}: failed to read fixture: {e}", file.display()));
        let parsed = parse(&src);
        assert!(
            parsed.is_err(),
            "{}: expected parse to fail but succeeded",
            file.display()
        );
    }
}
