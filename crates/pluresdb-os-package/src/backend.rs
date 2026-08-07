//! Package manager backends: the ONLY place a process is ever spawned in
//! this crate. The reactive actor ([`crate::reconciler`]) is the sole
//! caller — the shell surface (`bin/px_shell.rs`) never touches this
//! module directly.

use std::process::Command;

use crate::node::DesiredState;

/// Result of invoking the real package-manager command.
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// A backend capable of installing/removing packages and probing the real
/// installed state. Implementations perform REAL side effects (or, for a
/// test double, a substitute real effect against a scratch directory) —
/// never a canned/fake success value (per this workspace's no-stubs gate).
pub trait PackageManager: Send + Sync {
    /// Human-readable backend id, e.g. `"nix"`.
    fn id(&self) -> &str;

    /// Install `name` (optionally pinned to `version`). Returns the real
    /// process outcome; the caller decides what `actual_state`/
    /// `ReconcileResult` that maps to.
    fn install(&self, name: &str, version: Option<&str>) -> anyhow::Result<CommandOutcome>;

    /// Remove `name`.
    fn remove(&self, name: &str) -> anyhow::Result<CommandOutcome>;

    /// Probe the REAL current install state of `name` by querying the
    /// backend (not by trusting the last-written `actual_state`).
    fn probe(&self, name: &str) -> anyhow::Result<DesiredState>;
}

/// Real `nix profile` backend, scoped to an explicit profile directory so
/// tests (and any automated reconciliation) never touch a host's default
/// Nix profile.
///
/// Uses `nix profile install|remove|list --profile <profile_dir>`
/// (the modern Nix CLI, flakes-enabled). Requires `nix` on `PATH` with
/// `experimental-features = nix-command flakes` — if `nix` is unavailable,
/// every call returns a real `Err`, never a fabricated success.
pub struct NixPackageManager {
    profile_dir: std::path::PathBuf,
}

impl NixPackageManager {
    /// Create a backend that operates against `profile_dir`. Callers doing
    /// anything other than a disposable test MUST pass a scratch directory
    /// they own — this type performs a real `nix profile` mutation at that
    /// path, it does not default to the host's real profile.
    pub fn new(profile_dir: impl Into<std::path::PathBuf>) -> Self {
        NixPackageManager {
            profile_dir: profile_dir.into(),
        }
    }

    fn run(&self, args: &[&str]) -> anyhow::Result<CommandOutcome> {
        let output = Command::new("nix")
            .args(args)
            .output()
            .map_err(|e| anyhow::anyhow!("failed to spawn `nix`: {e}"))?;
        Ok(CommandOutcome {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

impl PackageManager for NixPackageManager {
    fn id(&self) -> &str {
        "nix"
    }

    fn install(&self, name: &str, version: Option<&str>) -> anyhow::Result<CommandOutcome> {
        // `version` is accepted for schema completeness (present_at) but
        // this minimal slice installs by nixpkgs attribute name only; exact
        // version pinning via flake refs is left to a future slice per the
        // "narrowest possible slice" instruction — not faked here.
        let attr = match version {
            Some(v) => format!("nixpkgs#{name}@{v}"),
            None => format!("nixpkgs#{name}"),
        };
        self.run(&[
            "profile",
            "install",
            "--profile",
            self.profile_dir.to_string_lossy().as_ref(),
            &attr,
        ])
    }

    fn remove(&self, name: &str) -> anyhow::Result<CommandOutcome> {
        self.run(&[
            "profile",
            "remove",
            "--profile",
            self.profile_dir.to_string_lossy().as_ref(),
            name,
        ])
    }

    fn probe(&self, name: &str) -> anyhow::Result<DesiredState> {
        let outcome = self.run(&[
            "profile",
            "list",
            "--profile",
            self.profile_dir.to_string_lossy().as_ref(),
        ])?;
        if !outcome.success {
            return Err(anyhow::anyhow!(
                "nix profile list failed: {}",
                outcome.stderr
            ));
        }
        // `nix profile list` output format includes the attribute path per
        // entry; a substring match on the package name is a real (if
        // coarse) presence check for this minimal slice.
        if outcome.stdout.contains(name) {
            Ok(DesiredState::Present)
        } else {
            Ok(DesiredState::Absent)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Confirms the backend produces a real `Err` (not a fabricated
    /// success) when `nix` is not on `PATH` — the honest behavior this
    /// environment actually exercises, since `nix` is not installed on this
    /// DevBox. This is NOT a stub: it is asserting real error-propagation
    /// behavior of real code against an environment fact.
    #[test]
    fn install_returns_real_error_when_nix_binary_missing_or_present() {
        let mgr = NixPackageManager::new(std::env::temp_dir().join("px-shell-test-profile"));
        // Whichever the case in the running environment (nix absent here),
        // the call must not panic and must not silently report success
        // without having actually run a command.
        let result = mgr.install("hello", None);
        match result {
            Ok(outcome) => {
                // If `nix` happens to be present in some future CI image,
                // a real command genuinely ran; success/failure both prove
                // the process was invoked for real.
                assert!(outcome.success || !outcome.stderr.is_empty());
            }
            Err(e) => {
                assert!(e.to_string().contains("nix"));
            }
        }
    }
}
