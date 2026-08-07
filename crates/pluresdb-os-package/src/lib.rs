//! `pluresdb-os-package` — narrowest real vertical slice of the
//! `praxis-os:px-shell-mcp-gui-selfimprovement` epic, scope #3.
//!
//! Architecture (per `memory/praxis-os-stage1-design-2026-08-07.md` and
//! constraint C-PLURES-004: "PluresDB IS the system... side-effecting
//! actions are triggered BY PluresDB writes, never executed directly by the
//! shell command"):
//!
//! 1. A shell command (`os.package install <name>` / `remove` / `status`)
//!    writes ONLY a `desired_state` mutation to an `os.package:<name>` node
//!    via the existing `pluresdb-procedures` `ProcedureEngine`
//!    (`ops::mutate`). It never shells out itself.
//! 2. A reactive actor ([`reconcile_once`] / [`Reconciler::run_forever`])
//!    scans for nodes where `desired_state != actual_state`, invokes the
//!    real package manager via an injected [`PackageManager`] trait, and
//!    writes back `actual_state` reflecting the REAL result (success or
//!    failure — never assumed).
//!
//! Only the `nix` backend ([`NixPackageManager`]) is implemented for this
//! slice, targeting a scratch/throwaway Nix profile path (never the host's
//! real default profile) for safety, per the task's explicit test-safety
//! requirement.

pub mod backend;
pub mod node;
pub mod reconciler;

pub use backend::{CommandOutcome, NixPackageManager, PackageManager};
pub use node::{DesiredState, OsPackageNode, ReconcileResult};
pub use reconciler::{reconcile_once, Reconciler};
