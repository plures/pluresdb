//! Integration tests: multi-step pipe chains executed against an in-memory
//! `CrdtStore`.

use pluresdb_core::CrdtStore;
use pluresdb_procedures::{
    builder::{MutateBuilder, QueryBuilder},
    engine::ProcedureEngine,
    ir::*,
};

fn make_store() -> CrdtStore {
    let store = CrdtStore::default();
    store.put("n1", "actor", serde_json::json!({"category": "decision", "score": 0.9, "status": "open",    "tag": "alpha"}));
    store.put("n2", "actor", serde_json::json!({"category": "note",     "score": 0.2, "status": "open",    "tag": "beta"}));
    store.put("n3", "actor", serde_json::json!({"category": "decision", "score": 0.5, "status": "closed",  "tag": "alpha"}));
    store.put("n4", "actor", serde_json::json!({"category": "task",     "score": 0.7, "status": "open",    "tag": "gamma"}));
    store.put("n5", "actor", serde_json::json!({"category": "decision", "score": 0.3, "status": "pending", "tag": "beta"}));
    store
}

// ── Chain 1: filter → sort → limit ─────────────────────────────────────────

#[test]
fn chain_filter_sort_limit() {
    let store = make_store();
    let engine = ProcedureEngine::new(&store, "test");
    let steps = QueryBuilder::new()
        .filter(Predicate::eq("category", "decision"))
        .sort_desc("score")
        .limit(2)
        .to_steps();
    let result = engine.exec(&steps).unwrap();
    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.nodes[0]["id"], "n1"); // highest score
}

// ── Chain 2: filter → filter (AND via two filter steps) → project ───────────

#[test]
fn chain_double_filter_project() {
    let store = make_store();
    let engine = ProcedureEngine::new(&store, "test");
    let steps = QueryBuilder::new()
        .filter(Predicate::eq("status", "open"))
        .filter(Predicate::Comparison {
            field: "score".to_string(),
            cmp: CmpOp::Gt,
            value: IrValue::Number(0.5),
        })
        .project(["category", "score"])
        .to_steps();
    let result = engine.exec(&steps).unwrap();
    assert_eq!(result.nodes.len(), 2); // n1 (0.9) and n4 (0.7)
    for node in &result.nodes {
        assert!(node["data"].get("tag").is_none());
    }
}

// ── Chain 3: filter(OR) → sort asc → limit ─────────────────────────────────

#[test]
fn chain_or_filter_sort_limit() {
    let store = make_store();
    let engine = ProcedureEngine::new(&store, "test");
    let steps = QueryBuilder::new()
        .filter(Predicate::or(vec![
            Predicate::eq("status", "open"),
            Predicate::eq("status", "pending"),
        ]))
        .sort("score")
        .limit(3)
        .to_steps();
    let result = engine.exec(&steps).unwrap();
    assert_eq!(result.nodes.len(), 3);
}

// ── Chain 4: filter → aggregate(sum) ───────────────────────────────────────

#[test]
fn chain_filter_aggregate_sum() {
    let store = make_store();
    let engine = ProcedureEngine::new(&store, "test");
    let steps = QueryBuilder::new()
        .filter(Predicate::eq("category", "decision"))
        .aggregate(AggFn::Sum, Some("score"))
        .to_steps();
    let result = engine.exec(&steps).unwrap();
    // decisions: n1=0.9, n3=0.5, n5=0.3 → 1.7
    if let Some(AggResult::Number(sum)) = result.aggregate {
        assert!((sum - 1.7).abs() < 1e-9, "expected 1.7 got {}", sum);
    } else {
        panic!("expected Number aggregate");
    }
}

// ── Chain 5: filter → sort → project → DSL string equivalent ───────────────

#[test]
fn chain_dsl_equivalent_to_builder() {
    let store = make_store();
    let engine = ProcedureEngine::new(&store, "test");

    let builder_result = engine
        .exec(
            &QueryBuilder::new()
                .filter(Predicate::eq("status", "open"))
                .sort_desc("score")
                .limit(2)
                .to_steps(),
        )
        .unwrap();

    let dsl_result = engine
        .exec_dsl(r#"filter(status == "open") |> sort(by: "score", dir: "desc") |> limit(2)"#)
        .unwrap();

    assert_eq!(builder_result.nodes.len(), dsl_result.nodes.len());
}

// ── Chain 6: mutate (put batch) then query ──────────────────────────────────

#[test]
fn chain_mutate_then_query() {
    let store = CrdtStore::default();
    let engine = ProcedureEngine::new(&store, "test");

    // Insert nodes via mutate
    let mutate_step = MutateBuilder::new()
        .put("x1", serde_json::json!({"value": 10, "group": "a"}))
        .put("x2", serde_json::json!({"value": 20, "group": "b"}))
        .put("x3", serde_json::json!({"value": 30, "group": "a"}))
        .to_step();

    let result = engine.exec(&[mutate_step]).unwrap();
    assert_eq!(result.mutated, Some(3));

    // Now query
    let query_result = engine
        .exec(
            &QueryBuilder::new()
                .filter(Predicate::eq("group", "a"))
                .sort_desc("value")
                .to_steps(),
        )
        .unwrap();
    assert_eq!(query_result.nodes.len(), 2);
    assert_eq!(query_result.nodes[0]["id"], "x3");
}

// ── Chain 7: filter(NOT) → aggregate(distinct) ─────────────────────────────

#[test]
fn chain_not_filter_distinct_agg() {
    let store = make_store();
    let engine = ProcedureEngine::new(&store, "test");
    let steps = QueryBuilder::new()
        .filter(Predicate::not(Predicate::eq("status", "closed")))
        .aggregate(AggFn::Distinct, Some("category"))
        .to_steps();
    let result = engine.exec(&steps).unwrap();
    if let Some(AggResult::Values(vals)) = result.aggregate {
        // open/pending nodes cover: decision, note, task, decision
        // distinct: decision, note, task
        assert!(vals.len() <= 4);
        assert!(vals.len() >= 2);
    } else {
        panic!("expected Values aggregate");
    }
}

// ── Chain 8: DSL exec_ir (JSON IR execution) ────────────────────────────────

#[test]
fn chain_exec_ir_json() {
    let store = make_store();
    let engine = ProcedureEngine::new(&store, "test");

    let ir_json = serde_json::json!([
        { "op": "filter", "predicate": { "field": "category", "cmp": "==", "value": "decision" } },
        { "op": "sort", "by": "score", "dir": "desc" },
        { "op": "limit", "n": 2 }
    ]);

    let result = engine.exec_ir(&ir_json).unwrap();
    assert_eq!(result.nodes.len(), 2);
    assert_eq!(result.nodes[0]["id"], "n1");
}

// ── Chain 9: edge operations ────────────────────────────────────────────────

#[test]
fn chain_put_and_delete_edge() {
    let store = CrdtStore::default();
    let engine = ProcedureEngine::new(&store, "test");

    let put_edge = Step::Mutate {
        ops: vec![MutateOp::PutEdge {
            from: "n1".to_string(),
            to: "n2".to_string(),
            label: Some("related".to_string()),
        }],
        atomic: false,
    };
    engine.exec(&[put_edge]).unwrap();
    assert!(store.get("edge:n1:n2").is_some());

    let del_edge = Step::Mutate {
        ops: vec![MutateOp::DeleteEdge {
            from: "n1".to_string(),
            to: "n2".to_string(),
        }],
        atomic: false,
    };
    engine.exec(&[del_edge]).unwrap();
    assert!(store.get("edge:n1:n2").is_none());
}

// ── Chain 10: 5-step pipeline (DSL) ─────────────────────────────────────────

#[test]
fn chain_5_step_dsl_pipeline() {
    let store = make_store();
    let engine = ProcedureEngine::new(&store, "test");
    let dsl = r#"filter(status == "open") |> filter(score > 0.5) |> sort(by: "score", dir: "desc") |> limit(3) |> project(["category", "score"])"#;
    let result = engine.exec_dsl(dsl).unwrap();
    assert!(result.nodes.len() <= 3);
    for node in &result.nodes {
        assert!(node["data"].get("category").is_some());
        assert!(node["data"].get("status").is_none());
    }
}

// ── Graph analytics steps ───────────────────────────────────────────────────

fn make_graph_store() -> CrdtStore {
    let store = CrdtStore::default();
    store.put("n1", "actor", serde_json::json!({"category": "decision", "label": "Alpha"}));
    store.put("n2", "actor", serde_json::json!({"category": "decision", "label": "Beta"}));
    store.put("n3", "actor", serde_json::json!({"category": "note",     "label": "Gamma"}));
    store.put("n4", "actor", serde_json::json!({"category": "task",     "label": "Delta"}));
    store.put("edge:n1:n2", "actor", serde_json::json!({"_edge": true, "from": "n1", "to": "n2", "weight": 0.9}));
    store.put("edge:n2:n3", "actor", serde_json::json!({"_edge": true, "from": "n2", "to": "n3", "weight": 0.8}));
    store.put("edge:n1:n3", "actor", serde_json::json!({"_edge": true, "from": "n1", "to": "n3", "weight": 0.7}));
    store.put("edge:n3:n4", "actor", serde_json::json!({"_edge": true, "from": "n3", "to": "n4", "weight": 0.5}));
    store
}

#[test]
fn dsl_graph_stats() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine.exec_dsl("graph_stats()").unwrap();
    assert_eq!(result.nodes.len(), 1);
    // Fields are stored inside "data" so downstream pipeline steps can access them.
    assert_eq!(result.nodes[0]["data"]["node_count"].as_u64().unwrap(), 4);
    assert_eq!(result.nodes[0]["data"]["edge_count"].as_u64().unwrap(), 4);
}

#[test]
fn dsl_graph_pagerank() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine.exec_dsl("graph_pagerank()").unwrap();
    assert_eq!(result.nodes.len(), 4);
    for node in &result.nodes {
        // pagerank_score is inside "data".
        assert!(node["data"].get("pagerank_score").is_some());
    }
}

#[test]
fn dsl_graph_pagerank_with_params() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine.exec_dsl("graph_pagerank(damping: 0.85, iterations: 50)").unwrap();
    assert_eq!(result.nodes.len(), 4);
}

#[test]
fn dsl_graph_clusters_louvain() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec_dsl(r#"graph_clusters(algorithm: "louvain", min_size: 2)"#)
        .unwrap();
    assert!(!result.nodes.is_empty());
    for node in &result.nodes {
        assert_eq!(node["data"]["algorithm"], "louvain");
    }
}

#[test]
fn dsl_graph_path_finds_route() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec_dsl(r#"graph_path(from: "n1", to: "n4")"#)
        .unwrap();
    // Path should exist: n1 → n3 → n4 (or n1 → n2 → n3 → n4)
    assert!(!result.nodes.is_empty());
    assert_eq!(result.nodes.first().unwrap()["id"], "n1");
    assert_eq!(result.nodes.last().unwrap()["id"], "n4");
}

#[test]
fn dsl_graph_path_limit() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    // max_hops: 1 means only direct neighbours of n1; n4 is 2 hops away.
    let result = engine
        .exec_dsl(r#"graph_path(from: "n1", to: "n4", max_hops: 1)"#)
        .unwrap();
    assert!(result.nodes.is_empty());
}

// ── Pipeline composition: graph step followed by sort / limit / project ─────

#[test]
fn dsl_graph_pagerank_sort_limit_pipeline() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    // PageRank results contain pagerank_score in data; sort and limit should work.
    let result = engine
        .exec_dsl(r#"graph_pagerank(damping: 0.85) |> sort(by: "pagerank_score", dir: "desc") |> limit(2)"#)
        .unwrap();
    assert_eq!(result.nodes.len(), 2);
    // Verify scores are descending.
    let s0 = result.nodes[0]["data"]["pagerank_score"].as_f64().unwrap();
    let s1 = result.nodes[1]["data"]["pagerank_score"].as_f64().unwrap();
    assert!(s0 >= s1, "expected descending order: {} >= {}", s0, s1);
}

#[test]
fn dsl_graph_pagerank_project_pipeline() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec_dsl(r#"graph_pagerank() |> project(["pagerank_score"])"#)
        .unwrap();
    assert_eq!(result.nodes.len(), 4);
    for node in &result.nodes {
        assert!(node["data"].get("pagerank_score").is_some());
        // Other data fields should have been projected away.
        assert!(node["data"].get("label").is_none());
    }
}

#[test]
fn dsl_graph_clusters_limit_pipeline() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec_dsl(r#"graph_clusters(algorithm: "louvain", min_size: 2) |> limit(1)"#)
        .unwrap();
    assert!(result.nodes.len() <= 1);
}

#[test]
fn dsl_graph_stats_project_pipeline() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec_dsl(r#"graph_stats() |> project(["node_count", "edge_count"])"#)
        .unwrap();
    assert_eq!(result.nodes.len(), 1);
    assert!(result.nodes[0]["data"].get("node_count").is_some());
    assert!(result.nodes[0]["data"].get("edge_count").is_some());
    assert!(result.nodes[0]["data"].get("density").is_none());
}
