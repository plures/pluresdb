use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

pub type Soul = String;
pub type HamState = HashMap<String, f64>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunMeta {
    #[serde(rename = "#")]
    pub soul: Soul,
    #[serde(rename = ">")]
    pub state: HamState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunNode {
    #[serde(rename = "_")]
    pub meta: GunMeta,
    #[serde(flatten)]
    pub fields: HashMap<String, JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunGetRequest {
    #[serde(rename = "#")]
    pub soul: Soul,
    #[serde(rename = ".", skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunPut {
    #[serde(rename = "#")]
    pub id: String,
    pub put: HashMap<Soul, GunNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunGet {
    #[serde(rename = "#")]
    pub id: String,
    pub get: GunGetRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GunAck {
    #[serde(rename = "#")]
    pub id: String,
    #[serde(rename = "@")]
    pub reply_to: String,
    #[serde(default)]
    pub err: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum GunMessage {
    Put(GunPut),
    Get(GunGet),
    Ack(GunAck),
}

impl GunMessage {
    pub fn encode(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }

    pub fn decode(raw: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip_put_get_ack() {
        let mut node_fields = HashMap::new();
        node_fields.insert("name".to_string(), json!("Alice"));

        let mut ham = HashMap::new();
        ham.insert("name".to_string(), 1_700_000_000_000.0);

        let mut graph = HashMap::new();
        graph.insert(
            "user:alice".to_string(),
            GunNode {
                meta: GunMeta {
                    soul: "user:alice".to_string(),
                    state: ham,
                },
                fields: node_fields,
            },
        );

        let messages = vec![
            GunMessage::Put(GunPut {
                id: "msg-put-1".to_string(),
                put: graph,
            }),
            GunMessage::Get(GunGet {
                id: "msg-get-1".to_string(),
                get: GunGetRequest {
                    soul: "user:alice".to_string(),
                    field: Some("name".to_string()),
                },
            }),
            GunMessage::Ack(GunAck {
                id: "msg-ack-1".to_string(),
                reply_to: "msg-get-1".to_string(),
                err: None,
                ok: Some(1),
            }),
        ];

        for msg in messages {
            let encoded = msg.encode().unwrap();
            let decoded = GunMessage::decode(&encoded).unwrap();
            assert_eq!(msg, decoded);
        }
    }
}
