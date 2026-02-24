//! Mutate operation — batch put / merge / delete / put_edge / delete_edge
//! with optional local atomicity (all-or-nothing within a single engine call).

use pluresdb_core::CrdtStore;

use crate::ir::MutateOp;

/// Apply a batch of mutate operations to `store` using `actor` as the CRDT
/// actor identifier.
///
/// When `atomic` is `true` the engine checks that all node IDs referenced by
/// `Delete` and `Merge` operations already exist before applying any changes.
/// If any check fails the entire batch is aborted and an error is returned.
///
/// Returns the number of operations successfully applied.
pub fn apply_mutate(
    store: &CrdtStore,
    actor: &str,
    ops: &[MutateOp],
    atomic: bool,
) -> anyhow::Result<usize> {
    if atomic {
        // Pre-flight check: ensure referenced nodes exist.
        for op in ops {
            match op {
                MutateOp::Delete { id } | MutateOp::Merge { id, .. } => {
                    if store.get(id).is_none() {
                        return Err(anyhow::anyhow!(
                            "atomic mutate aborted: node '{}' not found",
                            id
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    let mut applied = 0usize;
    for op in ops {
        match op {
            MutateOp::Put { id, data } => {
                store.put(id.clone(), actor, data.clone());
                applied += 1;
            }
            MutateOp::Delete { id } => {
                // Ignore NotFound errors in non-atomic mode.
                if store.delete(id).is_ok() {
                    applied += 1;
                } else if atomic {
                    return Err(anyhow::anyhow!("delete failed: node '{}' not found", id));
                }
            }
            MutateOp::Merge { id, patch } => {
                let existing = if let Some(rec) = store.get(id) {
                    rec.data
                } else if atomic {
                    return Err(anyhow::anyhow!("merge failed: node '{}' not found", id));
                } else {
                    serde_json::Value::Object(serde_json::Map::new())
                };
                let merged = merge_json(existing, patch.clone());
                store.put(id.clone(), actor, merged);
                applied += 1;
            }
            MutateOp::PutEdge { from, to, label } => {
                // Edges are represented as nodes with a special `_edge` type.
                let edge_id = format!("edge:{}:{}", from, to);
                let data = serde_json::json!({
                    "_edge": true,
                    "from": from,
                    "to": to,
                    "label": label.as_deref().unwrap_or(""),
                });
                store.put(edge_id, actor, data);
                applied += 1;
            }
            MutateOp::DeleteEdge { from, to } => {
                let edge_id = format!("edge:{}:{}", from, to);
                let _ = store.delete(&edge_id);
                applied += 1;
            }
        }
    }

    Ok(applied)
}

/// Merge two JSON objects (shallow patch: `patch` fields overwrite `base`).
///
/// Only the top-level keys of `patch` are merged; nested objects inside
/// `patch` are inserted as-is, replacing any existing nested object at the
/// same key rather than being merged recursively.
fn merge_json(base: serde_json::Value, patch: serde_json::Value) -> serde_json::Value {
    match (base, patch) {
        (serde_json::Value::Object(mut b), serde_json::Value::Object(p)) => {
            for (k, v) in p {
                b.insert(k, v);
            }
            serde_json::Value::Object(b)
        }
        (_, patch) => patch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pluresdb_core::CrdtStore;

    #[test]
    fn mutate_put() {
        let store = CrdtStore::default();
        let op = MutateOp::Put {
            id: "n1".to_string(),
            data: serde_json::json!({"x": 1}),
        };
        let count = apply_mutate(&store, "test", &[op], false).unwrap();
        assert_eq!(count, 1);
        assert!(store.get("n1").is_some());
    }

    #[test]
    fn mutate_delete() {
        let store = CrdtStore::default();
        store.put("n1", "test", serde_json::json!({"x": 1}));
        let op = MutateOp::Delete { id: "n1".to_string() };
        let count = apply_mutate(&store, "test", &[op], false).unwrap();
        assert_eq!(count, 1);
        assert!(store.get("n1").is_none());
    }

    #[test]
    fn mutate_merge() {
        let store = CrdtStore::default();
        store.put("n1", "test", serde_json::json!({"x": 1, "y": 2}));
        let op = MutateOp::Merge {
            id: "n1".to_string(),
            patch: serde_json::json!({"y": 99, "z": 3}),
        };
        apply_mutate(&store, "test", &[op], false).unwrap();
        let node = store.get("n1").unwrap();
        assert_eq!(node.data["x"], 1);
        assert_eq!(node.data["y"], 99);
        assert_eq!(node.data["z"], 3);
    }

    #[test]
    fn mutate_put_edge() {
        let store = CrdtStore::default();
        let op = MutateOp::PutEdge {
            from: "a".to_string(),
            to: "b".to_string(),
            label: Some("knows".to_string()),
        };
        apply_mutate(&store, "test", &[op], false).unwrap();
        let edge = store.get("edge:a:b").unwrap();
        assert_eq!(edge.data["from"], "a");
        assert_eq!(edge.data["to"], "b");
        assert_eq!(edge.data["label"], "knows");
    }

    #[test]
    fn mutate_delete_edge() {
        let store = CrdtStore::default();
        store.put("edge:a:b", "test", serde_json::json!({"_edge": true}));
        let op = MutateOp::DeleteEdge {
            from: "a".to_string(),
            to: "b".to_string(),
        };
        apply_mutate(&store, "test", &[op], false).unwrap();
        assert!(store.get("edge:a:b").is_none());
    }

    #[test]
    fn mutate_atomic_aborts_on_missing() {
        let store = CrdtStore::default();
        let op = MutateOp::Delete { id: "nonexistent".to_string() };
        let result = apply_mutate(&store, "test", &[op], true);
        assert!(result.is_err());
    }

    #[test]
    fn mutate_batch_multiple_ops() {
        let store = CrdtStore::default();
        let ops = vec![
            MutateOp::Put { id: "a".to_string(), data: serde_json::json!({"v": 1}) },
            MutateOp::Put { id: "b".to_string(), data: serde_json::json!({"v": 2}) },
        ];
        let count = apply_mutate(&store, "test", &ops, false).unwrap();
        assert_eq!(count, 2);
        assert!(store.get("a").is_some());
        assert!(store.get("b").is_some());
    }
}
