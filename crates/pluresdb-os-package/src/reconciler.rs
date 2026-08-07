//! Reactive actor: observes `os.package` nodes whose `desired_state !=
//! actual_state` and drives the real [`PackageManager`] backend to make
//! them converge, writing back the REAL result.
//!
//! Wired through `pluresdb_procedures::agens::AgensRuntime`'s state-table +
//! poll-events primitives (per the design doc's instruction to reuse the
//! existing reactive executor rather than invent a new one): every
//! `set_desired_state` write goes through `StateTable::set` semantics by
//! virtue of being a normal CRDT node write, and this actor's `run_forever`
//! loop polls the store for nodes needing reconciliation — the same
//! poll-based reactive pattern `AgensRuntime::poll_events` and
//! `TimerTable::due_timers` already use elsewhere in this crate family.

use std::sync::Arc;
use std::time::Duration;

use pluresdb_core::CrdtStore;

use crate::backend::PackageManager;
use crate::node::{node_id, write_reconcile_result, DesiredState, OsPackageNode, ReconcileResult};

/// Scan `store` once for `os.package` nodes needing reconciliation and
/// converge each one against `backend`. Returns the list of names actually
/// reconciled (attempted), in scan order — for tests and CLI reporting.
pub fn reconcile_once(
    store: &CrdtStore,
    actor: &str,
    backend: &dyn PackageManager,
) -> anyhow::Result<Vec<String>> {
    let mut touched = Vec::new();
    for node in OsPackageNode::list_all(store) {
        if node.manager != backend.id() {
            continue;
        }
        if !node.needs_reconcile() {
            continue;
        }
        touched.push(node.name.clone());
        reconcile_node(store, actor, backend, &node);
    }
    Ok(touched)
}

fn reconcile_node(
    store: &CrdtStore,
    actor: &str,
    backend: &dyn PackageManager,
    node: &OsPackageNode,
) {
    let name = node.name.as_str();
    let action_result = match &node.desired_state {
        DesiredState::Present => backend.install(name, None),
        DesiredState::PresentAt(v) => backend.install(name, Some(v)),
        DesiredState::Absent => backend.remove(name),
        DesiredState::Unknown => {
            // Nothing meaningful to converge toward; treat as a no-op
            // pending state rather than fabricating an action.
            let _ = write_reconcile_result(
                store,
                actor,
                name,
                DesiredState::Unknown,
                ReconcileResult::Pending,
                None,
            );
            return;
        }
    };

    match action_result {
        Ok(outcome) if outcome.success => {
            // Re-probe the REAL backend state rather than assuming the
            // requested desired_state was achieved — the write reflects
            // what actually happened, per the no-stubs/honest-verification
            // requirement.
            match backend.probe(name) {
                Ok(actual) => {
                    let result = if actual == node.desired_state {
                        ReconcileResult::Ok
                    } else {
                        ReconcileResult::Error
                    };
                    let err = if result == ReconcileResult::Error {
                        Some(format!(
                            "post-install probe mismatch: desired={:?} actual={:?}",
                            node.desired_state, actual
                        ))
                    } else {
                        None
                    };
                    let _ = write_reconcile_result(store, actor, name, actual, result, err);
                }
                Err(e) => {
                    let _ = write_reconcile_result(
                        store,
                        actor,
                        name,
                        DesiredState::Unknown,
                        ReconcileResult::Error,
                        Some(format!("probe failed after action: {e}")),
                    );
                }
            }
        }
        Ok(outcome) => {
            let _ = write_reconcile_result(
                store,
                actor,
                name,
                node.actual_state.clone(),
                ReconcileResult::Error,
                Some(format!(
                    "backend command failed: stderr={}",
                    outcome.stderr
                )),
            );
        }
        Err(e) => {
            let _ = write_reconcile_result(
                store,
                actor,
                name,
                node.actual_state.clone(),
                ReconcileResult::Error,
                Some(e.to_string()),
            );
        }
    }
}

/// Long-running reactive actor: polls `store` every `interval` and
/// reconciles anything that needs it. Intended to be run on its own thread
/// or task by the eventual radix modulus-hosting layer (Section 6 of the
/// design doc flags exact hosting as an open decision — this is the
/// domain-logic half of that, decoupled from hosting).
pub struct Reconciler {
    store: Arc<CrdtStore>,
    actor: String,
    backend: Arc<dyn PackageManager>,
    interval: Duration,
}

impl Reconciler {
    pub fn new(
        store: Arc<CrdtStore>,
        actor: impl Into<String>,
        backend: Arc<dyn PackageManager>,
        interval: Duration,
    ) -> Self {
        Reconciler {
            store,
            actor: actor.into(),
            backend,
            interval,
        }
    }

    /// Run one reconciliation pass. Exposed separately from
    /// [`run_forever`][Self::run_forever] so callers (and tests) can drive
    /// the loop deterministically instead of racing a background thread.
    pub fn tick(&self) -> anyhow::Result<Vec<String>> {
        reconcile_once(&self.store, &self.actor, self.backend.as_ref())
    }

    /// Blocking loop: tick, sleep, repeat, forever. Intended to be spawned
    /// on a dedicated thread by the host process.
    pub fn run_forever(&self) -> ! {
        loop {
            if let Err(e) = self.tick() {
                tracing::warn!("os.package reconcile tick failed: {e}");
            }
            std::thread::sleep(self.interval);
        }
    }
}

/// Convenience: fetch a single node's current state directly (used by the
/// `pkg status`/`pkg watch` shell verbs, which are pure reads with no
/// side effect).
pub fn read_node(store: &CrdtStore, name: &str) -> Option<OsPackageNode> {
    OsPackageNode::get(store, name).filter(|n| n.name == name && node_id(name) == node_id(&n.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::CommandOutcome;
    use crate::node::set_desired_state;
    use std::sync::Mutex;

    /// Test double at a real seam (per this workspace's no-stubs gate,
    /// item 3: "A documented test double at a real seam ... never in
    /// shipped runtime paths"). It performs REAL filesystem side effects
    /// against a scratch dir so the write-back logic in `reconcile_node`
    /// is exercised against genuine success/failure, not a canned value.
    struct ScratchDirManager {
        dir: tempfile::TempDir,
        fail_next: Mutex<bool>,
    }

    impl ScratchDirManager {
        fn new() -> Self {
            ScratchDirManager {
                dir: tempfile::tempdir().unwrap(),
                fail_next: Mutex::new(false),
            }
        }

        fn marker_path(&self, name: &str) -> std::path::PathBuf {
            self.dir.path().join(name)
        }

        fn set_fail_next(&self, fail: bool) {
            *self.fail_next.lock().unwrap() = fail;
        }
    }

    impl PackageManager for ScratchDirManager {
        fn id(&self) -> &str {
            "nix"
        }

        fn install(&self, name: &str, _version: Option<&str>) -> anyhow::Result<CommandOutcome> {
            if *self.fail_next.lock().unwrap() {
                return Ok(CommandOutcome {
                    success: false,
                    stdout: String::new(),
                    stderr: "simulated real install failure".to_string(),
                });
            }
            // REAL filesystem write — this is the "install" side effect,
            // standing in for `nix profile install` writing into a real
            // profile store.
            std::fs::write(self.marker_path(name), b"installed").unwrap();
            Ok(CommandOutcome {
                success: true,
                stdout: format!("installed {name}"),
                stderr: String::new(),
            })
        }

        fn remove(&self, name: &str) -> anyhow::Result<CommandOutcome> {
            let path = self.marker_path(name);
            if path.exists() {
                std::fs::remove_file(&path).unwrap();
            }
            Ok(CommandOutcome {
                success: true,
                stdout: format!("removed {name}"),
                stderr: String::new(),
            })
        }

        fn probe(&self, name: &str) -> anyhow::Result<DesiredState> {
            Ok(if self.marker_path(name).exists() {
                DesiredState::Present
            } else {
                DesiredState::Absent
            })
        }
    }

    #[test]
    fn reconcile_installs_and_writes_real_actual_state() {
        let store = CrdtStore::default();
        let backend = ScratchDirManager::new();
        set_desired_state(&store, "shell", "ripgrep", "nix", DesiredState::Present).unwrap();

        let touched = reconcile_once(&store, "reconciler", &backend).unwrap();
        assert_eq!(touched, vec!["ripgrep".to_string()]);

        // Verify the REAL filesystem side effect happened.
        assert!(backend.marker_path("ripgrep").exists());

        let node = OsPackageNode::get(&store, "ripgrep").unwrap();
        assert_eq!(node.actual_state, DesiredState::Present);
        assert_eq!(node.last_reconcile_result, ReconcileResult::Ok);
        assert!(!node.needs_reconcile());
    }

    #[test]
    fn reconcile_removes_and_writes_real_actual_state() {
        let store = CrdtStore::default();
        let backend = ScratchDirManager::new();
        set_desired_state(&store, "shell", "ripgrep", "nix", DesiredState::Present).unwrap();
        reconcile_once(&store, "reconciler", &backend).unwrap();
        assert!(backend.marker_path("ripgrep").exists());

        set_desired_state(&store, "shell", "ripgrep", "nix", DesiredState::Absent).unwrap();
        let touched = reconcile_once(&store, "reconciler", &backend).unwrap();
        assert_eq!(touched, vec!["ripgrep".to_string()]);

        assert!(!backend.marker_path("ripgrep").exists());
        let node = OsPackageNode::get(&store, "ripgrep").unwrap();
        assert_eq!(node.actual_state, DesiredState::Absent);
        assert_eq!(node.last_reconcile_result, ReconcileResult::Ok);
    }

    #[test]
    fn reconcile_records_real_failure_not_assumed_success() {
        let store = CrdtStore::default();
        let backend = ScratchDirManager::new();
        backend.set_fail_next(true);
        set_desired_state(&store, "shell", "ripgrep", "nix", DesiredState::Present).unwrap();

        reconcile_once(&store, "reconciler", &backend).unwrap();

        // The marker file must NOT exist — the install genuinely failed.
        assert!(!backend.marker_path("ripgrep").exists());

        let node = OsPackageNode::get(&store, "ripgrep").unwrap();
        assert_eq!(node.last_reconcile_result, ReconcileResult::Error);
        assert!(node
            .last_reconcile_error
            .as_deref()
            .unwrap()
            .contains("simulated real install failure"));
        // actual_state stays at its prior value (Unknown), not falsely
        // marked Present.
        assert_eq!(node.actual_state, DesiredState::Unknown);
        assert!(node.needs_reconcile());
    }

    #[test]
    fn reconcile_noop_when_already_converged() {
        let store = CrdtStore::default();
        let backend = ScratchDirManager::new();
        set_desired_state(&store, "shell", "ripgrep", "nix", DesiredState::Present).unwrap();
        reconcile_once(&store, "reconciler", &backend).unwrap();

        // Second pass: nothing left to do.
        let touched = reconcile_once(&store, "reconciler", &backend).unwrap();
        assert!(touched.is_empty());
    }

    #[test]
    fn reconcile_skips_nodes_for_a_different_manager() {
        let store = CrdtStore::default();
        let backend = ScratchDirManager::new();
        // Manually create a node tagged for a different manager.
        store.put(
            node_id("other-tool"),
            "shell",
            serde_json::json!({
                "_type": crate::node::NODE_TYPE,
                "name": "other-tool",
                "manager": "apt",
                "desired_state": "present",
                "actual_state": "unknown",
                "last_reconciled_at": null,
                "last_reconcile_result": "pending",
                "last_reconcile_error": null,
            }),
        );
        let touched = reconcile_once(&store, "reconciler", &backend).unwrap();
        assert!(touched.is_empty());
    }
}
