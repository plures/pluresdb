//! Parser unit tests.

use pluresdb_procedures::ir::*;
use pluresdb_procedures::parser::parse_query;

#[test]
fn parse_single_filter_eq_string() {
    let steps = parse_query(r#"filter(category == "decision")"#).unwrap();
    assert_eq!(
        steps[0],
        Step::Filter {
            predicate: Predicate::Comparison {
                field: "category".to_string(),
                cmp: CmpOp::Eq,
                value: IrValue::String("decision".to_string()),
            },
        }
    );
}

#[test]
fn parse_filter_neq() {
    let steps = parse_query(r#"filter(status != "closed")"#).unwrap();
    if let Step::Filter {
        predicate: Predicate::Comparison { cmp, .. },
    } = &steps[0]
    {
        assert_eq!(*cmp, CmpOp::Ne);
    }
}

#[test]
fn parse_filter_gt_float() {
    let steps = parse_query("filter(score > 0.5)").unwrap();
    if let Step::Filter {
        predicate: Predicate::Comparison { cmp, value, .. },
    } = &steps[0]
    {
        assert_eq!(*cmp, CmpOp::Gt);
        assert_eq!(*value, IrValue::Number(0.5));
    }
}

#[test]
fn parse_filter_ge_int() {
    let steps = parse_query("filter(count >= 10)").unwrap();
    if let Step::Filter {
        predicate: Predicate::Comparison { cmp, value, .. },
    } = &steps[0]
    {
        assert_eq!(*cmp, CmpOp::Ge);
        assert_eq!(*value, IrValue::Number(10.0));
    }
}

#[test]
fn parse_filter_lt() {
    let steps = parse_query("filter(priority < 3)").unwrap();
    if let Step::Filter {
        predicate: Predicate::Comparison { cmp, .. },
    } = &steps[0]
    {
        assert_eq!(*cmp, CmpOp::Lt);
    }
}

#[test]
fn parse_filter_le() {
    let steps = parse_query("filter(priority <= 3)").unwrap();
    if let Step::Filter {
        predicate: Predicate::Comparison { cmp, .. },
    } = &steps[0]
    {
        assert_eq!(*cmp, CmpOp::Le);
    }
}

#[test]
fn parse_filter_bool_true() {
    let steps = parse_query("filter(active == true)").unwrap();
    if let Step::Filter {
        predicate: Predicate::Comparison { value, .. },
    } = &steps[0]
    {
        assert_eq!(*value, IrValue::Bool(true));
    }
}

#[test]
fn parse_filter_bool_false() {
    let steps = parse_query("filter(archived == false)").unwrap();
    if let Step::Filter {
        predicate: Predicate::Comparison { value, .. },
    } = &steps[0]
    {
        assert_eq!(*value, IrValue::Bool(false));
    }
}

#[test]
fn parse_filter_null() {
    let steps = parse_query("filter(deleted_at == null)").unwrap();
    if let Step::Filter {
        predicate: Predicate::Comparison { value, .. },
    } = &steps[0]
    {
        assert_eq!(*value, IrValue::Null);
    }
}

#[test]
fn parse_dotted_field_path() {
    let steps = parse_query(r#"filter(data.meta.tag == "vip")"#).unwrap();
    if let Step::Filter {
        predicate: Predicate::Comparison { field, .. },
    } = &steps[0]
    {
        assert_eq!(field, "data.meta.tag");
    }
}

#[test]
fn parse_sort_default_dir() {
    let steps = parse_query(r#"sort(by: "name")"#).unwrap();
    assert_eq!(
        steps[0],
        Step::Sort {
            by: "name".to_string(),
            dir: SortDir::Asc,
            after: None,
        }
    );
}

#[test]
fn parse_sort_desc() {
    let steps = parse_query(r#"sort(by: "score", dir: "desc")"#).unwrap();
    if let Step::Sort { dir, .. } = &steps[0] {
        assert_eq!(*dir, SortDir::Desc);
    }
}

#[test]
fn parse_sort_asc() {
    let steps = parse_query(r#"sort(by: "score", dir: "asc")"#).unwrap();
    if let Step::Sort { dir, .. } = &steps[0] {
        assert_eq!(*dir, SortDir::Asc);
    }
}

#[test]
fn parse_limit_positive() {
    let steps = parse_query("limit(100)").unwrap();
    assert_eq!(steps[0], Step::Limit { n: 100 });
}

#[test]
fn parse_project_multiple_fields() {
    let steps = parse_query(r#"project(["id", "category", "data.text"])"#).unwrap();
    if let Step::Project { fields } = &steps[0] {
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0], FieldSpec::Plain("id".to_string()));
        assert_eq!(fields[1], FieldSpec::Plain("category".to_string()));
        assert_eq!(fields[2], FieldSpec::Plain("data.text".to_string()));
    }
}

#[test]
fn parse_project_empty_array() {
    let steps = parse_query("project([])").unwrap();
    if let Step::Project { fields } = &steps[0] {
        assert!(fields.is_empty());
    }
}

#[test]
fn parse_all_agg_functions() {
    for func_str in &["count", "sum", "avg", "min", "max", "distinct", "collect"] {
        let dsl = format!("aggregate({})", func_str);
        let steps = parse_query(&dsl).unwrap();
        assert!(matches!(steps[0], Step::Aggregate { .. }), "failed for: {}", func_str);
    }
}

#[test]
fn parse_pipe_chain_5_steps() {
    let dsl = r#"filter(status == "open") |> filter(score > 0.3) |> sort(by: "score", dir: "desc") |> limit(5) |> project(["id"])"#;
    let steps = parse_query(dsl).unwrap();
    assert_eq!(steps.len(), 5);
}

#[test]
fn parse_multiline_with_whitespace() {
    let dsl = "filter(category == \"decision\")\n|>\nsort(by: \"score\", dir: \"desc\")\n|>\nlimit(10)";
    let steps = parse_query(dsl).unwrap();
    assert_eq!(steps.len(), 3);
}

#[test]
fn parse_invalid_dsl_returns_error() {
    assert!(parse_query("not_a_valid_query()").is_err());
    assert!(parse_query("filter()").is_err());
    assert!(parse_query("|> limit(5)").is_err());
}
