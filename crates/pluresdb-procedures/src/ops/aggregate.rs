//! Aggregate operation — compute summary statistics over a node set.

use pluresdb_core::NodeRecord;

use crate::ir::{AggFn, AggResult};
use crate::ops::filter::get_nested;

/// Compute an aggregation over `nodes`.
///
/// Returns an [`AggResult`] whose variant corresponds to `func`:
///
/// | `func`     | returns                                                      |
/// |------------|--------------------------------------------------------------|
/// | `count`    | `AggResult::Count(n)`                                        |
/// | `sum`      | `AggResult::Number(sum)` or `AggResult::Null` (empty set)    |
/// | `avg`      | `AggResult::Number(mean)` or `AggResult::Null` (empty set)   |
/// | `min`      | `AggResult::Number(min)` or `AggResult::Null` (empty set)    |
/// | `max`      | `AggResult::Number(max)` or `AggResult::Null` (empty set)    |
/// | `distinct` | `AggResult::Values(unique_values)`                           |
/// | `collect`  | `AggResult::Values(all_field_values)`                        |
pub fn apply_aggregate(nodes: &[NodeRecord], func: &AggFn, field: Option<&str>) -> AggResult {
    match func {
        AggFn::Count => AggResult::Count(nodes.len() as u64),
        AggFn::Sum => {
            let nums = extract_numbers(nodes, field);
            if nums.is_empty() {
                AggResult::Null
            } else {
                AggResult::Number(nums.iter().copied().sum::<f64>())
            }
        }
        AggFn::Avg => {
            let nums = extract_numbers(nodes, field);
            if nums.is_empty() {
                AggResult::Null
            } else {
                AggResult::Number(nums.iter().copied().sum::<f64>() / nums.len() as f64)
            }
        }
        AggFn::Min => {
            let nums = extract_numbers(nodes, field);
            if nums.is_empty() {
                AggResult::Null
            } else {
                AggResult::Number(nums.iter().copied().fold(f64::INFINITY, f64::min))
            }
        }
        AggFn::Max => {
            let nums = extract_numbers(nodes, field);
            if nums.is_empty() {
                AggResult::Null
            } else {
                AggResult::Number(nums.iter().copied().fold(f64::NEG_INFINITY, f64::max))
            }
        }
        AggFn::Distinct => {
            let values = extract_values(nodes, field);
            // `serde_json::Value` does not implement `Hash`, so we serialise
            // each value to a canonical JSON string and use a `HashSet<String>`
            // for O(n) deduplication.  For deeply nested or very large values
            // the serialisation cost per element may be noticeable; a future
            // optimisation could use a custom hash wrapper.
            let mut seen_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut unique: Vec<serde_json::Value> = Vec::new();
            for v in values {
                let key = serde_json::to_string(&v).unwrap_or_default();
                if seen_keys.insert(key) {
                    unique.push(v);
                }
            }
            AggResult::Values(unique)
        }
        AggFn::Collect => AggResult::Values(extract_values(nodes, field)),
    }
}

fn extract_numbers(nodes: &[NodeRecord], field: Option<&str>) -> Vec<f64> {
    nodes
        .iter()
        .filter_map(|n| {
            let v = if let Some(f) = field {
                get_nested(&n.data, f)?
            } else {
                &n.data
            };
            v.as_f64()
        })
        .collect()
}

fn extract_values(nodes: &[NodeRecord], field: Option<&str>) -> Vec<serde_json::Value> {
    nodes
        .iter()
        .filter_map(|n| {
            if let Some(f) = field {
                get_nested(&n.data, f).cloned()
            } else {
                Some(n.data.clone())
            }
        })
        .collect()
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
    fn aggregate_count() {
        let nodes = vec![
            make_node("a", serde_json::json!({})),
            make_node("b", serde_json::json!({})),
        ];
        assert_eq!(apply_aggregate(&nodes, &AggFn::Count, None), AggResult::Count(2));
    }

    #[test]
    fn aggregate_sum() {
        let nodes = vec![
            make_node("a", serde_json::json!({"score": 1.0})),
            make_node("b", serde_json::json!({"score": 2.0})),
            make_node("c", serde_json::json!({"score": 3.0})),
        ];
        assert_eq!(
            apply_aggregate(&nodes, &AggFn::Sum, Some("score")),
            AggResult::Number(6.0)
        );
    }

    #[test]
    fn aggregate_avg() {
        let nodes = vec![
            make_node("a", serde_json::json!({"score": 1.0})),
            make_node("b", serde_json::json!({"score": 3.0})),
        ];
        assert_eq!(
            apply_aggregate(&nodes, &AggFn::Avg, Some("score")),
            AggResult::Number(2.0)
        );
    }

    #[test]
    fn aggregate_min_max() {
        let nodes = vec![
            make_node("a", serde_json::json!({"n": 5.0})),
            make_node("b", serde_json::json!({"n": 1.0})),
            make_node("c", serde_json::json!({"n": 9.0})),
        ];
        assert_eq!(
            apply_aggregate(&nodes, &AggFn::Min, Some("n")),
            AggResult::Number(1.0)
        );
        assert_eq!(
            apply_aggregate(&nodes, &AggFn::Max, Some("n")),
            AggResult::Number(9.0)
        );
    }

    #[test]
    fn aggregate_distinct() {
        let nodes = vec![
            make_node("a", serde_json::json!({"cat": "a"})),
            make_node("b", serde_json::json!({"cat": "b"})),
            make_node("c", serde_json::json!({"cat": "a"})),
        ];
        if let AggResult::Values(v) = apply_aggregate(&nodes, &AggFn::Distinct, Some("cat")) {
            assert_eq!(v.len(), 2);
        } else {
            panic!("expected Values");
        }
    }

    #[test]
    fn aggregate_collect() {
        let nodes = vec![
            make_node("a", serde_json::json!({"tag": "x"})),
            make_node("b", serde_json::json!({"tag": "y"})),
        ];
        if let AggResult::Values(v) = apply_aggregate(&nodes, &AggFn::Collect, Some("tag")) {
            assert_eq!(v.len(), 2);
        } else {
            panic!("expected Values");
        }
    }

    #[test]
    fn aggregate_min_max_avg_empty_returns_null() {
        let nodes: Vec<NodeRecord> = vec![];
        assert_eq!(apply_aggregate(&nodes, &AggFn::Min, Some("n")), AggResult::Null);
        assert_eq!(apply_aggregate(&nodes, &AggFn::Max, Some("n")), AggResult::Null);
        assert_eq!(apply_aggregate(&nodes, &AggFn::Avg, Some("n")), AggResult::Null);
        assert_eq!(apply_aggregate(&nodes, &AggFn::Sum, Some("n")), AggResult::Null);
    }

    #[test]
    fn aggregate_min_max_missing_field_returns_null() {
        // Field "n" absent on all nodes → empty numeric set → Null
        let nodes = vec![make_node("a", serde_json::json!({"other": 5.0}))];
        assert_eq!(apply_aggregate(&nodes, &AggFn::Min, Some("n")), AggResult::Null);
        assert_eq!(apply_aggregate(&nodes, &AggFn::Max, Some("n")), AggResult::Null);
        assert_eq!(apply_aggregate(&nodes, &AggFn::Sum, Some("n")), AggResult::Null);
    }
}
