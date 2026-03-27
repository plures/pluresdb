//! Graph analytics operations: clustering, path finding, PageRank, and stats.
//!
//! Edges are stored as CRDT nodes with `_edge: true`, `from`, and `to` fields.
//! All graph algorithms operate by reading these edge nodes from the store and
//! building an in-memory adjacency representation.
//!
//! All public functions return `Vec<NodeRecord>` so that their results can flow
//! through the rest of the query pipeline (filter, sort, limit, project).

use std::collections::{HashMap, HashSet, VecDeque};

use pluresdb_core::{CrdtStore, NodeRecord};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum Louvain iterations as a safety limit against infinite loops.
const MAX_LOUVAIN_ITERS: usize = 100;

/// Warn when more than this many edges are loaded; operations on very large
/// graphs may consume significant memory and CPU.
const EDGE_WARN_THRESHOLD: usize = 10_000;

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
///
/// Emits a `tracing::warn!` when the edge count exceeds
/// [`EDGE_WARN_THRESHOLD`] to alert callers of potential resource consumption.
fn read_edges(store: &CrdtStore) -> Vec<Edge> {
    let all = store.list();
    let mut edges: Vec<Edge> = Vec::new();

    for n in all {
        if !n.data.get("_edge").and_then(|v| v.as_bool()).unwrap_or(false) {
            continue;
        }
        let from = match n.data.get("from").and_then(|v| v.as_str()) {
            Some(v) => v.to_string(),
            None => continue,
        };
        let to = match n.data.get("to").and_then(|v| v.as_str()) {
            Some(v) => v.to_string(),
            None => continue,
        };
        let weight = n
            .data
            .get("weight")
            .or_else(|| n.data.get("strength"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        edges.push(Edge { from, to, weight });
    }

    if edges.len() > EDGE_WARN_THRESHOLD {
        tracing::warn!(
            "graph: loaded {} edges into memory; graph algorithms on large \
             graphs may consume significant memory and CPU",
            edges.len()
        );
    }

    edges
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
/// Maintains incremental community strength sums for O(n) per pass instead of
/// O(n²).  Bounded to [`MAX_LOUVAIN_ITERS`] iterations as a safety limit.
///
/// Returns a mapping `community[i] = community_id` for each node.
fn louvain_communities(adj: &[Vec<(usize, f64)>]) -> Vec<usize> {
    let n = adj.len();
    if n == 0 {
        return Vec::new();
    }

    let total_weight: f64 =
        adj.iter().flat_map(|row| row.iter().map(|(_, w)| w)).sum::<f64>() / 2.0;

    // Start: every node in its own community.
    let mut community: Vec<usize> = (0..n).collect();

    // Node strength (sum of incident edge weights).
    let strength: Vec<f64> =
        adj.iter().map(|row| row.iter().map(|(_, w)| w).sum()).collect();

    // Maintain total strength per community for O(1) lookup; updated
    // incrementally when nodes change communities.
    let mut community_sum: HashMap<usize, f64> = HashMap::new();
    for (i, &c) in community.iter().enumerate() {
        *community_sum.entry(c).or_insert(0.0) += strength[i];
    }

    let mut improved = true;
    let mut iter_count = 0;
    while improved && iter_count < MAX_LOUVAIN_ITERS {
        improved = false;
        iter_count += 1;

        for i in 0..n {
            let current_c = community[i];
            let ki = strength[i];

            // Weights from node i to each neighbouring community.
            let mut community_weights: HashMap<usize, f64> = HashMap::new();
            for &(j, w) in &adj[i] {
                *community_weights.entry(community[j]).or_insert(0.0) += w;
            }

            // Effective strength of the current community excluding i.
            let sum_c = community_sum.get(&current_c).copied().unwrap_or(0.0) - ki;

            let mut best_gain = 0.0;
            let mut best_c = current_c;

            for (&c, &k_i_in_c) in &community_weights {
                if c == current_c {
                    continue;
                }
                let sum_c_new = community_sum.get(&c).copied().unwrap_or(0.0);
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
                // Update community sums incrementally.
                *community_sum.entry(current_c).or_insert(0.0) -= ki;
                *community_sum.entry(best_c).or_insert(0.0) += ki;
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

/// Group nodes by their `category` field, defaulting to `"unknown"` for nodes without a category.
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
/// (milliseconds since epoch). Falls back to the record's `timestamp` field.
fn temporal_communities(store: &CrdtStore, nodes: &[String]) -> Vec<usize> {
    const BUCKET_MS: i64 = 3_600_000; // 1 hour

    let mut bucket_to_id: HashMap<i64, usize> = HashMap::new();
    let mut next_id = 0usize;

    nodes
        .iter()
        .map(|id| {
            let ts_ms = store
                .get(id)
                .and_then(|n| {
                    n.data
                        .get("created_at")
                        .and_then(|v| v.as_i64())
                        .or_else(|| Some(n.timestamp.timestamp_millis()))
                })
                .unwrap_or(0);
            let bucket = ts_ms.div_euclid(BUCKET_MS);
            *bucket_to_id.entry(bucket).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Helper: fetch a node or synthesize a minimal one
// ---------------------------------------------------------------------------

fn get_or_synthetic(store: &CrdtStore, id: &str) -> NodeRecord {
    store
        .get(id)
        .unwrap_or_else(|| NodeRecord::new(id.to_string(), "system", serde_json::json!({})))
}

// ---------------------------------------------------------------------------
// graph_clusters public function
// ---------------------------------------------------------------------------

/// Detect communities in the graph stored in `store`.
///
/// Returns a `Vec<NodeRecord>` where each record represents one cluster.
/// Graph-specific fields (`cluster_id`, `algorithm`, `member_ids`, `size`,
/// `coherence_score`) are stored inside the record's `data` map so that
/// downstream pipeline steps (filter, sort, project) can operate on them.
pub fn graph_clusters(
    store: &CrdtStore,
    algorithm: &str,
    min_size: Option<usize>,
    min_strength: Option<f64>,
) -> anyhow::Result<Vec<NodeRecord>> {
    let min_size = min_size.unwrap_or(2);

    if let Some(ms) = min_strength {
        if ms < 0.0 {
            return Err(anyhow::anyhow!("min_strength must be >= 0.0, got {}", ms));
        }
    }

    let edges = read_edges(store);
    let (adj, nodes) = build_adjacency(&edges, min_strength);

    let communities: Vec<usize> = match algorithm {
        "louvain" => louvain_communities(&adj),
        "semantic" => semantic_communities(store, &nodes),
        "temporal" => temporal_communities(store, &nodes),
        other => return Err(anyhow::anyhow!("unknown clustering algorithm: '{}'", other)),
    };

    // Group node indices by community id.
    let mut groups: HashMap<usize, Vec<String>> = HashMap::new();
    for (i, &c) in communities.iter().enumerate() {
        groups.entry(c).or_default().push(nodes[i].clone());
    }

    let mut results: Vec<NodeRecord> = groups
        .into_iter()
        .filter(|(_, members)| members.len() >= min_size)
        .enumerate()
        .map(|(cluster_idx, (_, members))| {
            // Use a HashSet for O(1) membership checks when computing edge weights.
            let member_set: HashSet<&String> = members.iter().collect();

            // Coherence: fraction of cluster-incident edge weight that is internal.
            let internal_weight: f64 = edges
                .iter()
                .filter(|e| member_set.contains(&e.from) && member_set.contains(&e.to))
                .map(|e| e.weight)
                .sum();
            let total_member_edge_weight: f64 = edges
                .iter()
                .filter(|e| member_set.contains(&e.from) || member_set.contains(&e.to))
                .map(|e| e.weight)
                .sum::<f64>()
                .max(1.0);
            let coherence = (internal_weight / total_member_edge_weight).min(1.0);

            let cluster_id = format!("{}-{:03}", algorithm, cluster_idx);
            NodeRecord::new(
                format!("cluster:{}", cluster_id),
                "system",
                serde_json::json!({
                    "cluster_id": cluster_id,
                    "algorithm": algorithm,
                    "member_ids": &members,
                    "size": members.len(),
                    "coherence_score": coherence,
                }),
            )
        })
        .collect();

    results.sort_by(|a, b| {
        let sa = a.data.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        let sb = b.data.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
        sb.cmp(&sa)
    });

    Ok(results)
}

// ---------------------------------------------------------------------------
// graph_path — BFS shortest path
// ---------------------------------------------------------------------------

/// Find the shortest path between two node IDs using BFS over edges.
///
/// Returns a `Vec<NodeRecord>` for each node on the path (inclusive of `from`
/// and `to`), or an empty `Vec` if no path exists within `max_hops`.
pub fn graph_path(
    store: &CrdtStore,
    from: &str,
    to: &str,
    max_hops: Option<usize>,
) -> anyhow::Result<Vec<NodeRecord>> {
    let max_hops = max_hops.unwrap_or(10);
    let edges = read_edges(store);

    // Build undirected neighbour list.
    let mut neighbours: HashMap<String, Vec<String>> = HashMap::new();
    for e in &edges {
        neighbours.entry(e.from.clone()).or_default().push(e.to.clone());
        neighbours.entry(e.to.clone()).or_default().push(e.from.clone());
    }

    if from == to {
        return Ok(vec![get_or_synthetic(store, from)]);
    }

    // BFS.
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
    queue.push_back((from.to_string(), vec![from.to_string()]));
    visited.insert(from.to_string());

    while let Some((current, path)) = queue.pop_front() {
        if path.len() > max_hops {
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
                    return Ok(new_path
                        .iter()
                        .map(|id| get_or_synthetic(store, id))
                        .collect());
                }
                visited.insert(next.clone());
                queue.push_back((next.clone(), new_path));
            }
        }
    }

    Ok(vec![])
}

// ---------------------------------------------------------------------------
// graph_pagerank
// ---------------------------------------------------------------------------

/// Compute PageRank for all non-edge nodes and return them sorted by score (desc).
///
/// The `pagerank_score` field is injected into each node's `data` map so that
/// downstream pipeline steps (`sort`, `filter`, `project`) can operate on it.
pub fn graph_pagerank(
    store: &CrdtStore,
    damping: Option<f64>,
    iterations: Option<usize>,
) -> anyhow::Result<Vec<NodeRecord>> {
    let d = damping.unwrap_or(0.85);
    if !(d > 0.0 && d < 1.0) {
        return Err(anyhow::anyhow!("damping must be in (0, 1), got {}", d));
    }
    let iters = iterations.unwrap_or(100);

    let edges = read_edges(store);

    // Collect all unique non-edge node IDs (connected + orphans).
    let mut node_set: HashSet<String> = HashSet::new();
    for e in &edges {
        node_set.insert(e.from.clone());
        node_set.insert(e.to.clone());
    }
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
        if let (Some(&i), Some(&j)) =
            (node_index.get(e.from.as_str()), node_index.get(e.to.as_str()))
        {
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

    // Attach pagerank_score into each node's data map.
    let mut results: Vec<(f64, NodeRecord)> = nodes
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let score = scores[i];
            let mut record = get_or_synthetic(store, id);
            if let Some(obj) = record.data.as_object_mut() {
                obj.insert(
                    "pagerank_score".to_string(),
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(score)
                            .unwrap_or(serde_json::Number::from(0)),
                    ),
                );
            }
            (score, record)
        })
        .collect();

    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(results.into_iter().map(|(_, r)| r).collect())
}

// ---------------------------------------------------------------------------
// graph_stats
// ---------------------------------------------------------------------------

/// Compute summary network statistics.
///
/// Returns a single `NodeRecord` with all statistics stored in its `data` map
/// so that downstream pipeline steps (filter, sort, project) can operate on
/// them.
pub fn graph_stats(store: &CrdtStore) -> anyhow::Result<Vec<NodeRecord>> {
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

    // Average degree over all data nodes (including orphans with degree 0).
    let avg_degree = if node_count == 0 {
        0.0
    } else {
        let total: f64 = data_nodes
            .iter()
            .map(|n| *degree.get(&n.id).unwrap_or(&0) as f64)
            .sum();
        total / node_count as f64
    };
    let max_degree = degree.values().copied().max().unwrap_or(0);

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

    let record = NodeRecord::new(
        "graph_stats".to_string(),
        "system",
        serde_json::json!({
            "node_count": node_count,
            "edge_count": edge_count,
            "avg_degree": avg_degree,
            "max_degree": max_degree,
            "orphan_count": orphan_count,
            "density": density,
        }),
    );

    Ok(vec![record])
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
        // Three interconnected nodes forming a triangle + one node connected by a tail.
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
        assert!(!clusters.is_empty());
        for c in &clusters {
            let size = c.data["size"].as_u64().unwrap();
            assert!(size >= 2, "min_size=2 must be respected; got {}", size);
            assert_eq!(c.data["algorithm"], "louvain");
        }
    }

    #[test]
    fn graph_clusters_semantic() {
        let store = make_graph();
        let clusters = graph_clusters(&store, "semantic", Some(2), None).unwrap();
        assert!(!clusters.is_empty());
        for c in &clusters {
            assert_eq!(c.data["algorithm"], "semantic");
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
        assert!(clusters.is_empty());
    }

    #[test]
    fn graph_clusters_negative_min_strength_errors() {
        let store = make_graph();
        assert!(graph_clusters(&store, "louvain", None, Some(-0.1)).is_err());
    }

    #[test]
    fn graph_clusters_coherence_score_in_bounds() {
        let store = make_graph();
        let clusters = graph_clusters(&store, "louvain", Some(2), None).unwrap();
        for c in &clusters {
            let score = c.data["coherence_score"].as_f64().unwrap();
            assert!((0.0..=1.0).contains(&score), "coherence out of [0,1]: {}", score);
        }
    }

    #[test]
    fn graph_path_finds_direct() {
        let store = make_graph();
        let path = graph_path(&store, "n1", "n2", None).unwrap();
        assert!(!path.is_empty(), "should find n1->n2");
        assert_eq!(path[0].id, "n1");
        assert_eq!(path.last().unwrap().id, "n2");
    }

    #[test]
    fn graph_path_finds_indirect() {
        let store = make_graph();
        let path = graph_path(&store, "n1", "n4", None).unwrap();
        assert!(!path.is_empty());
        assert_eq!(path.first().unwrap().id, "n1");
        assert_eq!(path.last().unwrap().id, "n4");
    }

    #[test]
    fn graph_path_no_path_returns_empty() {
        let store = CrdtStore::default();
        store.put("a", "x", serde_json::json!({}));
        store.put("b", "x", serde_json::json!({}));
        let path = graph_path(&store, "a", "b", None).unwrap();
        assert!(path.is_empty());
    }

    #[test]
    fn graph_path_self_is_trivial() {
        let store = make_graph();
        let path = graph_path(&store, "n1", "n1", None).unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0].id, "n1");
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
        assert_eq!(ranked.len(), 4);
        for node in &ranked {
            assert!(node.data.get("pagerank_score").is_some());
            let score = node.data["pagerank_score"].as_f64().unwrap();
            assert!(score >= 0.0 && score <= 1.0);
        }
        // Scores should be ordered descending.
        let scores: Vec<f64> = ranked
            .iter()
            .map(|r| r.data["pagerank_score"].as_f64().unwrap())
            .collect();
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
    fn graph_pagerank_invalid_damping_errors() {
        let store = make_graph();
        assert!(graph_pagerank(&store, Some(0.0), None).is_err());
        assert!(graph_pagerank(&store, Some(1.0), None).is_err());
        assert!(graph_pagerank(&store, Some(-0.5), None).is_err());
        assert!(graph_pagerank(&store, Some(1.5), None).is_err());
    }

    #[test]
    fn graph_stats_basic() {
        let store = make_graph();
        let stats = graph_stats(&store).unwrap();
        assert_eq!(stats.len(), 1);
        let s = &stats[0];
        assert_eq!(s.data["node_count"].as_u64().unwrap(), 4);
        assert_eq!(s.data["edge_count"].as_u64().unwrap(), 4);
        // All nodes have edges, so orphan_count = 0.
        assert_eq!(s.data["orphan_count"].as_u64().unwrap(), 0);
        assert!(s.data["avg_degree"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn graph_stats_orphan_detection() {
        let store = CrdtStore::default();
        store.put("n1", "a", serde_json::json!({"label": "connected"}));
        store.put("n2", "a", serde_json::json!({"label": "orphan"}));
        store.put("edge:n1:n1", "a", serde_json::json!({"_edge": true, "from": "n1", "to": "n1"}));
        let stats = graph_stats(&store).unwrap();
        assert_eq!(stats[0].data["orphan_count"].as_u64().unwrap(), 1);
    }

    #[test]
    fn graph_stats_avg_degree_includes_orphans() {
        let store = CrdtStore::default();
        // n1 connected to itself (degree 2 in undirected), n2 orphan (degree 0).
        store.put("n1", "a", serde_json::json!({}));
        store.put("n2", "a", serde_json::json!({}));
        store.put("edge:n1:n1", "a", serde_json::json!({"_edge": true, "from": "n1", "to": "n1"}));
        let stats = graph_stats(&store).unwrap();
        let avg = stats[0].data["avg_degree"].as_f64().unwrap();
        // n1 has degree 2, n2 has degree 0 → avg = (2+0)/2 = 1.0
        assert!((avg - 1.0).abs() < 1e-9, "expected avg_degree=1.0 got {}", avg);
    }
}

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
///
/// # Complexity
///
/// All three algorithms have **O(n²)** time and space complexity with respect
/// to the number of nodes in `input` (after edge-node filtering).  For *n*
/// nodes the number of edges produced can be as large as *n × (n − 1) / 2*.
///
/// | nodes | max edges |
/// |------:|----------:|
/// |    10 |        45 |
/// |   100 |     4,950 |
/// |   500 |   124,750 |
/// | 1,000 |   499,500 |
///
/// Pre-filter the dataset with a `filter` step before calling `auto_link` to
/// keep the working set to the smallest meaningful subset.  For example:
///
/// ```text
/// filter(category == "development") |> auto_link(algorithms: ["category"])
/// ```
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
mod tests_phase2a {
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

// ---------------------------------------------------------------------------
// ChronicleTrace — causal graph traversal for Chronos chronicle nodes
// ---------------------------------------------------------------------------

/// Walk the causal graph of Chronos chronicle nodes starting from `root`.
///
/// Chronicle nodes encode causal relationships in two ways:
/// 1. A `causal_parent` field whose value is the ID of the parent node.
/// 2. Edge nodes with `_edge: true`, `from`, `to`, and `link_type: "causal"`.
///
/// ## Direction
///
/// | Value        | Behaviour                                                           |
/// |--------------|---------------------------------------------------------------------|
/// | `"backward"` | Follow `causal_parent` links toward the chain root (default)        |
/// | `"forward"`  | Traverse from root outward by following `causal_parent` back-refs   |
/// | `"both"`     | Traverse in both directions simultaneously                          |
///
/// The root node itself is always included in the result.  BFS is used so
/// nodes are returned in breadth-first order from the root.
///
/// # Arguments
///
/// * `store`     — the CRDT store to read from.
/// * `root`      — ID of the starting chronicle node.
/// * `max_depth` — maximum traversal depth in hops.
/// * `direction` — `"backward"`, `"forward"`, or `"both"` (see table above).
///
/// # Returns
///
/// All reachable chronicle nodes (including `root`) in BFS order.  Returns
/// an empty `Vec` when `root` does not exist in the store.
pub fn chronicle_trace(
    store: &CrdtStore,
    root: &str,
    max_depth: usize,
    direction: &str,
) -> Vec<NodeRecord> {
    // Resolve the root node; return empty on missing root.
    if store.get(root).is_none() {
        return vec![];
    }

    // Build a forward-adjacency map (parent → [children]) from causal edge nodes
    // and causal_parent fields so we can traverse in either direction efficiently.
    // This is O(N) over all nodes but keeps the traversal simple and correct.
    let all_nodes = store.list();

    // forward_map[parent_id] = list of child_ids that declare parent as causal_parent
    let mut forward_map: HashMap<String, Vec<String>> = HashMap::new();
    // backward_map[child_id] = parent_id (from causal_parent field)
    let mut backward_map: HashMap<String, String> = HashMap::new();

    for n in &all_nodes {
        // Ingest causal_parent field references.
        if let Some(parent_id) = n.data.get("causal_parent").and_then(|v| v.as_str()) {
            if !parent_id.is_empty() {
                backward_map.insert(n.id.clone(), parent_id.to_owned());
                forward_map.entry(parent_id.to_owned()).or_default().push(n.id.clone());
            }
        }
        // Ingest causal edge nodes (_edge: true, link_type: "causal").
        if n.data.get("_edge").and_then(|v| v.as_bool()).unwrap_or(false)
            && n.data.get("link_type").and_then(|v| v.as_str()) == Some("causal")
        {
            let from = n.data.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let to = n.data.get("to").and_then(|v| v.as_str()).unwrap_or("");
            if !from.is_empty() && !to.is_empty() {
                // Treat edge direction: from → to means `from` is the causal parent.
                // `causal_parent` field takes precedence over edge-based parents: if
                // the child node already declares a causal_parent via its data field
                // we keep that value and only add the forward direction from this edge.
                //
                // To honour this precedence during traversal, avoid recording a
                // forward edge when the child already has a different causal parent
                // registered in `backward_map`.
                if let Some(existing_parent) = backward_map.get(to) {
                    if existing_parent != from {
                        // Child already bound to a different causal parent; ignore
                        // this edge for traversal purposes.
                        continue;
                    }
                }

                backward_map
                    .entry(to.to_owned())
                    .or_insert_with(|| from.to_owned());
                forward_map
                    .entry(from.to_owned())
                    .or_default()
                    .push(to.to_owned());
            }
        }
    }

    // BFS traversal.
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    let mut result: Vec<NodeRecord> = Vec::new();

    visited.insert(root.to_owned());
    queue.push_back((root.to_owned(), 0));

    while let Some((current_id, depth)) = queue.pop_front() {
        if let Some(node) = store.get(&current_id) {
            result.push(node);
        }

        if depth >= max_depth {
            continue;
        }

        // Determine which neighbours to visit based on direction.
        let go_backward =
            direction == "backward" || direction == "both";
        let go_forward =
            direction == "forward" || direction == "both";

        if go_backward {
            if let Some(parent_id) = backward_map.get(&current_id) {
                if visited.insert(parent_id.clone()) {
                    queue.push_back((parent_id.clone(), depth + 1));
                }
            }
        }
        if go_forward {
            if let Some(children) = forward_map.get(&current_id) {
                for child_id in children {
                    if visited.insert(child_id.clone()) {
                        queue.push_back((child_id.clone(), depth + 1));
                    }
                }
            }
        }
    }

    // The root node is always the first element in the result because it is the
    // first item dequeued from BFS.  No deduplication is needed here since the
    // visited set prevents the root from being enqueued a second time.
    result
}

#[cfg(test)]
mod chronicle_tests {
    use super::*;
    use pluresdb_core::CrdtStore;

    fn make_chain(store: &CrdtStore) {
        // root ← mid ← leaf  (causal_parent links)
        store.put(
            "root",
            "actor",
            serde_json::json!({"_type": "chronos:decision", "route": "analytical"}),
        );
        store.put(
            "mid",
            "actor",
            serde_json::json!({"_type": "chronos:decision", "causal_parent": "root", "route": "creative"}),
        );
        store.put(
            "leaf",
            "actor",
            serde_json::json!({"_type": "chronos:decision", "causal_parent": "mid", "route": "quick"}),
        );
    }

    #[test]
    fn backward_traversal_from_leaf() {
        let store = CrdtStore::default();
        make_chain(&store);
        let nodes = chronicle_trace(&store, "leaf", 10, "backward");
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"leaf"));
        assert!(ids.contains(&"mid"));
        assert!(ids.contains(&"root"));
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn forward_traversal_from_root() {
        let store = CrdtStore::default();
        make_chain(&store);
        let nodes = chronicle_trace(&store, "root", 10, "forward");
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"root"));
        assert!(ids.contains(&"mid"));
        assert!(ids.contains(&"leaf"));
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn both_traversal_from_mid() {
        let store = CrdtStore::default();
        make_chain(&store);
        let nodes = chronicle_trace(&store, "mid", 10, "both");
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"root"));
        assert!(ids.contains(&"mid"));
        assert!(ids.contains(&"leaf"));
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn max_depth_limits_traversal() {
        let store = CrdtStore::default();
        make_chain(&store);
        // From leaf with depth 1: only leaf → mid (not root).
        let nodes = chronicle_trace(&store, "leaf", 1, "backward");
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"leaf"));
        assert!(ids.contains(&"mid"));
        assert!(!ids.contains(&"root"));
    }

    #[test]
    fn missing_root_returns_empty() {
        let store = CrdtStore::default();
        let nodes = chronicle_trace(&store, "nonexistent", 10, "backward");
        assert!(nodes.is_empty());
    }

    #[test]
    fn causal_edge_nodes_are_traversed() {
        let store = CrdtStore::default();
        store.put("n1", "actor", serde_json::json!({"_type": "chronos:decision"}));
        store.put("n2", "actor", serde_json::json!({"_type": "chronos:decision"}));
        // Edge stored as a separate node with link_type: "causal"
        store.put(
            "edge:n1:n2",
            "actor",
            serde_json::json!({"_edge": true, "from": "n1", "to": "n2", "link_type": "causal"}),
        );
        // Forward from n1 should reach n2 via the causal edge node.
        let nodes = chronicle_trace(&store, "n1", 10, "forward");
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"n1"));
        assert!(ids.contains(&"n2"));
    }
}
