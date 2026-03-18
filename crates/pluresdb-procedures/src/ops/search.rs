//! Vector search and text search operations.
//!
//! These steps integrate with `CrdtStore`'s vector index and node data
//! to provide semantic and keyword search within procedure pipelines.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use pluresdb_core::{CrdtStore, NodeRecord};

/// Max-heap wrapper that orders [`NodeRecord`]s by their `id` field.
///
/// Used by [`apply_text_search`] to maintain a bounded set of the
/// lexicographically-smallest `limit` matching nodes without sorting the
/// full match collection.
struct ByIdDesc(NodeRecord);

impl PartialEq for ByIdDesc {
    fn eq(&self, other: &Self) -> bool {
        self.0.id == other.0.id
    }
}

impl Eq for ByIdDesc {}

impl PartialOrd for ByIdDesc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ByIdDesc {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.id.cmp(&other.0.id)
    }
}

/// Perform a vector similarity search.
///
/// Embeds `query` using the store's embedder, then searches the HNSW index.
/// Results are returned as `NodeRecord`s ordered by descending similarity.
///
/// If the store has no embedder configured, returns an empty set.
pub fn apply_vector_search(
    store: &CrdtStore,
    query: &str,
    limit: usize,
    min_score: f64,
    category: Option<&str>,
) -> Vec<NodeRecord> {
    // Embed the query text using the store's embedder
    let embedder = match store.embedder() {
        Some(e) => e,
        None => return Vec::new(),
    };
    let embeddings = match embedder.embed(&[query]) {
        Ok(e) if !e.is_empty() => e,
        _ => return Vec::new(),
    };
    let query_embedding = &embeddings[0];

    let results = store.vector_search(
        query_embedding,
        limit.saturating_mul(2),
        min_score as f32,
    );

    let mut nodes: Vec<NodeRecord> = results
        .into_iter()
        .filter_map(|vsr| {
            // Apply category filter if specified
            if let Some(cat) = category {
                let node_cat = vsr.record.data.get("category").and_then(|v| v.as_str());
                if node_cat != Some(cat) {
                    return None;
                }
            }
            // Inject the similarity score into the node data for downstream steps
            let mut record = vsr.record;
            if let serde_json::Value::Object(ref mut map) = record.data {
                let score_value = serde_json::json!(vsr.score);
                // Keep `_score` for backward compatibility, but also set `score`
                // so downstream pipeline steps that expect `score` can use it.
                map.insert("_score".to_string(), score_value.clone());
                map.insert("score".to_string(), score_value);
            }
            Some(record)
        })
        .take(limit)
        .collect();

    // Already ordered by score from vector_search, but ensure stability
    nodes.sort_by(|a, b| {
        let sa = a
            .data
            .get("score")
            .or_else(|| a.data.get("_score"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let sb = b
            .data
            .get("score")
            .or_else(|| b.data.get("_score"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });

    nodes
}

/// Perform a full-text keyword search over node data.
///
/// Returns nodes whose `data.<field>` contains **all** whitespace-separated
/// terms from `query` (case-insensitive).
pub fn apply_text_search(
    store: &CrdtStore,
    query: &str,
    limit: usize,
    field: &str,
) -> Vec<NodeRecord> {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|t| t.to_lowercase())
        .collect();

    if terms.is_empty() || limit == 0 {
        return Vec::new();
    }

    // Use a bounded max-heap (ordered by node id) to keep track of the
    // `limit` lexicographically-smallest matching nodes as we scan.
    //
    // Maintaining a max-heap of size `limit` means:
    //   • The current worst (largest id) is always at the top.
    //   • When a new match arrives whose id is smaller than the top, we evict
    //     the top and insert the new match — O(log limit) per operation.
    //   • We never hold more than `limit` matching nodes in memory at once,
    //     avoiding the O(N) working set of a collect-then-sort approach.
    //   • The final sort is O(limit log limit) over at most `limit` entries.
    let mut heap: BinaryHeap<ByIdDesc> = BinaryHeap::with_capacity(limit);

    for node in store.list() {
        let text = node
            .data
            .get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        if terms.iter().all(|term| text.contains(term.as_str())) {
            if heap.len() < limit {
                heap.push(ByIdDesc(node));
            } else if let Some(top) = heap.peek() {
                // `top` is the current worst (largest id) in the heap.
                // Evict it if the new match has a smaller id.
                if node.id < top.0.id {
                    heap.pop();
                    heap.push(ByIdDesc(node));
                }
            }
        }
    }

    // Drain heap into a Vec and sort ascending by id for a deterministic
    // result. The heap contains at most `limit` entries so this is cheap.
    let mut matches: Vec<NodeRecord> = heap.into_iter().map(|e| e.0).collect();
    matches.sort_by(|a, b| a.id.cmp(&b.id));
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use pluresdb_core::CrdtStore;

    #[test]
    fn text_search_matches_all_terms() {
        let store = CrdtStore::default();
        store.put(
            "n1",
            "a",
            serde_json::json!({"text": "Rust is a systems programming language"}),
        );
        store.put(
            "n2",
            "a",
            serde_json::json!({"text": "Python is great for scripting"}),
        );
        store.put(
            "n3",
            "a",
            serde_json::json!({"text": "Rust systems are fast"}),
        );

        let results = apply_text_search(&store, "rust systems", 10, "text");
        assert_eq!(results.len(), 2); // n1 and n3 match
        for r in &results {
            assert!(r.id == "n1" || r.id == "n3");
        }
    }

    #[test]
    fn text_search_case_insensitive() {
        let store = CrdtStore::default();
        store.put("n1", "a", serde_json::json!({"text": "RUST is FAST"}));

        let results = apply_text_search(&store, "rust fast", 10, "text");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn text_search_respects_limit() {
        let store = CrdtStore::default();
        for i in 0..20 {
            store.put(
                &format!("n{}", i),
                "a",
                serde_json::json!({"text": format!("memory entry {}", i)}),
            );
        }

        let results = apply_text_search(&store, "memory", 5, "text");
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn text_search_custom_field() {
        let store = CrdtStore::default();
        store.put(
            "n1",
            "a",
            serde_json::json!({"content": "important decision", "text": "unrelated"}),
        );

        let results = apply_text_search(&store, "decision", 10, "content");
        assert_eq!(results.len(), 1);

        let results = apply_text_search(&store, "decision", 10, "text");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn text_search_empty_query_returns_empty() {
        let store = CrdtStore::default();
        store.put("n1", "a", serde_json::json!({"text": "anything"}));

        let results = apply_text_search(&store, "", 10, "text");
        assert!(results.is_empty());
    }

    /// Verify the bounded-heap behaviour: with more matches than `limit` the
    /// result must contain the lexicographically-*smallest* node ids, not
    /// merely the first ones encountered during a scan.
    #[test]
    fn text_search_limit_returns_smallest_ids() {
        let store = CrdtStore::default();
        // Insert nodes with ids that are intentionally "out of order" to
        // exercise the heap eviction path.
        for suffix in &["z9", "a1", "m5", "b2", "y8", "c3"] {
            store.put(
                &format!("node-{}", suffix),
                "a",
                serde_json::json!({"text": "heap test entry"}),
            );
        }

        let results = apply_text_search(&store, "heap", 3, "text");
        assert_eq!(results.len(), 3);
        // Expected: the three lexicographically smallest ids are
        // "node-a1", "node-b2", "node-c3".
        let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["node-a1", "node-b2", "node-c3"]);
    }
}
