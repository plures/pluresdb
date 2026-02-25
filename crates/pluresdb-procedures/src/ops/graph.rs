//! Graph analytics operations: clustering, path finding, PageRank, and stats.
//!
//! Edges are stored as CRDT nodes with `_edge: true`, `from`, and `to` fields.
//! All graph algorithms operate by reading these edge nodes from the store and
//! building an in-memory adjacency representation.

use std::collections::{HashMap, HashSet, VecDeque};

use pluresdb_core::{CrdtStore, NodeRecord};

// ---------------------------------------------------------------------------
// Edge extraction helpers
// ---------------------------------------------------------------------------

/// An edge extracted from the store.
#[derive(Debug, Clone)]
struct Edge {
    from: String,
    to: String,
    /// Weight / strength of the edge (default 1.0).
    weight: f64,
}

/// Read all edge nodes from the store and return them as `Edge` values.
fn read_edges(store: &CrdtStore) -> Vec<Edge> {
    store
        .list()
        .into_iter()
        .filter(|n| n.data.get("_edge").and_then(|v| v.as_bool()).unwrap_or(false))
        .filter_map(|n| {
            let from = n.data.get("from")?.as_str()?.to_string();
            let to = n.data.get("to")?.as_str()?.to_string();
            let weight = n
                .data
                .get("weight")
                .or_else(|| n.data.get("strength"))
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            Some(Edge { from, to, weight })
        })
        .collect()
}

/// Build a weighted adjacency list (undirected: both directions stored).
///
/// Returns `(adjacency, nodes)` where `adjacency[i]` is a list of `(j, weight)`
/// pairs and `nodes[i]` is the node ID at index `i`.
fn build_adjacency(
    edges: &[Edge],
    min_strength: Option<f64>,
) -> (Vec<Vec<(usize, f64)>>, Vec<String>) {
    let min_w = min_strength.unwrap_or(0.0);

    // Collect unique node IDs in insertion order.
    let mut node_index: HashMap<String, usize> = HashMap::new();
    let mut nodes: Vec<String> = Vec::new();

    let mut get_or_insert = |id: &str| -> usize {
        if let Some(&idx) = node_index.get(id) {
            return idx;
        }
        let idx = nodes.len();
        nodes.push(id.to_string());
        node_index.insert(id.to_string(), idx);
        idx
    };

    // Pre-pass to register all nodes.
    for e in edges {
        if e.weight >= min_w {
            get_or_insert(&e.from);
            get_or_insert(&e.to);
        }
    }

    let n = nodes.len();
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];

    for e in edges {
        if e.weight < min_w {
            continue;
        }
        if let (Some(&i), Some(&j)) = (node_index.get(&e.from), node_index.get(&e.to)) {
            adj[i].push((j, e.weight));
            adj[j].push((i, e.weight)); // undirected
        }
    }

    (adj, nodes)
}

// ---------------------------------------------------------------------------
// Louvain community detection
// ---------------------------------------------------------------------------

/// Simplified Louvain-style modularity optimization.
///
/// Returns a mapping `community[i] = community_id` for each node.
fn louvain_communities(adj: &[Vec<(usize, f64)>]) -> Vec<usize> {
    let n = adj.len();
    if n == 0 {
        return Vec::new();
    }

    let total_weight: f64 = adj.iter().flat_map(|row| row.iter().map(|(_, w)| w)).sum::<f64>()
        / 2.0; // each edge counted twice

    // Start: every node in its own community.
    let mut community: Vec<usize> = (0..n).collect();

    // Node strength (sum of incident edge weights).
    let strength: Vec<f64> = adj.iter().map(|row| row.iter().map(|(_, w)| w).sum()).collect();

    let mut improved = true;
    while improved {
        improved = false;
        for i in 0..n {
            // Count weights to each neighbouring community.
            let mut community_weights: HashMap<usize, f64> = HashMap::new();
            for &(j, w) in &adj[i] {
                *community_weights.entry(community[j]).or_insert(0.0) += w;
            }

            let current_c = community[i];
            let ki = strength[i];

            // Sum of strengths in current community (excluding i itself).
            let sum_c: f64 = (0..n)
                .filter(|&j| j != i && community[j] == current_c)
                .map(|j| strength[j])
                .sum();

            // Modularity gain for moving to neighbour community c:
            // ΔQ ∝ (k_i_in_c - k_i * sum_c / (2m))
            let mut best_gain = 0.0;
            let mut best_c = current_c;

            for (&c, &k_i_in_c) in &community_weights {
                if c == current_c {
                    continue;
                }
                let sum_c_new: f64 = (0..n)
                    .filter(|&j| j != i && community[j] == c)
                    .map(|j| strength[j])
                    .sum();
                let gain = k_i_in_c / total_weight.max(1e-10)
                    - ki * sum_c_new / (2.0 * total_weight.max(1e-10).powi(2));
                let loss = community_weights.get(&current_c).copied().unwrap_or(0.0)
                    / total_weight.max(1e-10)
                    - ki * sum_c / (2.0 * total_weight.max(1e-10).powi(2));
                if gain - loss > best_gain {
                    best_gain = gain - loss;
                    best_c = c;
                }
            }

            if best_c != current_c {
                community[i] = best_c;
                improved = true;
            }
        }
    }

    // Renumber communities to 0-based contiguous IDs.
    let mut mapping: HashMap<usize, usize> = HashMap::new();
    let mut next_id = 0usize;
    for c in community.iter_mut() {
        let id = *mapping.entry(*c).or_insert_with(|| {
            let id = next_id;
            next_id += 1;
            id
        });
        *c = id;
    }

    community
}

// ---------------------------------------------------------------------------
// Semantic clustering (content-overlap based)
// ---------------------------------------------------------------------------

/// Group nodes by their `category` field, or fall back to timestamp buckets.
fn semantic_communities(store: &CrdtStore, nodes: &[String]) -> Vec<usize> {
    let mut cat_index: HashMap<String, usize> = HashMap::new();
    let mut next_id = 0usize;

    nodes
        .iter()
        .map(|id| {
            let cat = store
                .get(id)
                .and_then(|n| {
                    n.data.get("category").and_then(|v| v.as_str()).map(|s| s.to_string())
                })
                .unwrap_or_else(|| "unknown".to_string());
            *cat_index.entry(cat).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Temporal clustering (group by time window)
// ---------------------------------------------------------------------------

/// Group nodes into hourly buckets based on their `created_at` timestamp
/// (milliseconds since epoch).  Falls back to the record's `timestamp` field.
fn temporal_communities(store: &CrdtStore, nodes: &[String]) -> Vec<usize> {
    const BUCKET_MS: i64 = 3_600_000; // 1 hour

    nodes
        .iter()
        .map(|id| {
            let ts_ms = store
                .get(id)
                .and_then(|n| {
                    n.data
                        .get("created_at")
                        .and_then(|v| v.as_i64())
                        .or_else(|| {
                            // Use the record timestamp converted to ms.
                            Some(n.timestamp.timestamp_millis())
                        })
                })
                .unwrap_or(0);
            let bucket = ts_ms.div_euclid(BUCKET_MS);
            // Map negative buckets away from usize by shifting.
            (bucket + i64::MAX / 2) as usize
        })
        .collect::<Vec<_>>()
        .into_iter()
        .enumerate()
        .fold(
            (HashMap::<usize, usize>::new(), 0usize, Vec::new()),
            |(mut map, mut next, mut out), (_, raw)| {
                let id = *map.entry(raw).or_insert_with(|| {
                    let id = next;
                    next += 1;
                    id
                });
                out.push(id);
                (map, next, out)
            },
        )
        .2
}

// ---------------------------------------------------------------------------
// graph_clusters public function
// ---------------------------------------------------------------------------

/// Detect communities in the graph stored in `store`.
pub fn graph_clusters(
    store: &CrdtStore,
    algorithm: &str,
    min_size: Option<usize>,
    min_strength: Option<f64>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let min_size = min_size.unwrap_or(2);
    let edges = read_edges(store);
    let (adj, nodes) = build_adjacency(&edges, min_strength);

    let communities: Vec<usize> = match algorithm {
        "louvain" => louvain_communities(&adj),
        "semantic" => semantic_communities(store, &nodes),
        "temporal" => temporal_communities(store, &nodes),
        other => return Err(anyhow::anyhow!("unknown clustering algorithm: '{}'", other)),
    };

    // Group nodes by community id.
    let mut groups: HashMap<usize, Vec<String>> = HashMap::new();
    for (i, &c) in communities.iter().enumerate() {
        groups.entry(c).or_default().push(nodes[i].clone());
    }

    // Compute per-cluster coherence as average internal edge weight / max possible.
    let total_edge_weight: f64 = edges.iter().map(|e| e.weight).sum::<f64>().max(1.0);

    let mut results: Vec<serde_json::Value> = groups
        .into_iter()
        .filter(|(_, members)| members.len() >= min_size)
        .enumerate()
        .map(|(cluster_idx, (_, members))| {
            let internal_weight: f64 = edges
                .iter()
                .filter(|e| members.contains(&e.from) && members.contains(&e.to))
                .map(|e| e.weight)
                .sum();
            let coherence = (internal_weight / total_edge_weight).min(1.0);
            serde_json::json!({
                "id": format!("cluster:{}-{:03}", algorithm, cluster_idx),
                "cluster_id": format!("{}-{:03}", algorithm, cluster_idx),
                "algorithm": algorithm,
                "member_ids": members,
                "size": members.len(),
                "coherence_score": coherence,
            })
        })
        .collect();

    results.sort_by(|a, b| {
        let sa = a["size"].as_u64().unwrap_or(0);
        let sb = b["size"].as_u64().unwrap_or(0);
        sb.cmp(&sa)
    });

    Ok(results)
}

// ---------------------------------------------------------------------------
// graph_path — BFS shortest path
// ---------------------------------------------------------------------------

/// Find the shortest path between two node IDs using BFS over edges.
///
/// Returns the sequence of node IDs on the path (inclusive of `from` and `to`),
/// or an empty `Vec` if no path exists within `max_hops`.
pub fn graph_path(
    store: &CrdtStore,
    from: &str,
    to: &str,
    max_hops: Option<usize>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let max_hops = max_hops.unwrap_or(10);
    let edges = read_edges(store);

    // Build directed + undirected neighbour list (string-keyed for convenience).
    let mut neighbours: HashMap<String, Vec<String>> = HashMap::new();
    for e in &edges {
        neighbours.entry(e.from.clone()).or_default().push(e.to.clone());
        neighbours.entry(e.to.clone()).or_default().push(e.from.clone());
    }

    if from == to {
        // Trivial path.
        let node_val = node_to_json(store, from);
        return Ok(vec![node_val]);
    }

    // BFS.
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
    queue.push_back((from.to_string(), vec![from.to_string()]));
    visited.insert(from.to_string());

    while let Some((current, path)) = queue.pop_front() {
        if path.len() >= max_hops + 1 {
            // Already at the hop limit; don't expand further.
            continue;
        }
        if let Some(nbrs) = neighbours.get(&current) {
            for next in nbrs {
                if visited.contains(next) {
                    continue;
                }
                let mut new_path = path.clone();
                new_path.push(next.clone());
                if next == to {
                    return Ok(new_path.iter().map(|id| node_to_json(store, id)).collect());
                }
                visited.insert(next.clone());
                queue.push_back((next.clone(), new_path));
            }
        }
    }

    // No path found — return empty vec.
    Ok(vec![])
}

fn node_to_json(store: &CrdtStore, id: &str) -> serde_json::Value {
    match store.get(id) {
        Some(n) => serde_json::json!({
            "id": n.id,
            "data": n.data,
            "timestamp": n.timestamp.to_rfc3339(),
        }),
        None => serde_json::json!({ "id": id }),
    }
}

// ---------------------------------------------------------------------------
// graph_pagerank
// ---------------------------------------------------------------------------

/// Compute PageRank for all non-edge nodes and return them sorted by score.
pub fn graph_pagerank(
    store: &CrdtStore,
    dampening: Option<f64>,
    iterations: Option<usize>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let d = dampening.unwrap_or(0.85);
    let iters = iterations.unwrap_or(100);

    let edges = read_edges(store);

    // Collect all unique non-edge node IDs referenced by edges.
    let mut node_set: HashSet<String> = HashSet::new();
    for e in &edges {
        node_set.insert(e.from.clone());
        node_set.insert(e.to.clone());
    }

    // Also include all non-edge nodes from the store (orphans get base PR).
    for n in store.list() {
        if !n.data.get("_edge").and_then(|v| v.as_bool()).unwrap_or(false) {
            node_set.insert(n.id.clone());
        }
    }

    let nodes: Vec<String> = {
        let mut v: Vec<String> = node_set.into_iter().collect();
        v.sort();
        v
    };
    let n = nodes.len();
    if n == 0 {
        return Ok(vec![]);
    }

    let node_index: HashMap<&str, usize> =
        nodes.iter().enumerate().map(|(i, id)| (id.as_str(), i)).collect();

    // Build directed adjacency: outgoing links.
    let mut out_links: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in &edges {
        if let (Some(&i), Some(&j)) = (node_index.get(e.from.as_str()), node_index.get(e.to.as_str())) {
            out_links[i].push(j);
        }
    }

    // Initialize scores uniformly.
    let init = 1.0 / n as f64;
    let mut scores: Vec<f64> = vec![init; n];

    for _ in 0..iters {
        let mut new_scores: Vec<f64> = vec![(1.0 - d) / n as f64; n];

        // Dangling nodes (no outgoing edges) distribute rank equally to all.
        let dangling_sum: f64 = (0..n)
            .filter(|&i| out_links[i].is_empty())
            .map(|i| scores[i])
            .sum::<f64>()
            * d
            / n as f64;

        for i in 0..n {
            let out_count = out_links[i].len();
            if out_count == 0 {
                continue;
            }
            let contrib = d * scores[i] / out_count as f64;
            for &j in &out_links[i] {
                new_scores[j] += contrib;
            }
        }

        for s in new_scores.iter_mut() {
            *s += dangling_sum;
        }

        scores = new_scores;
    }

    // Build result nodes with pagerank_score attached.
    let mut results: Vec<(f64, serde_json::Value)> = nodes
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let score = scores[i];
            let mut val = node_to_json(store, id);
            if let Some(obj) = val.as_object_mut() {
                obj.insert(
                    "pagerank_score".to_string(),
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(score).unwrap_or(serde_json::Number::from(0)),
                    ),
                );
            }
            (score, val)
        })
        .collect();

    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(results.into_iter().map(|(_, v)| v).collect())
}

// ---------------------------------------------------------------------------
// graph_stats
// ---------------------------------------------------------------------------

/// Compute summary network statistics.
pub fn graph_stats(store: &CrdtStore) -> anyhow::Result<Vec<serde_json::Value>> {
    let all_nodes: Vec<NodeRecord> = store.list();
    let edges: Vec<&NodeRecord> = all_nodes
        .iter()
        .filter(|n| n.data.get("_edge").and_then(|v| v.as_bool()).unwrap_or(false))
        .collect();
    let data_nodes: Vec<&NodeRecord> = all_nodes
        .iter()
        .filter(|n| !n.data.get("_edge").and_then(|v| v.as_bool()).unwrap_or(false))
        .collect();

    let node_count = data_nodes.len();
    let edge_count = edges.len();

    // Build degree map.
    let mut degree: HashMap<String, usize> = HashMap::new();
    for e in &edges {
        if let (Some(from), Some(to)) = (
            e.data.get("from").and_then(|v| v.as_str()),
            e.data.get("to").and_then(|v| v.as_str()),
        ) {
            *degree.entry(from.to_string()).or_insert(0) += 1;
            *degree.entry(to.to_string()).or_insert(0) += 1;
        }
    }

    let degrees: Vec<f64> = degree.values().map(|&d| d as f64).collect();
    let avg_degree = if degrees.is_empty() {
        0.0
    } else {
        degrees.iter().sum::<f64>() / degrees.len() as f64
    };
    let max_degree = degrees.iter().cloned().fold(0.0_f64, f64::max) as usize;

    // Orphan nodes: data nodes with no edges.
    let orphan_count = data_nodes
        .iter()
        .filter(|n| !degree.contains_key(&n.id))
        .count();

    // Density: edges / (n*(n-1)/2) for undirected.
    let density = if node_count < 2 {
        0.0
    } else {
        edge_count as f64 / (node_count as f64 * (node_count as f64 - 1.0) / 2.0)
    };

    let stats = serde_json::json!({
        "id": "graph_stats",
        "node_count": node_count,
        "edge_count": edge_count,
        "avg_degree": avg_degree,
        "max_degree": max_degree,
        "orphan_count": orphan_count,
        "density": density,
    });

    Ok(vec![stats])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pluresdb_core::CrdtStore;

    fn make_graph() -> CrdtStore {
        let store = CrdtStore::default();
        // Three interconnected nodes forming a triangle + one isolated node.
        store.put("n1", "a", serde_json::json!({"category": "decision", "label": "Alpha"}));
        store.put("n2", "a", serde_json::json!({"category": "decision", "label": "Beta"}));
        store.put("n3", "a", serde_json::json!({"category": "note",     "label": "Gamma"}));
        store.put("n4", "a", serde_json::json!({"category": "task",     "label": "Delta"}));
        // Edges: n1-n2, n2-n3, n1-n3 (triangle) and n3-n4.
        store.put("edge:n1:n2", "a", serde_json::json!({"_edge": true, "from": "n1", "to": "n2", "weight": 0.9}));
        store.put("edge:n2:n3", "a", serde_json::json!({"_edge": true, "from": "n2", "to": "n3", "weight": 0.8}));
        store.put("edge:n1:n3", "a", serde_json::json!({"_edge": true, "from": "n1", "to": "n3", "weight": 0.7}));
        store.put("edge:n3:n4", "a", serde_json::json!({"_edge": true, "from": "n3", "to": "n4", "weight": 0.5}));
        store
    }

    #[test]
    fn graph_clusters_louvain_returns_clusters() {
        let store = make_graph();
        let clusters = graph_clusters(&store, "louvain", Some(2), None).unwrap();
        // Should find at least one cluster with ≥ 2 members.
        assert!(!clusters.is_empty());
        for c in &clusters {
            let size = c["size"].as_u64().unwrap();
            assert!(size >= 2, "min_size=2 must be respected; got {}", size);
            assert_eq!(c["algorithm"], "louvain");
        }
    }

    #[test]
    fn graph_clusters_semantic() {
        let store = make_graph();
        // semantic groups by category
        let clusters = graph_clusters(&store, "semantic", Some(2), None).unwrap();
        assert!(!clusters.is_empty());
        for c in &clusters {
            assert_eq!(c["algorithm"], "semantic");
        }
    }

    #[test]
    fn graph_clusters_temporal() {
        let store = make_graph();
        let clusters = graph_clusters(&store, "temporal", Some(1), None).unwrap();
        assert!(!clusters.is_empty());
    }

    #[test]
    fn graph_clusters_unknown_algorithm_errors() {
        let store = make_graph();
        assert!(graph_clusters(&store, "unknown_algo", None, None).is_err());
    }

    #[test]
    fn graph_clusters_min_strength_filters_edges() {
        let store = make_graph();
        // With very high min_strength, no edges pass → no clusters formed.
        let clusters = graph_clusters(&store, "louvain", Some(2), Some(0.99)).unwrap();
        // high threshold keeps only weight >= 0.99 (none in our test data).
        assert!(clusters.is_empty());
    }

    #[test]
    fn graph_path_finds_direct() {
        let store = make_graph();
        let path = graph_path(&store, "n1", "n2", None).unwrap();
        assert!(!path.is_empty(), "should find n1->n2");
        assert_eq!(path[0]["id"], "n1");
        let last = path.last().unwrap();
        assert_eq!(last["id"], "n2");
    }

    #[test]
    fn graph_path_finds_indirect() {
        let store = make_graph();
        // n1 can reach n4 via n3
        let path = graph_path(&store, "n1", "n4", None).unwrap();
        assert!(!path.is_empty());
        assert_eq!(path.first().unwrap()["id"], "n1");
        assert_eq!(path.last().unwrap()["id"], "n4");
    }

    #[test]
    fn graph_path_no_path_returns_empty() {
        let store = CrdtStore::default();
        store.put("a", "x", serde_json::json!({}));
        store.put("b", "x", serde_json::json!({}));
        // No edges between a and b.
        let path = graph_path(&store, "a", "b", None).unwrap();
        assert!(path.is_empty());
    }

    #[test]
    fn graph_path_self_is_trivial() {
        let store = make_graph();
        let path = graph_path(&store, "n1", "n1", None).unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0]["id"], "n1");
    }

    #[test]
    fn graph_path_respects_max_hops() {
        let store = make_graph();
        // n1 to n4 requires 2 hops; limit to 1 should return empty.
        let path = graph_path(&store, "n1", "n4", Some(1)).unwrap();
        assert!(path.is_empty());
    }

    #[test]
    fn graph_pagerank_returns_all_nodes() {
        let store = make_graph();
        let ranked = graph_pagerank(&store, None, None).unwrap();
        // Should have exactly 4 data nodes.
        assert_eq!(ranked.len(), 4);
        for node in &ranked {
            assert!(node.get("pagerank_score").is_some());
            let score = node["pagerank_score"].as_f64().unwrap();
            assert!(score >= 0.0 && score <= 1.0);
        }
        // Scores should be ordered descending.
        let scores: Vec<f64> = ranked.iter().map(|v| v["pagerank_score"].as_f64().unwrap()).collect();
        for pair in scores.windows(2) {
            assert!(pair[0] >= pair[1]);
        }
    }

    #[test]
    fn graph_pagerank_empty_store() {
        let store = CrdtStore::default();
        let ranked = graph_pagerank(&store, None, None).unwrap();
        assert!(ranked.is_empty());
    }

    #[test]
    fn graph_stats_basic() {
        let store = make_graph();
        let stats = graph_stats(&store).unwrap();
        assert_eq!(stats.len(), 1);
        let s = &stats[0];
        assert_eq!(s["node_count"].as_u64().unwrap(), 4);
        assert_eq!(s["edge_count"].as_u64().unwrap(), 4);
        // n4 is connected, so 0 orphans; all nodes have edges.
        assert_eq!(s["orphan_count"].as_u64().unwrap(), 0);
        assert!(s["avg_degree"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn graph_stats_orphan_detection() {
        let store = CrdtStore::default();
        store.put("n1", "a", serde_json::json!({"label": "connected"}));
        store.put("n2", "a", serde_json::json!({"label": "orphan"}));
        store.put("edge:n1:n1", "a", serde_json::json!({"_edge": true, "from": "n1", "to": "n1"}));
        let stats = graph_stats(&store).unwrap();
        // n2 has no edges.
        assert_eq!(stats[0]["orphan_count"].as_u64().unwrap(), 1);
    }
}
