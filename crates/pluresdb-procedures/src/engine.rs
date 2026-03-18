//! `ProcedureEngine` — executes a pipeline of [`Step`]s against a [`CrdtStore`].

use pluresdb_core::{CrdtStore, NodeRecord};

use crate::ir::{ProcedureResult, Step};
use crate::ops::{aggregate, filter, graph, mutate, project, sort};

/// Executes query pipelines (sequences of [`Step`]s) against a [`CrdtStore`].
///
/// # Performance note
///
/// The engine bootstraps each pipeline by calling [`CrdtStore::list`], which
/// returns all nodes currently in the store.  For stores backed by SQLite this
/// is a full table scan; for large databases you should apply selective
/// `filter` steps early in the pipeline to keep the working set small.
///
/// A lightweight optimisation is applied automatically: when the pipeline
/// contains a `Limit` step with **no preceding `Filter`** the initial list is
/// pre-truncated to that limit so that sort/project do not operate on more
/// nodes than necessary.
///
/// Push-down filtering to the storage layer (e.g. SQL `WHERE` clauses) is
/// planned for a future phase.
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
        // Optimisation: if the pipeline has a Limit before any Filter we can
        // truncate the initial list right away and avoid sorting/projecting
        // more nodes than the caller will ever see.
        let pre_limit = leading_limit_without_filter(steps);

        let mut nodes: Vec<NodeRecord> = {
            let mut all = self.store.list();
            if let Some(n) = pre_limit {
                all.truncate(n);
            }
            all
        };
        let mut pending_limit: Option<usize> = None;

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
                        after.as_deref(),
                    );
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
                // Graph steps replace the working node set and continue through the
                // pipeline, enabling downstream sort / filter / limit / project steps.
                Step::GraphClusters { algorithm, min_size, min_strength } => {
                    nodes = graph::graph_clusters(
                        self.store, algorithm, *min_size, *min_strength,
                    )?;
                }
                Step::GraphPath { from, to, max_hops } => {
                    nodes = graph::graph_path(self.store, from, to, *max_hops)?;
                }
                Step::GraphPagerank { damping, iterations } => {
                    nodes = graph::graph_pagerank(self.store, *damping, *iterations)?;
                }
                Step::GraphStats => {
                    nodes = graph::graph_stats(self.store)?;
                }
                Step::GraphNeighbors { root, depth, min_strength, link_type, bidirectional } => {
                    nodes = crate::ops::graph::graph_neighbors(
                        self.store,
                        root.as_str(),
                        *depth,
                        *min_strength,
                        link_type.as_deref(),
                        *bidirectional,
                    );
                }
                Step::GraphLinks { from, to, min_strength, link_type } => {
                    nodes = crate::ops::graph::graph_links(
                        self.store,
                        from.as_deref(),
                        to.as_deref(),
                        *min_strength,
                        link_type.as_deref(),
                    );
                }
                Step::AutoLink { algorithms, min_strength } => {
                    // When no algorithms are specified default to all three so
                    // that `auto_link()` is a useful no-arg shorthand.
                    let defaults: Vec<String>;
                    let effective: &[String] = if algorithms.is_empty() {
                        defaults = vec![
                            "semantic".to_string(),
                            "category".to_string(),
                            "temporal".to_string(),
                        ];
                        &defaults
                    } else {
                        algorithms
                    };
                    let alg_refs: Vec<&str> = effective.iter().map(|s| s.as_str()).collect();
                    let strength = min_strength.unwrap_or(0.5);
                    nodes = crate::ops::graph::auto_link(
                        self.store,
                        &self.actor,
                        &nodes,
                        &alg_refs,
                        strength,
                    );
                }
                Step::ChronicleTrace { root, max_depth, direction } => {
                    nodes = crate::ops::graph::chronicle_trace(
                        self.store,
                        root.as_str(),
                        *max_depth,
                        direction.as_str(),
                    );
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

/// Return the minimum `Limit` value that appears before any `Filter` step in
/// the pipeline, or `None` if no such limit exists.
///
/// This is used by [`ProcedureEngine::exec`] to pre-truncate the initial node
/// list when the query has no filter steps at all (e.g. `sort |> limit`),
/// avoiding unnecessary work on the full node set.
fn leading_limit_without_filter(steps: &[Step]) -> Option<usize> {
    let mut min_limit: Option<usize> = None;
    for step in steps {
        match step {
            Step::Filter { .. } => break, // filter found — optimisation doesn't apply
            // Graph steps replace the initial node set entirely, so pre-truncating
            // the initial list offers no benefit.
            Step::GraphClusters { .. }
            | Step::GraphPath { .. }
            | Step::GraphPagerank { .. }
            | Step::GraphStats
            | Step::ChronicleTrace { .. } => break,
            Step::Limit { n } => {
                min_limit = Some(match min_limit {
                    Some(prev) => prev.min(*n),
                    None => *n,
                });
            }
            _ => {}
        }
    }
    min_limit
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

    #[test]
    fn auto_link_empty_algorithms_defaults_to_all_three() {
        // auto_link() with no algorithms should default to semantic+category+temporal
        // and create edges (all 5 nodes were created moments apart → temporal links).
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");
        let result = engine
            .exec(&[Step::AutoLink { algorithms: vec![], min_strength: None }])
            .unwrap();
        // With all three algorithms, some edges must be created (temporal at minimum).
        assert!(!result.nodes.is_empty(), "expected edges from default algorithms");
    }
}
