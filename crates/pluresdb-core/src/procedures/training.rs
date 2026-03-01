//! Training data processing procedures for PluresDB.
//!
//! These procedures are designed for use with the `memory` node schema from
//! pluresLM (extended schema).  They cover two execution modes:
//!
//! ## Event-driven (on-insert)
//!
//! | Procedure                             | Description                                              |
//! |---------------------------------------|----------------------------------------------------------|
//! | [`on_memory_insert_enrich`]           | Parse text and extract structured fields by category     |
//! | [`on_memory_insert_detect_contradictions`] | Vector search for contradictions / reinforcements   |
//! | [`on_memory_insert_attach_context`]   | Attach a sliding conversation window to a memory node    |
//!
//! ## Periodic (cron-triggered)
//!
//! | Procedure                 | Description                                              |
//! |---------------------------|----------------------------------------------------------|
//! | [`consolidate_training_pairs`] | Generate SFT / DPO training pairs from memories     |
//! | [`score_quality`]         | Recalculate per-node confidence scores                   |
//! | [`export_training_set`]   | Export filtered training data as newline-delimited JSON  |
//!
//! ## Memory node schema
//!
//! All procedures expect memory nodes that follow this layout (fields marked
//! with `*` are optional):
//!
//! ```json
//! {
//!   "_type":           "memory",
//!   "text":            "...",
//!   "category":        "fact | instruction | conversation | code | preference",
//!   "conversation_id": "...",    // *
//!   "session_id":      "...",    // *
//!   "source":          "...",    // *
//!   "word_count":      42,       // * written by on_memory_insert_enrich
//!   "keywords":        ["..."],  // * written by on_memory_insert_enrich
//!   "contradicts":     ["id"],   // * written by on_memory_insert_detect_contradictions
//!   "reinforces":      ["id"],   // * written by on_memory_insert_detect_contradictions
//!   "context_window":  ["id"],   // * written by on_memory_insert_attach_context
//!   "quality_score":   0.0–1.0,  // * written by score_quality
//!   "confidence":      0.0–1.0   // * written by score_quality (alias for quality_score)
//! }
//! ```
//!
//! All procedures are **idempotent**: re-running them on the same data
//! produces the same result (existing derived fields are overwritten rather
//! than appended).

use uuid::Uuid;

use crate::CrdtStore;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Minimum cosine-similarity score required for a candidate node to be
/// considered a contradiction / reinforcement.
const CONTRADICTION_MIN_SCORE: f32 = 0.75;

/// Maximum number of candidates examined when detecting contradictions.
const CONTRADICTION_SEARCH_LIMIT: usize = 10;

/// Number of recent memories that form the conversation context window.
const CONTEXT_WINDOW_SIZE: usize = 5;

/// Minimum quality score a memory must have to be included in the exported
/// training set (when `min_quality` is not supplied by the caller).
const DEFAULT_MIN_QUALITY: f64 = 0.5;

// ---------------------------------------------------------------------------
// on_memory_insert_enrich
// ---------------------------------------------------------------------------

/// Enrich a memory node immediately after insertion by parsing its text and
/// extracting structured metadata fields.
///
/// The following fields are computed and merged back into the node:
///
/// | Field          | Description                                          |
/// |----------------|------------------------------------------------------|
/// | `word_count`   | Number of whitespace-separated tokens                |
/// | `char_count`   | Total character count (excluding leading/trailing ws)|
/// | `keywords`     | Up to 10 high-frequency non-stopword tokens          |
/// | `language_hint`| Detected script family (`"latin"` or `"other"`)      |
/// | `category`     | Preserved from the original node; defaults to `"general"` when absent |
///
/// # Arguments
///
/// * `store`     — the CRDT store to read from and write into.
/// * `actor`     — logical actor / author identifier (used for CRDT clocks).
/// * `memory_id` — ID of the memory node to enrich (must exist).
///
/// # Returns
///
/// A JSON object containing all extracted metadata fields.
///
/// # Errors
///
/// Returns an error when `memory_id` is empty, when the node is not found,
/// when the node's `_type` is not `"memory"`, or when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::training::on_memory_insert_enrich};
///
/// let store = CrdtStore::default();
/// store.put("mem1", "actor", serde_json::json!({
///     "_type": "memory",
///     "text": "Rust is a systems programming language.",
///     "category": "fact",
/// }));
/// let meta = on_memory_insert_enrich(&store, "actor", "mem1").unwrap();
/// assert!(meta["word_count"].as_u64().unwrap() > 0);
/// ```
pub fn on_memory_insert_enrich(
    store: &CrdtStore,
    actor: &str,
    memory_id: &str,
) -> anyhow::Result<serde_json::Value> {
    validate_common(actor, memory_id, "on_memory_insert_enrich")?;

    let node = store.get(memory_id).ok_or_else(|| {
        anyhow::anyhow!("on_memory_insert_enrich: memory '{}' not found", memory_id)
    })?;
    ensure_memory_type(&node.data, memory_id, "on_memory_insert_enrich")?;

    let text = node.data.get("text").and_then(|v| v.as_str()).unwrap_or("").trim();

    let word_count = text.split_whitespace().count();
    let char_count = text.chars().count();
    let keywords = extract_keywords(text, 10);
    let language_hint = detect_language_hint(text);

    // Normalize the category to a known set; default to "general".
    let raw_category = node
        .data
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let category = normalize_category(raw_category);

    let enriched = serde_json::json!({
        "word_count":     word_count,
        "char_count":     char_count,
        "keywords":       keywords,
        "language_hint":  language_hint,
        "category":       category,
    });

    // Merge enrichment fields back into the stored node.
    let mut updated = node.data.clone();
    if let (Some(obj), Some(extra)) = (updated.as_object_mut(), enriched.as_object()) {
        for (k, v) in extra {
            obj.insert(k.clone(), v.clone());
        }
    }
    store.put(memory_id, actor, updated);

    Ok(enriched)
}

// ---------------------------------------------------------------------------
// on_memory_insert_detect_contradictions
// ---------------------------------------------------------------------------

/// Scan similar memories for potential contradictions or reinforcements and
/// annotate the node accordingly.
///
/// The procedure uses the node's pre-computed embedding (if available) to
/// perform an approximate nearest-neighbour search.  For each candidate whose
/// similarity exceeds [`CONTRADICTION_MIN_SCORE`]:
///
/// * If the candidate's text starts with a **negation pattern** (e.g. "not",
///   "never", "don't"), or the current node's text starts with one while the
///   candidate's does not, the candidate is added to `contradicts`.
/// * Otherwise, the candidate is added to `reinforces`.
///
/// The fields `contradicts` and `reinforces` on the target node are
/// **replaced** (not appended) so this procedure is idempotent.
///
/// If no embedding is present on the node the procedure writes empty lists and
/// returns successfully without performing a search.
///
/// # Arguments
///
/// * `store`     — the CRDT store to read from and write into.
/// * `actor`     — logical actor / author identifier.
/// * `memory_id` — ID of the memory node to process (must exist).
///
/// # Returns
///
/// A JSON object with two arrays: `"contradicts"` and `"reinforces"`, each
/// containing IDs of related memory nodes.
///
/// # Errors
///
/// Returns an error when `memory_id` is empty, when the node is not found,
/// when the node's `_type` is not `"memory"`, or when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::training::on_memory_insert_detect_contradictions};
///
/// let store = CrdtStore::default();
/// store.put("mem1", "actor", serde_json::json!({
///     "_type": "memory",
///     "text": "The sky is blue.",
/// }));
/// let result = on_memory_insert_detect_contradictions(&store, "actor", "mem1").unwrap();
/// assert!(result["contradicts"].is_array());
/// assert!(result["reinforces"].is_array());
/// ```
pub fn on_memory_insert_detect_contradictions(
    store: &CrdtStore,
    actor: &str,
    memory_id: &str,
) -> anyhow::Result<serde_json::Value> {
    validate_common(actor, memory_id, "on_memory_insert_detect_contradictions")?;

    let node = store.get(memory_id).ok_or_else(|| {
        anyhow::anyhow!(
            "on_memory_insert_detect_contradictions: memory '{}' not found",
            memory_id
        )
    })?;
    ensure_memory_type(&node.data, memory_id, "on_memory_insert_detect_contradictions")?;

    let mut contradicts: Vec<String> = Vec::new();
    let mut reinforces: Vec<String> = Vec::new();

    if let Some(embedding) = &node.embedding {
        let candidates =
            store.vector_search(embedding, CONTRADICTION_SEARCH_LIMIT, CONTRADICTION_MIN_SCORE);

        let self_text = node.data.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let self_negated = is_negated(self_text);

        for result in candidates {
            // Skip the node itself.
            if result.record.id == memory_id {
                continue;
            }
            // Only compare against other memory nodes.
            if result.record.data.get("_type").and_then(|v| v.as_str()) != Some("memory") {
                continue;
            }

            let other_text =
                result.record.data.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let other_negated = is_negated(other_text);

            if self_negated != other_negated {
                contradicts.push(result.record.id);
            } else {
                reinforces.push(result.record.id);
            }
        }
    }

    let annotation = serde_json::json!({
        "contradicts": contradicts,
        "reinforces":  reinforces,
    });

    // Write annotation back to the node (idempotent — replaces prior values).
    let mut updated = node.data.clone();
    if let Some(obj) = updated.as_object_mut() {
        obj.insert("contradicts".to_owned(), serde_json::json!(contradicts));
        obj.insert("reinforces".to_owned(), serde_json::json!(reinforces));
    }
    store.put(memory_id, actor, updated);

    Ok(annotation)
}

// ---------------------------------------------------------------------------
// on_memory_insert_attach_context
// ---------------------------------------------------------------------------

/// Attach a sliding conversation context window to a newly inserted memory.
///
/// The procedure finds up to [`CONTEXT_WINDOW_SIZE`] other memories that share
/// the same `conversation_id` (or `session_id`) as the target node, ordered
/// by their stored timestamp (most-recent first), and writes their IDs into
/// the `context_window` field of the target node.
///
/// If the target node has neither a `conversation_id` nor a `session_id`, the
/// procedure writes an empty `context_window` and returns successfully.
///
/// The `context_window` field is **replaced** on each call so the procedure is
/// idempotent.
///
/// # Arguments
///
/// * `store`     — the CRDT store to read from and write into.
/// * `actor`     — logical actor / author identifier.
/// * `memory_id` — ID of the memory node to process (must exist).
///
/// # Returns
///
/// A JSON object with a `"context_window"` array of memory IDs (excluding the
/// target node itself).
///
/// # Errors
///
/// Returns an error when `memory_id` is empty, when the node is not found,
/// when the node's `_type` is not `"memory"`, or when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::training::on_memory_insert_attach_context};
///
/// let store = CrdtStore::default();
/// store.put("mem1", "actor", serde_json::json!({
///     "_type": "memory",
///     "text": "Hello",
///     "conversation_id": "conv-1",
/// }));
/// let result = on_memory_insert_attach_context(&store, "actor", "mem1").unwrap();
/// assert!(result["context_window"].is_array());
/// ```
pub fn on_memory_insert_attach_context(
    store: &CrdtStore,
    actor: &str,
    memory_id: &str,
) -> anyhow::Result<serde_json::Value> {
    validate_common(actor, memory_id, "on_memory_insert_attach_context")?;

    let node = store.get(memory_id).ok_or_else(|| {
        anyhow::anyhow!(
            "on_memory_insert_attach_context: memory '{}' not found",
            memory_id
        )
    })?;
    ensure_memory_type(&node.data, memory_id, "on_memory_insert_attach_context")?;

    // Determine the conversation key: prefer conversation_id, then session_id.
    let conv_key = node
        .data
        .get("conversation_id")
        .or_else(|| node.data.get("session_id"))
        .and_then(|v| v.as_str())
        .map(str::to_owned);

    let context_window: Vec<String> = if let Some(key) = &conv_key {
        // Collect up to CONTEXT_WINDOW_SIZE most recent memories sharing the same
        // conversation key, without sorting all siblings.
        let mut top: Vec<crate::NodeRecord> = Vec::new();

        for n in store.list().into_iter() {
            // Skip the newly inserted memory itself.
            if n.id == memory_id {
                continue;
            }
            // Only consider memory nodes.
            if n.data.get("_type").and_then(|v| v.as_str()) != Some("memory") {
                continue;
            }
            // Require the same conversation/session key.
            let n_conv = n
                .data
                .get("conversation_id")
                .or_else(|| n.data.get("session_id"))
                .and_then(|v| v.as_str());
            if n_conv != Some(key.as_str()) {
                continue;
            }

            // Insert into `top` keeping it sorted by timestamp descending
            // (most recent first), and cap length at CONTEXT_WINDOW_SIZE.
            let insert_pos = top
                .binary_search_by(|existing| existing.timestamp.cmp(&n.timestamp).reverse())
                .unwrap_or_else(|pos| pos);
            top.insert(insert_pos, n);
            if top.len() > CONTEXT_WINDOW_SIZE {
                top.pop();
            }
        }

        top.into_iter().map(|n| n.id).collect()
    } else {
        Vec::new()
    };

    let result = serde_json::json!({ "context_window": context_window });

    // Write back — idempotent.
    let mut updated = node.data.clone();
    if let Some(obj) = updated.as_object_mut() {
        obj.insert("context_window".to_owned(), serde_json::json!(context_window));
    }
    store.put(memory_id, actor, updated);

    Ok(result)
}

// ---------------------------------------------------------------------------
// consolidate_training_pairs
// ---------------------------------------------------------------------------

/// Output produced by [`consolidate_training_pairs`].
#[derive(Debug, Clone, PartialEq)]
pub struct TrainingPair {
    /// Unique identifier for this training pair.
    pub id: String,
    /// SFT / DPO format: `"sft"` or `"dpo"`.
    pub format: String,
    /// Prompt text.
    pub prompt: String,
    /// Chosen / accepted response.
    pub chosen: String,
    /// Rejected response (DPO only; empty for SFT pairs).
    pub rejected: String,
    /// Source memory node ID for the prompt.
    pub prompt_memory_id: String,
    /// Source memory node ID for the chosen response.
    pub chosen_memory_id: String,
}

/// Generate SFT and DPO training pairs from the memories in the store.
///
/// The algorithm works as follows:
///
/// 1. Collect all nodes with `_type == "memory"`.
/// 2. For each memory that has a non-empty `reinforces` list, emit one **SFT**
///    pair where the memory's `text` is the chosen response and the first
///    reinforcing memory's `text` is the prompt.
/// 3. For each memory that has both a `reinforces` entry and a `contradicts`
///    entry, emit one **DPO** pair where the reinforcing memory is `chosen` and
///    the contradicting memory is `rejected`.
///
/// Pairs are written to the store as nodes with `_type == "training_pair"` so
/// that they can be exported later.  Re-running the procedure is **idempotent**:
/// pair IDs are derived deterministically from the constituent memory IDs, so
/// duplicate pairs are overwritten rather than duplicated.
///
/// # Arguments
///
/// * `store` — the CRDT store to read from and write into.
/// * `actor` — logical actor / author identifier.
///
/// # Returns
///
/// A `Vec` of all generated [`TrainingPair`]s (including those that already
/// existed in the store and were overwritten).
///
/// # Errors
///
/// Returns an error when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::training::consolidate_training_pairs};
///
/// let store = CrdtStore::default();
/// let pairs = consolidate_training_pairs(&store, "actor").unwrap();
/// assert!(pairs.is_empty()); // no memories → no pairs
/// ```
pub fn consolidate_training_pairs(
    store: &CrdtStore,
    actor: &str,
) -> anyhow::Result<Vec<TrainingPair>> {
    anyhow::ensure!(!actor.is_empty(), "consolidate_training_pairs: actor must not be empty");

    let memories: Vec<_> = store
        .list()
        .into_iter()
        .filter(|n| n.data.get("_type").and_then(|v| v.as_str()) == Some("memory"))
        .collect();

    let mut pairs: Vec<TrainingPair> = Vec::new();

    for mem in &memories {
        let reinforces: Vec<String> = mem
            .data
            .get("reinforces")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();

        let contradicts: Vec<String> = mem
            .data
            .get("contradicts")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();

        let mem_text = mem.data.get("text").and_then(|v| v.as_str()).unwrap_or("");

        // SFT pair: prompt = first reinforcing memory, chosen = this memory.
        if let Some(reinforce_id) = reinforces.first() {
            if let Some(reinforce_node) = store.get(reinforce_id) {
                let prompt_text =
                    reinforce_node.data.get("text").and_then(|v| v.as_str()).unwrap_or("");

                if !prompt_text.is_empty() && !mem_text.is_empty() {
                    let pair_id =
                        deterministic_pair_id(&["sft", reinforce_id, mem.id.as_str()]);
                    let pair = TrainingPair {
                        id: pair_id.clone(),
                        format: "sft".to_owned(),
                        prompt: prompt_text.to_owned(),
                        chosen: mem_text.to_owned(),
                        rejected: String::new(),
                        prompt_memory_id: reinforce_id.clone(),
                        chosen_memory_id: mem.id.clone(),
                    };
                    store.put(
                        pair_id,
                        actor,
                        serde_json::json!({
                            "_type":             "training_pair",
                            "format":            "sft",
                            "prompt":            pair.prompt,
                            "chosen":            pair.chosen,
                            "rejected":          "",
                            "prompt_memory_id":  pair.prompt_memory_id,
                            "chosen_memory_id":  pair.chosen_memory_id,
                        }),
                    );
                    pairs.push(pair);
                }
            }
        }

        // DPO pair: chosen = reinforcing memory, rejected = contradicting memory.
        if let (Some(reinforce_id), Some(contradict_id)) =
            (reinforces.first(), contradicts.first())
        {
            if let (Some(reinforce_node), Some(contradict_node)) =
                (store.get(reinforce_id), store.get(contradict_id))
            {
                let chosen_text =
                    reinforce_node.data.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let rejected_text =
                    contradict_node.data.get("text").and_then(|v| v.as_str()).unwrap_or("");

                if !chosen_text.is_empty() && !rejected_text.is_empty() {
                    let pair_id = deterministic_pair_id(&[
                        "dpo",
                        mem.id.as_str(),
                        reinforce_id.as_str(),
                        contradict_id.as_str(),
                    ]);
                    let pair = TrainingPair {
                        id: pair_id.clone(),
                        format: "dpo".to_owned(),
                        prompt: mem_text.to_owned(),
                        chosen: chosen_text.to_owned(),
                        rejected: rejected_text.to_owned(),
                        prompt_memory_id: mem.id.clone(),
                        chosen_memory_id: reinforce_id.clone(),
                    };
                    store.put(
                        pair_id,
                        actor,
                        serde_json::json!({
                            "chosen_memory_id":  pair.chosen_memory_id,
                            "rejected_memory_id": contradict_id,
                            "format":            "dpo",
                            "prompt":            pair.prompt,
                            "chosen":            pair.chosen,
                            "rejected":          pair.rejected,
                            "prompt_memory_id":  pair.prompt_memory_id,
                            "chosen_memory_id":  pair.chosen_memory_id,
                        }),
                    );
                    pairs.push(pair);
                }
            }
        }
    }

    Ok(pairs)
}

// ---------------------------------------------------------------------------
// score_quality
// ---------------------------------------------------------------------------

/// Recalculate quality / confidence scores for all memory nodes in the store.
///
/// The score is a heuristic composite of:
///
/// | Component                        | Weight | Rationale                             |
/// |----------------------------------|--------|---------------------------------------|
/// | `word_count` ≥ 5 → bonus +0.2   | 0–0.2  | Longer memories tend to be richer     |
/// | Has `keywords` → bonus +0.2     | 0–0.2  | Structured enrichment was run         |
/// | Has `reinforces` entries → +0.3 | 0–0.3  | Corroborated by similar memories      |
/// | Has no `contradicts` → +0.3     | 0–0.3  | Not contradicted by other memories    |
///
/// The resulting score is clamped to `[0.0, 1.0]` and written back as both
/// `quality_score` and `confidence` on each memory node (`confidence` is the
/// field name used by the pluresLM extended schema; `quality_score` is the
/// canonical internal name).
///
/// This procedure is idempotent: re-running it updates the score to reflect
/// the current state of the store.
///
/// # Arguments
///
/// * `store` — the CRDT store to read from and write into.
/// * `actor` — logical actor / author identifier.
///
/// # Returns
///
/// A JSON object mapping each updated `memory_id` to its new `quality_score`.
///
/// # Errors
///
/// Returns an error when `actor` is empty.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::training::score_quality};
///
/// let store = CrdtStore::default();
/// store.put("mem1", "actor", serde_json::json!({
///     "_type": "memory",
///     "text": "Rust is fast and safe",
///     "word_count": 5,
/// }));
/// let scores = score_quality(&store, "actor").unwrap();
/// assert!(scores["mem1"].as_f64().unwrap() > 0.0);
/// ```
pub fn score_quality(
    store: &CrdtStore,
    actor: &str,
) -> anyhow::Result<serde_json::Value> {
    anyhow::ensure!(!actor.is_empty(), "score_quality: actor must not be empty");

    let memories: Vec<_> = store
        .list()
        .into_iter()
        .filter(|n| n.data.get("_type").and_then(|v| v.as_str()) == Some("memory"))
        .collect();

    let mut scores = serde_json::Map::new();

    for mem in memories {
        let mut score: f64 = 0.0;

        // Component 1: sufficient word count.
        let word_count = mem.data.get("word_count").and_then(|v| v.as_u64()).unwrap_or(0);
        if word_count >= 5 {
            score += 0.2;
        }

        // Component 2: keywords were extracted.
        let has_keywords = mem
            .data
            .get("keywords")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        if has_keywords {
            score += 0.2;
        }

        // Component 3: corroborated by other memories.
        let has_reinforces = mem
            .data
            .get("reinforces")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        if has_reinforces {
            score += 0.3;
        }

        // Component 4: not contradicted by other memories.
        let contradicts_empty = mem
            .data
            .get("contradicts")
            .and_then(|v| v.as_array())
            .map(|a| a.is_empty())
            .unwrap_or(true);
        if contradicts_empty {
            score += 0.3;
        }

        let clamped = score.clamp(0.0, 1.0);
        scores.insert(mem.id.clone(), serde_json::json!(clamped));

        // Write back both quality_score (internal) and confidence (pluresLM schema) to the node.
        let mut updated = mem.data.clone();
        if let Some(obj) = updated.as_object_mut() {
            obj.insert("quality_score".to_owned(), serde_json::json!(clamped));
            obj.insert("confidence".to_owned(), serde_json::json!(clamped));
        }
        store.put(mem.id.clone(), actor, updated);
    }

    Ok(serde_json::Value::Object(scores))
}

// ---------------------------------------------------------------------------
// export_training_set
// ---------------------------------------------------------------------------

/// Export filtered training pairs as newline-delimited JSON (JSONL).
///
/// The procedure collects all nodes with `_type == "training_pair"` from the
/// store, optionally filters them by:
///
/// * `format_filter` — only include pairs whose `format` field matches
///   (e.g. `"sft"` or `"dpo"`).  Pass `None` to include all formats.
/// * `min_quality`   — only include pairs whose associated prompt memory has a
///   `quality_score` ≥ this value.  Pass `None` to use
///   [`DEFAULT_MIN_QUALITY`].
///
/// Each included pair is serialised to a single JSON line in the format:
///
/// ```json
/// {"id":"...","format":"sft","prompt":"...","chosen":"...","rejected":""}
/// ```
///
/// # Arguments
///
/// * `store`         — the CRDT store to read from.
/// * `format_filter` — optional format selector (`"sft"` / `"dpo"` / `None`).
/// * `min_quality`   — minimum quality score for the prompt memory
///   (`None` uses [`DEFAULT_MIN_QUALITY`]).  Values outside `[0.0, 1.0]`
///   are silently clamped to that range.
///
/// # Returns
///
/// A `String` of newline-separated JSON objects (may be empty if no pairs
/// satisfy the filters).
///
/// # Errors
///
/// Returns an error if any pair node cannot be serialised.
///
/// # Examples
///
/// ```rust
/// use pluresdb_core::{CrdtStore, procedures::training::export_training_set};
///
/// let store = CrdtStore::default();
/// // No training pairs in store → empty export.
/// let jsonl = export_training_set(&store, None, None).unwrap();
/// assert!(jsonl.is_empty());
/// ```
pub fn export_training_set(
    store: &CrdtStore,
    format_filter: Option<&str>,
    min_quality: Option<f64>,
) -> anyhow::Result<String> {
    let min_q = min_quality.unwrap_or(DEFAULT_MIN_QUALITY).clamp(0.0, 1.0);

    let pairs: Vec<_> = store
        .list()
        .into_iter()
        .filter(|n| n.data.get("_type").and_then(|v| v.as_str()) == Some("training_pair"))
        .collect();

    let mut lines: Vec<String> = Vec::new();

    for pair_node in pairs {
        // Optional format filter.
        if let Some(fmt) = format_filter {
            let node_fmt = pair_node.data.get("format").and_then(|v| v.as_str()).unwrap_or("");
            if node_fmt != fmt {
                continue;

        // If a quality gate is in effect, treat missing / unresolvable prompt memories
        // as failing the gate, to match the documented behavior.
        if min_q > 0.0 {
            if prompt_memory_id.is_empty() {
                // No prompt memory associated: fail the quality gate.
                continue;
            }

            match store.get(prompt_memory_id) {
                Some(prompt_mem) => {
                    let quality = prompt_mem
                        .data
                        .get("quality_score")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    if quality < min_q {
                        continue;
                    }
                }
                None => {
                    // Unresolvable prompt memory: fail the quality gate.
            .unwrap_or("");
        if !prompt_memory_id.is_empty() {
            if let Some(prompt_mem) = store.get(prompt_memory_id) {
                let quality = prompt_mem
                    .data
                    .get("quality_score")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                if quality < min_q {
                    continue;
                }
            }
        }

        // Build the export record (exclude internal fields).
        let record = serde_json::json!({
            "id":      pair_node.id,
            "format":  pair_node.data.get("format").cloned().unwrap_or(serde_json::Value::Null),
            "prompt":  pair_node.data.get("prompt").cloned().unwrap_or(serde_json::Value::Null),
            "chosen":  pair_node.data.get("chosen").cloned().unwrap_or(serde_json::Value::Null),
            "rejected":pair_node.data.get("rejected").cloned().unwrap_or(serde_json::Value::Null),
        });

        lines.push(serde_json::to_string(&record)?);
    }

    Ok(lines.join("\n"))
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Normalize a raw category string to one of the known valid values for the
/// pluresLM schema: `"fact"`, `"instruction"`, `"conversation"`, `"code"`, or
/// `"preference"`.  Matching is case-insensitive and strips surrounding
/// whitespace.  Unrecognised values map to `"general"`.
fn normalize_category(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "fact" => "fact".to_owned(),
        "instruction" => "instruction".to_owned(),
        "conversation" => "conversation".to_owned(),
        "code" => "code".to_owned(),
        "preference" => "preference".to_owned(),
        _ => "general".to_owned(),
    }
}

/// Validate the common (`actor`, `memory_id`) arguments shared by event-driven
/// procedures.
fn validate_common(actor: &str, memory_id: &str, ctx: &str) -> anyhow::Result<()> {
    anyhow::ensure!(!actor.is_empty(), "{}: actor must not be empty", ctx);
    anyhow::ensure!(!memory_id.is_empty(), "{}: memory_id must not be empty", ctx);
    Ok(())
}

/// Assert that a node's `_type` field is `"memory"`.
fn ensure_memory_type(
    data: &serde_json::Value,
    id: &str,
    ctx: &str,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        data.get("_type").and_then(|v| v.as_str()) == Some("memory"),
        "{}: node '{}' is not a memory",
        ctx,
        id
    );
    Ok(())
}

/// Returns `true` if `text` begins with a common English negation word,
/// suggesting that it expresses a negative / contrary claim.
fn is_negated(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    NEGATION_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

/// Common negation prefixes used to detect contradictions.
const NEGATION_PREFIXES: &[&str] = &[
    "not ", "never ", "no ", "don't ", "doesn't ", "didn't ", "won't ", "can't ", "cannot ",
    "isn't ", "aren't ", "wasn't ", "weren't ",
];

/// Simple heuristic: classify text as `"latin"` if the majority of alphabetic
/// characters are ASCII letters, `"other"` for predominantly non-Latin scripts.
/// Returns `"latin"` for text without any alphabetic characters (e.g. digits /
/// punctuation only) because such text is typically code or numeric data that
/// originates from a Latin-script context.
fn detect_language_hint(text: &str) -> &'static str {
    let alpha: usize = text.chars().filter(|c| c.is_alphabetic()).count();
    if alpha == 0 {
        return "latin";
    }
    let ascii_alpha: usize = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if ascii_alpha * 2 >= alpha {
        "latin"
    } else {
        "other"
    }
}

/// Common English stop-words excluded from keyword extraction.
const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
    "from", "is", "it", "its", "this", "that", "be", "as", "are", "was", "were", "has", "have",
    "had", "not", "no", "so", "if", "do", "did", "he", "she", "we", "you", "they", "i", "me",
    "my", "your", "our", "their", "his", "her", "up", "out", "will", "can", "may", "all",
];

/// Returns `true` if `c` is a word-separator character for keyword extraction:
/// whitespace, or ASCII punctuation other than apostrophes and hyphens (so that
/// contractions and hyphenated words remain intact as single tokens).
#[inline]
fn is_word_separator(c: char) -> bool {
    c.is_whitespace() || (c.is_ascii_punctuation() && c != '\'' && c != '-')
}

/// Return up to `limit` most-frequent non-stop-word tokens from `text`.
/// The resulting UUID is order-sensitive: `pair_id(a, b)` is not guaranteed
/// to equal `pair_id(b, a)`. This allows callers to encode role- and
/// format-specific semantics in the argument order.
fn deterministic_pair_id(a: &str, b: &str) -> String {
    let combined = format!("{}:{}", a, b);
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

/// Derive a deterministic training-pair ID from an ordered list of components.
///
/// Components are joined with `:` and hashed using UUID-v5 with a
/// project-specific namespace.  The caller is responsible for providing
/// components in a consistent, role-significant order so that distinct pairs
/// always produce distinct IDs (e.g. pass `["sft", prompt_id, chosen_id]` for
/// SFT pairs and `["dpo", prompt_id, chosen_id, rejected_id]` for DPO pairs).
/// Unlike the previous two-argument signature this function does **not** sort
/// its inputs, so the order of arguments matters.
fn deterministic_pair_id(components: &[&str]) -> String {
    let combined = components.join(":");
    // A randomly chosen namespace UUID specific to this use case.  Using a
    // project-specific namespace ensures pair IDs do not collide with UUIDs
    // generated by other parts of the system.
    const PAIR_NS: Uuid = Uuid::from_bytes([
        0x7c, 0x9e, 0x6e, 0x1a, 0x3f, 0x2b, 0x11, 0xee, 0x8f, 0x3d, 0x02, 0x42, 0xac, 0x13,
        0x00, 0x02,
    ]);
    Uuid::new_v5(&PAIR_NS, combined.as_bytes()).to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CrdtStore;

    fn make_memory(store: &CrdtStore, id: &str, text: &str, category: &str) {
        store.put(
            id.to_owned(),
            "actor",
            serde_json::json!({
                "_type":    "memory",
                "text":     text,
                "category": category,
            }),
        );
    }

    // -----------------------------------------------------------------------
    // on_memory_insert_enrich
    // -----------------------------------------------------------------------

    #[test]
    fn enrich_adds_word_and_char_count() {
        let store = CrdtStore::default();
        make_memory(&store, "m1", "Rust is a systems language", "fact");
        let meta = on_memory_insert_enrich(&store, "actor", "m1").unwrap();
        assert_eq!(meta["word_count"], 5u64);
        assert!(meta["char_count"].as_u64().unwrap() > 0);
    }

    #[test]
    fn enrich_adds_keywords() {
        let store = CrdtStore::default();
        make_memory(&store, "m1", "Rust programming language performance memory", "fact");
        let meta = on_memory_insert_enrich(&store, "actor", "m1").unwrap();
        assert!(meta["keywords"].is_array());
        let kw = meta["keywords"].as_array().unwrap();
        assert!(!kw.is_empty());
    }

    #[test]
    fn enrich_preserves_category() {
        let store = CrdtStore::default();
        make_memory(&store, "m1", "Some text here", "instruction");
        let meta = on_memory_insert_enrich(&store, "actor", "m1").unwrap();
        assert_eq!(meta["category"], "instruction");
    }

    #[test]
    fn enrich_defaults_category_when_missing() {
        let store = CrdtStore::default();
        store.put(
            "m1",
            "actor",
            serde_json::json!({"_type": "memory", "text": "some text here"}),
        );
        let meta = on_memory_insert_enrich(&store, "actor", "m1").unwrap();
        assert_eq!(meta["category"], "general");
    }

    #[test]
    fn enrich_normalizes_category_case() {
        let store = CrdtStore::default();
        store.put(
            "m1",
            "actor",
            serde_json::json!({"_type": "memory", "text": "some text here", "category": "FACT"}),
        );
        let meta = on_memory_insert_enrich(&store, "actor", "m1").unwrap();
        assert_eq!(meta["category"], "fact");
    }

    #[test]
    fn enrich_normalizes_unknown_category_to_general() {
        let store = CrdtStore::default();
        store.put(
            "m1",
            "actor",
            serde_json::json!({"_type": "memory", "text": "text", "category": "unknown_type"}),
        );
        let meta = on_memory_insert_enrich(&store, "actor", "m1").unwrap();
        assert_eq!(meta["category"], "general");
    }

    #[test]
    fn enrich_persists_to_node() {
        let store = CrdtStore::default();
        make_memory(&store, "m1", "one two three four five words", "fact");
        on_memory_insert_enrich(&store, "actor", "m1").unwrap();
        let node = store.get("m1").unwrap();
        assert_eq!(node.data["word_count"], 6u64);
    }

    #[test]
    fn enrich_rejects_empty_actor() {
        let store = CrdtStore::default();
        make_memory(&store, "m1", "text", "fact");
        let err = on_memory_insert_enrich(&store, "", "m1").unwrap_err();
        assert!(err.to_string().contains("actor"));
    }

    #[test]
    fn enrich_rejects_empty_memory_id() {
        let store = CrdtStore::default();
        let err = on_memory_insert_enrich(&store, "actor", "").unwrap_err();
        assert!(err.to_string().contains("memory_id"));
    }

    #[test]
    fn enrich_rejects_non_memory_node() {
        let store = CrdtStore::default();
        store.put("doc1", "actor", serde_json::json!({"_type": "document", "text": "hi"}));
        let err = on_memory_insert_enrich(&store, "actor", "doc1").unwrap_err();
        assert!(err.to_string().contains("not a memory"));
    }

    #[test]
    fn enrich_rejects_missing_node() {
        let store = CrdtStore::default();
        let err = on_memory_insert_enrich(&store, "actor", "no-such-id").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    // -----------------------------------------------------------------------
    // on_memory_insert_detect_contradictions
    // -----------------------------------------------------------------------

    #[test]
    fn detect_contradictions_returns_empty_when_no_embedding() {
        let store = CrdtStore::default();
        make_memory(&store, "m1", "The sky is blue", "fact");
        let result =
            on_memory_insert_detect_contradictions(&store, "actor", "m1").unwrap();
        assert_eq!(result["contradicts"].as_array().unwrap().len(), 0);
        assert_eq!(result["reinforces"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn detect_contradictions_writes_back_to_node() {
        let store = CrdtStore::default();
        make_memory(&store, "m1", "Rust is fast", "fact");
        on_memory_insert_detect_contradictions(&store, "actor", "m1").unwrap();
        let node = store.get("m1").unwrap();
        assert!(node.data["contradicts"].is_array());
        assert!(node.data["reinforces"].is_array());
    }

    #[test]
    fn detect_contradictions_rejects_non_memory_node() {
        let store = CrdtStore::default();
        store.put("doc1", "actor", serde_json::json!({"_type": "document", "text": "hi"}));
        let err =
            on_memory_insert_detect_contradictions(&store, "actor", "doc1").unwrap_err();
        assert!(err.to_string().contains("not a memory"));
    }

    // -----------------------------------------------------------------------
    // on_memory_insert_attach_context
    // -----------------------------------------------------------------------

    #[test]
    fn attach_context_empty_without_conversation_id() {
        let store = CrdtStore::default();
        make_memory(&store, "m1", "Hello", "conversation");
        let result = on_memory_insert_attach_context(&store, "actor", "m1").unwrap();
        assert_eq!(result["context_window"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn attach_context_finds_sibling_memories() {
        let store = CrdtStore::default();
        for i in 0..3usize {
            store.put(
                format!("m{}", i),
                "actor",
                serde_json::json!({
                    "_type":           "memory",
                    "text":            format!("message {}", i),
                    "conversation_id": "conv-1",
                }),
            );
        }
        let result = on_memory_insert_attach_context(&store, "actor", "m0").unwrap();
        let window = result["context_window"].as_array().unwrap();
        // m1 and m2 share the same conversation but are not m0.
        assert_eq!(window.len(), 2);
    }

    #[test]
    fn attach_context_uses_session_id_fallback() {
        let store = CrdtStore::default();
        store.put(
            "m1",
            "actor",
            serde_json::json!({"_type": "memory", "text": "a", "session_id": "s1"}),
        );
        store.put(
            "m2",
            "actor",
            serde_json::json!({"_type": "memory", "text": "b", "session_id": "s1"}),
        );
        let result = on_memory_insert_attach_context(&store, "actor", "m1").unwrap();
        let window = result["context_window"].as_array().unwrap();
        assert_eq!(window.len(), 1);
        assert_eq!(window[0], "m2");
    }

    #[test]
    fn attach_context_caps_at_window_size() {
        let store = CrdtStore::default();
        for i in 0..=CONTEXT_WINDOW_SIZE {
            store.put(
                format!("m{}", i),
                "actor",
                serde_json::json!({
                    "_type":           "memory",
                    "text":            format!("msg {}", i),
                    "conversation_id": "conv-big",
                }),
            );
        }
        // "m0" should see at most CONTEXT_WINDOW_SIZE others.
        let result = on_memory_insert_attach_context(&store, "actor", "m0").unwrap();
        let window = result["context_window"].as_array().unwrap();
        assert!(window.len() <= CONTEXT_WINDOW_SIZE);
    }

    #[test]
    fn attach_context_writes_back_to_node() {
        let store = CrdtStore::default();
        make_memory(&store, "m1", "hello", "conversation");
        on_memory_insert_attach_context(&store, "actor", "m1").unwrap();
        let node = store.get("m1").unwrap();
        assert!(node.data["context_window"].is_array());
    }

    // -----------------------------------------------------------------------
    // consolidate_training_pairs
    // -----------------------------------------------------------------------

    #[test]
    fn consolidate_empty_store_returns_no_pairs() {
        let store = CrdtStore::default();
        let pairs = consolidate_training_pairs(&store, "actor").unwrap();
        assert!(pairs.is_empty());
    }

    #[test]
    fn consolidate_generates_sft_pair() {
        let store = CrdtStore::default();
        // m1 reinforces m2.
        store.put(
            "m1",
            "actor",
            serde_json::json!({
                "_type":      "memory",
                "text":       "Rust is fast",
                "reinforces": [],
            }),
        );
        store.put(
            "m2",
            "actor",
            serde_json::json!({
                "_type":      "memory",
                "text":       "Rust has great performance",
                "reinforces": ["m1"],
            }),
        );

        let pairs = consolidate_training_pairs(&store, "actor").unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].format, "sft");
        assert_eq!(pairs[0].prompt, "Rust is fast");
        assert_eq!(pairs[0].chosen, "Rust has great performance");
        assert!(pairs[0].rejected.is_empty());
    }

    #[test]
    fn consolidate_generates_dpo_pair() {
        let store = CrdtStore::default();
        store.put("m1", "actor", serde_json::json!({"_type": "memory", "text": "Rust is fast"}));
        store.put(
            "m2",
            "actor",
            serde_json::json!({"_type": "memory", "text": "Not fast at all"}),
        );
        store.put(
            "m3",
            "actor",
            serde_json::json!({
                "_type":       "memory",
                "text":        "Performance matters",
                "reinforces":  ["m1"],
                "contradicts": ["m2"],
            }),
        );

        let pairs = consolidate_training_pairs(&store, "actor").unwrap();
        // Should include at least one DPO pair.
        let dpo: Vec<_> = pairs.iter().filter(|p| p.format == "dpo").collect();
        assert!(!dpo.is_empty());
        assert!(!dpo[0].rejected.is_empty());
    }

    #[test]
    fn consolidate_is_idempotent() {
        let store = CrdtStore::default();
        store.put(
            "m1",
            "actor",
            serde_json::json!({"_type": "memory", "text": "hello world is great"}),
        );
        store.put(
            "m2",
            "actor",
            serde_json::json!({"_type": "memory", "text": "world is great", "reinforces": ["m1"]}),
        );

        let pairs1 = consolidate_training_pairs(&store, "actor").unwrap();
        let pairs2 = consolidate_training_pairs(&store, "actor").unwrap();
        assert_eq!(pairs1.len(), pairs2.len());
        assert_eq!(pairs1[0].id, pairs2[0].id);
    }

    #[test]
    fn consolidate_rejects_empty_actor() {
        let store = CrdtStore::default();
        let err = consolidate_training_pairs(&store, "").unwrap_err();
        assert!(err.to_string().contains("actor"));
    }

    // -----------------------------------------------------------------------
    // score_quality
    // -----------------------------------------------------------------------

    #[test]
    fn score_quality_returns_scores_map() {
        let store = CrdtStore::default();
        store.put(
            "m1",
            "actor",
            serde_json::json!({"_type": "memory", "text": "Rust is fast and safe systems"}),
        );
        on_memory_insert_enrich(&store, "actor", "m1").unwrap();

        let scores = score_quality(&store, "actor").unwrap();
        assert!(scores["m1"].as_f64().is_some());
    }

    #[test]
    fn score_quality_base_score_no_contradictions() {
        let store = CrdtStore::default();
        store.put(
            "m1",
            "actor",
            serde_json::json!({"_type": "memory", "text": "short"}),
        );
        let scores = score_quality(&store, "actor").unwrap();
        // word_count < 5 → no word bonus; no keywords; no reinforces; no contradicts → 0.3
        let s = scores["m1"].as_f64().unwrap();
        assert!((s - 0.3).abs() < 1e-9);
    }

    #[test]
    fn score_quality_persists_to_node() {
        let store = CrdtStore::default();
        store.put("m1", "actor", serde_json::json!({"_type": "memory", "text": "hello world"}));
        score_quality(&store, "actor").unwrap();
        let node = store.get("m1").unwrap();
        assert!(node.data["quality_score"].as_f64().is_some());
        // Also writes the pluresLM `confidence` alias.
        assert_eq!(node.data["confidence"], node.data["quality_score"]);
    }

    #[test]
    fn score_quality_rejects_empty_actor() {
        let store = CrdtStore::default();
        let err = score_quality(&store, "").unwrap_err();
        assert!(err.to_string().contains("actor"));
    }

    // -----------------------------------------------------------------------
    // export_training_set
    // -----------------------------------------------------------------------

    #[test]
    fn export_empty_store_returns_empty_string() {
        let store = CrdtStore::default();
        let jsonl = export_training_set(&store, None, None).unwrap();
        assert!(jsonl.is_empty());
    }

    #[test]
    fn export_produces_valid_jsonl() {
        let store = CrdtStore::default();
        // Create a memory with a sufficient quality score and a training pair.
        store.put(
            "mem1",
            "actor",
            serde_json::json!({
                "_type":         "memory",
                "text":          "Rust is a systems programming language",
                "quality_score": 0.9,
            }),
        );
        store.put(
            "pair1",
            "actor",
            serde_json::json!({
                "_type":            "training_pair",
                "format":           "sft",
                "prompt":           "What is Rust?",
                "chosen":           "Rust is a systems programming language",
                "rejected":         "",
                "prompt_memory_id": "mem1",
                "chosen_memory_id": "mem1",
            }),
        );

        let jsonl = export_training_set(&store, None, Some(0.5)).unwrap();
        assert!(!jsonl.is_empty());
        // Each line must be valid JSON.
        for line in jsonl.lines() {
            let parsed: serde_json::Value = serde_json::from_str(line).expect("invalid JSON line");
            assert!(parsed["id"].is_string());
            assert!(parsed["format"].is_string());
        }
    }

    #[test]
    fn export_filters_by_format() {
        let store = CrdtStore::default();
        store.put(
            "mem1",
            "actor",
            serde_json::json!({"_type": "memory", "text": "x", "quality_score": 1.0}),
        );
        store.put(
            "pair_sft",
            "actor",
            serde_json::json!({
                "_type": "training_pair", "format": "sft",
                "prompt": "q", "chosen": "a", "rejected": "",
                "prompt_memory_id": "mem1", "chosen_memory_id": "mem1",
            }),
        );
        store.put(
            "pair_dpo",
            "actor",
            serde_json::json!({
                "_type": "training_pair", "format": "dpo",
                "prompt": "q", "chosen": "a", "rejected": "b",
                "prompt_memory_id": "mem1", "chosen_memory_id": "mem1",
            }),
        );

        let jsonl = export_training_set(&store, Some("sft"), Some(0.0)).unwrap();
        for line in jsonl.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(v["format"], "sft");
        }
    }

    #[test]
    fn export_respects_min_quality() {
        let store = CrdtStore::default();
        // Low-quality memory.
        store.put(
            "low_mem",
            "actor",
            serde_json::json!({"_type": "memory", "text": "x", "quality_score": 0.1}),
        );
        store.put(
            "pair1",
            "actor",
            serde_json::json!({
                "_type": "training_pair", "format": "sft",
                "prompt": "q", "chosen": "a", "rejected": "",
                "prompt_memory_id": "low_mem", "chosen_memory_id": "low_mem",
            }),
        );

        // With high min_quality the pair should be excluded.
        let jsonl = export_training_set(&store, None, Some(0.8)).unwrap();
        assert!(jsonl.is_empty());

        // With low min_quality the pair should be included.
        let jsonl = export_training_set(&store, None, Some(0.0)).unwrap();
        assert!(!jsonl.is_empty());
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    #[test]
    fn is_negated_detects_prefix() {
        assert!(is_negated("not correct"));
        assert!(is_negated("never true"));
        assert!(is_negated("No way"));
        assert!(!is_negated("the sky is blue"));
    }

    #[test]
    fn detect_language_hint_latin() {
        assert_eq!(detect_language_hint("Hello world"), "latin");
    }

    #[test]
    fn detect_language_hint_other() {
        // Cyrillic characters.
        assert_eq!(detect_language_hint("Привет мир"), "other");
    }

    #[test]
    fn deterministic_pair_id_is_stable() {
        let id1 = deterministic_pair_id(&["sft", "a", "b"]);
        let id2 = deterministic_pair_id(&["sft", "a", "b"]);
        assert_eq!(id1, id2);
    }

    #[test]
    fn deterministic_pair_id_includes_format() {
        let sft_id = deterministic_pair_id(&["sft", "a", "b"]);
        let dpo_id = deterministic_pair_id(&["dpo", "a", "b"]);
        assert_ne!(sft_id, dpo_id, "different formats must yield different IDs");
    }

    #[test]
    fn deterministic_pair_id_includes_all_roles() {
        // Same reinforce+contradict but different prompt → different DPO pair IDs.
        let id1 = deterministic_pair_id(&["dpo", "prompt-a", "chosen", "rejected"]);
        let id2 = deterministic_pair_id(&["dpo", "prompt-b", "chosen", "rejected"]);
        assert_ne!(id1, id2, "different prompt memories must yield different DPO pair IDs");
    }
}
