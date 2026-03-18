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

    let results = store.vector_search(query_embedding, limit * 2, min_score as f32);

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

    let all_nodes = store.list();
    let mut matches: Vec<NodeRecord> = all_nodes
        .into_iter()
        .filter(|node| {
            let text = node
                .data
                .get(field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            terms.iter().all(|term| text.contains(term.as_str()))
        })
        .collect();

    matches.truncate(limit);
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
}
