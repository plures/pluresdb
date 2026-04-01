//! Project operation — extract, alias, and reshape fields from each node.

use pluresdb_core::NodeRecord;

use crate::ir::FieldSpec;
use crate::ops::filter::get_nested;

/// Reshape each node so that only the fields in `specs` are present in
/// its `data` payload.  Aliased specs rename the output key.
pub fn apply_project(nodes: Vec<NodeRecord>, specs: &[FieldSpec]) -> Vec<NodeRecord> {
    nodes
        .into_iter()
        .map(|mut node| {
            let projected = project_data(&node.data, specs);
            node.data = projected;
            node
        })
        .collect()
}

/// Extract the specified fields from a single JSON document.
///
/// When two specs produce the same output key (e.g. `["data.score", "user.score"]`
/// both map to `"score"`), the **last** writer wins — earlier values are
/// silently overwritten.  Callers that need to avoid collisions should ensure
/// all output names are unique, or use explicit aliases.
pub fn project_data(data: &serde_json::Value, specs: &[FieldSpec]) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for spec in specs {
        let path = spec.path();
        let key = spec.output_name();

        if let Some(val) = get_nested(data, path) {
            out.insert(key.to_string(), val.clone());
        }
    }
    serde_json::Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::FieldSpec;
    use pluresdb_core::CrdtStore;

    fn make_node(id: &str, data: serde_json::Value) -> NodeRecord {
        let store = CrdtStore::default();
        store.put(id.to_string(), "test", data);
        store.get(id).unwrap()
    }

    #[test]
    fn project_plain_fields() {
        let node = make_node(
            "a",
            serde_json::json!({ "id": "a", "category": "note", "extra": 42 }),
        );
        let specs = vec![
            FieldSpec::Plain("id".to_string()),
            FieldSpec::Plain("category".to_string()),
        ];
        let result = apply_project(vec![node], &specs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].data["id"], "a");
        assert_eq!(result[0].data["category"], "note");
        assert!(result[0].data.get("extra").is_none());
    }

    #[test]
    fn project_nested_field() {
        let node = make_node(
            "a",
            serde_json::json!({ "data": { "text": "hello", "score": 0.9 } }),
        );
        let specs = vec![FieldSpec::Plain("data.text".to_string())];
        let result = apply_project(vec![node], &specs);
        assert_eq!(result[0].data["text"], "hello");
    }

    #[test]
    fn project_aliased_field() {
        let node = make_node("a", serde_json::json!({ "data": { "score": 0.9 } }));
        let specs = vec![FieldSpec::Aliased {
            path: "data.score".to_string(),
            alias: "score".to_string(),
        }];
        let result = apply_project(vec![node], &specs);
        assert_eq!(result[0].data["score"], 0.9);
    }

    #[test]
    fn project_missing_field_omitted() {
        let node = make_node("a", serde_json::json!({ "name": "Alice" }));
        let specs = vec![FieldSpec::Plain("missing".to_string())];
        let result = apply_project(vec![node], &specs);
        assert!(result[0].data.as_object().unwrap().is_empty());
    }
}
