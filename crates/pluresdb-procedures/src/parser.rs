//! PEG parser: converts a DSL string into a `Vec<Step>` (the JSON IR).
//!
//! Uses the `pest` crate with the grammar defined in `query.pest`.

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::ir::{AggFn, CmpOp, FieldSpec, IrValue, MutateOp, Predicate, SortDir, Step};

#[derive(Parser)]
#[grammar = "query.pest"]
struct QueryParser;

/// Parse errors returned by [`parse_query`].
#[derive(Debug, thiserror::Error)]
#[error("parse error: {0}")]
pub struct ParseError(#[from] pest::error::Error<Rule>);

/// Parse a DSL query string into a sequence of [`Step`]s.
///
/// # Example
///
/// ```
/// use pluresdb_procedures::parser::parse_query;
///
/// let steps = parse_query(r#"filter(category == "decision") |> sort(by: "updated_at", dir: "desc") |> limit(10)"#).unwrap();
/// assert_eq!(steps.len(), 3);
/// ```
pub fn parse_query(input: &str) -> Result<Vec<Step>, ParseError> {
    let pairs = QueryParser::parse(Rule::query, input)?;
    let mut steps = Vec::new();
    for pair in pairs {
        if pair.as_rule() == Rule::query {
            for inner in pair.into_inner() {
                if inner.as_rule() == Rule::step {
                    steps.push(parse_step(inner)?);
                }
            }
        }
    }
    Ok(steps)
}

fn parse_step(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let inner = pair.into_inner().next().expect("step has one child");
    match inner.as_rule() {
        Rule::filter_step => parse_filter(inner),
        Rule::sort_step => parse_sort(inner),
        Rule::limit_step => parse_limit(inner),
        Rule::project_step => parse_project(inner),
        Rule::mutate_step => parse_mutate(inner),
        Rule::aggregate_step => parse_aggregate(inner),
        r => unreachable!("unexpected rule: {:?}", r),
    }
}

// ---- filter ----

fn parse_filter(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let pred = pair
        .into_inner()
        .next()
        .expect("filter has predicate");
    Ok(Step::Filter {
        predicate: parse_predicate(pred)?,
    })
}

fn parse_predicate(pair: Pair<Rule>) -> Result<Predicate, ParseError> {
    // predicate → or_pred
    let inner = pair.into_inner().next().expect("predicate → or_pred");
    parse_or_pred(inner)
}

fn parse_or_pred(pair: Pair<Rule>) -> Result<Predicate, ParseError> {
    let mut children = pair.into_inner().peekable();
    // Collect: and_pred (or_kw and_pred)*
    let first = parse_and_pred(children.next().expect("first and_pred"))?;
    let mut rest = Vec::new();
    while let Some(p) = children.next() {
        // p is an or_kw; skip it and get the next and_pred
        if p.as_rule() == Rule::or_kw {
            if let Some(next) = children.next() {
                rest.push(parse_and_pred(next)?);
            }
        }
    }
    if rest.is_empty() {
        Ok(first)
    } else {
        let mut all = vec![first];
        all.extend(rest);
        Ok(Predicate::Or { or: all })
    }
}

fn parse_and_pred(pair: Pair<Rule>) -> Result<Predicate, ParseError> {
    let mut children = pair.into_inner().peekable();
    let first = parse_not_pred(children.next().expect("first not_pred"))?;
    let mut rest = Vec::new();
    while let Some(p) = children.next() {
        if p.as_rule() == Rule::and_kw {
            if let Some(next) = children.next() {
                rest.push(parse_not_pred(next)?);
            }
        }
    }
    if rest.is_empty() {
        Ok(first)
    } else {
        let mut all = vec![first];
        all.extend(rest);
        Ok(Predicate::And { and: all })
    }
}

fn parse_not_pred(pair: Pair<Rule>) -> Result<Predicate, ParseError> {
    let mut inner = pair.into_inner();
    let first = inner.next().expect("not_pred child");
    match first.as_rule() {
        Rule::not_kw => {
            // "not" keyword followed by atom_pred
            let atom = inner.next().expect("atom after not_kw");
            Ok(Predicate::not(parse_atom_pred(atom)?))
        }
        Rule::atom_pred => parse_atom_pred(first),
        r => unreachable!("unexpected not_pred child: {:?}", r),
    }
}

fn parse_atom_pred(pair: Pair<Rule>) -> Result<Predicate, ParseError> {
    let inner = pair.into_inner().next().expect("atom_pred child");
    match inner.as_rule() {
        Rule::predicate => parse_predicate(inner),
        Rule::comparison => parse_comparison(inner),
        r => unreachable!("unexpected atom_pred child: {:?}", r),
    }
}

fn parse_comparison(pair: Pair<Rule>) -> Result<Predicate, ParseError> {
    let mut inner = pair.into_inner();
    let field = inner.next().expect("field_path").as_str().to_string();
    let cmp = parse_cmp_op(inner.next().expect("cmp_op"))?;
    let value = parse_value(inner.next().expect("value"))?;
    Ok(Predicate::Comparison { field, cmp, value })
}

fn parse_cmp_op(pair: Pair<Rule>) -> Result<CmpOp, ParseError> {
    Ok(match pair.as_str() {
        "==" => CmpOp::Eq,
        "!=" => CmpOp::Ne,
        ">" => CmpOp::Gt,
        ">=" => CmpOp::Ge,
        "<" => CmpOp::Lt,
        "<=" => CmpOp::Le,
        "contains" => CmpOp::Contains,
        "starts_with" => CmpOp::StartsWith,
        "matches" => CmpOp::Matches,
        other => {
            return Err(ParseError(pest::error::Error::new_from_span(
                pest::error::ErrorVariant::CustomError {
                    message: format!("unknown operator: {}", other),
                },
                pair.as_span(),
            )))
        }
    })
}

fn parse_value(pair: Pair<Rule>) -> Result<IrValue, ParseError> {
    let inner = pair.into_inner().next().expect("value child");
    Ok(match inner.as_rule() {
        Rule::string => {
            let s = inner.as_str();
            // Strip surrounding quotes
            IrValue::String(s[1..s.len() - 1].to_string())
        }
        Rule::float => {
            let n: f64 = inner.as_str().parse().map_err(|_| {
                ParseError(pest::error::Error::new_from_span(
                    pest::error::ErrorVariant::CustomError {
                        message: "invalid float".to_string(),
                    },
                    inner.as_span(),
                ))
            })?;
            IrValue::Number(n)
        }
        Rule::integer => {
            let n: i64 = inner.as_str().parse().map_err(|_| {
                ParseError(pest::error::Error::new_from_span(
                    pest::error::ErrorVariant::CustomError {
                        message: "invalid integer".to_string(),
                    },
                    inner.as_span(),
                ))
            })?;
            IrValue::Number(n as f64)
        }
        Rule::bool_true => IrValue::Bool(true),
        Rule::bool_false => IrValue::Bool(false),
        Rule::null_val => IrValue::Null,
        r => unreachable!("unexpected value child: {:?}", r),
    })
}

// ---- sort ----

fn parse_sort(pair: Pair<Rule>) -> Result<Step, ParseError> {
    // sort_step = { "sort" ~ "(" ~ sort_by_kv ~ ("," ~ sort_dir_kv)? ~ ("," ~ sort_after_kv)? ~ ")" }
    let mut inner = pair.into_inner();

    // First child: sort_by_kv = { "by" ~ ":" ~ string }
    let by_kv = inner.next().expect("sort_by_kv");
    let by_raw = by_kv.into_inner().next().expect("sort by string").as_str();
    let by = by_raw[1..by_raw.len() - 1].to_string();

    let mut dir = SortDir::default();
    let mut after: Option<String> = None;

    for kv in inner {
        match kv.as_rule() {
            Rule::sort_dir_kv => {
                let dir_pair = kv.into_inner().next().expect("dir string");
                let val_raw = dir_pair.as_str();
                let val = &val_raw[1..val_raw.len() - 1];
                dir = match val {
                    "asc" => SortDir::Asc,
                    "desc" => SortDir::Desc,
                    other => {
                        return Err(ParseError(pest::error::Error::new_from_span(
                            pest::error::ErrorVariant::CustomError {
                                message: format!("unknown sort direction: {}", other),
                            },
                            dir_pair.as_span(),
                        )))
                    }
                };
            }
            Rule::sort_after_kv => {
                let val_raw = kv.into_inner().next().expect("after string").as_str().to_string();
                after = Some(val_raw[1..val_raw.len() - 1].to_string());
            }
            _ => {}
        }
    }

    Ok(Step::Sort { by, dir, after })
}

// ---- limit ----

fn parse_limit(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let span = pair.as_span();
    let n_str = pair.into_inner().next().expect("limit integer").as_str();
    let n: usize = n_str.parse().map_err(|_| {
        ParseError(pest::error::Error::new_from_span(
            pest::error::ErrorVariant::CustomError {
                message: "invalid limit value".to_string(),
            },
            span,
        ))
    })?;
    Ok(Step::Limit { n })
}

// ---- project ----

fn parse_project(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let arr = pair.into_inner().next().expect("field_array");
    let fields: Vec<FieldSpec> = arr
        .into_inner()
        .map(|p| {
            let s = p.as_str();
            FieldSpec::Plain(s[1..s.len() - 1].to_string())
        })
        .collect();
    Ok(Step::Project { fields })
}

// ---- mutate ----

fn parse_mutate(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let mut inner = pair.into_inner();
    let op_pair = inner.next().expect("mutate_op");
    let op_name = op_pair.as_str();

    // Collect key-value pairs that follow
    let mut kvs: std::collections::HashMap<String, IrValue> = std::collections::HashMap::new();
    for kv_pair in inner {
        if kv_pair.as_rule() == Rule::mutate_kv {
            let mut kv_inner = kv_pair.into_inner();
            let key = kv_inner.next().expect("key").as_str().to_string();
            let val = parse_value(kv_inner.next().expect("val"))?;
            kvs.insert(key, val);
        }
    }

    let get_str = |key: &str| -> String {
        kvs.get(key)
            .and_then(|v| {
                if let IrValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    };

    let op = match op_name {
        "put" => MutateOp::Put {
            id: get_str("id"),
            data: kvs
                .get("data")
                .map(|v| v.to_json())
                .unwrap_or(serde_json::Value::Null),
        },
        "delete" => MutateOp::Delete { id: get_str("id") },
        "merge" => MutateOp::Merge {
            id: get_str("id"),
            patch: kvs
                .get("patch")
                .map(|v| v.to_json())
                .unwrap_or(serde_json::Value::Null),
        },
        "put_edge" => MutateOp::PutEdge {
            from: get_str("from"),
            to: get_str("to"),
            label: kvs.get("label").and_then(|v| {
                if let IrValue::String(s) = v {
                    Some(s.clone())
                } else {
                    None
                }
            }),
        },
        "delete_edge" => MutateOp::DeleteEdge {
            from: get_str("from"),
            to: get_str("to"),
        },
        other => {
            return Err(ParseError(pest::error::Error::new_from_span(
                pest::error::ErrorVariant::CustomError {
                    message: format!("unknown mutate op: {}", other),
                },
                op_pair.as_span(),
            )))
        }
    };

    Ok(Step::Mutate {
        ops: vec![op],
        atomic: false,
    })
}

// ---- aggregate ----

fn parse_aggregate(pair: Pair<Rule>) -> Result<Step, ParseError> {
    // aggregate_step = { "aggregate" ~ "(" ~ agg_fn ~ ("," ~ agg_field_kv)? ~ ")" }
    // agg_field_kv   = { "field" ~ ":" ~ string }
    let mut inner = pair.into_inner();
    let fn_pair = inner.next().expect("agg_fn");
    let func = match fn_pair.as_str() {
        "count" => AggFn::Count,
        "sum" => AggFn::Sum,
        "avg" => AggFn::Avg,
        "min" => AggFn::Min,
        "max" => AggFn::Max,
        "distinct" => AggFn::Distinct,
        "collect" => AggFn::Collect,
        other => {
            return Err(ParseError(pest::error::Error::new_from_span(
                pest::error::ErrorVariant::CustomError {
                    message: format!("unknown aggregate function: {}", other),
                },
                fn_pair.as_span(),
            )))
        }
    };

    let mut field: Option<String> = None;
    if let Some(kv) = inner.next() {
        if kv.as_rule() == Rule::agg_field_kv {
            let val_raw = kv.into_inner().next().expect("field string").as_str();
            field = Some(val_raw[1..val_raw.len() - 1].to_string());
        }
    }

    Ok(Step::Aggregate { func, field })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;

    #[test]
    fn parse_simple_filter() {
        let steps = parse_query(r#"filter(category == "decision")"#).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(
            steps[0],
            Step::Filter {
                predicate: Predicate::Comparison {
                    field: "category".to_string(),
                    cmp: CmpOp::Eq,
                    value: IrValue::String("decision".to_string()),
                }
            }
        );
    }

    #[test]
    fn parse_sort_desc() {
        let steps = parse_query(r#"sort(by: "updated_at", dir: "desc")"#).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(
            steps[0],
            Step::Sort {
                by: "updated_at".to_string(),
                dir: SortDir::Desc,
                after: None,
            }
        );
    }

    #[test]
    fn parse_limit() {
        let steps = parse_query("limit(10)").unwrap();
        assert_eq!(steps[0], Step::Limit { n: 10 });
    }

    #[test]
    fn parse_project() {
        let steps = parse_query(r#"project(["id", "data.text"])"#).unwrap();
        assert_eq!(
            steps[0],
            Step::Project {
                fields: vec![
                    FieldSpec::Plain("id".to_string()),
                    FieldSpec::Plain("data.text".to_string()),
                ]
            }
        );
    }

    #[test]
    fn parse_pipe_chain() {
        let input = r#"filter(category == "decision") |> sort(by: "updated_at", dir: "desc") |> limit(10)"#;
        let steps = parse_query(input).unwrap();
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn parse_and_predicate() {
        let input = r#"filter(category == "decision" and data.score > 0.7)"#;
        let steps = parse_query(input).unwrap();
        assert_eq!(steps.len(), 1);
        if let Step::Filter { predicate } = &steps[0] {
            assert!(matches!(predicate, Predicate::And { .. }));
        } else {
            panic!("expected filter step");
        }
    }

    #[test]
    fn parse_or_predicate() {
        let input = r#"filter(status == "open" or status == "pending")"#;
        let steps = parse_query(input).unwrap();
        if let Step::Filter { predicate } = &steps[0] {
            assert!(matches!(predicate, Predicate::Or { .. }));
        }
    }

    #[test]
    fn parse_not_predicate() {
        let input = r#"filter(not (archived == true))"#;
        let steps = parse_query(input).unwrap();
        if let Step::Filter { predicate } = &steps[0] {
            assert!(matches!(predicate, Predicate::Not { .. }));
        }
    }

    #[test]
    fn parse_numeric_comparison() {
        let input = "filter(data.score >= 0.5)";
        let steps = parse_query(input).unwrap();
        if let Step::Filter {
            predicate:
                Predicate::Comparison {
                    field, cmp, value, ..
                },
        } = &steps[0]
        {
            assert_eq!(field, "data.score");
            assert_eq!(*cmp, CmpOp::Ge);
            assert_eq!(*value, IrValue::Number(0.5));
        }
    }

    #[test]
    fn parse_aggregate_count() {
        let input = "aggregate(count)";
        let steps = parse_query(input).unwrap();
        assert_eq!(steps[0], Step::Aggregate { func: AggFn::Count, field: None });
    }

    #[test]
    fn parse_aggregate_sum_field() {
        let input = r#"aggregate(sum, field: "data.score")"#;
        let steps = parse_query(input).unwrap();
        assert_eq!(
            steps[0],
            Step::Aggregate {
                func: AggFn::Sum,
                field: Some("data.score".to_string()),
            }
        );
    }

    #[test]
    fn parse_error_on_invalid() {
        assert!(parse_query("not_a_step(x)").is_err());
    }

    #[test]
    fn parse_contains_op() {
        let input = r#"filter(data.text contains "hello")"#;
        let steps = parse_query(input).unwrap();
        if let Step::Filter {
            predicate: Predicate::Comparison { cmp, .. },
        } = &steps[0]
        {
            assert_eq!(*cmp, CmpOp::Contains);
        }
    }
}
