//! Sort operation — order nodes by a field path, apply a limit, and support
//! cursor-based pagination via `after`.

use pluresdb_core::NodeRecord;

use crate::ir::SortDir;
use crate::ops::filter::get_nested;

/// Sort `nodes` by the JSON field at `by`, apply an optional `limit`, and
/// skip all nodes that appear at or before the cursor identified by `after`
/// (cursor-based pagination).
pub fn apply_sort(
    mut nodes: Vec<NodeRecord>,
    by: &str,
    dir: &SortDir,
    limit: Option<usize>,
    after: Option<&str>,
) -> Vec<NodeRecord> {
    nodes.sort_by(|a, b| {
        let va = get_nested(&a.data, by);
        let vb = get_nested(&b.data, by);
        let ord = compare_json_values(va, vb);
        match dir {
            SortDir::Asc => ord,
            SortDir::Desc => ord.reverse(),
        }
    });

    // Cursor pagination: skip everything up to and including the `after` node.
    if let Some(cursor_id) = after {
        if let Some(pos) = nodes.iter().position(|n| n.id == cursor_id) {
            nodes.drain(..=pos);
        }
    }

    if let Some(n) = limit {
        nodes.truncate(n);
    }

    nodes
}

/// Lexicographic/numeric comparison of two optional JSON values.
///
/// Ordering: `None` (missing field) < `Null` < `Bool` < `Number` < `String`.
fn compare_json_values(
    a: Option<&serde_json::Value>,
    b: Option<&serde_json::Value>,
) -> std::cmp::Ordering {
    use serde_json::Value;
    use std::cmp::Ordering;

    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, _) => Ordering::Less,
        (_, None) => Ordering::Greater,
        (Some(Value::Null), Some(Value::Null)) => Ordering::Equal,
        (Some(Value::Null), _) => Ordering::Less,
        (_, Some(Value::Null)) => Ordering::Greater,
        (Some(Value::Number(na)), Some(Value::Number(nb))) => {
            let fa = na.as_f64().unwrap_or(f64::NEG_INFINITY);
            let fb = nb.as_f64().unwrap_or(f64::NEG_INFINITY);
            fa.partial_cmp(&fb).unwrap_or(Ordering::Equal)
        }
        (Some(Value::String(sa)), Some(Value::String(sb))) => sa.cmp(sb),
        (Some(Value::Bool(ba)), Some(Value::Bool(bb))) => ba.cmp(bb),
        // Cross-type: compare by type discriminant for stable ordering.
        (Some(a), Some(b)) => type_rank(a).cmp(&type_rank(b)),
    }
}

fn type_rank(v: &serde_json::Value) -> u8 {
    match v {
        serde_json::Value::Null => 0,
        serde_json::Value::Bool(_) => 1,
        serde_json::Value::Number(_) => 2,
        serde_json::Value::String(_) => 3,
        serde_json::Value::Array(_) => 4,
        serde_json::Value::Object(_) => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pluresdb_core::CrdtStore;

    fn make_node(id: &str, data: serde_json::Value) -> NodeRecord {
        let store = CrdtStore::default();
        store.put(id.to_string(), "test", data);
        store.get(id).unwrap()
    }

    #[test]
    fn sort_asc_by_number() {
        let nodes = vec![
            make_node("b", serde_json::json!({"score": 0.3})),
            make_node("a", serde_json::json!({"score": 0.9})),
            make_node("c", serde_json::json!({"score": 0.5})),
        ];
        let result = apply_sort(nodes, "score", &SortDir::Asc, None, None);
        let ids: Vec<_> = result.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "c", "a"]);
    }

    #[test]
    fn sort_desc_by_string() {
        let nodes = vec![
            make_node("a", serde_json::json!({"name": "charlie"})),
            make_node("b", serde_json::json!({"name": "alice"})),
            make_node("c", serde_json::json!({"name": "bob"})),
        ];
        let result = apply_sort(nodes, "name", &SortDir::Desc, None, None);
        let ids: Vec<_> = result.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "c", "b"]);
    }

    #[test]
    fn sort_with_limit() {
        let nodes = vec![
            make_node("a", serde_json::json!({"n": 1})),
            make_node("b", serde_json::json!({"n": 2})),
            make_node("c", serde_json::json!({"n": 3})),
        ];
        let result = apply_sort(nodes, "n", &SortDir::Asc, Some(2), None);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "a");
        assert_eq!(result[1].id, "b");
    }

    #[test]
    fn sort_cursor_pagination() {
        let nodes = vec![
            make_node("a", serde_json::json!({"n": 1})),
            make_node("b", serde_json::json!({"n": 2})),
            make_node("c", serde_json::json!({"n": 3})),
            make_node("d", serde_json::json!({"n": 4})),
        ];
        // After "b" → return c, d
        let result = apply_sort(nodes, "n", &SortDir::Asc, None, Some("b"));
        let ids: Vec<_> = result.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["c", "d"]);
    }

    #[test]
    fn sort_missing_field_goes_last_asc() {
        let nodes = vec![
            make_node("a", serde_json::json!({"score": 0.5})),
            make_node("b", serde_json::json!({})),
        ];
        let result = apply_sort(nodes, "score", &SortDir::Asc, None, None);
        assert_eq!(result[0].id, "b"); // None sorts first in Asc
        assert_eq!(result[1].id, "a");
    }
}
