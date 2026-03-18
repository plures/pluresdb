//! `ProcedureEngine` — executes a pipeline of [`Step`]s against a [`CrdtStore`].

use pluresdb_core::{CrdtStore, NodeRecord};

use crate::ir::{ProcedureResult, Step};
use crate::ops::{aggregate, filter, graph, mutate, project, search, sort, transform};

use std::collections::HashMap;

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
        let mut variables: HashMap<String, Vec<NodeRecord>> = HashMap::new();

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

                // ---- Cognitive architecture steps ----

                Step::VectorSearch { query, limit, min_score, category } => {
                    nodes = search::apply_vector_search(
                        self.store,
                        query,
                        *limit,
                        *min_score,
                        category.as_deref(),
                    );
                }
                Step::TextSearch { query, limit, field } => {
                    nodes = search::apply_text_search(
                        self.store,
                        query,
                        *limit,
                        field,
                    );
                }
                Step::Transform { format, max_chars } => {
                    nodes = transform::apply_transform(nodes, format, *max_chars);
                }
                Step::Conditional { condition, then_steps, else_steps } => {
                    let take_then = nodes
                        .first()
                        .map(|n| filter::eval_predicate(condition, &n.data))
                        .unwrap_or(false);
                    let branch = if take_then { then_steps } else { else_steps };
                    if !branch.is_empty() {
                        nodes = self.exec_with_nodes(branch, nodes, &mut variables)?;
                    }
                }
                Step::Assign { name } => {
                    variables.insert(name.clone(), nodes.clone());
                }
                Step::Emit { label, from_var } => {
                    let emit_nodes = match from_var {
                        Some(var) => variables.get(var).cloned().unwrap_or_default(),
                        None => nodes,
                    };
                    let node_json: Vec<serde_json::Value> = emit_nodes
                        .into_iter()
                        .map(|n| {
                            serde_json::json!({
                                "id": n.id,
                                "data": n.data,
                                "timestamp": n.timestamp.to_rfc3339(),
                                "_label": label,
                            })
                        })
                        .collect();
                    return Ok(ProcedureResult::from_nodes(node_json));
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

    /// Execute a pipeline starting with a pre-populated node set and shared
    /// variable context (used by `Conditional` branches).
    ///
    /// Returns the transformed node set so that the outer pipeline can continue
    /// operating on it.  Only data-transform steps are permitted; other steps
    /// return an error.
    fn exec_with_nodes(
        &self,
        steps: &[Step],
        initial_nodes: Vec<NodeRecord>,
        variables: &mut HashMap<String, Vec<NodeRecord>>,
    ) -> anyhow::Result<Vec<NodeRecord>> {
        let mut nodes = initial_nodes;
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
                Step::Transform { format, max_chars } => {
                    nodes = transform::apply_transform(nodes, format, *max_chars);
                }
                Step::Assign { name } => {
                    variables.insert(name.clone(), nodes.clone());
                }
                Step::Emit { from_var, .. } => {
                    // In a branch sub-pipeline `Emit` acts as a terminal
                    // selector: return the named variable's node set (or the
                    // current set) so that the outer pipeline can continue.
                    let emit_nodes = match from_var {
                        Some(var) => variables.get(var).cloned().unwrap_or_default(),
                        None => nodes,
                    };
                    return Ok(emit_nodes);
                }
                _ => {
                    // For branch sub-pipelines, only support data-transform steps.
                    // Full step set available via top-level exec().
                    return Err(anyhow::anyhow!(
                        "Unsupported step in branch sub-pipeline; only data-transform steps \
                         (filter, sort, limit, project, transform, assign, emit) are allowed"
                    ));
                }
            }
        }

        if let Some(n) = pending_limit {
            nodes.truncate(n);
        }

        Ok(nodes)
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
            // Graph and search steps replace the initial node set entirely, so pre-truncating
            // the initial list offers no benefit.
            Step::GraphClusters { .. }
            | Step::GraphPath { .. }
            | Step::GraphPagerank { .. }
            | Step::GraphStats
            | Step::VectorSearch { .. }
            | Step::TextSearch { .. } => break,
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

    // ---- Cognitive architecture step tests ----

    #[test]
    fn text_search_step_via_engine() {
        let store = CrdtStore::default();
        store.put("t1", "actor", serde_json::json!({"text": "Rust is fast and safe", "score": 0.9}));
        store.put("t2", "actor", serde_json::json!({"text": "Python is great for scripting", "score": 0.5}));
        store.put("t3", "actor", serde_json::json!({"text": "Rust powers PluresDB performance", "score": 0.7}));

        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![Step::TextSearch {
            query: "rust".to_string(),
            limit: 10,
            field: "text".to_string(),
        }];
        let result = engine.exec(&steps).unwrap();
        // Both t1 and t3 contain "rust"
        assert_eq!(result.nodes.len(), 2);
        let ids: Vec<&str> = result.nodes.iter()
            .map(|n| n["id"].as_str().unwrap())
            .collect();
        assert!(ids.contains(&"t1"));
        assert!(ids.contains(&"t3"));
    }

    #[test]
    fn text_search_is_case_insensitive() {
        let store = CrdtStore::default();
        store.put("t1", "actor", serde_json::json!({"text": "RUST is blazingly fast"}));
        store.put("t2", "actor", serde_json::json!({"text": "Python is dynamic"}));

        let engine = ProcedureEngine::new(&store, "test");
        // Query "rust" (lowercase) should match "RUST" (uppercase) in t1
        let result = engine.exec(&[Step::TextSearch {
            query: "rust".to_string(),
            limit: 10,
            field: "text".to_string(),
        }]).unwrap();
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0]["id"], "t1");
    }

    #[test]
    fn text_search_step_respects_limit() {
        let store = CrdtStore::default();
        for i in 0..10 {
            store.put(
                &format!("n{}", i),
                "actor",
                serde_json::json!({"text": format!("memory entry {}", i)}),
            );
        }
        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![Step::TextSearch {
            query: "memory".to_string(),
            limit: 3,
            field: "text".to_string(),
        }];
        let result = engine.exec(&steps).unwrap();
        assert_eq!(result.nodes.len(), 3);
    }

    #[test]
    fn transform_structured_via_engine() {
        let store = CrdtStore::default();
        store.put("d1", "actor", serde_json::json!({"category": "decision", "text": "Use Rust", "score": 0.9}));
        store.put("d2", "actor", serde_json::json!({"category": "decision", "text": "Adopt CRDT", "score": 0.8}));
        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![
            Step::Filter { predicate: Predicate::eq("category", "decision") },
            Step::Transform { format: crate::ir::TransformFormat::Structured, max_chars: 0 },
        ];
        let result = engine.exec(&steps).unwrap();
        assert_eq!(result.nodes.len(), 2);
        // Structured format replaces data with {category, text, score}
        for node in &result.nodes {
            assert!(node["data"]["category"].is_string());
            assert!(node["data"]["text"].is_string());
            assert!(node["data"]["score"].is_number());
        }
    }

    #[test]
    fn transform_toon_via_engine() {
        let store = CrdtStore::default();
        store.put("x1", "actor", serde_json::json!({"category": "decision", "text": "Use Rust", "score": 0.9}));
        let engine = ProcedureEngine::new(&store, "test");
        let steps = vec![
            Step::Transform { format: crate::ir::TransformFormat::Toon, max_chars: 0 },
        ];
        let result = engine.exec(&steps).unwrap();
        assert_eq!(result.nodes.len(), 1);
        let toon = result.nodes[0]["data"]["toon"].as_str().unwrap();
        assert!(toon.starts_with("[D|0.9]"), "expected TOON notation starting with [D|0.9], got: {}", toon);
    }

    #[test]
    fn assign_then_emit_from_var() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");

        // Filter decisions into `my_var`, then filter further to open ones,
        // then emit from `my_var` — should return the decisions (not just open ones).
        let steps = vec![
            Step::Filter { predicate: Predicate::eq("category", "decision") },
            Step::Assign { name: "my_var".to_string() },
            Step::Filter { predicate: Predicate::eq("status", "open") },
            Step::Emit {
                label: "decisions".to_string(),
                from_var: Some("my_var".to_string()),
            },
        ];
        let result = engine.exec(&steps).unwrap();
        // my_var holds all 3 decisions; the further filter narrows to 1 open
        // decision, but Emit(from_var) returns the full assigned set of 3.
        assert_eq!(result.nodes.len(), 3, "emit from_var should return the assigned variable, not the post-filter set");
        assert!(result.nodes.iter().all(|n| n["_label"] == "decisions"));
    }

    #[test]
    fn emit_current_nodes_with_label() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");

        let steps = vec![
            Step::Filter { predicate: Predicate::eq("status", "open") },
            Step::Emit { label: "open_nodes".to_string(), from_var: None },
        ];
        let result = engine.exec(&steps).unwrap();
        // 3 open nodes from populate()
        assert_eq!(result.nodes.len(), 3);
        assert!(result.nodes.iter().all(|n| n["_label"] == "open_nodes"));
    }

    #[test]
    fn conditional_takes_then_branch() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");

        // Filter to decisions (category == "decision"), then run a Conditional
        // that checks if first node has category == "decision" (true).
        // then_steps: keep only score > 0.7 → n1 (0.9)
        // else_steps: keep only score < 0.3 → none from this set
        let steps = vec![
            Step::Filter { predicate: Predicate::eq("category", "decision") },
            Step::Conditional {
                condition: Predicate::eq("category", "decision"),
                then_steps: vec![
                    Step::Filter {
                        predicate: Predicate::Comparison {
                            field: "score".to_string(),
                            cmp: CmpOp::Gt,
                            value: IrValue::Number(0.7),
                        },
                    },
                ],
                else_steps: vec![
                    Step::Filter { predicate: Predicate::eq("status", "closed") },
                ],
            },
        ];
        let result = engine.exec(&steps).unwrap();
        // then branch: decisions with score > 0.7 → only n1 (score 0.9)
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0]["id"], "n1");
    }

    #[test]
    fn conditional_takes_else_branch() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");

        // Filter to notes (category == "note") — only n2.
        // Conditional checks category == "decision" (false for a note node).
        // else_steps: filter status == "open" → n2 is open, so it stays.
        // then_steps: filter status == "closed" → would remove n2.
        let steps = vec![
            Step::Filter { predicate: Predicate::eq("category", "note") },
            Step::Conditional {
                condition: Predicate::eq("category", "decision"),
                then_steps: vec![
                    Step::Filter { predicate: Predicate::eq("status", "closed") },
                ],
                else_steps: vec![
                    Step::Filter { predicate: Predicate::eq("status", "open") },
                ],
            },
        ];
        let result = engine.exec(&steps).unwrap();
        // else branch taken: note nodes with status == "open" → only n2
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0]["id"], "n2");
    }

    #[test]
    fn conditional_with_empty_set_takes_else() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");

        // Filter to non-existent category → empty set.
        // Conditional: empty nodes → else branch taken (condition not met).
        let steps = vec![
            Step::Filter { predicate: Predicate::eq("category", "nonexistent") },
            Step::Conditional {
                condition: Predicate::eq("category", "decision"),
                then_steps: vec![],
                else_steps: vec![
                    // else_steps is empty too, so nodes stay empty
                ],
            },
        ];
        let result = engine.exec(&steps).unwrap();
        assert_eq!(result.nodes.len(), 0);
    }

    #[test]
    fn steps_after_conditional_still_run() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");

        // Filter to decisions (3 nodes), Conditional passes them through
        // (then_steps empty), then Limit(1) should still apply.
        let steps = vec![
            Step::Filter { predicate: Predicate::eq("category", "decision") },
            Step::Conditional {
                condition: Predicate::eq("category", "decision"),
                then_steps: vec![],  // empty: nodes pass through unchanged
                else_steps: vec![],
            },
            Step::Sort {
                by: "score".to_string(),
                dir: SortDir::Desc,
                after: None,
            },
            Step::Limit { n: 1 },
        ];
        let result = engine.exec(&steps).unwrap();
        // Conditional passes all 3 decisions through; Sort+Limit yields just n1
        assert_eq!(result.nodes.len(), 1, "steps after Conditional must still execute");
        assert_eq!(result.nodes[0]["id"], "n1");
    }

    #[test]
    fn assign_binds_current_set_and_pipeline_continues() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");

        // Assign does NOT consume the node set; the pipeline should continue
        // with the same nodes after Assign.
        let steps = vec![
            Step::Filter { predicate: Predicate::eq("category", "decision") },
            Step::Assign { name: "snap".to_string() },
            // Further filter: should still work on the decision set
            Step::Filter { predicate: Predicate::eq("status", "open") },
        ];
        let result = engine.exec(&steps).unwrap();
        // Only n1 is a decision AND open
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0]["id"], "n1");
    }

    #[test]
    fn emit_from_nonexistent_var_returns_empty() {
        let store = CrdtStore::default();
        populate(&store);
        let engine = ProcedureEngine::new(&store, "test");

        // Emit referencing a variable that was never Assign-ed → empty result.
        let steps = vec![
            Step::Filter { predicate: Predicate::eq("category", "decision") },
            Step::Emit {
                label: "result".to_string(),
                from_var: Some("no_such_var".to_string()),
            },
        ];
        let result = engine.exec(&steps).unwrap();
        assert_eq!(result.nodes.len(), 0, "emit from unknown variable should return empty set");
    }
}
