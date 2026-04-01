//! Document storage and enrichment procedures.
//!
//! Each procedure validates its inputs, writes nodes to the [`CrdtStore`] with
//! appropriate `_type` tags, creates graph relationships as edge nodes, and
//! handles content-based deduplication using deterministic UUIDs.
//!
//! ## Node schemas
//!
//! ### Document node
//! ```json
//! {
//!   "_type": "document",
//!   "title": "...",
//!   "content": "...",
//!   "content_hash": "<uuid-v5>",
//!   "source": "...",       // optional
//!   "word_count": 42,      // set by enrich_document_metadata
//!   "tags": ["...", "..."] // optional, from metadata
//! }
//! ```
//!
//! ### Chunk node
//! ```json
//! {
//!   "_type": "chunk",
//!   "content": "...",
//!   "content_hash": "<uuid-v5>",
//!   "index": 0,
//!   "parent_id": "<document-id>"
//! }
//! ```
//!
//! ### Relationship edges
//! Stored as CRDT nodes with `_edge: true` following the `edge:{from}:{to}`
//! identifier convention.

use uuid::Uuid;

use crate::CrdtStore;

// ---------------------------------------------------------------------------
// Namespace UUID for content hashing
// ---------------------------------------------------------------------------

/// A fixed namespace UUID used by [`Uuid::new_v5`] when deriving
/// content-based identifiers.  Using a project-specific namespace ensures
/// hashes produced by this library do not collide with those computed by
/// other applications using the same algorithm.
const CONTENT_NS: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x14, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

// ---------------------------------------------------------------------------
// store_document
// ---------------------------------------------------------------------------

/// Store a document with its metadata in the [`CrdtStore`].
///
/// Deduplication is performed by content hash: if a node whose
/// `content_hash` field matches the SHA-1-based UUID derived from `content`
/// already exists, its ID is returned without creating a duplicate.
///
/// When an [`crate::EmbedText`] backend is attached to `store`, an embedding
/// is generated automatically by the underlying [`CrdtStore::put`] call.
///
/// # Arguments
///
/// * `store`    — the CRDT store to write into.
/// * `actor`    — logical actor / author identifier (used for CRDT clocks).
/// * `title`    — human-readable document title (must be non-empty).
/// * `content`  — full document text (must be non-empty).
/// * `metadata` — optional JSON object whose keys are merged into the stored
///   node.  Passing `None` is equivalent to passing an empty object.
///
/// # Errors
///
/// Returns an error when `title` or `content` is empty, or when `metadata`
/// is provided but is not a JSON object.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::document::store_document};
///
/// let store = CrdtStore::default();
/// let id = store_document(&store, "actor", "My Doc", "Hello world", None).unwrap();
/// assert!(store.get(&id).is_some());
/// ```
pub fn store_document(
    store: &CrdtStore,
    actor: &str,
    title: &str,
    content: &str,
    metadata: Option<serde_json::Value>,
) -> anyhow::Result<String> {
    // --- input validation ---
    let title = title.trim();
    let content = content.trim();
    anyhow::ensure!(!title.is_empty(), "store_document: title must not be empty");
    anyhow::ensure!(
        !content.is_empty(),
        "store_document: content must not be empty"
    );
    anyhow::ensure!(!actor.is_empty(), "store_document: actor must not be empty");

    if let Some(ref meta) = metadata {
        anyhow::ensure!(
            meta.is_object(),
            "store_document: metadata must be a JSON object"
        );
    }

    // --- content-based dedup ---
    let content_hash = Uuid::new_v5(&CONTENT_NS, content.as_bytes()).to_string();

    // Check whether a document with this content hash already exists.
    if let Some(existing) = find_node_by_content_hash(store, &content_hash) {
        return Ok(existing);
    }

    // --- build node data ---
    let mut data = serde_json::json!({
        "_type": "document",
        "title": title,
        "content": content,
        "content_hash": content_hash,
    });

    if let Some(meta) = metadata {
        if let (Some(obj), Some(meta_obj)) = (data.as_object_mut(), meta.as_object()) {
            for (k, v) in meta_obj {
                obj.insert(k.clone(), v.clone());
            }
        }
    }

    let id = Uuid::new_v4().to_string();
    store.put(id.clone(), actor, data);
    Ok(id)
}

// ---------------------------------------------------------------------------
// store_document_chunk
// ---------------------------------------------------------------------------

/// Store a chunk (sub-section) of a document and link it to its parent.
///
/// A directed `has_chunk` edge is created from `parent_id` to the new chunk
/// node so that chunk membership can be queried via graph traversal.
///
/// Deduplication is applied on the chunk `content`: if a chunk with the same
/// `content_hash` already exists **and** has the same `parent_id`, its ID is
/// returned without creating a duplicate.
///
/// # Arguments
///
/// * `store`     — the CRDT store to write into.
/// * `actor`     — logical actor / author identifier.
/// * `parent_id` — ID of the parent document (must exist in `store`).
/// * `content`   — chunk text (must be non-empty).
/// * `index`     — zero-based position of this chunk within its parent
///   document.
///
/// # Errors
///
/// Returns an error when `parent_id` or `content` is empty, when the parent
/// document does not exist in the store, or when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::document::{store_document, store_document_chunk}};
///
/// let store = CrdtStore::default();
/// let doc_id = store_document(&store, "actor", "Doc", "Full text here", None).unwrap();
/// let chunk_id = store_document_chunk(&store, "actor", &doc_id, "Full text here", 0).unwrap();
/// assert!(store.get(&chunk_id).is_some());
/// ```
pub fn store_document_chunk(
    store: &CrdtStore,
    actor: &str,
    parent_id: &str,
    content: &str,
    index: usize,
) -> anyhow::Result<String> {
    // --- input validation ---
    let content = content.trim();
    anyhow::ensure!(
        !content.is_empty(),
        "store_document_chunk: content must not be empty"
    );
    anyhow::ensure!(
        !parent_id.is_empty(),
        "store_document_chunk: parent_id must not be empty"
    );
    anyhow::ensure!(
        !actor.is_empty(),
        "store_document_chunk: actor must not be empty"
    );
    anyhow::ensure!(
        store.get(parent_id).is_some(),
        "store_document_chunk: parent document '{}' not found",
        parent_id
    );

    // --- content-based dedup: same content + same parent ---
    let dedup_input = format!("{}:{}", parent_id, content);
    let content_hash = Uuid::new_v5(&CONTENT_NS, dedup_input.as_bytes()).to_string();

    if let Some(existing) = find_node_by_content_hash(store, &content_hash) {
        return Ok(existing);
    }

    // --- build chunk node ---
    let chunk_id = Uuid::new_v4().to_string();
    let data = serde_json::json!({
        "_type": "chunk",
        "content": content,
        "content_hash": content_hash,
        "index": index,
        "parent_id": parent_id,
    });
    store.put(chunk_id.clone(), actor, data);

    // --- create has_chunk edge: parent_id → chunk_id ---
    let edge_id = format!("edge:{}:{}", parent_id, chunk_id);
    let edge_data = serde_json::json!({
        "_edge": true,
        "from": parent_id,
        "to": chunk_id,
        "label": "has_chunk",
    });
    store.put(edge_id, actor, edge_data);

    Ok(chunk_id)
}

// ---------------------------------------------------------------------------
// link_document_chunks
// ---------------------------------------------------------------------------

/// Create a directed relationship edge between two document chunks.
///
/// Both `from_chunk` and `to_chunk` must already exist in the store and have
/// `_type == "chunk"`.  If an edge between the same pair already exists it is
/// silently overwritten (CRDT merge semantics).
///
/// # Arguments
///
/// * `store`      — the CRDT store to write into.
/// * `actor`      — logical actor / author identifier.
/// * `from_chunk` — ID of the source chunk.
/// * `to_chunk`   — ID of the target chunk.
/// * `label`      — optional relationship label (e.g. `"related"`,
///   `"continues"`).  Defaults to `"related"` when `None`.
/// * `strength`   — optional edge strength in the range `[0.0, 1.0]`.
///   Clamped to that range when provided.
///
/// # Errors
///
/// Returns an error when either chunk ID is empty, when the corresponding
/// nodes are not found in the store, when either node's `_type` is not
/// `"chunk"`, or when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::document::{store_document, store_document_chunk, link_document_chunks}};
///
/// let store = CrdtStore::default();
/// let doc_id = store_document(&store, "actor", "Doc", "text", None).unwrap();
/// let c1 = store_document_chunk(&store, "actor", &doc_id, "chunk one", 0).unwrap();
/// let c2 = store_document_chunk(&store, "actor", &doc_id, "chunk two", 1).unwrap();
/// link_document_chunks(&store, "actor", &c1, &c2, Some("related"), Some(0.8)).unwrap();
/// ```
pub fn link_document_chunks(
    store: &CrdtStore,
    actor: &str,
    from_chunk: &str,
    to_chunk: &str,
    label: Option<&str>,
    strength: Option<f64>,
) -> anyhow::Result<()> {
    // --- input validation ---
    anyhow::ensure!(
        !from_chunk.is_empty(),
        "link_document_chunks: from_chunk must not be empty"
    );
    anyhow::ensure!(
        !to_chunk.is_empty(),
        "link_document_chunks: to_chunk must not be empty"
    );
    anyhow::ensure!(
        !actor.is_empty(),
        "link_document_chunks: actor must not be empty"
    );
    anyhow::ensure!(
        from_chunk != to_chunk,
        "link_document_chunks: from_chunk and to_chunk must be different"
    );

    let from_node = store
        .get(from_chunk)
        .ok_or_else(|| anyhow::anyhow!("link_document_chunks: chunk '{}' not found", from_chunk))?;
    let to_node = store
        .get(to_chunk)
        .ok_or_else(|| anyhow::anyhow!("link_document_chunks: chunk '{}' not found", to_chunk))?;

    anyhow::ensure!(
        from_node.data.get("_type").and_then(|v| v.as_str()) == Some("chunk"),
        "link_document_chunks: node '{}' is not a chunk",
        from_chunk
    );
    anyhow::ensure!(
        to_node.data.get("_type").and_then(|v| v.as_str()) == Some("chunk"),
        "link_document_chunks: node '{}' is not a chunk",
        to_chunk
    );

    // --- build edge ---
    let edge_label = label.unwrap_or("related");
    let edge_strength = strength.map(|s| s.clamp(0.0, 1.0)).unwrap_or(1.0);

    let edge_id = format!("edge:{}:{}", from_chunk, to_chunk);
    let edge_data = serde_json::json!({
        "_edge": true,
        "from": from_chunk,
        "to": to_chunk,
        "label": edge_label,
        "strength": edge_strength,
    });
    store.put(edge_id, actor, edge_data);

    Ok(())
}

// ---------------------------------------------------------------------------
// enrich_document_metadata
// ---------------------------------------------------------------------------

/// Extract additional metadata from a document's content and merge it back
/// into the stored node.
///
/// The following fields are computed and written back to the document node:
///
/// | Field          | Description                                      |
/// |----------------|--------------------------------------------------|
/// | `word_count`   | Number of whitespace-separated tokens            |
/// | `char_count`   | Total character count (excl. leading/trailing ws)|
/// | `sentence_count` | Approximate sentence count (`.`/`!`/`?` split) |
/// | `top_keywords` | Up to 10 most-frequent non-stopword tokens       |
///
/// # Arguments
///
/// * `store`       — the CRDT store to read from and write into.
/// * `actor`       — logical actor / author identifier.
/// * `document_id` — ID of the document node to enrich (must exist).
///
/// # Returns
///
/// A JSON object containing the extracted metadata fields.
///
/// # Errors
///
/// Returns an error when `document_id` is empty, when the node is not found,
/// when the node's `_type` is not `"document"`, or when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::document::{store_document, enrich_document_metadata}};
///
/// let store = CrdtStore::default();
/// let id = store_document(&store, "actor", "My Doc", "Hello world. Foo bar.", None).unwrap();
/// let meta = enrich_document_metadata(&store, "actor", &id).unwrap();
/// assert_eq!(meta["word_count"], 4);
/// ```
pub fn enrich_document_metadata(
    store: &CrdtStore,
    actor: &str,
    document_id: &str,
) -> anyhow::Result<serde_json::Value> {
    // --- input validation ---
    anyhow::ensure!(
        !document_id.is_empty(),
        "enrich_document_metadata: document_id must not be empty"
    );
    anyhow::ensure!(
        !actor.is_empty(),
        "enrich_document_metadata: actor must not be empty"
    );

    let node = store.get(document_id).ok_or_else(|| {
        anyhow::anyhow!(
            "enrich_document_metadata: document '{}' not found",
            document_id
        )
    })?;

    anyhow::ensure!(
        node.data.get("_type").and_then(|v| v.as_str()) == Some("document"),
        "enrich_document_metadata: node '{}' is not a document",
        document_id
    );

    let content = node
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    // --- compute metadata ---
    let word_count = count_words(content);
    let char_count = content.chars().count();
    let sentence_count = count_sentences(content);
    let top_keywords = extract_top_keywords(content, 10);

    let enriched = serde_json::json!({
        "word_count": word_count,
        "char_count": char_count,
        "sentence_count": sentence_count,
        "top_keywords": top_keywords,
    });

    // --- merge back into the stored node ---
    let mut updated_data = node.data.clone();
    if let (Some(obj), Some(extra)) = (updated_data.as_object_mut(), enriched.as_object()) {
        for (k, v) in extra {
            obj.insert(k.clone(), v.clone());
        }
    }
    store.put(document_id.to_owned(), actor, updated_data);

    Ok(enriched)
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Scan all nodes in the store and return the ID of the first one whose
/// `content_hash` field equals `hash`, or `None` if no match is found.
///
/// # Performance note
///
/// This performs a full scan of all nodes in the store (O(n)).  For stores
/// with a large number of documents, callers may wish to maintain an
/// external index on `content_hash`.  In practice, deduplication checks are
/// only performed on insert paths, so the cost is amortised.
fn find_node_by_content_hash(store: &CrdtStore, hash: &str) -> Option<String> {
    store.list().into_iter().find_map(|n| {
        let h = n.data.get("content_hash")?.as_str()?;
        if h == hash {
            Some(n.id)
        } else {
            None
        }
    })
}

/// Count whitespace-separated tokens in `text`.
fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Approximate sentence count by splitting on `.`, `!`, and `?`.
fn count_sentences(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.chars()
        .filter(|&c| matches!(c, '.' | '!' | '?'))
        .count()
        .max(1)
}

/// Common English stop-words to exclude from keyword extraction.
const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "is", "it", "its", "this", "that", "be", "as", "are", "was", "were", "has", "have",
    "had", "not", "no", "so", "if", "do", "did", "he", "she", "we", "you", "they", "i", "me", "my",
    "your", "our", "their", "his", "her", "up", "out", "will", "can", "may", "all",
];

/// Return the top `limit` most frequent non-stop-word tokens from `text`,
/// ordered by descending frequency (ties broken lexicographically).
///
/// Tokens are split on whitespace and standard ASCII punctuation (excluding
/// apostrophes and hyphens so that contractions and hyphenated words remain
/// intact).  Tokens shorter than 3 characters are excluded because they are
/// rarely meaningful keywords (single letters, "be", "go", etc.).
fn extract_top_keywords(text: &str, limit: usize) -> Vec<String> {
    use std::collections::HashMap;

    let mut freq: HashMap<String, usize> = HashMap::new();
    for word in text
        .split(|c: char| c.is_whitespace() || (c.is_ascii_punctuation() && c != '\'' && c != '-'))
    {
        let lower = word.to_lowercase();
        let w = lower.trim_matches(|c: char| !c.is_alphanumeric());
        // Skip short tokens (single chars, "be", "go", etc.) — rarely meaningful as keywords.
        if w.len() < 3 {
            continue;
        }
        if STOP_WORDS.contains(&w) {
            continue;
        }
        *freq.entry(w.to_owned()).or_insert(0) += 1;
    }

    let mut pairs: Vec<(String, usize)> = freq.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    pairs.truncate(limit);
    pairs.into_iter().map(|(w, _)| w).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CrdtStore;

    // -----------------------------------------------------------------------
    // store_document
    // -----------------------------------------------------------------------

    #[test]
    fn store_document_creates_node() {
        let store = CrdtStore::default();
        let id = store_document(&store, "actor", "Title", "Some content here", None).unwrap();
        let node = store.get(&id).expect("node should exist");
        assert_eq!(node.data["_type"], "document");
        assert_eq!(node.data["title"], "Title");
        assert_eq!(node.data["content"], "Some content here");
        assert!(node.data["content_hash"].as_str().is_some());
    }

    #[test]
    fn store_document_deduplicates_by_content() {
        let store = CrdtStore::default();
        let id1 = store_document(&store, "a", "T1", "same content", None).unwrap();
        let id2 = store_document(&store, "a", "T2", "same content", None).unwrap();
        assert_eq!(id1, id2, "identical content should return the same ID");
    }

    #[test]
    fn store_document_different_content_different_id() {
        let store = CrdtStore::default();
        let id1 = store_document(&store, "a", "T", "content one", None).unwrap();
        let id2 = store_document(&store, "a", "T", "content two", None).unwrap();
        assert_ne!(id1, id2);
    }

    #[test]
    fn store_document_merges_metadata() {
        let store = CrdtStore::default();
        let meta = serde_json::json!({"source": "web", "tags": ["rust", "db"]});
        let id = store_document(&store, "a", "T", "body text", Some(meta)).unwrap();
        let node = store.get(&id).unwrap();
        assert_eq!(node.data["source"], "web");
        assert_eq!(node.data["tags"][0], "rust");
    }

    #[test]
    fn store_document_rejects_empty_title() {
        let store = CrdtStore::default();
        let err = store_document(&store, "a", "", "content", None).unwrap_err();
        assert!(err.to_string().contains("title"));
    }

    #[test]
    fn store_document_rejects_empty_content() {
        let store = CrdtStore::default();
        let err = store_document(&store, "a", "T", "", None).unwrap_err();
        assert!(err.to_string().contains("content"));
    }

    #[test]
    fn store_document_rejects_empty_actor() {
        let store = CrdtStore::default();
        let err = store_document(&store, "", "T", "content", None).unwrap_err();
        assert!(err.to_string().contains("actor"));
    }

    #[test]
    fn store_document_rejects_non_object_metadata() {
        let store = CrdtStore::default();
        let err = store_document(&store, "a", "T", "content", Some(serde_json::json!([1, 2])))
            .unwrap_err();
        assert!(err.to_string().contains("metadata"));
    }

    // -----------------------------------------------------------------------
    // store_document_chunk
    // -----------------------------------------------------------------------

    #[test]
    fn store_document_chunk_creates_node_and_edge() {
        let store = CrdtStore::default();
        let doc_id = store_document(&store, "a", "Doc", "full text", None).unwrap();
        let chunk_id = store_document_chunk(&store, "a", &doc_id, "first chunk", 0).unwrap();

        let chunk = store.get(&chunk_id).expect("chunk should exist");
        assert_eq!(chunk.data["_type"], "chunk");
        assert_eq!(chunk.data["content"], "first chunk");
        assert_eq!(chunk.data["index"], 0);
        assert_eq!(chunk.data["parent_id"], doc_id.as_str());

        let edge_id = format!("edge:{}:{}", doc_id, chunk_id);
        let edge = store.get(&edge_id).expect("edge should exist");
        assert_eq!(edge.data["label"], "has_chunk");
        assert_eq!(edge.data["from"], doc_id.as_str());
        assert_eq!(edge.data["to"], chunk_id.as_str());
    }

    #[test]
    fn store_document_chunk_deduplicates() {
        let store = CrdtStore::default();
        let doc_id = store_document(&store, "a", "Doc", "text", None).unwrap();
        let c1 = store_document_chunk(&store, "a", &doc_id, "chunk text", 0).unwrap();
        let c2 = store_document_chunk(&store, "a", &doc_id, "chunk text", 0).unwrap();
        assert_eq!(c1, c2);
    }

    #[test]
    fn store_document_chunk_rejects_missing_parent() {
        let store = CrdtStore::default();
        let err = store_document_chunk(&store, "a", "nonexistent", "chunk", 0).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn store_document_chunk_rejects_empty_content() {
        let store = CrdtStore::default();
        let doc_id = store_document(&store, "a", "D", "text", None).unwrap();
        let err = store_document_chunk(&store, "a", &doc_id, "", 0).unwrap_err();
        assert!(err.to_string().contains("content"));
    }

    #[test]
    fn store_document_chunk_rejects_empty_parent_id() {
        let store = CrdtStore::default();
        let err = store_document_chunk(&store, "a", "", "chunk", 0).unwrap_err();
        assert!(err.to_string().contains("parent_id"));
    }

    // -----------------------------------------------------------------------
    // link_document_chunks
    // -----------------------------------------------------------------------

    #[test]
    fn link_document_chunks_creates_edge() {
        let store = CrdtStore::default();
        let doc_id = store_document(&store, "a", "D", "text", None).unwrap();
        let c1 = store_document_chunk(&store, "a", &doc_id, "chunk one", 0).unwrap();
        let c2 = store_document_chunk(&store, "a", &doc_id, "chunk two", 1).unwrap();

        link_document_chunks(&store, "a", &c1, &c2, Some("related"), Some(0.7)).unwrap();

        let edge_id = format!("edge:{}:{}", c1, c2);
        let edge = store.get(&edge_id).expect("edge should exist");
        assert_eq!(edge.data["label"], "related");
        assert!((edge.data["strength"].as_f64().unwrap() - 0.7).abs() < 1e-10);
    }

    #[test]
    fn link_document_chunks_default_label_and_strength() {
        let store = CrdtStore::default();
        let doc_id = store_document(&store, "a", "D", "text", None).unwrap();
        let c1 = store_document_chunk(&store, "a", &doc_id, "chunk a", 0).unwrap();
        let c2 = store_document_chunk(&store, "a", &doc_id, "chunk b", 1).unwrap();

        link_document_chunks(&store, "a", &c1, &c2, None, None).unwrap();

        let edge_id = format!("edge:{}:{}", c1, c2);
        let edge = store.get(&edge_id).unwrap();
        assert_eq!(edge.data["label"], "related");
        assert_eq!(edge.data["strength"], 1.0);
    }

    #[test]
    fn link_document_chunks_clamps_strength() {
        let store = CrdtStore::default();
        let doc_id = store_document(&store, "a", "D", "text", None).unwrap();
        let c1 = store_document_chunk(&store, "a", &doc_id, "ca", 0).unwrap();
        let c2 = store_document_chunk(&store, "a", &doc_id, "cb", 1).unwrap();

        link_document_chunks(&store, "a", &c1, &c2, None, Some(5.0)).unwrap();

        let edge_id = format!("edge:{}:{}", c1, c2);
        let edge = store.get(&edge_id).unwrap();
        assert_eq!(edge.data["strength"], 1.0);
    }

    #[test]
    fn link_document_chunks_rejects_non_chunk_node() {
        let store = CrdtStore::default();
        let doc_id = store_document(&store, "a", "D", "text", None).unwrap();
        let c1 = store_document_chunk(&store, "a", &doc_id, "chunk one", 0).unwrap();

        // doc_id has _type == "document", not "chunk"
        let err = link_document_chunks(&store, "a", &doc_id, &c1, None, None).unwrap_err();
        assert!(err.to_string().contains("not a chunk"));
    }

    #[test]
    fn link_document_chunks_rejects_same_chunk() {
        let store = CrdtStore::default();
        let doc_id = store_document(&store, "a", "D", "text", None).unwrap();
        let c1 = store_document_chunk(&store, "a", &doc_id, "chunk one", 0).unwrap();

        let err = link_document_chunks(&store, "a", &c1, &c1, None, None).unwrap_err();
        assert!(err.to_string().contains("different"));
    }

    #[test]
    fn link_document_chunks_rejects_missing_chunk() {
        let store = CrdtStore::default();
        let doc_id = store_document(&store, "a", "D", "text", None).unwrap();
        let c1 = store_document_chunk(&store, "a", &doc_id, "chunk x", 0).unwrap();

        let err = link_document_chunks(&store, "a", &c1, "ghost", None, None).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    // -----------------------------------------------------------------------
    // enrich_document_metadata
    // -----------------------------------------------------------------------

    #[test]
    fn enrich_document_metadata_adds_fields() {
        let store = CrdtStore::default();
        let id = store_document(&store, "a", "Doc", "Hello world. Foo bar baz.", None).unwrap();
        let meta = enrich_document_metadata(&store, "a", &id).unwrap();

        assert_eq!(meta["word_count"], 5);
        assert!(meta["char_count"].as_u64().unwrap() > 0);
        assert!(meta["sentence_count"].as_u64().unwrap() > 0);
        assert!(meta["top_keywords"].is_array());
    }

    #[test]
    fn enrich_document_metadata_persists_to_node() {
        let store = CrdtStore::default();
        let id = store_document(&store, "a", "D", "word1 word2 word3", None).unwrap();
        enrich_document_metadata(&store, "a", &id).unwrap();

        let node = store.get(&id).unwrap();
        assert_eq!(node.data["word_count"], 3);
    }

    #[test]
    fn enrich_document_metadata_rejects_non_document() {
        let store = CrdtStore::default();
        let doc_id = store_document(&store, "a", "D", "text", None).unwrap();
        let chunk_id = store_document_chunk(&store, "a", &doc_id, "chunk", 0).unwrap();

        let err = enrich_document_metadata(&store, "a", &chunk_id).unwrap_err();
        assert!(err.to_string().contains("not a document"));
    }

    #[test]
    fn enrich_document_metadata_rejects_missing_document() {
        let store = CrdtStore::default();
        let err = enrich_document_metadata(&store, "a", "no-such-id").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    #[test]
    fn count_words_empty() {
        assert_eq!(count_words(""), 0);
    }

    #[test]
    fn count_words_multiple() {
        assert_eq!(count_words("one two three"), 3);
    }

    #[test]
    fn count_sentences_empty() {
        assert_eq!(count_sentences(""), 0);
    }

    #[test]
    fn count_sentences_no_punctuation_returns_one() {
        assert_eq!(count_sentences("hello world"), 1);
    }

    #[test]
    fn count_sentences_punctuation() {
        assert_eq!(count_sentences("Hello. World! How?"), 3);
    }

    #[test]
    fn extract_top_keywords_removes_stopwords() {
        let kw = extract_top_keywords("the quick brown fox jumps and the lazy dog", 10);
        assert!(!kw.contains(&"the".to_owned()));
        assert!(!kw.contains(&"and".to_owned()));
    }

    #[test]
    fn extract_top_keywords_frequency_order() {
        let kw = extract_top_keywords("rust rust rust python python java", 3);
        assert_eq!(kw[0], "rust", "highest frequency word should be first");
    }
}
