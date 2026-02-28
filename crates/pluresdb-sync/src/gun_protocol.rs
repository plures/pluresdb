//! Minimal GUN-compatible wire protocol for PluresDB P2P synchronization.
//!
//! This module implements the subset of the GUN.js wire protocol required for
//! Phase 1 interoperability.  The supported message types are:
//!
//! | Type | Discriminant key | Purpose                        |
//! |------|-----------------|--------------------------------|
//! | PUT  | `"put"`          | Insert / merge graph node data |
//! | GET  | `"get"`          | Request a node (or field)      |
//! | ACK  | `"@"`            | Acknowledge a PUT or GET       |
//!
//! See `docs/GUN_WIRE_PROTOCOL.md` for the full specification.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// A GUN node soul (unique identifier).
pub type Soul = String;

/// HAM (Hypothetical Amnesia Machine) state: field name → f64 timestamp
/// (milliseconds since Unix epoch).
///
/// Each field carries its own state value, enabling per-field last-write-wins
/// CRDT merges fully compatible with GUN.js.
pub type HamState = HashMap<String, f64>;

/// Metadata section of a GUN node (the `_` field in wire format).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunMeta {
    /// Soul of this node.
    #[serde(rename = "#")]
    pub soul: Soul,

    /// HAM state per field.
    #[serde(rename = ">")]
    pub state: HamState,
}

/// A GUN graph node with metadata and arbitrary data fields.
///
/// Wire format (abbreviated):
/// ```json
/// {
///   "_": {"#": "user:alice", ">": {"name": 1700000000000.0}},
///   "name": "Alice"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunNode {
    /// Metadata: soul + per-field HAM state.
    #[serde(rename = "_")]
    pub meta: GunMeta,

    /// Arbitrary data fields.
    #[serde(flatten)]
    pub fields: HashMap<String, JsonValue>,
}

impl GunNode {
    /// Create a [`GunNode`] from a soul and a flat JSON object.
    ///
    /// `state_ms` is the HAM timestamp assigned to every field (milliseconds
    /// since Unix epoch).  Use [`now_ms`] to obtain the current time.
    pub fn from_data(
        soul: impl Into<Soul>,
        data: HashMap<String, JsonValue>,
        state_ms: f64,
    ) -> Self {
        let soul = soul.into();
        let state: HamState = data.keys().map(|k| (k.clone(), state_ms)).collect();
        Self {
            meta: GunMeta {
                soul: soul.clone(),
                state,
            },
            fields: data,
        }
    }

    /// Merge `other` into `self` using per-field last-write-wins semantics.
    ///
    /// For each field in `other`:
    /// - If the HAM state timestamp of `other` is strictly greater, the field
    ///   (and its state) are taken from `other`.
    /// - On a tie the serialized values are compared lexicographically so that
    ///   the result is deterministic across all peers.
    pub fn merge(&mut self, other: GunNode) {
        for (field, other_value) in other.fields {
            let other_state = other.meta.state.get(&field).copied().unwrap_or(0.0);
            let self_state = self.meta.state.get(&field).copied().unwrap_or(0.0);
            let take_other = if other_state > self_state {
                true
            } else if (other_state - self_state).abs() < f64::EPSILON {
                // Tie-break: lexicographic comparison of JSON serializations.
                let other_s = other_value.to_string();
                let self_s = self
                    .fields
                    .get(&field)
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                other_s > self_s
            } else {
                false
            };
            if take_other {
                self.fields.insert(field.clone(), other_value);
                self.meta.state.insert(field, other_state);
            }
        }
    }
}

/// The request payload inside a [`GunGet`] message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunGetRequest {
    /// Soul (node ID) to request.
    #[serde(rename = "#")]
    pub soul: Soul,

    /// Optional field filter; when omitted the entire node is requested.
    #[serde(rename = ".", skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

/// PUT message: insert or merge one or more nodes.
///
/// ```json
/// { "#": "msg-id", "put": { "<soul>": { "_": {...}, "<field>": <value> } } }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunPut {
    /// Unique message identifier.
    #[serde(rename = "#")]
    pub id: String,

    /// Map of soul → node.
    pub put: HashMap<Soul, GunNode>,
}

/// GET message: request a node or a single field.
///
/// ```json
/// { "#": "msg-id", "get": { "#": "<soul>" } }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunGet {
    /// Unique message identifier.
    #[serde(rename = "#")]
    pub id: String,

    /// What to fetch.
    pub get: GunGetRequest,
}

/// ACK message: acknowledge a previously-received message.
///
/// ```json
/// { "#": "ack-id", "@": "original-msg-id", "ok": 1, "err": null }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunAck {
    /// This ACK's unique identifier.
    #[serde(rename = "#")]
    pub id: String,

    /// Identifier of the message being acknowledged.
    #[serde(rename = "@")]
    pub reply_to: String,

    /// Human-readable error; `None` / `null` means success.
    #[serde(default)]
    pub err: Option<String>,

    /// Success flag (`1` = ok); omitted on error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<u8>,
}

/// Any GUN Phase 1 wire message.
///
/// Serialized and deserialized as untagged JSON (the discriminating key
/// determines which variant to use).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum GunMessage {
    /// Node data push.
    Put(GunPut),
    /// Node data request.
    Get(GunGet),
    /// Acknowledgement.
    Ack(GunAck),
}

impl GunMessage {
    /// Return the message ID (`"#"` field).
    pub fn id(&self) -> &str {
        match self {
            GunMessage::Put(m) => &m.id,
            GunMessage::Get(m) => &m.id,
            GunMessage::Ack(m) => &m.id,
        }
    }

    /// Encode the message as UTF-8 JSON bytes.
    pub fn encode(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Decode a [`GunMessage`] from UTF-8 JSON bytes.
    pub fn decode(raw: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(raw)?)
    }
}

/// Return the current time as milliseconds since the Unix epoch (f64).
///
/// Used to populate HAM state timestamps.
pub fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_put(soul: &str, fields: HashMap<String, JsonValue>) -> GunMessage {
        let node = GunNode::from_data(soul, fields, 1_700_000_000_000.0);
        let mut put_map = HashMap::new();
        put_map.insert(soul.to_string(), node);
        GunMessage::Put(GunPut {
            id: "msg-001".to_string(),
            put: put_map,
        })
    }

    #[test]
    fn test_put_round_trip() {
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), json!("Alice"));
        fields.insert("role".to_string(), json!("admin"));

        let msg = make_put("user:alice", fields);

        let encoded = msg.encode().unwrap();
        let decoded = GunMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_get_round_trip() {
        let msg = GunMessage::Get(GunGet {
            id: "msg-002".to_string(),
            get: GunGetRequest {
                soul: "user:alice".to_string(),
                field: None,
            },
        });
        let encoded = msg.encode().unwrap();
        let decoded = GunMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_get_with_field_round_trip() {
        let msg = GunMessage::Get(GunGet {
            id: "msg-003".to_string(),
            get: GunGetRequest {
                soul: "user:alice".to_string(),
                field: Some("name".to_string()),
            },
        });
        let encoded = msg.encode().unwrap();
        let decoded = GunMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_ack_round_trip() {
        let msg = GunMessage::Ack(GunAck {
            id: "ack-001".to_string(),
            reply_to: "msg-001".to_string(),
            err: None,
            ok: Some(1),
        });
        let encoded = msg.encode().unwrap();
        let decoded = GunMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_ack_error_round_trip() {
        let msg = GunMessage::Ack(GunAck {
            id: "ack-002".to_string(),
            reply_to: "msg-002".to_string(),
            err: Some("soul not found".to_string()),
            ok: None,
        });
        let encoded = msg.encode().unwrap();
        let decoded = GunMessage::decode(&encoded).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_gun_wire_json_shape() {
        // Verify the encoded JSON matches GUN.js expected shape.
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), json!("Alice"));
        let node = GunNode::from_data("user:alice", fields, 1_700_000_000_000.0);
        let mut put_map = HashMap::new();
        put_map.insert("user:alice".to_string(), node);
        let msg = GunMessage::Put(GunPut {
            id: "msg-001".to_string(),
            put: put_map,
        });

        let value: serde_json::Value = serde_json::from_slice(&msg.encode().unwrap()).unwrap();
        assert_eq!(value["#"], "msg-001");
        assert!(value["put"]["user:alice"]["_"]["#"] == "user:alice");
        assert!(value["put"]["user:alice"]["name"] == "Alice");
    }

    #[test]
    fn test_node_merge_last_write_wins() {
        let mut fields_a = HashMap::new();
        fields_a.insert("name".to_string(), json!("Alice"));
        fields_a.insert("role".to_string(), json!("user"));
        let mut node_a = GunNode::from_data("soul", fields_a, 1000.0);

        let mut fields_b = HashMap::new();
        fields_b.insert("role".to_string(), json!("admin")); // newer
        fields_b.insert("age".to_string(), json!(30)); // new field
        let node_b = GunNode::from_data("soul", fields_b, 2000.0);

        node_a.merge(node_b);

        assert_eq!(node_a.fields["name"], json!("Alice")); // untouched
        assert_eq!(node_a.fields["role"], json!("admin")); // updated
        assert_eq!(node_a.fields["age"], json!(30)); // added
    }

    #[test]
    fn test_now_ms_is_positive() {
        let ts = now_ms();
        assert!(ts > 0.0);
    }

    #[test]
    fn test_message_id() {
        let mut fields = HashMap::new();
        fields.insert("x".to_string(), json!(1));
        let msg = make_put("soul", fields);
        assert_eq!(msg.id(), "msg-001");
    }
}
