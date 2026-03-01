//! Foundational git-repository replication over the GUN P2P graph.
//!
//! Git objects and refs are modelled as GUN graph nodes so they can be
//! replicated between peers using the existing [`Replicator`] and
//! [`Connection`] infrastructure.
//!
//! ## Node layout
//!
//! | Soul pattern               | Purpose                               |
//! |----------------------------|---------------------------------------|
//! | `git:{repo_id}:manifest`   | Manifest node listing named refs      |
//! | `git:{repo_id}:ref:{name}` | A single named ref (branch/tag/HEAD)  |
//! | `git:{repo_id}:obj:{oid}`  | A single git object descriptor        |
//!
//! ## Replication flow
//!
//! 1. Pusher encodes a [`GitManifest`] into GUN nodes via
//!    [`encode_manifest_nodes`].
//! 2. Nodes are pushed over any [`Connection`] with
//!    [`Replicator::push_all`][crate::Replicator::push_all].
//! 3. Receiver reconstructs the [`GitManifest`] from the PUT'd nodes via
//!    [`decode_manifest_nodes`].
//! 4. Receiver fetches missing objects (blobs/trees/commits) via
//!    content-addressed blob storage.
//!
//! This module covers the *protocol* layer (encoding / decoding).  The
//! transport and blob-fetch orchestration live at a higher abstraction layer.

use crate::gun_protocol::{GunNode, Soul};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// Kind of git object being described.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GitObjectKind {
    Blob,
    Tree,
    Commit,
    Tag,
}

impl GitObjectKind {
    /// String representation as stored in GUN node fields.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blob => "blob",
            Self::Tree => "tree",
            Self::Commit => "commit",
            Self::Tag => "tag",
        }
    }
}

impl std::fmt::Display for GitObjectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A git object descriptor: its type and 40-hex SHA-1 OID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitObject {
    /// 40-character lowercase hex SHA-1 object ID.
    pub oid: String,
    /// Object type.
    pub kind: GitObjectKind,
}

/// A git named reference (branch, tag, or `HEAD`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitRef {
    /// Full ref name, e.g. `refs/heads/main` or `HEAD`.
    pub name: String,
    /// Target OID (40-hex SHA-1 commit or tag object).
    pub oid: String,
}

/// A complete git repository manifest.
///
/// Encodes the refs advertised by the remote (analogous to `git upload-pack`'s
/// advertisement phase) plus an optional flat list of object descriptors for
/// thin-pack negotiation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitManifest {
    /// Repository identifier (e.g. a CAS content hash or user-chosen name).
    pub repo_id: String,
    /// Named refs advertised by this peer.
    pub refs: Vec<GitRef>,
    /// Object descriptors known by this peer (may be empty for ref-only push).
    pub objects: Vec<GitObject>,
}

// ---------------------------------------------------------------------------
// Soul helpers
// ---------------------------------------------------------------------------

/// Compute the soul for the manifest node of a repository.
pub fn manifest_soul(repo_id: &str) -> Soul {
    format!("git:{}:manifest", repo_id)
}

/// Compute the soul for a named ref node.
pub fn ref_soul(repo_id: &str, ref_name: &str) -> Soul {
    format!("git:{}:ref:{}", repo_id, ref_name)
}

/// Compute the soul for a git object descriptor node.
pub fn obj_soul(repo_id: &str, oid: &str) -> Soul {
    format!("git:{}:obj:{}", repo_id, oid)
}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

/// Encode a [`GitManifest`] as a list of `(Soul, serde_json::Value)` pairs
/// ready to be pushed via [`Replicator::push_all`][crate::Replicator::push_all].
///
/// The returned list always contains:
/// - One manifest node (lists all ref names and object OIDs).
/// - One node per [`GitRef`].
/// - One node per [`GitObject`].
pub fn encode_manifest_nodes(
    manifest: &GitManifest,
) -> Vec<(Soul, serde_json::Value)> {
    let ts = crate::gun_protocol::now_ms();
    let mut nodes = Vec::new();

    // Manifest node: lists ref names and object OIDs.
    let ref_names: Vec<serde_json::Value> =
        manifest.refs.iter().map(|r| json!(r.name)).collect();
    let obj_oids: Vec<serde_json::Value> =
        manifest.objects.iter().map(|o| json!(o.oid)).collect();
    let manifest_node = GunNode::from_data(
        manifest_soul(&manifest.repo_id),
        [
            ("_type".to_string(), json!("git:manifest")),
            ("repo_id".to_string(), json!(manifest.repo_id)),
            ("refs".to_string(), json!(ref_names)),
            ("objects".to_string(), json!(obj_oids)),
        ]
        .into_iter()
        .collect(),
        ts,
    );
    nodes.push((manifest_soul(&manifest.repo_id), node_to_value(manifest_node)));

    // One node per ref.
    for r in &manifest.refs {
        let ref_node = GunNode::from_data(
            ref_soul(&manifest.repo_id, &r.name),
            [
                ("_type".to_string(), json!("git:ref")),
                ("name".to_string(), json!(r.name)),
                ("oid".to_string(), json!(r.oid)),
            ]
            .into_iter()
            .collect(),
            ts,
        );
        nodes.push((ref_soul(&manifest.repo_id, &r.name), node_to_value(ref_node)));
    }

    // One node per object descriptor.
    for obj in &manifest.objects {
        let obj_node = GunNode::from_data(
            obj_soul(&manifest.repo_id, &obj.oid),
            [
                ("_type".to_string(), json!("git:obj")),
                ("oid".to_string(), json!(obj.oid)),
                ("kind".to_string(), json!(obj.kind.as_str())),
            ]
            .into_iter()
            .collect(),
            ts,
        );
        nodes.push((obj_soul(&manifest.repo_id, &obj.oid), node_to_value(obj_node)));
    }

    nodes
}

/// Convert a [`GunNode`] to a plain `serde_json::Value::Object` for use with
/// [`Replicator::send_all`][crate::Replicator::send_all].
fn node_to_value(node: GunNode) -> serde_json::Value {
    serde_json::Value::Object(node.fields.into_iter().collect())
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

/// Reconstruct a [`GitManifest`] from a collection of decoded GUN nodes.
///
/// `nodes` should be the `Vec<(Soul, GunNode)>` returned by
/// [`Replicator::receive_all`][crate::Replicator::receive_all] (or the
/// `sync` equivalents after a [`Replicator::push_all`][crate::Replicator::push_all]).
///
/// Returns `None` if no manifest node for `repo_id` is present in `nodes`.
pub fn decode_manifest_nodes(
    repo_id: &str,
    nodes: &[(Soul, GunNode)],
) -> Option<GitManifest> {
    // Index nodes by soul for O(1) lookup.
    let by_soul: HashMap<&str, &GunNode> =
        nodes.iter().map(|(s, n)| (s.as_str(), n)).collect();

    // Check manifest node exists.
    let msoul = manifest_soul(repo_id);
    by_soul.get(msoul.as_str())?;

    // Collect refs.
    let mut refs = Vec::new();
    for (soul, node) in nodes {
        if soul.starts_with(&format!("git:{}:ref:", repo_id)) {
            let name = node
                .fields
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let oid = node
                .fields
                .get("oid")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            refs.push(GitRef { name, oid });
        }
    }

    // Collect object descriptors.
    let mut objects = Vec::new();
    for (soul, node) in nodes {
        if soul.starts_with(&format!("git:{}:obj:", repo_id)) {
            let oid = node
                .fields
                .get("oid")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let kind_str = node
                .fields
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("blob");
            let kind = match kind_str {
                "tree" => GitObjectKind::Tree,
                "commit" => GitObjectKind::Commit,
                "tag" => GitObjectKind::Tag,
                _ => GitObjectKind::Blob,
            };
            objects.push(GitObject { oid, kind });
        }
    }

    Some(GitManifest {
        repo_id: repo_id.to_string(),
        refs,
        objects,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemConnection, Replicator};

    fn sample_manifest() -> GitManifest {
        GitManifest {
            repo_id: "my-repo".to_string(),
            refs: vec![
                GitRef {
                    name: "refs/heads/main".to_string(),
                    oid: "a".repeat(40),
                },
                GitRef {
                    name: "HEAD".to_string(),
                    oid: "a".repeat(40),
                },
            ],
            objects: vec![
                GitObject {
                    oid: "b".repeat(40),
                    kind: GitObjectKind::Commit,
                },
                GitObject {
                    oid: "c".repeat(40),
                    kind: GitObjectKind::Tree,
                },
            ],
        }
    }

    #[test]
    fn test_soul_helpers() {
        assert_eq!(manifest_soul("repo"), "git:repo:manifest");
        assert_eq!(ref_soul("repo", "refs/heads/main"), "git:repo:ref:refs/heads/main");
        assert_eq!(obj_soul("repo", "abc123"), "git:repo:obj:abc123");
    }

    #[test]
    fn test_encode_produces_correct_count() {
        let manifest = sample_manifest();
        let nodes = encode_manifest_nodes(&manifest);
        // 1 manifest + 2 refs + 2 objects = 5 nodes.
        assert_eq!(nodes.len(), 5);
    }

    #[test]
    fn test_decode_roundtrip() {
        let manifest = sample_manifest();
        let encoded = encode_manifest_nodes(&manifest);

        // Simulate receiving over a channel by converting to GunNode.
        let gun_nodes: Vec<(Soul, crate::gun_protocol::GunNode)> = encoded
            .iter()
            .map(|(soul, data)| {
                let fields: HashMap<String, serde_json::Value> =
                    if let serde_json::Value::Object(map) = data {
                        map.clone().into_iter().collect()
                    } else {
                        HashMap::new()
                    };
                let ts = crate::gun_protocol::now_ms();
                (soul.clone(), crate::gun_protocol::GunNode::from_data(soul, fields, ts))
            })
            .collect();

        let decoded = decode_manifest_nodes("my-repo", &gun_nodes).unwrap();

        assert_eq!(decoded.repo_id, "my-repo");
        assert_eq!(decoded.refs.len(), 2);
        assert_eq!(decoded.objects.len(), 2);

        let ref_names: Vec<&str> = decoded.refs.iter().map(|r| r.name.as_str()).collect();
        assert!(ref_names.contains(&"refs/heads/main"));
        assert!(ref_names.contains(&"HEAD"));

        let obj_kinds: Vec<GitObjectKind> = decoded.objects.iter().map(|o| o.kind).collect();
        assert!(obj_kinds.contains(&GitObjectKind::Commit));
        assert!(obj_kinds.contains(&GitObjectKind::Tree));
    }

    #[test]
    fn test_decode_returns_none_for_missing_repo() {
        let manifest = sample_manifest();
        let encoded = encode_manifest_nodes(&manifest);
        let gun_nodes: Vec<(Soul, crate::gun_protocol::GunNode)> = encoded
            .iter()
            .map(|(soul, data)| {
                let fields: HashMap<String, serde_json::Value> =
                    if let serde_json::Value::Object(map) = data {
                        map.clone().into_iter().collect()
                    } else {
                        HashMap::new()
                    };
                let ts = crate::gun_protocol::now_ms();
                (soul.clone(), crate::gun_protocol::GunNode::from_data(soul, fields, ts))
            })
            .collect();

        assert!(decode_manifest_nodes("nonexistent-repo", &gun_nodes).is_none());
    }

    #[tokio::test]
    async fn test_git_manifest_replication_over_mem_connection() {
        let (mut conn_a, mut conn_b) = MemConnection::pair("peer-a", "peer-b");
        let rep_a = Replicator::new("peer-a");
        let rep_b = Replicator::new("peer-b");

        let manifest = sample_manifest();
        let nodes_to_send = encode_manifest_nodes(&manifest);

        let (push_result, received) = tokio::join!(
            rep_a.push_all(&mut conn_a, &nodes_to_send),
            rep_b.receive_all(&mut conn_b),
        );
        push_result.unwrap();
        let received = received.unwrap();

        // 5 nodes should be received (1 manifest + 2 refs + 2 objects).
        assert_eq!(received.len(), 5);

        // Reconstruct manifest from received nodes.
        let decoded = decode_manifest_nodes("my-repo", &received).unwrap();
        assert_eq!(decoded.repo_id, "my-repo");
        assert_eq!(decoded.refs.len(), 2);
        assert_eq!(decoded.objects.len(), 2);
    }
}
