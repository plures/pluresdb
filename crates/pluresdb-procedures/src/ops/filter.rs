//! Filter operation — evaluate a predicate against each node's data.

use pluresdb_core::NodeRecord;

use crate::ir::{CmpOp, IrValue, Predicate};

/// Retain only nodes whose data satisfies `predicate`.
pub fn apply_filter(nodes: Vec<NodeRecord>, predicate: &Predicate) -> Vec<NodeRecord> {
    nodes
        .into_iter()
        .filter(|node| eval_predicate(predicate, &node.data))
        .collect()
}

/// Evaluate `predicate` against a JSON `data` document.
pub fn eval_predicate(predicate: &Predicate, data: &serde_json::Value) -> bool {
    match predicate {
        Predicate::Comparison { field, cmp, value } => {
            let field_val = get_nested(data, field);
            eval_cmp(field_val, cmp, value)
        }
        Predicate::And { and } => and.iter().all(|p| eval_predicate(p, data)),
        Predicate::Or { or } => or.iter().any(|p| eval_predicate(p, data)),
        Predicate::Not { not } => !eval_predicate(not, data),
    }
}

/// Resolve a dotted field path like `"data.score"` inside a JSON document.
///
/// Returns `None` when the path does not exist.
pub fn get_nested<'a>(data: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut current = data;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

fn eval_cmp(field_val: Option<&serde_json::Value>, cmp: &CmpOp, rhs: &IrValue) -> bool {
    match cmp {
        CmpOp::Eq => compare_eq(field_val, rhs),
        CmpOp::Ne => !compare_eq(field_val, rhs),
        CmpOp::Gt => compare_numeric(field_val, rhs, |a, b| a > b),
        CmpOp::Ge => compare_numeric(field_val, rhs, |a, b| a >= b),
        CmpOp::Lt => compare_numeric(field_val, rhs, |a, b| a < b),
        CmpOp::Le => compare_numeric(field_val, rhs, |a, b| a <= b),
        CmpOp::Contains => compare_string(field_val, rhs, |a, b| a.contains(b)),
        CmpOp::StartsWith => compare_string(field_val, rhs, |a, b| a.starts_with(b)),
        CmpOp::Matches => compare_regex(field_val, rhs),
    }
}

fn compare_eq(field_val: Option<&serde_json::Value>, rhs: &IrValue) -> bool {
    match (field_val, rhs) {
        (None, IrValue::Null) => true,
        (Some(serde_json::Value::Null), IrValue::Null) => true,
        (Some(serde_json::Value::String(s)), IrValue::String(r)) => s == r,
        (Some(serde_json::Value::Number(n)), IrValue::Number(r)) => {
            n.as_f64().map_or(false, |f| f == *r)
        }
        (Some(serde_json::Value::Bool(b)), IrValue::Bool(r)) => b == r,
        _ => false,
    }
}

fn compare_numeric(
    field_val: Option<&serde_json::Value>,
    rhs: &IrValue,
    op: impl Fn(f64, f64) -> bool,
) -> bool {
    if let (Some(serde_json::Value::Number(n)), IrValue::Number(r)) = (field_val, rhs) {
        if let Some(f) = n.as_f64() {
            return op(f, *r);
        }
    }
    false
}

fn compare_string(
    field_val: Option<&serde_json::Value>,
    rhs: &IrValue,
    op: impl Fn(&str, &str) -> bool,
) -> bool {
    if let (Some(serde_json::Value::String(s)), IrValue::String(r)) = (field_val, rhs) {
        return op(s.as_str(), r.as_str());
    }
    false
}

fn compare_regex(field_val: Option<&serde_json::Value>, rhs: &IrValue) -> bool {
    // Simple substring match as a fallback (no regex crate dependency for Phase 1).
    if let (Some(serde_json::Value::String(s)), IrValue::String(pattern)) = (field_val, rhs) {
        return s.contains(pattern.as_str());
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
    use pluresdb_core::{CrdtStore, NodeRecord};

    fn make_node(id: &str, data: serde_json::Value) -> NodeRecord {
        let store = CrdtStore::default();
        store.put(id.to_string(), "test", data);
        store.get(id).unwrap()
    }

    #[test]
    fn filter_eq_string() {
        let nodes = vec![
            make_node("a", serde_json::json!({"category": "decision"})),
            make_node("b", serde_json::json!({"category": "note"})),
        ];
        let pred = Predicate::eq("category", "decision");
        let result = apply_filter(nodes, &pred);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "a");
    }

    #[test]
    fn filter_gt_number() {
        let nodes = vec![
            make_node("a", serde_json::json!({"score": 0.9})),
            make_node("b", serde_json::json!({"score": 0.3})),
        ];
        let pred = Predicate::Comparison {
            field: "score".to_string(),
            cmp: CmpOp::Gt,
            value: IrValue::Number(0.5),
        };
        let result = apply_filter(nodes, &pred);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "a");
    }

    #[test]
    fn filter_and() {
        let nodes = vec![
            make_node("a", serde_json::json!({"category": "decision", "score": 0.9})),
            make_node("b", serde_json::json!({"category": "decision", "score": 0.2})),
            make_node("c", serde_json::json!({"category": "note", "score": 0.9})),
        ];
        let pred = Predicate::and(vec![
            Predicate::eq("category", "decision"),
            Predicate::Comparison {
                field: "score".to_string(),
                cmp: CmpOp::Gt,
                value: IrValue::Number(0.5),
            },
        ]);
        let result = apply_filter(nodes, &pred);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "a");
    }

    #[test]
    fn filter_or() {
        let nodes = vec![
            make_node("a", serde_json::json!({"status": "open"})),
            make_node("b", serde_json::json!({"status": "pending"})),
            make_node("c", serde_json::json!({"status": "closed"})),
        ];
        let pred = Predicate::or(vec![
            Predicate::eq("status", "open"),
            Predicate::eq("status", "pending"),
        ]);
        let result = apply_filter(nodes, &pred);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_not() {
        let nodes = vec![
            make_node("a", serde_json::json!({"archived": false})),
            make_node("b", serde_json::json!({"archived": true})),
        ];
        let pred = Predicate::not(Predicate::Comparison {
            field: "archived".to_string(),
            cmp: CmpOp::Eq,
            value: IrValue::Bool(true),
        });
        let result = apply_filter(nodes, &pred);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "a");
    }

    #[test]
    fn filter_contains() {
        let nodes = vec![
            make_node("a", serde_json::json!({"text": "hello world"})),
            make_node("b", serde_json::json!({"text": "goodbye"})),
        ];
        let pred = Predicate::Comparison {
            field: "text".to_string(),
            cmp: CmpOp::Contains,
            value: IrValue::String("hello".to_string()),
        };
        let result = apply_filter(nodes, &pred);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_nested_field() {
        let nodes = vec![
            make_node("a", serde_json::json!({"data": {"score": 0.9}})),
            make_node("b", serde_json::json!({"data": {"score": 0.1}})),
        ];
        let pred = Predicate::Comparison {
            field: "data.score".to_string(),
            cmp: CmpOp::Gt,
            value: IrValue::Number(0.5),
        };
        let result = apply_filter(nodes, &pred);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "a");
    }
}
