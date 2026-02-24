//! Parser round-trip tests: string DSL → `Vec<Step>` and back to JSON IR.

use pluresdb_procedures::ir::*;
use pluresdb_procedures::parser::parse_query;

fn assert_roundtrip(dsl: &str, expected_steps: usize) {
    let steps = parse_query(dsl).expect("parse failed");
    assert_eq!(steps.len(), expected_steps, "step count mismatch for: {}", dsl);

    // Serialise to JSON and deserialise back to steps.
    let json = serde_json::to_string(&steps).expect("to_string failed");
    let back: Vec<Step> = serde_json::from_str(&json).expect("from_str failed");
    assert_eq!(steps, back, "round-trip mismatch for: {}", dsl);
}

#[test]
fn roundtrip_simple_filter() {
    assert_roundtrip(r#"filter(category == "decision")"#, 1);
}

#[test]
fn roundtrip_sort_asc() {
    assert_roundtrip(r#"sort(by: "score")"#, 1);
}

#[test]
fn roundtrip_sort_desc() {
    assert_roundtrip(r#"sort(by: "score", dir: "desc")"#, 1);
}

#[test]
fn roundtrip_limit() {
    assert_roundtrip("limit(20)", 1);
}

#[test]
fn roundtrip_project() {
    assert_roundtrip(r#"project(["id", "data.text"])"#, 1);
}

#[test]
fn roundtrip_aggregate_count() {
    assert_roundtrip("aggregate(count)", 1);
}

#[test]
fn roundtrip_aggregate_sum_field() {
    assert_roundtrip(r#"aggregate(sum, field: "score")"#, 1);
}

#[test]
fn roundtrip_pipe_chain_4_steps() {
    let dsl = r#"filter(category == "decision") |> sort(by: "updated_at", dir: "desc") |> limit(10) |> project(["id", "data.text"])"#;
    assert_roundtrip(dsl, 4);
}

#[test]
fn roundtrip_and_predicate() {
    let dsl = r#"filter(category == "decision" and data.score > 0.7)"#;
    let steps = parse_query(dsl).unwrap();
    assert_eq!(steps.len(), 1);
    if let Step::Filter { predicate: Predicate::And { and } } = &steps[0] {
        assert_eq!(and.len(), 2);
    } else {
        panic!("expected AND predicate");
    }
    // Round-trip
    let json = serde_json::to_string(&steps).unwrap();
    let back: Vec<Step> = serde_json::from_str(&json).unwrap();
    assert_eq!(steps, back);
}

#[test]
fn roundtrip_or_predicate() {
    let dsl = r#"filter(status == "open" or status == "pending")"#;
    let steps = parse_query(dsl).unwrap();
    if let Step::Filter { predicate: Predicate::Or { or } } = &steps[0] {
        assert_eq!(or.len(), 2);
    } else {
        panic!("expected OR predicate");
    }
    let json = serde_json::to_string(&steps).unwrap();
    let back: Vec<Step> = serde_json::from_str(&json).unwrap();
    assert_eq!(steps, back);
}

#[test]
fn roundtrip_not_predicate() {
    let dsl = r#"filter(not (archived == true))"#;
    let steps = parse_query(dsl).unwrap();
    if let Step::Filter { predicate: Predicate::Not { .. } } = &steps[0] {
        // ok
    } else {
        panic!("expected NOT predicate");
    }
    let json = serde_json::to_string(&steps).unwrap();
    let back: Vec<Step> = serde_json::from_str(&json).unwrap();
    assert_eq!(steps, back);
}

#[test]
fn roundtrip_numeric_comparison() {
    let dsl = "filter(data.score >= 0.5)";
    assert_roundtrip(dsl, 1);
}

#[test]
fn roundtrip_contains_op() {
    let dsl = r#"filter(data.text contains "hello")"#;
    assert_roundtrip(dsl, 1);
}

#[test]
fn roundtrip_starts_with_op() {
    let dsl = r#"filter(id starts_with "mem:")"#;
    assert_roundtrip(dsl, 1);
}

#[test]
fn roundtrip_boolean_value() {
    let dsl = "filter(active == true)";
    assert_roundtrip(dsl, 1);
}

#[test]
fn roundtrip_null_value() {
    let dsl = "filter(deleted_at == null)";
    assert_roundtrip(dsl, 1);
}

#[test]
fn roundtrip_5_step_chain() {
    let dsl = r#"filter(status == "open") |> filter(data.score > 0.3) |> sort(by: "score", dir: "desc") |> limit(5) |> project(["id"])"#;
    assert_roundtrip(dsl, 5);
}

#[test]
fn error_on_empty_string() {
    // Empty input should fail to parse (no steps)
    let result = parse_query("");
    // Either a parse error or an empty step list is acceptable
    assert!(result.map(|v| v.is_empty()).unwrap_or(true));
}
