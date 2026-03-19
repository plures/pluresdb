//! PEG parser: converts a DSL string into a `Vec<Step>` (the JSON IR).
//!
//! Uses the `pest` crate with the grammar defined in `query.pest`.

use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use pest::error::{Error as PestError, ErrorVariant};

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
        Rule::graph_clusters_step => parse_graph_clusters(inner),
        Rule::graph_path_step => parse_graph_path(inner),
        Rule::graph_pagerank_step => parse_graph_pagerank(inner),
        Rule::graph_stats_step => Ok(Step::GraphStats),
        Rule::graph_neighbors_step => parse_graph_neighbors(inner),
        Rule::graph_links_step => parse_graph_links(inner),
        Rule::auto_link_step => parse_auto_link(inner),
        Rule::vector_search_step => parse_vector_search(inner),
        Rule::text_search_step => parse_text_search(inner),
        Rule::transform_step => parse_transform(inner),
        Rule::conditional_step => parse_conditional(inner),
        Rule::assign_step => parse_assign(inner),
        Rule::emit_step => parse_emit(inner),
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

fn unescape_string_content(s: &str) -> Result<String, String> {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }

        let escaped = match chars.next() {
            Some(ch) => ch,
            None => {
                return Err("incomplete escape sequence at end of string".to_string());
            }
        };

        match escaped {
            '\\' => result.push('\\'),
            '"' => result.push('"'),
            'n' => result.push('\n'),
            'r' => result.push('\r'),
            't' => result.push('\t'),
            'b' => result.push('\u{0008}'),
            'f' => result.push('\u{000C}'),
            'u' => {
                let mut hex = String::with_capacity(4);
                for _ in 0..4 {
                    match chars.next() {
                        Some(h) if h.is_ascii_hexdigit() => hex.push(h),
                        _ => {
                            return Err("invalid unicode escape sequence".to_string());
                        }
                    }
                }
                let code_point = u32::from_str_radix(&hex, 16)
                    .map_err(|_| "invalid unicode escape sequence".to_string())?;
                match std::char::from_u32(code_point) {
                    Some(ch) => result.push(ch),
                    None => {
                        return Err("invalid unicode scalar value in escape sequence".to_string());
                    }
                }
            }
            other => {
                // For any other escaped character, just include it literally.
                result.push(other);
            }
        }
    }

    Ok(result)
}

fn parse_value(pair: Pair<Rule>) -> Result<IrValue, ParseError> {
    let inner = pair.into_inner().next().expect("value child");
    Ok(match inner.as_rule() {
        Rule::string => {
            let s = inner.as_str();
            // Strip surrounding quotes and unescape escape sequences inside.
            let content = &s[1..s.len() - 1];
            let unescaped = unescape_string_content(content).map_err(|msg| {
                ParseError(pest::error::Error::new_from_span(
                    pest::error::ErrorVariant::CustomError { message: msg },
                    inner.as_span(),
                ))
            })?;
            IrValue::String(unescaped)
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
    // Grammar guarantees pos_integer (ASCII digits only), so parse cannot fail
    // for values that fit in usize.  We keep the error path to handle overflow.
    let n_str = pair.into_inner().next().expect("limit integer").as_str();
    let n: usize = n_str.parse().map_err(|_| {
        ParseError(pest::error::Error::new_from_span(
            pest::error::ErrorVariant::CustomError {
                message: format!("limit value '{}' is too large (overflow)", n_str),
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

// ---- graph_clusters ----

fn parse_graph_clusters(pair: Pair<Rule>) -> Result<Step, ParseError> {
    // graph_clusters_step → graph_clusters_params
    //   graph_algorithm_kv, graph_min_size_kv?, graph_min_strength_kv?
    let params = pair.into_inner().next().expect("graph_clusters_params");
    let mut algorithm = "louvain".to_string();
    let mut min_size: Option<usize> = None;
    let mut min_strength: Option<f64> = None;

    for kv in params.into_inner() {
        match kv.as_rule() {
            Rule::graph_algorithm_kv => {
                let string_pair = kv.into_inner().next().expect("algorithm string");
                let raw = string_pair.as_str();
                let content = &raw[1..raw.len() - 1];
                algorithm = unescape_string_content(content).map_err(|msg| {
                    ParseError(pest::error::Error::new_from_span(
                        pest::error::ErrorVariant::CustomError { message: msg },
                        string_pair.as_span(),
                    ))
                })?;
            }
            Rule::graph_min_size_kv => {
                let int_pair = kv.into_inner().next().expect("min_size integer");
                let raw = int_pair.as_str();
                min_size = Some(raw.parse::<usize>().map_err(|_| {
                    ParseError(pest::error::Error::new_from_span(
                        pest::error::ErrorVariant::CustomError {
                            message: format!("min_size value '{}' is too large (overflow)", raw),
                        },
                        int_pair.as_span(),
                    ))
                })?);
            }
            Rule::graph_min_strength_kv => {
                let float_pair = kv.into_inner().next().expect("min_strength float");
                let raw = float_pair.as_str();
                min_strength = Some(raw.parse::<f64>().map_err(|_| {
                    ParseError(pest::error::Error::new_from_span(
                        pest::error::ErrorVariant::CustomError {
                            message: format!("invalid min_strength value '{}'", raw),
                        },
                        float_pair.as_span(),
                    ))
                })?);
            }
            _ => {}
        }
    }

    Ok(Step::GraphClusters { algorithm, min_size, min_strength })
}

// ---- graph_path ----

fn parse_graph_path(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let params = pair.into_inner().next().expect("graph_path_params");
    let mut from = String::new();
    let mut to = String::new();
    let mut max_hops: Option<usize> = None;

    for kv in params.into_inner() {
        match kv.as_rule() {
            Rule::graph_from_kv => {
                let string_pair = kv.into_inner().next().expect("from string");
                let raw = string_pair.as_str();
                let content = &raw[1..raw.len() - 1];
                from = unescape_string_content(content).map_err(|msg| {
                    ParseError(pest::error::Error::new_from_span(
                        pest::error::ErrorVariant::CustomError { message: msg },
                        string_pair.as_span(),
                    ))
                })?;
            }
            Rule::graph_to_kv => {
                let string_pair = kv.into_inner().next().expect("to string");
                let raw = string_pair.as_str();
                let content = &raw[1..raw.len() - 1];
                to = unescape_string_content(content).map_err(|msg| {
                    ParseError(pest::error::Error::new_from_span(
                        pest::error::ErrorVariant::CustomError { message: msg },
                        string_pair.as_span(),
                    ))
                })?;
            }
            Rule::graph_max_hops_kv => {
                let raw = kv.into_inner().next().expect("max_hops integer").as_str();
                max_hops = raw.parse().ok();
            }
            _ => {}
        }
    }

    Ok(Step::GraphPath { from, to, max_hops })
}

// ---- graph_pagerank ----

fn parse_graph_pagerank(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let mut damping: Option<f64> = None;
    let mut iterations: Option<usize> = None;

    // params are optional (the whole group may be absent)
    if let Some(params) = pair.into_inner().next() {
        for kv in params.into_inner() {
            match kv.as_rule() {
                Rule::graph_dampening_kv => {
                    let raw = kv.into_inner().next().expect("damping float").as_str();
                    damping = raw.parse().ok();
                }
                Rule::graph_iterations_kv => {
                    let raw = kv.into_inner().next().expect("iterations integer").as_str();
                    iterations = raw.parse().ok();
                }
                _ => {}
            }
        }
    }

    Ok(Step::GraphPagerank { damping, iterations })
}
// ---- graph_neighbors ----

fn parse_graph_neighbors(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let mut inner = pair.into_inner();

    // First child is always the root string.  Use the full unescape pipeline so
    // that IDs like `"memory:abc\u0031"` are decoded correctly.
    let root_pair = inner.next().expect("root string");
    let root_raw = root_pair.as_str();
    let root_content = &root_raw[1..root_raw.len() - 1];
    let root = unescape_string_content(root_content).map_err(|msg| {
        ParseError(pest::error::Error::new_from_span(
            pest::error::ErrorVariant::CustomError { message: msg },
            root_pair.as_span(),
        ))
    })?;

    let mut depth: usize = 1;
    let mut min_strength: Option<f64> = None;
    let mut link_type: Option<String> = None;
    let mut bidirectional = false;

    for kv in inner {
        // kv is a graph_step_kv: ident ":" value
        let mut kv_inner = kv.into_inner();
        let key_pair = kv_inner.next().expect("key ident");
        let key = key_pair.as_str();
        let val = parse_value(kv_inner.next().expect("value"))?;
        match key {
            "depth" => {
                if let IrValue::Number(n) = val {
                    // Reject non-integer or negative values to prevent silent
                    // truncation or wrapping to a huge usize.
                    if n < 0.0 || n.fract() != 0.0 || n > usize::MAX as f64 {
                        return Err(ParseError(pest::error::Error::new_from_span(
                            pest::error::ErrorVariant::CustomError {
                                message: format!(
                                    "depth must be a non-negative integer, got {}",
                                    n
                                ),
                            },
                            key_pair.as_span(),
                        )));
                    }
                    // Avoid casting negative or excessively large floating point values
                    // directly to usize, which can wrap or overflow. Treat non-positive
                    // depths as 0, clamp very large values, and explicitly floor
                    // fractional depths before conversion.
                    if n <= 0.0 {
                        depth = 0;
                    } else if n >= (usize::MAX as f64) {
                        depth = usize::MAX;
                    } else {
                        depth = n.floor() as usize;
                    }
                }
            }
            "min_strength" => {
                if let IrValue::Number(n) = val {
                    min_strength = Some(n);
                }
            }
            "type" => {
                if let IrValue::String(s) = val {
                    link_type = Some(s);
                }
            }
            "bidirectional" => {
                if let IrValue::Bool(b) = val {
                    bidirectional = b;
                }
            }
            _ => {
                // Unknown keys are silently ignored, consistent with the mutate
                // step parser, to allow forward compatibility with future params.
            }
        }
    }

    Ok(Step::GraphNeighbors { root, depth, min_strength, link_type, bidirectional })
}

// ---- graph_links ----

fn parse_graph_links(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let mut from: Option<String> = None;
    let mut to: Option<String> = None;
    let mut min_strength: Option<f64> = None;
    let mut link_type: Option<String> = None;

    for kv in pair.into_inner() {
        // kv is a graph_step_kv: ident ":" value
        let mut kv_inner = kv.into_inner();
        let key = kv_inner.next().expect("key ident").as_str();
        let val = parse_value(kv_inner.next().expect("value"))?;
        match key {
            "from" => {
                if let IrValue::String(s) = val {
                    from = Some(s);
                }
            }
            "to" => {
                if let IrValue::String(s) = val {
                    to = Some(s);
                }
            }
            "min_strength" => {
                if let IrValue::Number(n) = val {
                    min_strength = Some(n);
                }
            }
            "type" => {
                if let IrValue::String(s) = val {
                    link_type = Some(s);
                }
            }
            _ => {
                // Unknown keys are silently ignored for forward compatibility.
            }
        }
    }

    Ok(Step::GraphLinks { from, to, min_strength, link_type })
}

// ---- auto_link ----

fn parse_auto_link(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let mut algorithms: Vec<String> = Vec::new();
    let mut min_strength: Option<f64> = None;

    for kv in pair.into_inner() {
        // kv is an auto_link_kv: either auto_link_alg_kv or auto_link_other_kv
        let inner = kv.into_inner().next().expect("auto_link_kv child");
        match inner.as_rule() {
            Rule::auto_link_alg_kv => {
                // auto_link_alg_kv: "algorithms" ":" field_array
                // Each element of field_array is a `string` atomic rule; use
                // unescape_string_content directly (the same logic parse_value
                // applies for Rule::string) to correctly handle escape sequences.
                let arr = inner.into_inner().next().expect("field_array");
                for p in arr.into_inner() {
                    let s = p.as_str();
                    let content = &s[1..s.len() - 1]; // strip surrounding quotes
                    let unescaped = unescape_string_content(content).map_err(|msg| {
                        ParseError(pest::error::Error::new_from_span(
                            pest::error::ErrorVariant::CustomError { message: msg },
                            p.as_span(),
                        ))
                    })?;
                    algorithms.push(unescaped);
                }
            }
            Rule::auto_link_other_kv => {
                // auto_link_other_kv: ident ":" value
                let mut kv_inner = inner.into_inner();
                let key = kv_inner.next().expect("key ident").as_str();
                let val = parse_value(kv_inner.next().expect("value"))?;
                if key == "min_strength" {
                    if let IrValue::Number(n) = val {
                        min_strength = Some(n);
                    }
                }
            }
            r => unreachable!("unexpected auto_link_kv child: {:?}", r),
        }
    }

    Ok(Step::AutoLink { algorithms, min_strength })
}

// ---- cognitive architecture steps ----

fn parse_vector_search(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let mut children = pair.into_inner();
    let query_pair = children
        .next()
        .expect("vector_search query string");
    let query_value = parse_value(query_pair.clone())?;
    let query = if let IrValue::String(s) = query_value {
        s
    } else {
        // Fallback to the raw text to avoid changing existing non-failing behavior
        // if the grammar ever allows non-string values here.
        query_pair.as_str().to_string()
    };
    let mut limit = 10usize;
    let mut min_score = 0.0f64;
    let mut category: Option<String> = None;

    for kv in children {
        if kv.as_rule() == Rule::vector_search_kv {
            let mut inner = kv.into_inner();
            let key = inner.next().expect("kv key").as_str();
            let val = inner.next().expect("kv value");
            match key {
                "limit" => limit = parse_value_as_usize(val),
                "min_score" => min_score = parse_value_as_f64(val),
                "category" => {
                    let parsed = parse_value(val)?;
                    if let IrValue::String(s) = parsed {
                        category = Some(s);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(Step::VectorSearch { query, limit, min_score, category })
}

fn parse_text_search(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let mut children = pair.into_inner();
    let query_pair = children
        .next()
        .expect("text_search query string");
    let query_value = parse_value(query_pair.clone())?;
    let query = if let IrValue::String(s) = query_value {
        s
    } else {
        // Fallback to the raw text to avoid changing existing non-failing behavior
        // if the grammar ever allows non-string values here.
        query_pair.as_str().to_string()
    };
    let mut limit = 10usize;
    let mut field = "text".to_string();

    for kv in children {
        if kv.as_rule() == Rule::text_search_kv {
            let mut inner = kv.into_inner();
            let key = inner.next().expect("kv key").as_str();
            let val = inner.next().expect("kv value");
            match key {
                "limit" => limit = parse_value_as_usize(val),
                "field" => field = parse_value_as_string(val),
                _ => {}
            }
        }
    }

    Ok(Step::TextSearch { query, limit, field })
}

fn parse_transform(pair: Pair<Rule>) -> Result<Step, ParseError> {
    use crate::ir::TransformFormat;
    let mut children = pair.into_inner();

    let format_kv = children.next().expect("transform format kv");
    let format_pair = format_kv
        .into_inner()
        .find(|p| p.as_rule() == Rule::string)
        .expect("format string");
    let format_str = unquote(format_pair.as_str());
    let format = match format_str.as_str() {
        "structured" => TransformFormat::Structured,
        "fused" => TransformFormat::Fused,
        "toon" => TransformFormat::Toon,
        other => {
            return Err(ParseError(pest::error::Error::new_from_span(
                pest::error::ErrorVariant::CustomError {
                    message: format!("unknown transform format: {}", other),
                },
                format_pair.as_span(),
            )));
        }
    };

    let mut max_chars = 0usize;
    if let Some(mc_kv) = children.next() {
        if mc_kv.as_rule() == Rule::transform_max_chars_kv {
            let val_pair = mc_kv
                .into_inner()
                .find(|p| p.as_rule() == Rule::pos_integer)
                .expect("max_chars value");
            let val_str = val_pair.as_str();
            max_chars = val_str.parse().map_err(|_| {
                ParseError(pest::error::Error::new_from_span(
                    pest::error::ErrorVariant::CustomError {
                        message: format!("invalid max_chars value: {}", val_str),
                    },
                    val_pair.as_span(),
                ))
            })?;
        }
    }

    Ok(Step::Transform { format, max_chars })
}

fn parse_conditional(pair: Pair<Rule>) -> Result<Step, ParseError> {
    // DSL conditional only parses the condition predicate; then/else are JSON-IR only
    let pred = pair.into_inner().next().expect("conditional predicate");
    Ok(Step::Conditional {
        condition: parse_predicate(pred)?,
        then_steps: Vec::new(),
        else_steps: Vec::new(),
    })
}

fn parse_assign(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let kv = pair.into_inner().next().expect("assign name kv");
    let name = unquote(
        kv.into_inner()
            .find(|p| p.as_rule() == Rule::string)
            .expect("assign name string")
            .as_str(),
    );
    Ok(Step::Assign { name })
}

fn parse_emit(pair: Pair<Rule>) -> Result<Step, ParseError> {
    let mut children = pair.into_inner();

    let label_kv = children.next().expect("emit label kv");
    let label = unquote(
        label_kv
            .into_inner()
            .find(|p| p.as_rule() == Rule::string)
            .expect("emit label string")
            .as_str(),
    );

    let from_var = children.next().map(|from_kv| {
        unquote(
            from_kv
                .into_inner()
                .find(|p| p.as_rule() == Rule::string)
                .expect("emit from string")
                .as_str(),
        )
    });

    Ok(Step::Emit { label, from_var })
}

#[cfg(test)]
mod tests_new_dsl_steps {
    use super::parse_query;
    use crate::ir::{Step, TransformFormat};

    #[test]
    fn parse_transform_with_default_max_chars() {
        // Only format is provided; max_chars should default to 0.
        let steps = parse_query(r#"transform(format: "structured")"#).unwrap();
        assert_eq!(steps.len(), 1);
        match &steps[0] {
            Step::Transform { format, max_chars } => {
                assert_eq!(*format, TransformFormat::Structured);
                assert_eq!(*max_chars, 0);
            }
            other => panic!("expected Transform step, got {:?}", other),
        }
    }

    #[test]
    fn parse_conditional_initializes_empty_then_else() {
        let steps = parse_query(r#"conditional(category == "decision")"#).unwrap();
        assert_eq!(steps.len(), 1);
        match &steps[0] {
            Step::Conditional {
                condition: _,
                then_steps,
                else_steps,
            } => {
                assert!(then_steps.is_empty());
                assert!(else_steps.is_empty());
            }
            other => panic!("expected Conditional step, got {:?}", other),
        }
    }

    #[test]
    fn parse_assign_unquotes_escaped_string() {
        let steps = parse_query(r#"assign(name: "foo\"bar")"#).unwrap();
        assert_eq!(steps.len(), 1);
        match &steps[0] {
            Step::Assign { name } => {
                assert_eq!(name, "foo\"bar");
            }
            other => panic!("expected Assign step, got {:?}", other),
        }
    }

    #[test]
    fn parse_emit_with_and_without_from_var() {
        // Without from: should default to None.
        let steps = parse_query(r#"emit(label: "result")"#).unwrap();
        assert_eq!(steps.len(), 1);
        match &steps[0] {
            Step::Emit { label, from_var } => {
                assert_eq!(label, "result");
                assert!(from_var.is_none());
            }
            other => panic!("expected Emit step, got {:?}", other),
        }

        // With from: should parse into Some(..).
        let steps = parse_query(r#"emit(label: "result", from: "tmp_var")"#).unwrap();
        assert_eq!(steps.len(), 1);
        match &steps[0] {
            Step::Emit { label, from_var } => {
                assert_eq!(label, "result");
                assert_eq!(from_var.as_deref(), Some("tmp_var"));
            }
            other => panic!("expected Emit step with from_var, got {:?}", other),
        }
    }

    #[test]
    fn parse_vector_search_step_variant() {
        // Basic smoke test to ensure the vector_search DSL parses to the right variant.
        // The exact argument structure is validated elsewhere; here we only care about the step kind.
        let steps = parse_query(r#"vector_search(query: "foo", limit: 5)"#).unwrap();
        assert_eq!(steps.len(), 1);
        match &steps[0] {
            Step::VectorSearch { .. } => {}
            other => panic!("expected VectorSearch step, got {:?}", other),
        }
    }

    #[test]
    fn parse_text_search_step_variant() {
        // Basic smoke test to ensure the text_search DSL parses to the right variant.
        let steps = parse_query(r#"text_search(query: "foo", limit: 10)"#).unwrap();
        assert_eq!(steps.len(), 1);
        match &steps[0] {
            Step::TextSearch { .. } => {}
            other => panic!("expected TextSearch step, got {:?}", other),
        }
    }
}

// Helper: extract usize from a value pair
fn parse_value_as_usize(pair: Pair<Rule>) -> Result<usize, ParseError> {
    let span = pair.as_span();
    let mut inner_pairs = pair.into_inner();
    let inner = inner_pairs.next().ok_or_else(|| {
        ParseError(PestError::new_from_span(
            ErrorVariant::CustomError {
                message: "expected numeric value".into(),
            },
            span,
        ))
    })?;

    inner
        .as_str()
        .parse::<usize>()
        .map_err(|_| {
            ParseError(PestError::new_from_span(
                ErrorVariant::CustomError {
                    message: "invalid unsigned integer in value".into(),
                },
                inner.as_span(),
            ))
        })
}

// Helper: extract f64 from a value pair
fn parse_value_as_f64(pair: Pair<Rule>) -> Result<f64, ParseError> {
    let span = pair.as_span();
    let mut inner_pairs = pair.into_inner();
    let inner = inner_pairs.next().ok_or_else(|| {
        ParseError(PestError::new_from_span(
            ErrorVariant::CustomError {
                message: "expected numeric value".into(),
            },
            span,
        ))
    })?;

    inner
        .as_str()
        .parse::<f64>()
        .map_err(|_| {
            ParseError(PestError::new_from_span(
                ErrorVariant::CustomError {
                    message: "invalid float in value".into(),
                },
                inner.as_span(),
            ))
        })
}

// Helper: extract String from a value pair (expects a string literal)
fn parse_value_as_string(pair: Pair<Rule>) -> String {
    let inner = pair.into_inner().next().expect("value child");
    unquote(inner.as_str())
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

    #[test]
    fn parse_graph_clusters_full() {
        let input = r#"graph_clusters(algorithm: "louvain", min_size: 3, min_strength: 0.5)"#;
        let steps = parse_query(input).unwrap();
        assert_eq!(steps.len(), 1);
        if let Step::GraphClusters { algorithm, min_size, min_strength } = &steps[0] {
            assert_eq!(algorithm, "louvain");
            assert_eq!(*min_size, Some(3));
            assert!((min_strength.unwrap() - 0.5).abs() < 1e-9);
        } else {
            panic!("expected GraphClusters step");
        }
    }

    // ── Graph operation parser tests ──────────────────────────────────────────

    #[test]
    fn parse_graph_neighbors_minimal() {
        let steps = parse_query(r#"graph_neighbors("memory:123", depth: 2)"#).unwrap();
        assert_eq!(steps.len(), 1);
        if let Step::GraphNeighbors { root, depth, min_strength, link_type, bidirectional } =
            &steps[0]
        {
            assert_eq!(root, "memory:123");
            assert_eq!(*depth, 2);
            assert!(min_strength.is_none());
            assert!(link_type.is_none());
            assert!(!bidirectional);
        } else {
            panic!("expected GraphNeighbors");
        }
    }

    #[test]
    fn parse_graph_clusters_minimal() {
        let input = r#"graph_clusters(algorithm: "semantic")"#;
        let steps = parse_query(input).unwrap();
        if let Step::GraphClusters { algorithm, min_size, min_strength } = &steps[0] {
            assert_eq!(algorithm, "semantic");
            assert!(min_size.is_none());
            assert!(min_strength.is_none());
        } else {
            panic!("expected GraphClusters step");
        }
    }

    #[test]
    fn parse_graph_neighbors_all_params() {
        let steps = parse_query(
            r#"graph_neighbors("memory:123", depth: 3, min_strength: 0.8, type: "related", bidirectional: true)"#,
        )
        .unwrap();
        if let Step::GraphNeighbors { root, depth, min_strength, link_type, bidirectional } =
            &steps[0]
        {
            assert_eq!(root, "memory:123");
            assert_eq!(*depth, 3);
            assert_eq!(*min_strength, Some(0.8));
            assert_eq!(link_type.as_deref(), Some("related"));
            assert!(*bidirectional);
        } else {
            panic!("expected GraphNeighbors");
        }
    }

    #[test]
    fn parse_graph_links_from_only() {
        let steps = parse_query(r#"graph_links(from: "memory:123")"#).unwrap();
        assert_eq!(steps.len(), 1);
        if let Step::GraphLinks { from, to, min_strength, link_type } = &steps[0] {
            assert_eq!(from.as_deref(), Some("memory:123"));
            assert!(to.is_none());
            assert!(min_strength.is_none());
            assert!(link_type.is_none());
        } else {
            panic!("expected GraphLinks");
        }
    }

    #[test]
    fn parse_graph_path_full() {
        let input = r#"graph_path(from: "memory:123", to: "memory:456", max_hops: 5)"#;
        let steps = parse_query(input).unwrap();
        if let Step::GraphPath { from, to, max_hops } = &steps[0] {
            assert_eq!(from, "memory:123");
            assert_eq!(to, "memory:456");
            assert_eq!(*max_hops, Some(5));
        } else {
            panic!("expected GraphPath step");
        }
    }

    #[test]
    fn parse_graph_links_all_params() {
        let steps = parse_query(
            r#"graph_links(from: "n1", to: "n2", min_strength: 0.5, type: "semantic")"#,
        )
        .unwrap();
        if let Step::GraphLinks { from, to, min_strength, link_type } = &steps[0] {
            assert_eq!(from.as_deref(), Some("n1"));
            assert_eq!(to.as_deref(), Some("n2"));
            assert_eq!(*min_strength, Some(0.5));
            assert_eq!(link_type.as_deref(), Some("semantic"));
        } else {
            panic!("expected GraphLinks");
        }
    }

    #[test]
    fn parse_graph_path_no_max_hops() {
        let input = r#"graph_path(from: "a", to: "b")"#;
        let steps = parse_query(input).unwrap();
        if let Step::GraphPath { max_hops, .. } = &steps[0] {
            assert!(max_hops.is_none());
        } else {
            panic!("expected GraphPath step");
        }
    }

    #[test]
    fn parse_graph_links_empty() {
        let steps = parse_query("graph_links()").unwrap();
        assert_eq!(steps.len(), 1);
        if let Step::GraphLinks { from, to, min_strength, link_type } = &steps[0] {
            assert!(from.is_none());
            assert!(to.is_none());
            assert!(min_strength.is_none());
            assert!(link_type.is_none());
        } else {
            panic!("expected GraphLinks");
        }
    }

    #[test]
    fn parse_graph_pagerank_full() {
        let input = "graph_pagerank(damping: 0.85, iterations: 50)";
        let steps = parse_query(input).unwrap();
        if let Step::GraphPagerank { damping, iterations } = &steps[0] {
            assert!((damping.unwrap() - 0.85).abs() < 1e-9);
            assert_eq!(*iterations, Some(50));
        } else {
            panic!("expected GraphPagerank step");
        }
    }

    #[test]
    fn parse_auto_link_empty() {
        let steps = parse_query("auto_link()").unwrap();
        assert_eq!(steps.len(), 1);
        if let Step::AutoLink { algorithms, min_strength } = &steps[0] {
            assert!(algorithms.is_empty());
            assert!(min_strength.is_none());
        } else {
            panic!("expected AutoLink");
        }
    }

    #[test]
    fn parse_graph_pagerank_empty() {
        let input = "graph_pagerank()";
        let steps = parse_query(input).unwrap();
        if let Step::GraphPagerank { damping, iterations } = &steps[0] {
            assert!(damping.is_none());
            assert!(iterations.is_none());
        } else {
            panic!("expected GraphPagerank step");
        }
    }

    #[test]
    fn parse_auto_link_with_algorithms() {
        let steps =
            parse_query(r#"auto_link(algorithms: ["semantic", "category"])"#).unwrap();
        if let Step::AutoLink { algorithms, .. } = &steps[0] {
            assert_eq!(algorithms, &["semantic", "category"]);
        } else {
            panic!("expected AutoLink");
        }
    }

    #[test]
    fn parse_graph_stats() {
        let steps = parse_query("graph_stats()").unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0], Step::GraphStats);
    }

    #[test]
    fn parse_graph_pagerank_pipe_chain() {
        let input = r#"graph_pagerank(damping: 0.85) |> limit(10)"#;
        let steps = parse_query(input).unwrap();
        assert_eq!(steps.len(), 2);
        assert!(matches!(steps[0], Step::GraphPagerank { .. }));
        assert_eq!(steps[1], Step::Limit { n: 10 });
    }

    #[test]
    fn parse_auto_link_with_min_strength() {
        let steps = parse_query(r#"auto_link(algorithms: ["temporal"], min_strength: 0.6)"#)
            .unwrap();
        if let Step::AutoLink { algorithms, min_strength } = &steps[0] {
            assert_eq!(algorithms, &["temporal"]);
            assert_eq!(*min_strength, Some(0.6));
        } else {
            panic!("expected AutoLink");
        }
    }

    #[test]
    fn parse_graph_pipeline_complex() {
        let input = r#"filter(category == "development") |> auto_link() |> graph_neighbors("memory:1", depth: 2)"#;
        let steps = parse_query(input).unwrap();
        assert_eq!(steps.len(), 3);
        assert!(matches!(steps[0], Step::Filter { .. }));
        assert!(matches!(steps[1], Step::AutoLink { .. }));
        assert!(matches!(steps[2], Step::GraphNeighbors { .. }));
    }

    #[test]
    fn parse_graph_neighbors_in_pipeline() {
        let input = r#"graph_neighbors("memory:123", depth: 2) |> filter(category == "dev") |> limit(10)"#;
        let steps = parse_query(input).unwrap();
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn parse_graph_links_then_sort() {
        let input = r#"graph_links(min_strength: 0.8) |> sort(by: "strength", dir: "desc")"#;
        let steps = parse_query(input).unwrap();
        assert_eq!(steps.len(), 2);
    }

    // ── Regression / correctness tests for fixes in review round 2 ───────────

    #[test]
    fn parse_graph_neighbors_depth_negative_is_error() {
        assert!(
            parse_query(r#"graph_neighbors("n1", depth: -1)"#).is_err(),
            "negative depth must be rejected"
        );
    }

    #[test]
    fn parse_graph_neighbors_depth_float_is_error() {
        assert!(
            parse_query(r#"graph_neighbors("n1", depth: 2.5)"#).is_err(),
            "non-integer depth must be rejected"
        );
    }

    #[test]
    fn parse_graph_neighbors_depth_zero_is_ok() {
        let steps = parse_query(r#"graph_neighbors("n1", depth: 0)"#).unwrap();
        if let Step::GraphNeighbors { depth, .. } = &steps[0] {
            assert_eq!(*depth, 0);
        } else {
            panic!("expected GraphNeighbors");
        }
    }

    #[test]
    fn parse_graph_neighbors_root_escape_sequences() {
        // Root IDs with escape sequences must be unescaped.
        let steps = parse_query(r#"graph_neighbors("memory:abc\u0031", depth: 1)"#).unwrap();
        if let Step::GraphNeighbors { root, .. } = &steps[0] {
            assert_eq!(root, "memory:abc1"); // \u0031 = '1'
        } else {
            panic!("expected GraphNeighbors");
        }
    }

    #[test]
    fn parse_auto_link_algorithm_escape_sequences() {
        // Algorithm strings with escape sequences must be unescaped.
        let steps =
            parse_query(r#"auto_link(algorithms: ["sem\u0061ntic"])"#).unwrap();
        if let Step::AutoLink { algorithms, .. } = &steps[0] {
            assert_eq!(algorithms, &["semantic"]); // \u0061 = 'a'
        } else {
            panic!("expected AutoLink");
        }
    }
}
