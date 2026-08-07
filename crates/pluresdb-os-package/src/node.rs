//! `os.package` node schema + PluresDB read/write helpers.
//!
//! Schema (per Stage 1 design doc, Section 2):
//!
//! ```text
//! node kind: "os.package"
//!   id: "os.package:<name>"
//!   fields:
//!     name: text
//!     manager: text                      // "nix" for this slice
//!     desired_state: enum(absent, present, present_at(version))
//!     actual_state:  enum(absent, present, present_at(version), unknown)
//!     last_reconciled_at: text (ISO8601), nullable
//!     last_reconcile_result: enum(ok, error, pending)
//!     last_reconcile_error: text, nullable
//! ```

use chrono::{DateTime, Utc};
use pluresdb_core::CrdtStore;
use pluresdb_procedures::builder::MutateBuilder;
use pluresdb_procedures::engine::ProcedureEngine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

/// Node-type tag stored under `_type` so `os.package` nodes are
/// distinguishable from any other node kind sharing the same store.
pub const NODE_TYPE: &str = "os.package";

/// Desired or actual install state of a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesiredState {
    Absent,
    Present,
    /// Pinned to an exact version/attribute string (backend-defined format).
    PresentAt(String),
    /// Only valid as an `actual_state` value: reconciliation has not yet run.
    Unknown,
}

impl DesiredState {
    fn as_wire(&self) -> JsonValue {
        match self {
            DesiredState::Absent => json!("absent"),
            DesiredState::Present => json!("present"),
            DesiredState::PresentAt(v) => json!(format!("present@{v}")),
            DesiredState::Unknown => json!("unknown"),
        }
    }

    fn from_wire(v: &JsonValue) -> Option<Self> {
        let s = v.as_str()?;
        Some(match s {
            "absent" => DesiredState::Absent,
            "present" => DesiredState::Present,
            "unknown" => DesiredState::Unknown,
            other => other
                .strip_prefix("present@")
                .map(|ver| DesiredState::PresentAt(ver.to_string()))?,
        })
    }

    /// True if `self` (as an actual_state) already satisfies `desired`.
    pub fn satisfies(&self, desired: &DesiredState) -> bool {
        self == desired
    }
}

/// Outcome of the most recent reconciliation attempt for a node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconcileResult {
    Ok,
    Error,
    Pending,
}

/// In-memory view of an `os.package` node, read from / written to PluresDB.
#[derive(Debug, Clone, PartialEq)]
pub struct OsPackageNode {
    pub name: String,
    pub manager: String,
    pub desired_state: DesiredState,
    pub actual_state: DesiredState,
    pub last_reconciled_at: Option<DateTime<Utc>>,
    pub last_reconcile_result: ReconcileResult,
    pub last_reconcile_error: Option<String>,
}

/// Build the deterministic node id for `name`, e.g. `"os.package:ripgrep"`.
pub fn node_id(name: &str) -> String {
    format!("os.package:{name}")
}

impl OsPackageNode {
    fn from_data(name: &str, data: &JsonValue) -> Option<Self> {
        if data.get("_type").and_then(|v| v.as_str()) != Some(NODE_TYPE) {
            return None;
        }
        let manager = data.get("manager")?.as_str()?.to_string();
        let desired_state = DesiredState::from_wire(data.get("desired_state")?)?;
        let actual_state = data
            .get("actual_state")
            .and_then(DesiredState::from_wire)
            .unwrap_or(DesiredState::Unknown);
        let last_reconciled_at = data
            .get("last_reconciled_at")
            .and_then(|v| v.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));
        let last_reconcile_result = match data
            .get("last_reconcile_result")
            .and_then(|v| v.as_str())
        {
            Some("ok") => ReconcileResult::Ok,
            Some("error") => ReconcileResult::Error,
            _ => ReconcileResult::Pending,
        };
        let last_reconcile_error = data
            .get("last_reconcile_error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Some(OsPackageNode {
            name: name.to_string(),
            manager,
            desired_state,
            actual_state,
            last_reconciled_at,
            last_reconcile_result,
            last_reconcile_error,
        })
    }

    /// Fetch the node for `name` from `store`, if it exists and has the
    /// expected `_type`.
    pub fn get(store: &CrdtStore, name: &str) -> Option<Self> {
        let rec = store.get(node_id(name))?;
        Self::from_data(name, &rec.data)
    }

    /// List all `os.package` nodes currently in `store`.
    pub fn list_all(store: &CrdtStore) -> Vec<Self> {
        store
            .list()
            .into_iter()
            .filter_map(|rec| {
                let name = rec.data.get("name")?.as_str()?.to_string();
                Self::from_data(&name, &rec.data)
            })
            .collect()
    }

    /// True when `desired_state` and `actual_state` differ — the trigger
    /// condition the reactive actor watches for.
    pub fn needs_reconcile(&self) -> bool {
        self.desired_state != self.actual_state
    }
}

/// Write ONLY the `desired_state` for `name` via the `ProcedureEngine`'s
/// mutate step. This is the ONLY thing the shell surface is allowed to do —
/// it never touches `actual_state` and never invokes a package manager.
///
/// Creates the node (with `actual_state: unknown`) if it does not yet exist,
/// or merges the new `desired_state` into the existing node otherwise.
pub fn set_desired_state(
    store: &CrdtStore,
    actor: &str,
    name: &str,
    manager: &str,
    desired: DesiredState,
) -> anyhow::Result<()> {
    let id = node_id(name);
    let existing = store.get(&id);
    if let Some(rec) = &existing {
        let existing_mgr = rec
            .data
            .get("manager")
            .and_then(|v| v.as_str())
            .unwrap_or("<missing>");
        if existing_mgr != manager {
            return Err(anyhow::anyhow!(
                "os.package node '{name}' manager mismatch: existing={existing_mgr} requested={manager}"
            ));
        }
    }
    let patch = if existing.is_some() {
        json!({
            "desired_state": desired.as_wire(),
        })
    } else {
        json!({
            "_type": NODE_TYPE,
            "name": name,
            "manager": manager,
            "desired_state": desired.as_wire(),
            "actual_state": DesiredState::Unknown.as_wire(),
            "last_reconciled_at": null,
            "last_reconcile_result": "pending",
            "last_reconcile_error": null,
        })
    };

    let engine = ProcedureEngine::new(store, actor);
    let step = if existing.is_some() {
        MutateBuilder::new().merge(&id, patch).to_step()
    } else {
        MutateBuilder::new().put(&id, patch).to_step()
    };
    engine.exec(&[step])?;
    Ok(())
}

/// Write back the result of a reconciliation attempt: `actual_state`,
/// `last_reconciled_at`, `last_reconcile_result`, and (on failure)
/// `last_reconcile_error`. Called ONLY by the reactive actor, never by the
/// shell surface.
pub fn write_reconcile_result(
    store: &CrdtStore,
    actor: &str,
    name: &str,
    actual_state: DesiredState,
    result: ReconcileResult,
    error: Option<String>,
) -> anyhow::Result<()> {
    let id = node_id(name);
    let result_str = match result {
        ReconcileResult::Ok => "ok",
        ReconcileResult::Error => "error",
        ReconcileResult::Pending => "pending",
    };
    let patch = json!({
        "actual_state": actual_state.as_wire(),
        "last_reconciled_at": Utc::now().to_rfc3339(),
        "last_reconcile_result": result_str,
        "last_reconcile_error": error,
    });
    let engine = ProcedureEngine::new(store, actor);
    let step = MutateBuilder::new().merge(&id, patch).to_step();
    engine.exec(&[step])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_desired_state_creates_node_with_unknown_actual() {
        let store = CrdtStore::default();
        set_desired_state(&store, "shell", "ripgrep", "nix", DesiredState::Present).unwrap();
        let node = OsPackageNode::get(&store, "ripgrep").expect("node should exist");
        assert_eq!(node.desired_state, DesiredState::Present);
        assert_eq!(node.actual_state, DesiredState::Unknown);
        assert!(node.needs_reconcile());
    }

    #[test]
    fn set_desired_state_merges_into_existing_node_without_touching_actual() {
        let store = CrdtStore::default();
        set_desired_state(&store, "shell", "ripgrep", "nix", DesiredState::Present).unwrap();
        write_reconcile_result(
            &store,
            "reconciler",
            "ripgrep",
            DesiredState::Present,
            ReconcileResult::Ok,
            None,
        )
        .unwrap();
        // Now request removal — only desired_state should change.
        set_desired_state(&store, "shell", "ripgrep", "nix", DesiredState::Absent).unwrap();
        let node = OsPackageNode::get(&store, "ripgrep").unwrap();
        assert_eq!(node.desired_state, DesiredState::Absent);
        assert_eq!(node.actual_state, DesiredState::Present); // untouched by shell write
        assert!(node.needs_reconcile());
    }

    #[test]
    fn write_reconcile_result_clears_error_on_success() {
        let store = CrdtStore::default();
        set_desired_state(&store, "shell", "ripgrep", "nix", DesiredState::Present).unwrap();
        write_reconcile_result(
            &store,
            "reconciler",
            "ripgrep",
            DesiredState::Absent,
            ReconcileResult::Error,
            Some("nix profile install failed: exit 1".to_string()),
        )
        .unwrap();
        let node = OsPackageNode::get(&store, "ripgrep").unwrap();
        assert_eq!(node.last_reconcile_result, ReconcileResult::Error);
        assert_eq!(
            node.last_reconcile_error.as_deref(),
            Some("nix profile install failed: exit 1")
        );

        write_reconcile_result(
            &store,
            "reconciler",
            "ripgrep",
            DesiredState::Present,
            ReconcileResult::Ok,
            None,
        )
        .unwrap();
        let node = OsPackageNode::get(&store, "ripgrep").unwrap();
        assert_eq!(node.actual_state, DesiredState::Present);
        assert_eq!(node.last_reconcile_result, ReconcileResult::Ok);
        assert_eq!(node.last_reconcile_error, None);
    }

    #[test]
    fn list_all_ignores_non_os_package_nodes() {
        let store = CrdtStore::default();
        set_desired_state(&store, "shell", "ripgrep", "nix", DesiredState::Present).unwrap();
        store.put("other:1", "actor", json!({"_type": "something-else"}));
        let nodes = OsPackageNode::list_all(&store);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "ripgrep");
    }
}
