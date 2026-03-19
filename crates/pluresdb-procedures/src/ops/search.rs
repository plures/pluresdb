//! Vector search and text search operations.
//!
//! These steps integrate with `CrdtStore`'s vector index and node data
//! to provide semantic and keyword search within procedure pipelines.

use pluresdb_core::{CrdtStore, NodeRecord};

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

    if terms.is_empty() {
        return Vec::new();
    }

    // Collect up to `limit` matching nodes, short-circuiting the scan once
    // enough matches have been found to avoid unnecessary work on large stores.
    let mut matches: Vec<NodeRecord> = Vec::with_capacity(limit);

    for node in store.list().into_iter() {
        if matches.len() == limit {
            break;
        }

        let text = node
            .data
            .get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        if terms.iter().all(|term| text.contains(term.as_str())) {
            matches.push(node);
        }
    }

    // Ensure deterministic ordering for the returned subset. We sort by a
    // stable key (the node's id) so that the result order is reproducible.
    matches.sort_by(|a, b| a.id.cmp(&b.id));

    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use pluresdb_core::{CrdtStore, EmbedText};
    use std::sync::Arc;

    // ---------------------------------------------------------------------------
    // Mock embedder: always returns [1.0, 0.0, 0.0] regardless of input text.
    // Two texts therefore always have cosine similarity 1.0, which makes
    // vector-search results fully deterministic in unit tests.
    // ---------------------------------------------------------------------------
    #[derive(Debug)]
    struct AlwaysOneEmbedder;

    impl EmbedText for AlwaysOneEmbedder {
        fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![1.0_f32, 0.0, 0.0]).collect())
        }

        fn dimension(&self) -> usize {
            3
        }
    }

    // Helper: build a store with the mock embedder already attached.
    fn store_with_embedder() -> CrdtStore {
        CrdtStore::default().with_embedder(Arc::new(AlwaysOneEmbedder))
    }

    // ---------------------------------------------------------------------------
    // apply_vector_search tests
    // ---------------------------------------------------------------------------

    #[test]
    fn vector_search_no_embedder_returns_empty() {
        // A store with no embedder must return an empty result immediately.
        let store = CrdtStore::default();
        store.put("n1", "a", serde_json::json!({"content": "hello"}));

        let results = apply_vector_search(&store, "hello", 10, 0.0, None);
        assert!(
            results.is_empty(),
            "expected empty result when store has no embedder"
        );
    }

    #[test]
    fn vector_search_injects_score_fields() {
        // Nodes pre-seeded with the same unit vector as our mock embedder
        // produces → cosine similarity = 1.0.
        let store = store_with_embedder();
        store.put_with_embedding(
            "n1",
            "a",
            serde_json::json!({"content": "hello"}),
            vec![1.0_f32, 0.0, 0.0],
        );

        let results = apply_vector_search(&store, "any text", 10, 0.0, None);
        assert_eq!(results.len(), 1, "expected exactly one result");

        let data = &results[0].data;
        assert!(
            data.get("score").is_some(),
            "`score` field should be injected into result data"
        );
        assert!(
            data.get("_score").is_some(),
            "`_score` field should be injected into result data for backward compatibility"
        );

        let score = data["score"].as_f64().expect("`score` should be a number");
        let _score = data["_score"].as_f64().expect("`_score` should be a number");
        assert!(
            (score - 1.0_f64).abs() < 1e-5,
            "`score` should be ~1.0 for identical unit vectors, got {score}"
        );
        assert!(
            (_score - score).abs() < 1e-10,
            "`score` and `_score` should hold the same value"
        );
    }

    #[test]
    fn vector_search_category_filter_excludes_non_matching() {
        let store = store_with_embedder();
        store.put_with_embedding(
            "cat_a",
            "a",
            serde_json::json!({"content": "alpha", "category": "A"}),
            vec![1.0_f32, 0.0, 0.0],
        );
        store.put_with_embedding(
            "cat_b",
            "a",
            serde_json::json!({"content": "beta", "category": "B"}),
            vec![1.0_f32, 0.0, 0.0],
        );

        let results = apply_vector_search(&store, "any", 10, 0.0, Some("A"));
        assert_eq!(results.len(), 1, "only the node in category A should match");
        assert_eq!(results[0].id, "cat_a");
    }

    #[test]
    fn vector_search_min_score_filters_results() {
        // Nodes with embedding orthogonal to the query ([0,1,0] vs [1,0,0])
        // have cosine similarity 0.0 and must be excluded when min_score > 0.
        let store = store_with_embedder();
        // This node aligns with the query → similarity = 1.0
        store.put_with_embedding(
            "aligned",
            "a",
            serde_json::json!({"content": "aligned"}),
            vec![1.0_f32, 0.0, 0.0],
        );
        // This node is orthogonal to the query → similarity = 0.0
        store.put_with_embedding(
            "orthogonal",
            "a",
            serde_json::json!({"content": "orthogonal"}),
            vec![0.0_f32, 1.0, 0.0],
        );

        // With a strict min_score only the aligned node should survive.
        let results = apply_vector_search(&store, "any text", 10, 0.5, None);
        assert_eq!(results.len(), 1, "exactly one node should pass min_score=0.5");
        assert_eq!(
            results[0].id, "aligned",
            "orthogonal node should be filtered out by min_score=0.5"
        );
    }

    // ---------------------------------------------------------------------------
    // apply_text_search tests
    // ---------------------------------------------------------------------------

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
}
