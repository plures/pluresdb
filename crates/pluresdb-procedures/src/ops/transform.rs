//! Transform operation — compress node data into compact representations.

use pluresdb_core::NodeRecord;

use crate::ir::TransformFormat;

/// Transform a set of nodes into a compressed representation.
///
/// Each node is converted to a single compact value based on `format`.
/// The result replaces the `data` field with the transformed content.
pub fn apply_transform(
    nodes: Vec<NodeRecord>,
    format: &TransformFormat,
    max_chars: usize,
) -> Vec<NodeRecord> {
    match format {
        TransformFormat::Structured => transform_structured(nodes, max_chars),
        TransformFormat::Fused => transform_fused(nodes, max_chars),
        TransformFormat::Toon => transform_toon(nodes, max_chars),
    }
}

/// Structured: dense JSON assertions — keep only category, text (truncated), and score.
fn truncate_text_utf8(text: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return text.to_string();
    }

    let mut end_byte = text.len();
    let mut char_count = 0usize;

    for (idx, _) in text.char_indices() {
        if char_count == max_chars {
            end_byte = idx;
            break;
        }
        char_count += 1;
    }

    // If the string has <= max_chars characters, return it unchanged.
    if char_count <= max_chars && end_byte == text.len() {
        text.to_string()
    } else {
        format!("{}…", &text[..end_byte])
    }
}

fn transform_structured(nodes: Vec<NodeRecord>, max_chars: usize) -> Vec<NodeRecord> {
    nodes
        .into_iter()
        .map(|mut node| {
            let category = node
                .data
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let text = extract_text(&node.data);
            let score = node
                .data
                .get("score")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let truncated = if max_chars > 0 {
                truncate_text_utf8(&text, max_chars)
            } else {
                text
            };

            node.data = serde_json::json!({
                "category": category,
                "text": truncated,
                "score": score,
            });
            node
        })
        .collect()
}

/// Fused: category-grouped text block. Returns a single synthetic node with
/// all memories grouped by category.
fn transform_fused(nodes: Vec<NodeRecord>, max_chars: usize) -> Vec<NodeRecord> {
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for node in &nodes {
        let category = node
            .data
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("other")
            .to_string();
        let text = extract_text(&node.data);
        let score = node
            .data
            .get("score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        groups
            .entry(category)
            .or_default()
            .push(format!("[{:.1}] {}", score, text));
    }

    let mut output = String::new();
    for (cat, entries) in &groups {
        output.push_str(&format!("## {}\n", cat));
        for entry in entries {
            let line = if max_chars > 0 && entry.len() > max_chars {
                format!("{}…\n", &entry[..max_chars])
            } else {
                format!("{}\n", entry)
            };
            output.push_str(&line);
        }
    }

    let fused_node = NodeRecord::new(
        "__fused__".to_string(),
        "__transform__",
        serde_json::json!({
            "category": "fused_context",
            "text": output.trim_end(),
            "count": nodes.len(),
        }),
    );
    vec![fused_node]
}

/// TOON: ultra-compact single-line notation.
///
/// Format: `[C|0.9] text content → metadata`
///
/// Category codes: D=decision, F=fact, R=rule, C=constraint, K=risk,
/// G=guidance, N=note, T=task, P=preference, ?=other
fn transform_toon(nodes: Vec<NodeRecord>, max_chars: usize) -> Vec<NodeRecord> {
    nodes
        .into_iter()
        .map(|mut node| {
            let category = node
                .data
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("other");
            let code = category_code(category);
            let score = node
                .data
                .get("score")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let text = extract_text(&node.data);

            let toon = if max_chars > 0 && text.len() > max_chars {
                format!("[{}|{:.1}] {}…", code, score, &text[..max_chars])
            } else {
                format!("[{}|{:.1}] {}", code, score, text)
            };

            node.data = serde_json::json!({
                "toon": toon,
            });
            node
        })
        .collect()
}

fn category_code(category: &str) -> char {
    match category {
        "decision" => 'D',
        "fact" | "facts" => 'F',
        "rule" | "rules" => 'R',
        "constraint" | "constraints" => 'C',
        "risk" | "risks" => 'K',
        "guidance" => 'G',
        "note" => 'N',
        "task" => 'T',
        "preference" => 'P',
        "entity" => 'E',
        "conversation" => 'V',
        "insight" => 'I',
        _ => '?',
    }
}

/// Extract readable text from a node's data, checking common field names.
fn extract_text(data: &serde_json::Value) -> String {
    for field in &["text", "content", "description", "summary"] {
        if let Some(s) = data.get(field).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    // Fallback: serialize the data compactly
    serde_json::to_string(data).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_nodes() -> Vec<NodeRecord> {
        vec![
            NodeRecord::new(
                "n1".into(),
                "test",
                serde_json::json!({"category": "decision", "text": "Use Rust for performance", "score": 0.9}),
            ),
            NodeRecord::new(
                "n2".into(),
                "test",
                serde_json::json!({"category": "fact", "text": "PluresDB uses CRDTs", "score": 0.8}),
            ),
            NodeRecord::new(
                "n3".into(),
                "test",
                serde_json::json!({"category": "risk", "text": "Memory pressure under load", "score": 0.6}),
            ),
        ]
    }

    #[test]
    fn structured_preserves_category_and_score() {
        let result = apply_transform(sample_nodes(), &TransformFormat::Structured, 0);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].data["category"], "decision");
        assert_eq!(result[0].data["score"], 0.9);
    }

    #[test]
    fn structured_truncates_text() {
        let result = apply_transform(sample_nodes(), &TransformFormat::Structured, 10);
        let text = result[0].data["text"].as_str().unwrap();
        assert!(text.len() <= 12); // 10 + "…"
    }

    #[test]
    fn fused_produces_single_node() {
        let result = apply_transform(sample_nodes(), &TransformFormat::Fused, 0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].data["count"], 3);
        let text = result[0].data["text"].as_str().unwrap();
        assert!(text.contains("## decision"));
        assert!(text.contains("## fact"));
    }

    #[test]
    fn toon_produces_compact_codes() {
        let result = apply_transform(sample_nodes(), &TransformFormat::Toon, 0);
        assert_eq!(result.len(), 3);
        let toon = result[0].data["toon"].as_str().unwrap();
        assert!(toon.starts_with("[D|0.9]"));
        let toon2 = result[2].data["toon"].as_str().unwrap();
        assert!(toon2.starts_with("[K|0.6]"));
    }

    #[test]
    fn toon_truncates() {
        let result = apply_transform(sample_nodes(), &TransformFormat::Toon, 5);
        let toon = result[0].data["toon"].as_str().unwrap();
        assert!(toon.contains("…"));
    }
}
