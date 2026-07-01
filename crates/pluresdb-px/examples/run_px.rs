//! End-to-end runner example: parse a real `.px` file with the imported
//! praxis-lang engine (px-compiler), compile it to executor records with the
//! rewired `compiler`, and actually execute the procedure with the `executor`.
//!
//! This is the M6.2 "build the binary, run the binary" gate: it exercises the
//! full parse → compile → execute pipeline on a real file, not a test fixture.
//!
//! Run: `cargo run -p pluresdb-px --example run_px -- crates/pluresdb-px/examples/pipeline.px`
//! (defaults to examples/pipeline.px when no path is given).

use std::collections::HashMap;

use pluresdb_px::px::compiler::compile;
use pluresdb_px::px::executor::{execute, ActionHandler, ExecutionError};
use pluresdb_px::px::pxlang::parse;
use serde_json::{json, Value};

/// A small but real action handler for the demo procedure. Each action computes
/// its result from the inputs (no canned pass-through standing in for logic).
struct DemoHandler;

impl ActionHandler for DemoHandler {
    fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError> {
        match name {
            // Produce a concrete list to iterate over.
            "get_items" => Ok(json!(["alpha", "beta", "gamma"])),
            // Transform reads the loop item and returns an uppercased echo.
            "transform" => {
                let val = params
                    .get("val")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                Ok(json!(val.to_uppercase()))
            }
            other => Err(ExecutionError::UnknownAction(other.to_string())),
        }
    }
}

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "crates/pluresdb-px/examples/pipeline.px".to_string());

    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));

    // 1. Parse with the imported praxis-lang parser (SSOT).
    let doc = parse(&source).unwrap_or_else(|e| panic!("parse failed: {e}"));

    // 2. Compile to executor records with the rewired compiler.
    let records = compile(&doc);
    println!("compiled {} record(s):", records.len());
    for rec in &records {
        let ty = rec.data.get("type").and_then(|v| v.as_str()).unwrap_or("?");
        println!("  - {} ({})", rec.key, ty);
    }

    // 3. Execute every compiled procedure record end-to-end.
    let mut ran = 0usize;
    for rec in &records {
        let is_proc = rec.data.get("type").and_then(|v| v.as_str()) == Some("procedure");
        if !is_proc {
            continue;
        }
        let result = execute(&rec.data, &DemoHandler)
            .unwrap_or_else(|e| panic!("execute failed for {}: {e}", rec.key));
        ran += 1;
        println!(
            "\nexecuted `{}`: success={} steps={}",
            result.procedure_name,
            result.success,
            result.step_results.len()
        );
        if let Some(results) = result.variables.get("results") {
            println!("  $results = {results}");
        }
        if let Some(emit) = result.variables.get("emit") {
            println!("  emitted  = {emit}");
        }
        assert!(result.success, "procedure {} did not succeed", rec.key);
        // The loop transformed 3 items to uppercase; assert the real output.
        if result.procedure_name == "pipeline" {
            let expected = json!(["ALPHA", "BETA", "GAMMA"]);
            assert_eq!(
                result.variables.get("results"),
                Some(&expected),
                "pipeline results mismatch"
            );
        }
    }

    // Guard: prove we actually ran a procedure (not a silently-empty pass).
    assert!(ran > 0, "no procedure records were executed");
    let _ = HashMap::<String, Value>::new(); // execute_with_vars is also available
    println!("\nOK: parsed, compiled, and executed {ran} procedure(s) from {path}");
}
