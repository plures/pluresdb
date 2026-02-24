//! `ProcedureEngine` — executes a pipeline of [`Step`]s against a [`CrdtStore`].

use pluresdb_core::{CrdtStore, NodeRecord};

use crate::ir::{ProcedureResult, Step};
use crate::ops::{aggregate, filter, mutate, project, sort};

/// Executes query pipelines (sequences of [`Step`]s) against a [`CrdtStore`].
///
/// # Example
///
/// ```rust
/// use pluresdb_core::CrdtStore;
/// use pluresdb_procedures::engine::ProcedureEngine;
/// use pluresdb_procedures::ir::{Predicate, Step, SortDir};
///
/// let store = CrdtStore::default();
/// store.put("n1", "actor", serde_json::json!({"category": "decision", "score": 0.9}));
/// store.put("n2", "actor", serde_json::json!({"category": "note", "score": 0.2}));
///
/// let engine = ProcedureEngine::new(&store, "actor");
/// let steps = vec![
///     Step::Filter { predicate: Predicate::eq("category", "decision") },
/// ];
/// let result = engine.exec(&steps).unwrap();
/// assert_eq!(result.nodes.len(), 1);
/// ```
pub struct ProcedureEngine<'a> {
    store: &'a CrdtStore,
    actor: String,
}

impl<'a> ProcedureEngine<'a> {
    /// Create a new engine bound to `store`, using `actor` for any mutate ops.
    pub fn new(store: &'a CrdtStore, actor: impl Into<String>) -> Self {
        ProcedureEngine {
            store,
            actor: actor.into(),
        }
    }

    /// Execute a pipeline of [`Step`]s and return the result.
    ///
    /// The pipeline starts with all nodes in the store.  Each step transforms
    /// the running set in order.  A `mutate` step writes to the store but
    /// passes the (unchanged) node set through.  An `aggregate` step is
    /// terminal — the engine stops there and returns an `AggResult`.
    pub fn exec(&self, steps: &[Step]) -> anyhow::Result<ProcedureResult> {
        // Bootstrap with all nodes from the store.
        let mut nodes: Vec<NodeRecord> = self.store.list();
        let mut pending_limit: Option<usize> = None;
        let mut pending_after: Option<String> = None;

        for step in steps {
            match step {
                Step::Filter { predicate } => {
                    nodes = filter::apply_filter(nodes, predicate);
                }
                Step::Sort { by, dir, after } => {
                    nodes = sort::apply_sort(
                        nodes,
                        by.as_str(),
                        dir,
                        pending_limit.take(),
                        after.as_deref().or(pending_after.as_deref()),
                    );
                    pending_after = None;
                }
                Step::Limit { n } => {
                    pending_limit = Some(*n);
                }
                Step::Project { fields } => {
                    nodes = project::apply_project(nodes, fields);
                }
                Step::Mutate { ops, atomic } => {
                    let n = mutate::apply_mutate(self.store, &self.actor, ops, *atomic)?;
                    return Ok(ProcedureResult {
                        nodes: vec![],
                        aggregate: None,
                        mutated: Some(n),
                    });
                }
                Step::Aggregate { func, field } => {
                    let result =
                        aggregate::apply_aggregate(&nodes, func, field.as_deref());
                    return Ok(ProcedureResult {
                        nodes: vec![],
                        aggregate: Some(result),
                        mutated: None,
                    });
                }
            }
        }

        // Apply any trailing limit that wasn't consumed by a sort step.
        if let Some(n) = pending_limit {
            nodes.truncate(n);
        }

        let node_json: Vec<serde_json::Value> = nodes
            .into_iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.id,
                    "data": n.data,
                    "timestamp": n.timestamp.to_rfc3339(),
                })
            })
            .collect();

        Ok(ProcedureResult::from_nodes(node_json))
    }

    /// Execute a DSL query string.
    ///
    /// Parses the string with [`crate::parser::parse_query`] then calls
    /// [`exec`][Self::exec].
    pub fn exec_dsl(&self, query: &str) -> anyhow::Result<ProcedureResult> {
        let steps = crate::parser::parse_query(query)
            .map_err(|e| anyhow::anyhow!("parse error: {}", e))?;
        self.exec(&steps)
    }

    /// Execute a JSON IR payload.
    ///
    /// The `ir` value must be a JSON array of step objects as produced by
    /// [`serde_json::to_value`] on a `Vec<Step>`.
    pub fn exec_ir(&self, ir: &serde_json::Value) -> anyhow::Result<ProcedureResult> {
        let steps: Vec<Step> = serde_json::from_value(ir.clone())
            .map_err(|e| anyhow::anyhow!("IR deserialisation error: {}", e))?;
        self.exec(&steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::*;
    use pluresdb_core::CrdtStore;

    fn populate(store: &CrdtStore) {
        store.put("n1", "actor", serde_json::json!({"category": "decision", "score": 0.9, "status": "open"}));
        store.put("n2", "actor", serde_json::json!({"category": "note",     "score": 0.2, "status": "open"}));
        store.put("n3", "actor", serde_json::json!({"category": "decision", "score": 0.5, "status": "closed"}));
        store.put("n4", "actor", serde_json::json!({"category": "task",     "score": 0.7, "status": "open"}));
        store.put("n5", "actor", serde_json::json!({"category": "decision", "score": 0.3, "status": "pending"}));
    }

    #[test]
    fn filter_then_sort_then_limit() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![
            Step::Filter {
                predicate: Predicate::eq("category", "decision"),
            },
            Step::Sort {
                by: "score".to_string(),
                dir: SortDir::Desc,
                after: None,
            },
            Step::Limit { n: 2 },
        ];
        let result = engine.exec(&steps).unwrap();
        assert_eq!(result.nodes.len(), 2);
        // Highest scores first: n1 (0.9), n3 (0.5)
        assert_eq!(result.nodes[0]["id"], "n1");
    }

    #[test]
    fn filter_project_pipeline() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![
            Step::Filter {
                predicate: Predicate::eq("status", "open"),
            },
            Step::Project {
                fields: vec![FieldSpec::Plain("category".to_string())],
            },
        ];
        let result = engine.exec(&steps).unwrap();
        assert_eq!(result.nodes.len(), 3);
        // Projected nodes should only have "category" key
        for node in &result.nodes {
            assert!(node["data"].get("score").is_none());
            assert!(node["data"].get("category").is_some());
        }
    }

    #[test]
    fn aggregate_count_all() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![Step::Aggregate {
            func: AggFn::Count,
            field: None,
        }];
        let result = engine.exec(&steps).unwrap();
        assert_eq!(result.aggregate, Some(AggResult::Count(5)));
    }

    #[test]
    fn mutate_put_via_engine() {
        let store = CrdtStore::default();
        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![Step::Mutate {
            ops: vec![MutateOp::Put {
                id: "new1".to_string(),
                data: serde_json::json!({"value": 42}),
            }],
            atomic: false,
        }];
        let result = engine.exec(&steps).unwrap();
        assert_eq!(result.mutated, Some(1));
        assert!(store.get("new1").is_some());
    }

    #[test]
    fn dsl_pipe_chain() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");
        let result = engine
            .exec_dsl(r#"filter(category == "decision") |> sort(by: "score", dir: "desc") |> limit(2)"#)
            .unwrap();
        assert_eq!(result.nodes.len(), 2);
    }

    #[test]
    fn json_ir_roundtrip_execution() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![
            Step::Filter {
                predicate: Predicate::eq("category", "decision"),
            },
            Step::Sort {
                by: "score".to_string(),
                dir: SortDir::Desc,
                after: None,
            },
        ];
        let ir_json = serde_json::to_value(&steps).unwrap();
        let result = engine.exec_ir(&ir_json).unwrap();
        assert_eq!(result.nodes.len(), 3);
    }

    #[test]
    fn multi_step_with_or_predicate() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![
            Step::Filter {
                predicate: Predicate::or(vec![
                    Predicate::eq("status", "open"),
                    Predicate::eq("status", "pending"),
                ]),
            },
            Step::Sort {
                by: "score".to_string(),
                dir: SortDir::Desc,
                after: None,
            },
            Step::Limit { n: 3 },
        ];
        let result = engine.exec(&steps).unwrap();
        assert_eq!(result.nodes.len(), 3);
    }

    #[test]
    fn filter_aggregate_sum() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![
            Step::Filter {
                predicate: Predicate::eq("category", "decision"),
            },
            Step::Aggregate {
                func: AggFn::Sum,
                field: Some("score".to_string()),
            },
        ];
        let result = engine.exec(&steps).unwrap();
        // decisions: 0.9 + 0.5 + 0.3 = 1.7
        if let Some(AggResult::Number(sum)) = result.aggregate {
            assert!((sum - 1.7).abs() < 1e-9);
        } else {
            panic!("expected Number aggregate");
        }
    }

    #[test]
    fn trailing_limit_without_sort() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![Step::Limit { n: 2 }];
        let result = engine.exec(&steps).unwrap();
        assert_eq!(result.nodes.len(), 2);
    }
}
