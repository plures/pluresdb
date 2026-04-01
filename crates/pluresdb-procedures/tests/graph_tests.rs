//! Integration tests for Phase 2A graph operations.
//!
//! These tests exercise `graph_neighbors`, `graph_links`, and `auto_link`
//! through the [`ProcedureEngine`] using both the builder API and the DSL.

use pluresdb_core::CrdtStore;
use pluresdb_procedures::{engine::ProcedureEngine, ir::*, parser::parse_query};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Build a store with 6 content nodes and 5 pre-defined edges.
///
/// ```
/// n1 (dev)  --related/0.9--> n2 (dev)
/// n2 (dev)  --related/0.5--> n3 (design)
/// n3 (design) --blocks/0.8--> n4 (dev)
/// n1 (dev)  --depends/0.7--> n4 (dev)
/// n5 (ops)  --related/0.9--> n6 (ops)
/// ```
fn make_graph_store() -> CrdtStore {
    let store = CrdtStore::default();
    store.put(
        "n1",
        "actor",
        serde_json::json!({"category": "dev",    "text": "rust async programming", "score": 0.9}),
    );
    store.put(
        "n2",
        "actor",
        serde_json::json!({"category": "dev",    "text": "rust sync programming",  "score": 0.7}),
    );
    store.put(
        "n3",
        "actor",
        serde_json::json!({"category": "design", "text": "ui layout wireframe",    "score": 0.5}),
    );
    store.put(
        "n4",
        "actor",
        serde_json::json!({"category": "dev",    "text": "python async scripting", "score": 0.6}),
    );
    store.put(
        "n5",
        "actor",
        serde_json::json!({"category": "ops",    "text": "kubernetes deployment",  "score": 0.8}),
    );
    store.put(
        "n6",
        "actor",
        serde_json::json!({"category": "ops",    "text": "docker compose setup",   "score": 0.4}),
    );

    // Edges
    store.put(
        "edge:n1:n2",
        "actor",
        serde_json::json!({"_edge":true,"from":"n1","to":"n2","label":"related","strength":0.9}),
    );
    store.put(
        "edge:n2:n3",
        "actor",
        serde_json::json!({"_edge":true,"from":"n2","to":"n3","label":"related","strength":0.5}),
    );
    store.put(
        "edge:n3:n4",
        "actor",
        serde_json::json!({"_edge":true,"from":"n3","to":"n4","label":"blocks","strength":0.8}),
    );
    store.put(
        "edge:n1:n4",
        "actor",
        serde_json::json!({"_edge":true,"from":"n1","to":"n4","label":"depends","strength":0.7}),
    );
    store.put(
        "edge:n5:n6",
        "actor",
        serde_json::json!({"_edge":true,"from":"n5","to":"n6","label":"related","strength":0.9}),
    );
    store
}

// ---------------------------------------------------------------------------
// graph_links via engine
// ---------------------------------------------------------------------------

#[test]
fn engine_graph_links_returns_all_edges() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::GraphLinks {
            from: None,
            to: None,
            min_strength: None,
            link_type: None,
        }])
        .unwrap();
    assert_eq!(result.nodes.len(), 5);
    for node in &result.nodes {
        assert_eq!(node["data"]["_edge"], true);
    }
}

#[test]
fn engine_graph_links_filter_from() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::GraphLinks {
            from: Some("n1".to_string()),
            to: None,
            min_strength: None,
            link_type: None,
        }])
        .unwrap();
    assert_eq!(result.nodes.len(), 2); // n1→n2, n1→n4
    for node in &result.nodes {
        assert_eq!(node["data"]["from"], "n1");
    }
}

#[test]
fn engine_graph_links_filter_min_strength() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::GraphLinks {
            from: None,
            to: None,
            min_strength: Some(0.8),
            link_type: None,
        }])
        .unwrap();
    // edges with strength ≥ 0.8: n1→n2 (0.9), n3→n4 (0.8), n5→n6 (0.9)
    assert_eq!(result.nodes.len(), 3);
    for node in &result.nodes {
        let strength = node["data"]["strength"].as_f64().unwrap();
        assert!(strength >= 0.8);
    }
}

#[test]
fn engine_graph_links_filter_type() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::GraphLinks {
            from: None,
            to: None,
            min_strength: None,
            link_type: Some("blocks".to_string()),
        }])
        .unwrap();
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0]["data"]["label"], "blocks");
}

// ---------------------------------------------------------------------------
// graph_neighbors via engine
// ---------------------------------------------------------------------------

#[test]
fn engine_graph_neighbors_depth_1() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::GraphNeighbors {
            root: "n1".to_string(),
            depth: 1,
            min_strength: None,
            link_type: None,
            bidirectional: false,
        }])
        .unwrap();
    // n1 → n2 (via related), n1 → n4 (via depends)
    assert_eq!(result.nodes.len(), 2);
    let ids: Vec<&str> = result
        .nodes
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"n2"));
    assert!(ids.contains(&"n4"));
}

#[test]
fn engine_graph_neighbors_depth_2() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::GraphNeighbors {
            root: "n1".to_string(),
            depth: 2,
            min_strength: None,
            link_type: None,
            bidirectional: false,
        }])
        .unwrap();
    // Depth 2: n2, n4 (depth 1) + n3 (via n2→n3, depth 2)
    let ids: Vec<&str> = result
        .nodes
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"n2"));
    assert!(ids.contains(&"n3"));
    assert!(ids.contains(&"n4"));
    assert!(!ids.contains(&"n1")); // root excluded
}

#[test]
fn engine_graph_neighbors_strength_filter() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    // min_strength 0.8: only n1→n2 (0.9) is above threshold; n1→n4 (0.7) is excluded
    let result = engine
        .exec(&[Step::GraphNeighbors {
            root: "n1".to_string(),
            depth: 2,
            min_strength: Some(0.8),
            link_type: None,
            bidirectional: false,
        }])
        .unwrap();
    let ids: Vec<&str> = result
        .nodes
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"n2"));
    assert!(
        !ids.contains(&"n4"),
        "n4 should be excluded by min_strength"
    );
    // n2→n3 has strength 0.5 → also excluded
    assert!(
        !ids.contains(&"n3"),
        "n3 should be excluded via weak n2→n3 edge"
    );
}

#[test]
fn engine_graph_neighbors_type_filter() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    // Only "related" edges: n1→n2, n2→n3
    let result = engine
        .exec(&[Step::GraphNeighbors {
            root: "n1".to_string(),
            depth: 3,
            min_strength: None,
            link_type: Some("related".to_string()),
            bidirectional: false,
        }])
        .unwrap();
    let ids: Vec<&str> = result
        .nodes
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"n2"));
    assert!(ids.contains(&"n3")); // via n2→n3 (related)
    assert!(
        !ids.contains(&"n4"),
        "n4 reachable only via 'depends'/'blocks', not 'related'"
    );
}

#[test]
fn engine_graph_neighbors_bidirectional() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    // Bidirectional from n3: outgoing → n4 (blocks), incoming ← n2 (related)
    let result = engine
        .exec(&[Step::GraphNeighbors {
            root: "n3".to_string(),
            depth: 1,
            min_strength: None,
            link_type: None,
            bidirectional: true,
        }])
        .unwrap();
    let ids: Vec<&str> = result
        .nodes
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"n2"), "n2 should be found via incoming edge");
    assert!(ids.contains(&"n4"), "n4 should be found via outgoing edge");
}

#[test]
fn engine_graph_neighbors_no_edges_returns_empty() {
    let store = CrdtStore::default();
    store.put("solo", "test", serde_json::json!({}));
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::GraphNeighbors {
            root: "solo".to_string(),
            depth: 3,
            min_strength: None,
            link_type: None,
            bidirectional: false,
        }])
        .unwrap();
    assert!(result.nodes.is_empty());
}

// ---------------------------------------------------------------------------
// auto_link via engine
// ---------------------------------------------------------------------------

#[test]
fn engine_auto_link_category_creates_edges() {
    let store = CrdtStore::default();
    store.put("a", "test", serde_json::json!({"category": "backend"}));
    store.put("b", "test", serde_json::json!({"category": "backend"}));
    store.put("c", "test", serde_json::json!({"category": "frontend"}));

    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::AutoLink {
            algorithms: vec!["category".to_string()],
            min_strength: None,
        }])
        .unwrap();
    // Only a↔b share "backend"
    assert_eq!(result.nodes.len(), 1);
    assert_eq!(result.nodes[0]["data"]["label"], "category");
}

#[test]
fn engine_auto_link_semantic_creates_edges() {
    let store = CrdtStore::default();
    store.put(
        "a",
        "test",
        serde_json::json!({"text": "machine learning model training"}),
    );
    store.put(
        "b",
        "test",
        serde_json::json!({"text": "machine learning inference serving"}),
    );
    store.put(
        "c",
        "test",
        serde_json::json!({"text": "frontend css styling"}),
    );

    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::AutoLink {
            algorithms: vec!["semantic".to_string()],
            min_strength: Some(0.2),
        }])
        .unwrap();
    // a and b share "machine", "learning" → Jaccard ≥ 0.2
    assert!(!result.nodes.is_empty());
    let has_ab = result.nodes.iter().any(|n| {
        let from = n["data"]["from"].as_str().unwrap_or("");
        let to = n["data"]["to"].as_str().unwrap_or("");
        (from == "a" && to == "b") || (from == "b" && to == "a")
    });
    assert!(has_ab, "expected semantic link between 'a' and 'b'");
}

#[test]
fn engine_auto_link_temporal_all_recent() {
    let store = CrdtStore::default();
    store.put("a", "test", serde_json::json!({"x": 1}));
    store.put("b", "test", serde_json::json!({"x": 2}));
    store.put("c", "test", serde_json::json!({"x": 3}));

    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::AutoLink {
            algorithms: vec!["temporal".to_string()],
            min_strength: Some(0.5),
        }])
        .unwrap();
    // All created within milliseconds → 3 pairs
    assert_eq!(result.nodes.len(), 3);
}

// ---------------------------------------------------------------------------
// Combined pipelines
// ---------------------------------------------------------------------------

#[test]
fn engine_auto_link_then_graph_neighbors() {
    // Create nodes, auto-link by category, then traverse neighbors
    let store = CrdtStore::default();
    store.put("x1", "test", serde_json::json!({"category": "api"}));
    store.put("x2", "test", serde_json::json!({"category": "api"}));
    store.put("x3", "test", serde_json::json!({"category": "db"}));

    let engine = ProcedureEngine::new(&store, "test");

    // First: auto-link by category (x1↔x2 get a "category" edge)
    engine
        .exec(&[Step::AutoLink {
            algorithms: vec!["category".to_string()],
            min_strength: None,
        }])
        .unwrap();

    // Then: find neighbors of x1 bidirectionally (edge direction depends on
    // iteration order of DashMap, so we traverse both directions).
    let result = engine
        .exec(&[Step::GraphNeighbors {
            root: "x1".to_string(),
            depth: 1,
            min_strength: None,
            link_type: None,
            bidirectional: true,
        }])
        .unwrap();
    let ids: Vec<&str> = result
        .nodes
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&"x2"),
        "x2 should be a neighbor of x1 via auto-link"
    );
}

#[test]
fn engine_graph_neighbors_then_filter() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    // Get depth-2 neighbors of n1, then filter to only "dev" category
    let result = engine
        .exec(&[
            Step::GraphNeighbors {
                root: "n1".to_string(),
                depth: 2,
                min_strength: None,
                link_type: None,
                bidirectional: false,
            },
            Step::Filter {
                predicate: Predicate::eq("category", "dev"),
            },
        ])
        .unwrap();
    // n2 (dev), n4 (dev) are dev; n3 (design) is not
    for node in &result.nodes {
        assert_eq!(node["data"]["category"], "dev");
    }
    assert!(!result.nodes.is_empty());
}

#[test]
fn engine_graph_neighbors_then_sort_and_limit() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[
            Step::GraphNeighbors {
                root: "n1".to_string(),
                depth: 2,
                min_strength: None,
                link_type: None,
                bidirectional: false,
            },
            Step::Sort {
                by: "score".to_string(),
                dir: SortDir::Desc,
                after: None,
            },
            Step::Limit { n: 2 },
        ])
        .unwrap();
    assert_eq!(result.nodes.len(), 2);
    // Verify descending score order
    let scores: Vec<f64> = result
        .nodes
        .iter()
        .map(|n| n["data"]["score"].as_f64().unwrap_or(0.0))
        .collect();
    assert!(scores[0] >= scores[1]);
}

#[test]
fn engine_graph_links_then_sort() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[
            Step::GraphLinks {
                from: None,
                to: None,
                min_strength: None,
                link_type: None,
            },
            Step::Sort {
                by: "strength".to_string(),
                dir: SortDir::Desc,
                after: None,
            },
            Step::Limit { n: 3 },
        ])
        .unwrap();
    assert_eq!(result.nodes.len(), 3);
    let strengths: Vec<f64> = result
        .nodes
        .iter()
        .map(|n| n["data"]["strength"].as_f64().unwrap_or(0.0))
        .collect();
    assert!(strengths[0] >= strengths[1]);
    assert!(strengths[1] >= strengths[2]);
}

#[test]
fn engine_filter_then_auto_link_pipeline() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    // filter dev nodes, then auto-link them by category
    let steps = vec![
        Step::Filter {
            predicate: Predicate::eq("category", "dev"),
        },
        Step::AutoLink {
            algorithms: vec!["category".to_string()],
            min_strength: None,
        },
    ];
    let result = engine.exec(&steps).unwrap();
    // dev nodes: n1, n2, n4 → 3 pairs
    assert_eq!(result.nodes.len(), 3);
    for node in &result.nodes {
        assert_eq!(node["data"]["label"], "category");
    }
}

// ---------------------------------------------------------------------------
// DSL parsing + execution
// ---------------------------------------------------------------------------

#[test]
fn dsl_graph_neighbors_minimal() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec_dsl(r#"graph_neighbors("n1", depth: 1)"#)
        .unwrap();
    assert_eq!(result.nodes.len(), 2); // n2, n4
}

#[test]
fn dsl_graph_links_all() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine.exec_dsl("graph_links()").unwrap();
    assert_eq!(result.nodes.len(), 5);
}

#[test]
fn dsl_graph_links_with_min_strength() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine.exec_dsl("graph_links(min_strength: 0.8)").unwrap();
    assert_eq!(result.nodes.len(), 3);
}

#[test]
fn dsl_auto_link_category() {
    let store = CrdtStore::default();
    store.put("p1", "test", serde_json::json!({"category": "ml"}));
    store.put("p2", "test", serde_json::json!({"category": "ml"}));
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec_dsl(r#"auto_link(algorithms: ["category"])"#)
        .unwrap();
    assert_eq!(result.nodes.len(), 1);
}

#[test]
fn dsl_full_pipeline_filter_auto_link_neighbors() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let dsl = r#"filter(category == "dev") |> auto_link(algorithms: ["category"]) |> graph_neighbors("n1", depth: 1, bidirectional: true)"#;
    // parse check
    let steps = parse_query(dsl).unwrap();
    assert_eq!(steps.len(), 3);
    // execution check: after filter+auto_link(category), n1/n2/n4 are linked;
    // graph_neighbors(n1, bidirectional) finds n2, n4 (direction may vary)
    let result = engine.exec_dsl(dsl).unwrap();
    assert!(!result.nodes.is_empty());
}

#[test]
fn dsl_graph_neighbors_pipeline_with_filter_and_limit() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let dsl = r#"graph_neighbors("n1", depth: 2) |> filter(category == "dev") |> limit(5)"#;
    let result = engine.exec_dsl(dsl).unwrap();
    for node in &result.nodes {
        assert_eq!(node["data"]["category"], "dev");
    }
    assert!(result.nodes.len() <= 5);
}

#[test]
fn dsl_graph_links_from_filter() {
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec_dsl(r#"graph_links(from: "n1", min_strength: 0.7)"#)
        .unwrap();
    for node in &result.nodes {
        assert_eq!(node["data"]["from"], "n1");
        let s = node["data"]["strength"].as_f64().unwrap();
        assert!(s >= 0.7);
    }
}

// ---------------------------------------------------------------------------
// JSON IR serialisation for graph operations
// ---------------------------------------------------------------------------

#[test]
fn json_ir_graph_neighbors_roundtrip() {
    let ir = serde_json::json!([
        {"op": "graph_neighbors", "root": "n1", "depth": 2}
    ]);
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine.exec_ir(&ir).unwrap();
    assert_eq!(result.nodes.len(), 3); // n2, n3, n4
}

#[test]
fn json_ir_graph_links_roundtrip() {
    let ir = serde_json::json!([
        {"op": "graph_links", "min_strength": 0.9}
    ]);
    let store = make_graph_store();
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine.exec_ir(&ir).unwrap();
    assert_eq!(result.nodes.len(), 2); // n1→n2 and n5→n6
}

#[test]
fn json_ir_auto_link_roundtrip() {
    let ir = serde_json::json!([
        {"op": "auto_link", "algorithms": ["category"]}
    ]);
    let store = CrdtStore::default();
    store.put("q1", "test", serde_json::json!({"category": "infra"}));
    store.put("q2", "test", serde_json::json!({"category": "infra"}));
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine.exec_ir(&ir).unwrap();
    assert_eq!(result.nodes.len(), 1);
}

#[test]
fn json_ir_graph_step_serialise_deserialise() {
    // Verify that Step variants round-trip through JSON IR
    let steps = vec![
        Step::GraphNeighbors {
            root: "r1".to_string(),
            depth: 3,
            min_strength: Some(0.7),
            link_type: Some("related".to_string()),
            bidirectional: true,
        },
        Step::GraphLinks {
            from: Some("r1".to_string()),
            to: None,
            min_strength: Some(0.5),
            link_type: None,
        },
        Step::AutoLink {
            algorithms: vec!["semantic".to_string(), "temporal".to_string()],
            min_strength: Some(0.4),
        },
    ];
    let json = serde_json::to_value(&steps).unwrap();
    let back: Vec<Step> = serde_json::from_value(json).unwrap();
    assert_eq!(steps, back);
}

// ---------------------------------------------------------------------------
// Performance / larger dataset tests
// ---------------------------------------------------------------------------

#[test]
fn engine_graph_neighbors_large_chain() {
    // Linear chain: n0→n1→…→n49
    let store = CrdtStore::default();
    for i in 0..50usize {
        store.put(format!("n{}", i), "test", serde_json::json!({"i": i}));
    }
    for i in 0..49usize {
        store.put(
            format!("edge:n{}:n{}", i, i + 1),
            "test",
            serde_json::json!({"_edge":true,"from":format!("n{}", i),"to":format!("n{}", i+1),"strength":1.0}),
        );
    }
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::GraphNeighbors {
            root: "n0".to_string(),
            depth: 10,
            min_strength: None,
            link_type: None,
            bidirectional: false,
        }])
        .unwrap();
    assert_eq!(result.nodes.len(), 10);
}

#[test]
fn engine_graph_links_large_number_of_edges() {
    let store = CrdtStore::default();
    for i in 0..200usize {
        let strength = if i % 3 == 0 { 0.9 } else { 0.3 };
        store.put(
            format!("edge:src{}:dst{}", i, i),
            "test",
            serde_json::json!({"_edge":true,"from":format!("src{}", i),"to":format!("dst{}", i),"label":"test","strength":strength}),
        );
    }
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::GraphLinks {
            from: None,
            to: None,
            min_strength: Some(0.8),
            link_type: None,
        }])
        .unwrap();
    // Every 3rd edge (i % 3 == 0) has strength 0.9; that's indices 0,3,6,… → ceil(200/3) = 67
    assert_eq!(result.nodes.len(), 67);
}

#[test]
fn engine_auto_link_many_nodes_category() {
    // 50 nodes, half in "alpha" half in "beta"
    let store = CrdtStore::default();
    for i in 0..50usize {
        let cat = if i < 25 { "alpha" } else { "beta" };
        store.put(
            format!("m{}", i),
            "test",
            serde_json::json!({"category": cat}),
        );
    }
    let engine = ProcedureEngine::new(&store, "test");
    let result = engine
        .exec(&[Step::AutoLink {
            algorithms: vec!["category".to_string()],
            min_strength: None,
        }])
        .unwrap();
    // 25 alpha nodes → 25*24/2 = 300 pairs; 25 beta nodes → 300 pairs; total 600
    assert_eq!(result.nodes.len(), 600);
}
