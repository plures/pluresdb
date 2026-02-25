//! Graph operations — neighbor traversal, link querying, and auto-linking.
//!
//! Edges are stored as regular nodes with an `_edge: true` marker and a key
//! of the form `"edge::{from}::{to}"` (double-colon separator, produced by
//! [`auto_link`]).  Edges created via `mutate(put_edge, ...)` use the legacy
//! single-colon format `"edge:{from}:{to}"` defined in `ops/mutate.rs`.
//! The double-colon separator in auto-linked edges prevents ambiguity when
//! node IDs themselves contain colons (e.g. `"memory:abc"`).
//!
//! An optional `strength` field (f64 in \[0, 1\]) is written by [`auto_link`];
//! manually created edges (via `mutate(put_edge, ...)`) default to strength
//! `1.0` when absent.

use std::collections::{HashSet, VecDeque};

use pluresdb_core::{CrdtStore, NodeRecord};

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Return `true` if `node` represents an edge.
fn is_edge(node: &NodeRecord) -> bool {
    node.data.get("_edge").and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Extract the numeric strength from an edge node, defaulting to `1.0`.
fn edge_strength(node: &NodeRecord) -> f64 {
    node.data.get("strength").and_then(|v| v.as_f64()).unwrap_or(1.0)
}

/// Extract the label/type from an edge node.
fn edge_label(node: &NodeRecord) -> &str {
    node.data.get("label").and_then(|v| v.as_str()).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Return all edge [`NodeRecord`]s that match the supplied filters.
///
/// All parameters are optional; omitting them returns all edges in the store.
///
/// * `from` – only edges whose `from` field equals this value.
/// * `to` – only edges whose `to` field equals this value.
/// * `min_strength` – edges with `strength < min_strength` are excluded
///   (edges without an explicit strength are treated as `1.0`).
/// * `link_type` – only edges whose `label` field equals this value.
pub fn graph_links(
    store: &CrdtStore,
    from: Option<&str>,
    to: Option<&str>,
    min_strength: Option<f64>,
    link_type: Option<&str>,
) -> Vec<NodeRecord> {
    store
        .list()
        .into_iter()
        .filter(|n| {
            if !is_edge(n) {
                return false;
            }
            if let Some(f) = from {
                if n.data.get("from").and_then(|v| v.as_str()) != Some(f) {
                    return false;
                }
            }
            if let Some(t) = to {
                if n.data.get("to").and_then(|v| v.as_str()) != Some(t) {
                    return false;
                }
            }
            if let Some(s) = min_strength {
                if edge_strength(n) < s {
                    return false;
                }
            }
            if let Some(lt) = link_type {
                if edge_label(n) != lt {
                    return false;
                }
            }
            true
        })
        .collect()
}

/// Traverse the graph from `root` using BFS and return reachable
/// [`NodeRecord`]s within `depth` hops (the root itself is excluded).
///
/// * `min_strength` – edges weaker than this threshold are not traversed.
/// * `link_type` – only edges with this label are traversed.
/// * `bidirectional` – when `true`, incoming edges (where `to == current`)
///   are also followed in addition to outgoing edges.
pub fn graph_neighbors(
    store: &CrdtStore,
    root: &str,
    depth: usize,
    min_strength: Option<f64>,
    link_type: Option<&str>,
    bidirectional: bool,
) -> Vec<NodeRecord> {
    if depth == 0 {
        return Vec::new();
    }

    // Pre-fetch all edges once so we don't repeatedly scan the store.
    let all_edges: Vec<NodeRecord> = graph_links(store, None, None, min_strength, link_type);

    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(root.to_string());

    // Queue entries: (node_id, remaining_depth)
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((root.to_string(), depth));

    while let Some((current, remaining)) = queue.pop_front() {
        if remaining == 0 {
            continue;
        }
        for edge in &all_edges {
            let edge_from = match edge.data.get("from").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s,
                _ => continue, // skip malformed edges missing a non-empty "from"
            };
            let edge_to = match edge.data.get("to").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s,
                _ => continue, // skip malformed edges missing a non-empty "to"
            };

            // Outgoing edge: current → neighbour
            if edge_from == current && !visited.contains(edge_to) {
                visited.insert(edge_to.to_string());
                queue.push_back((edge_to.to_string(), remaining - 1));
            }
            // Incoming edge: neighbour → current (only when bidirectional)
            if bidirectional && edge_to == current && !visited.contains(edge_from) {
                visited.insert(edge_from.to_string());
                queue.push_back((edge_from.to_string(), remaining - 1));
            }
        }
    }

    // Remove root from the set – we return *neighbours*, not the root itself.
    visited.remove(root);

    store
        .list()
        .into_iter()
        .filter(|n| visited.contains(&n.id) && !is_edge(n))
        .collect()
}

/// Automatically create links between the nodes in `input` using the
/// specified `algorithms` and store them in `store`.
///
/// Returns the newly created edge [`NodeRecord`]s.
///
/// # Algorithms
///
/// * `"semantic"` – Jaccard similarity over whitespace-split tokens from
///   `data.text`, `data.tags`, and `data.category`.  Links are created for
///   pairs whose similarity ≥ `min_strength`.
/// * `"category"` – Links any two nodes that share the same non-empty
///   `data.category`.  Strength is set to `1.0`.
/// * `"temporal"` – Links pairs of nodes whose timestamps are within 24 h of
///   each other.  Strength decays linearly from `1.0` (same instant) to
///   `min_strength` (exactly 24 h apart).
pub fn auto_link(
    store: &CrdtStore,
    actor: &str,
    input: &[NodeRecord],
    algorithms: &[&str],
    min_strength: f64,
) -> Vec<NodeRecord> {
    let mut created: Vec<NodeRecord> = Vec::new();

    // Filter out edge nodes from the input to avoid creating links between edges.
    let filtered_input: Vec<NodeRecord> = input
        .iter()
        .filter(|node| !is_edge(node))
        .cloned()
        .collect();

    for alg in algorithms {
        match *alg {
            "semantic" => {
                for (from, to, strength) in semantic_pairs(&filtered_input, min_strength) {
                    if let Some(rec) = put_edge(store, actor, &from, &to, "semantic", strength) {
                        created.push(rec);
                    }
                }
            }
            "category" => {
                for (from, to) in category_pairs(&filtered_input) {
                    if let Some(rec) = put_edge(store, actor, &from, &to, "category", 1.0) {
                        created.push(rec);
                    }
                }
            }
            "temporal" => {
                for (from, to, strength) in temporal_pairs(&filtered_input, min_strength) {
                    if let Some(rec) = put_edge(store, actor, &from, &to, "temporal", strength) {
                        created.push(rec);
                    }
                }
            }
            _ => {
                // Unknown algorithms are silently ignored for forward compatibility:
                // new algorithm names may be introduced in later phases without
                // breaking callers that send a fixed set of names.
            }
        }
    }
    created
}

// ---------------------------------------------------------------------------
// Algorithm helpers
// ---------------------------------------------------------------------------

/// Compute keyword tokens for a node: whitespace-split words from `text`,
/// `tags` array elements, and `category`.
fn token_set(data: &serde_json::Value) -> HashSet<String> {
    let mut tokens: HashSet<String> = HashSet::new();
    if let Some(text) = data.get("text").and_then(|v| v.as_str()) {
        for word in text.split_whitespace() {
            tokens.insert(word.to_lowercase());
        }
    }
    if let Some(category) = data.get("category").and_then(|v| v.as_str()) {
        tokens.insert(category.to_lowercase());
    }
    if let Some(tags) = data.get("tags").and_then(|v| v.as_array()) {
        for tag in tags {
            if let Some(s) = tag.as_str() {
                tokens.insert(s.to_lowercase());
            }
        }
    }
    tokens
}

/// Jaccard similarity between two token sets (returns 0.0 when both are empty).
fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Return `(from, to, strength)` pairs whose semantic similarity ≥ `threshold`.
fn semantic_pairs(nodes: &[NodeRecord], threshold: f64) -> Vec<(String, String, f64)> {
    let token_sets: Vec<HashSet<String>> = nodes.iter().map(|n| token_set(&n.data)).collect();
    let mut pairs = Vec::new();
    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            let sim = jaccard(&token_sets[i], &token_sets[j]);
            if sim >= threshold {
                pairs.push((nodes[i].id.clone(), nodes[j].id.clone(), sim));
            }
        }
    }
    pairs
}

/// Return `(from, to)` pairs that share the same non-empty `category`.
fn category_pairs(nodes: &[NodeRecord]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for i in 0..nodes.len() {
        let cat_a = nodes[i].data.get("category").and_then(|v| v.as_str());
        for j in (i + 1)..nodes.len() {
            let cat_b = nodes[j].data.get("category").and_then(|v| v.as_str());
            if let (Some(a), Some(b)) = (cat_a, cat_b) {
                if !a.is_empty() && a == b {
                    pairs.push((nodes[i].id.clone(), nodes[j].id.clone()));
                }
            }
        }
    }
    pairs
}

/// Temporal linking window: 24 hours expressed as seconds.
const TEMPORAL_WINDOW_SECS: i64 = 24 * 3600; // 24 h × 3600 s/h

/// Return `(from, to, strength)` pairs whose timestamps are within 24 h of
/// each other.  Strength decays linearly: `1.0` at Δt = 0, `min_strength` at
/// Δt = 24 h.  Only pairs with strength ≥ `min_strength` are returned.
fn temporal_pairs(nodes: &[NodeRecord], min_strength: f64) -> Vec<(String, String, f64)> {
    let mut pairs = Vec::new();
    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            let diff_secs = (nodes[i].timestamp.timestamp() - nodes[j].timestamp.timestamp()).abs();
            if diff_secs <= TEMPORAL_WINDOW_SECS {
                let frac = diff_secs as f64 / TEMPORAL_WINDOW_SECS as f64;
                let strength = 1.0 - (1.0 - min_strength) * frac;
                if strength >= min_strength {
                    pairs.push((nodes[i].id.clone(), nodes[j].id.clone(), strength));
                }
            }
        }
    }
    pairs
}

// ---------------------------------------------------------------------------
// Store helper
// ---------------------------------------------------------------------------

/// Write an edge to the store and return the resulting [`NodeRecord`], or
/// `None` if the edge data could not be constructed.
///
/// Uses a double-colon separator (`edge::{from}::{to}`) to avoid ambiguity
/// when node IDs contain colons (e.g. `"memory:abc"`), which would otherwise
/// produce an indistinguishable key under a single-colon scheme.
fn put_edge(
    store: &CrdtStore,
    actor: &str,
    from: &str,
    to: &str,
    label: &str,
    strength: f64,
) -> Option<NodeRecord> {
    let edge_id = format!("edge::{}::{}", from, to);
    let data = serde_json::json!({
        "_edge": true,
        "from": from,
        "to": to,
        "label": label,
        "strength": strength,
    });
    store.put(edge_id.clone(), actor, data);
    store.get(&edge_id)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pluresdb_core::CrdtStore;

    fn make_store_with_edges() -> CrdtStore {
        let store = CrdtStore::default();
        // Nodes
        store.put("n1", "actor", serde_json::json!({"category": "dev", "text": "rust async code"}));
        store.put("n2", "actor", serde_json::json!({"category": "dev", "text": "rust sync code"}));
        store.put("n3", "actor", serde_json::json!({"category": "design", "text": "ui layout"}));
        store.put("n4", "actor", serde_json::json!({"category": "dev", "text": "python async script"}));
        // Edges: n1→n2 (strength 0.9), n2→n3 (strength 0.5), n3→n4 (strength 0.8)
        store.put("edge:n1:n2", "actor", serde_json::json!({"_edge": true, "from": "n1", "to": "n2", "label": "related", "strength": 0.9}));
        store.put("edge:n2:n3", "actor", serde_json::json!({"_edge": true, "from": "n2", "to": "n3", "label": "related", "strength": 0.5}));
        store.put("edge:n3:n4", "actor", serde_json::json!({"_edge": true, "from": "n3", "to": "n4", "label": "blocks", "strength": 0.8}));
        store
    }

    // ── graph_links ──────────────────────────────────────────────────────────

    #[test]
    fn graph_links_returns_all_edges() {
        let store = make_store_with_edges();
        let links = graph_links(&store, None, None, None, None);
        assert_eq!(links.len(), 3);
        assert!(links.iter().all(is_edge));
    }

    #[test]
    fn graph_links_filter_by_from() {
        let store = make_store_with_edges();
        let links = graph_links(&store, Some("n1"), None, None, None);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].data["from"], "n1");
        assert_eq!(links[0].data["to"], "n2");
    }

    #[test]
    fn graph_links_filter_by_to() {
        let store = make_store_with_edges();
        let links = graph_links(&store, None, Some("n3"), None, None);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].data["to"], "n3");
    }

    #[test]
    fn graph_links_filter_by_min_strength() {
        let store = make_store_with_edges();
        // Only edges with strength ≥ 0.8 should be returned (n1→n2 and n3→n4)
        let links = graph_links(&store, None, None, Some(0.8), None);
        assert_eq!(links.len(), 2);
        for link in &links {
            assert!(edge_strength(link) >= 0.8);
        }
    }

    #[test]
    fn graph_links_filter_by_type() {
        let store = make_store_with_edges();
        let links = graph_links(&store, None, None, None, Some("blocks"));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].data["label"], "blocks");
    }

    #[test]
    fn graph_links_combined_filters() {
        let store = make_store_with_edges();
        let links = graph_links(&store, None, None, Some(0.8), Some("related"));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].data["from"], "n1");
        assert_eq!(links[0].data["to"], "n2");
    }

    #[test]
    fn graph_links_empty_store_returns_empty() {
        let store = CrdtStore::default();
        let links = graph_links(&store, None, None, None, None);
        assert!(links.is_empty());
    }

    #[test]
    fn graph_links_no_match_returns_empty() {
        let store = make_store_with_edges();
        let links = graph_links(&store, Some("nonexistent"), None, None, None);
        assert!(links.is_empty());
    }

    // ── graph_neighbors ──────────────────────────────────────────────────────

    #[test]
    fn graph_neighbors_depth_1() {
        let store = make_store_with_edges();
        let neighbors = graph_neighbors(&store, "n1", 1, None, None, false);
        let ids: Vec<&str> = neighbors.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["n2"]);
    }

    #[test]
    fn graph_neighbors_depth_2() {
        let store = make_store_with_edges();
        let mut neighbors = graph_neighbors(&store, "n1", 2, None, None, false);
        neighbors.sort_by_key(|n| n.id.clone());
        let ids: Vec<&str> = neighbors.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["n2", "n3"]);
    }

    #[test]
    fn graph_neighbors_depth_3() {
        let store = make_store_with_edges();
        let mut neighbors = graph_neighbors(&store, "n1", 3, None, None, false);
        neighbors.sort_by_key(|n| n.id.clone());
        let ids: Vec<&str> = neighbors.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["n2", "n3", "n4"]);
    }

    #[test]
    fn graph_neighbors_depth_0_returns_empty() {
        let store = make_store_with_edges();
        let neighbors = graph_neighbors(&store, "n1", 0, None, None, false);
        assert!(neighbors.is_empty());
    }

    #[test]
    fn graph_neighbors_excludes_root() {
        let store = make_store_with_edges();
        let neighbors = graph_neighbors(&store, "n1", 3, None, None, false);
        assert!(neighbors.iter().all(|n| n.id != "n1"));
    }

    #[test]
    fn graph_neighbors_min_strength_filters_weak_edges() {
        let store = make_store_with_edges();
        // n2→n3 has strength 0.5, so depth 2 from n1 with min_strength 0.8 should only reach n2
        let neighbors = graph_neighbors(&store, "n1", 2, Some(0.8), None, false);
        let ids: Vec<&str> = neighbors.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["n2"]);
    }

    #[test]
    fn graph_neighbors_link_type_filter() {
        let store = make_store_with_edges();
        // Only follow "related" edges: n1→n2 and n2→n3; not n3→n4 (blocks)
        let mut neighbors = graph_neighbors(&store, "n1", 3, None, Some("related"), false);
        neighbors.sort_by_key(|n| n.id.clone());
        let ids: Vec<&str> = neighbors.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["n2", "n3"]);
    }

    #[test]
    fn graph_neighbors_bidirectional_traversal() {
        let store = make_store_with_edges();
        // From n3 bidirectionally: outgoing → n4, incoming ← n2 (from n2→n3)
        let mut neighbors = graph_neighbors(&store, "n3", 1, None, None, true);
        neighbors.sort_by_key(|n| n.id.clone());
        let ids: Vec<&str> = neighbors.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"n2"));
        assert!(ids.contains(&"n4"));
    }

    #[test]
    fn graph_neighbors_no_outgoing_edges_returns_empty() {
        let store = make_store_with_edges();
        // n4 has no outgoing edges
        let neighbors = graph_neighbors(&store, "n4", 2, None, None, false);
        assert!(neighbors.is_empty());
    }

    #[test]
    fn graph_neighbors_isolated_node() {
        let store = CrdtStore::default();
        store.put("solo", "actor", serde_json::json!({"x": 1}));
        let neighbors = graph_neighbors(&store, "solo", 2, None, None, false);
        assert!(neighbors.is_empty());
    }

    // ── auto_link ────────────────────────────────────────────────────────────

    #[test]
    fn auto_link_semantic_creates_edges_for_similar_nodes() {
        let store = CrdtStore::default();
        let nodes = vec![
            store.put("a", "actor", serde_json::json!({"text": "rust async programming concurrent"})),
            store.put("b", "actor", serde_json::json!({"text": "rust async programming parallel"})),
            store.put("c", "actor", serde_json::json!({"text": "python machine learning data"})),
        ];
        let _ = nodes;
        let input: Vec<NodeRecord> = store.list().into_iter().filter(|n| !is_edge(n)).collect();
        let created = auto_link(&store, "actor", &input, &["semantic"], 0.3);
        // a and b share "rust", "async", "programming" → Jaccard = 3/5 = 0.6 ≥ 0.3 ✓
        // a/b vs c: no overlap → below threshold ✗
        assert!(!created.is_empty());
        let has_ab = created.iter().any(|e| {
            let from = e.data["from"].as_str().unwrap_or("");
            let to = e.data["to"].as_str().unwrap_or("");
            (from == "a" && to == "b") || (from == "b" && to == "a")
        });
        assert!(has_ab, "expected semantic link between a and b");
    }

    #[test]
    fn auto_link_semantic_similarity_above_threshold() {
        // Nodes with more than 60% keyword overlap should link with strength > 0.6
        let store = CrdtStore::default();
        store.put("a", "actor", serde_json::json!({"text": "alpha beta gamma delta epsilon"}));
        store.put("b", "actor", serde_json::json!({"text": "alpha beta gamma delta zeta"}));
        let input: Vec<NodeRecord> = store.list().into_iter().filter(|n| !is_edge(n)).collect();
        let created = auto_link(&store, "actor", &input, &["semantic"], 0.0);
        assert_eq!(created.len(), 1);
        // Jaccard: intersection={alpha,beta,gamma,delta}=4, union=6 → 4/6 ≈ 0.667
        let strength = created[0].data["strength"].as_f64().unwrap();
        assert!(strength > 0.6, "expected strength > 0.6, got {}", strength);
    }

    #[test]
    fn auto_link_category_links_same_category() {
        let store = CrdtStore::default();
        store.put("a", "actor", serde_json::json!({"category": "development"}));
        store.put("b", "actor", serde_json::json!({"category": "development"}));
        store.put("c", "actor", serde_json::json!({"category": "design"}));
        let input: Vec<NodeRecord> = store.list().into_iter().filter(|n| !is_edge(n)).collect();
        let created = auto_link(&store, "actor", &input, &["category"], 0.5);
        // Only a↔b share a category
        assert_eq!(created.len(), 1);
        let from = created[0].data["from"].as_str().unwrap();
        let to = created[0].data["to"].as_str().unwrap();
        assert!((from == "a" && to == "b") || (from == "b" && to == "a"));
        assert_eq!(created[0].data["label"], "category");
    }

    #[test]
    fn auto_link_category_no_link_for_different_categories() {
        let store = CrdtStore::default();
        store.put("a", "actor", serde_json::json!({"category": "dev"}));
        store.put("b", "actor", serde_json::json!({"category": "ops"}));
        let input: Vec<NodeRecord> = store.list().into_iter().filter(|n| !is_edge(n)).collect();
        let created = auto_link(&store, "actor", &input, &["category"], 0.5);
        assert!(created.is_empty());
    }

    #[test]
    fn auto_link_temporal_links_recent_nodes() {
        let store = CrdtStore::default();
        // All nodes created within milliseconds of each other → well within 24 h
        store.put("a", "actor", serde_json::json!({"x": 1}));
        store.put("b", "actor", serde_json::json!({"x": 2}));
        store.put("c", "actor", serde_json::json!({"x": 3}));
        let input: Vec<NodeRecord> = store.list().into_iter().filter(|n| !is_edge(n)).collect();
        let created = auto_link(&store, "actor", &input, &["temporal"], 0.5);
        // 3 nodes → up to 3 pairs (a↔b, a↔c, b↔c); all within 24 h so all should link
        assert_eq!(created.len(), 3);
        for edge in &created {
            let strength = edge.data["strength"].as_f64().unwrap();
            // Timestamps differ by milliseconds → strength very close to 1.0
            assert!(strength > 0.99, "expected strength ≈ 1.0, got {}", strength);
            assert_eq!(edge.data["label"], "temporal");
        }
    }

    #[test]
    fn auto_link_multiple_algorithms() {
        let store = CrdtStore::default();
        store.put("a", "actor", serde_json::json!({"category": "dev", "text": "rust code"}));
        store.put("b", "actor", serde_json::json!({"category": "dev", "text": "rust language"}));
        let input: Vec<NodeRecord> = store.list().into_iter().filter(|n| !is_edge(n)).collect();
        // Running semantic and category together; category will overwrite semantic edge since
        // they share the same edge:a:b key (latest write wins).
        let created = auto_link(&store, "actor", &input, &["semantic", "category"], 0.0);
        assert!(!created.is_empty());
    }

    #[test]
    fn auto_link_unknown_algorithm_ignored() {
        let store = CrdtStore::default();
        store.put("a", "actor", serde_json::json!({"x": 1}));
        store.put("b", "actor", serde_json::json!({"x": 2}));
        let input: Vec<NodeRecord> = store.list().into_iter().filter(|n| !is_edge(n)).collect();
        // Should not panic; returns empty since the algorithm does nothing
        let created = auto_link(&store, "actor", &input, &["nonexistent"], 0.5);
        assert!(created.is_empty());
    }

    #[test]
    fn auto_link_empty_input_returns_empty() {
        let store = CrdtStore::default();
        let created = auto_link(&store, "actor", &[], &["semantic", "category", "temporal"], 0.5);
        assert!(created.is_empty());
    }

    #[test]
    fn auto_link_single_node_returns_empty() {
        let store = CrdtStore::default();
        store.put("a", "actor", serde_json::json!({"text": "hello world"}));
        let input: Vec<NodeRecord> = store.list().into_iter().filter(|n| !is_edge(n)).collect();
        let created = auto_link(&store, "actor", &input, &["semantic", "category", "temporal"], 0.0);
        assert!(created.is_empty());
    }

    #[test]
    fn auto_link_edges_persisted_in_store() {
        let store = CrdtStore::default();
        store.put("a", "actor", serde_json::json!({"category": "eng"}));
        store.put("b", "actor", serde_json::json!({"category": "eng"}));
        let input: Vec<NodeRecord> = store.list().into_iter().filter(|n| !is_edge(n)).collect();
        auto_link(&store, "actor", &input, &["category"], 0.5);
        // Auto-linked edges use the double-colon format edge::{from}::{to}.
        let has_edge =
            store.get("edge::a::b").is_some() || store.get("edge::b::a").is_some();
        assert!(has_edge, "expected either edge::a::b or edge::b::a to exist");
    }

    // ── performance / larger dataset ─────────────────────────────────────────

    #[test]
    fn graph_neighbors_large_dataset() {
        // 200 nodes in a simple chain n0→n1→…→n199 plus many isolated nodes
        let store = CrdtStore::default();
        for i in 0..200usize {
            store.put(format!("n{}", i), "actor", serde_json::json!({"i": i}));
        }
        // Create a linear chain of 100 edges: n0→n1→…→n99
        for i in 0..100usize {
            store.put(
                format!("edge:n{}:n{}", i, i + 1),
                "actor",
                serde_json::json!({"_edge": true, "from": format!("n{}", i), "to": format!("n{}", i+1), "strength": 1.0}),
            );
        }
        // BFS from n0 with depth 5 should find n1…n5
        let neighbors = graph_neighbors(&store, "n0", 5, None, None, false);
        assert_eq!(neighbors.len(), 5, "expected 5 neighbors, got {}", neighbors.len());
    }

    #[test]
    fn graph_links_large_number_of_edges() {
        // Insert 100 edges, all with strength ≥ 0.5; then filter by strength ≥ 0.8
        let store = CrdtStore::default();
        for i in 0..100usize {
            let strength = if i % 2 == 0 { 0.9 } else { 0.4 };
            store.put(
                format!("edge:a{}:b{}", i, i),
                "actor",
                serde_json::json!({
                    "_edge": true,
                    "from": format!("a{}", i),
                    "to": format!("b{}", i),
                    "label": "test",
                    "strength": strength,
                }),
            );
        }
        let strong = graph_links(&store, None, None, Some(0.8), None);
        assert_eq!(strong.len(), 50); // even-indexed edges
    }

    // ── Regression / correctness tests for fixes in review round 2 ───────────

    #[test]
    fn graph_neighbors_skips_malformed_edges_missing_to() {
        let store = CrdtStore::default();
        store.put("a", "actor", serde_json::json!({}));
        store.put("b", "actor", serde_json::json!({}));
        // Malformed edge: has `from` but no `to`
        store.put("edge:a:x", "actor", serde_json::json!({"_edge": true, "from": "a"}));
        // Valid edge: a → b
        store.put("edge:a:b", "actor", serde_json::json!({"_edge": true, "from": "a", "to": "b", "strength": 1.0}));
        let neighbors = graph_neighbors(&store, "a", 1, None, None, false);
        // Only b should be reachable; the malformed edge must not enqueue an empty id
        let ids: Vec<&str> = neighbors.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["b"]);
    }

    #[test]
    fn graph_neighbors_skips_malformed_edges_empty_from() {
        let store = CrdtStore::default();
        store.put("a", "actor", serde_json::json!({}));
        store.put("b", "actor", serde_json::json!({}));
        // Malformed edge: `from` is empty string, `to` is "a"
        store.put("bad_edge", "actor", serde_json::json!({"_edge": true, "from": "", "to": "a"}));
        // Valid edge: a → b
        store.put("edge:a:b", "actor", serde_json::json!({"_edge": true, "from": "a", "to": "b"}));
        // BFS from a should only reach b, not enqueue ""
        let neighbors = graph_neighbors(&store, "a", 1, None, None, false);
        let ids: Vec<&str> = neighbors.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["b"]);
    }
}
