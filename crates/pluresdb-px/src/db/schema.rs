//! PluresDB schema types for the praxis runtime.
//!
//! These types mirror the PluresDB schema defined in the issue:
//!
//! ```text
//! Constraint { id, when, require, fix, evidence -> ADR[], severity }
//! ADR         { id, title, status, evidence -> Evidence[] }
//! Evidence    { id, tested_at, condition, result, reference }
//! AgentContext{ action_type, target, metadata, session_type }
//! ```
//!
//! All types are serialisable so that the store can persist to/from JSON and
//! be compared in tests.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Condition
// ---------------------------------------------------------------------------

/// A composable predicate evaluated against an [`AgentContext`].
///
/// Conditions are stored as data (not code) so they can be serialised into
/// PluresDB records and re-loaded at runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Condition {
    /// Always satisfied — used as a wildcard precondition.
    Always,
    /// The [`AgentContext::action_type`] exactly equals the given value.
    ActionTypeEq {
        /// Expected action type string.
        value: String,
    },
    /// The named metadata field equals the given JSON value.
    FieldEq {
        /// Dot-separated path into [`AgentContext::metadata`] (e.g. `"privilege_level"`).
        field: String,
        /// Expected value.
        value: serde_json::Value,
    },
    /// The named metadata field is present (regardless of value).
    FieldExists {
        /// Field path in [`AgentContext::metadata`].
        field: String,
    },
    /// The named metadata field (numeric) is greater than `threshold`.
    FieldGt {
        /// Field path in [`AgentContext::metadata`].
        field: String,
        /// Lower bound (exclusive).
        threshold: f64,
    },
    /// The named metadata field (numeric) is less than `threshold`.
    FieldLt {
        /// Field path in [`AgentContext::metadata`].
        field: String,
        /// Upper bound (exclusive).
        threshold: f64,
    },
    /// The [`AgentContext::action_type`] starts with the given prefix.
    ActionStartsWith {
        /// Prefix to match.
        prefix: String,
    },
    /// The [`AgentContext::session_type`] matches.
    SessionIs {
        /// Expected session type.
        session_type: SessionType,
    },
    /// All sub-conditions must hold.
    All {
        /// Inner conditions (conjunction).
        conditions: Vec<Condition>,
    },
    /// At least one sub-condition must hold.
    Any {
        /// Inner conditions (disjunction).
        conditions: Vec<Condition>,
    },
    /// The inner condition must NOT hold.
    Not {
        /// Negated condition.
        condition: Box<Condition>,
    },
}

impl Condition {
    /// Evaluate this condition against `ctx`.
    #[must_use]
    pub fn evaluate(&self, ctx: &AgentContext) -> bool {
        match self {
            Self::Always => true,
            Self::ActionTypeEq { value } => ctx.action_type == *value,

            Self::FieldEq { field, value } => ctx.metadata.get(field).is_some_and(|v| v == value),

            Self::FieldExists { field } => ctx.metadata.contains_key(field),

            Self::FieldGt { field, threshold } => ctx
                .metadata
                .get(field)
                .and_then(|v| v.as_f64())
                .is_some_and(|n| n > *threshold),

            Self::FieldLt { field, threshold } => ctx
                .metadata
                .get(field)
                .and_then(|v| v.as_f64())
                .is_some_and(|n| n < *threshold),

            Self::ActionStartsWith { prefix } => ctx.action_type.starts_with(prefix.as_str()),

            Self::SessionIs { session_type } => &ctx.session_type == session_type,

            Self::All { conditions } => conditions.iter().all(|c| c.evaluate(ctx)),

            Self::Any { conditions } => conditions.iter().any(|c| c.evaluate(ctx)),

            Self::Not { condition } => !condition.evaluate(ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// How severe a constraint violation is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The action must be blocked.
    Error,
    /// The action is allowed but a warning is emitted.
    Warning,
}

// ---------------------------------------------------------------------------
// Constraint
// ---------------------------------------------------------------------------

/// A praxis constraint record stored in PluresDB.
///
/// A constraint fires when `when` holds for the current [`AgentContext`] and
/// `require` does **not** hold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// Stable, unique identifier (e.g. `"C-0001"`).
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// Precondition — the constraint is only checked when this holds.
    pub when: Condition,
    /// Invariant — must hold after `when` passes.  Violation triggers a result.
    pub require: Condition,
    /// Remediation instruction shown when the constraint is violated.
    pub fix: String,
    /// Graph edges to ADR records that provide supporting evidence.
    pub evidence: Vec<String>,
    /// How serious a violation of this constraint is.
    pub severity: Severity,
}

// ---------------------------------------------------------------------------
// AdrStatus
// ---------------------------------------------------------------------------

/// Lifecycle status of an ADR record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AdrStatus {
    /// Under discussion, not yet binding.
    Proposed,
    /// Ratified and binding.
    Accepted,
    /// Replaced by a newer ADR.
    Superseded,
}

// ---------------------------------------------------------------------------
// ADR
// ---------------------------------------------------------------------------

/// Architecture Decision Record stored in PluresDB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adr {
    /// Stable identifier in `ADR-NNNN` format (e.g. `"ADR-0004"`).
    pub id: String,
    /// Short title.
    pub title: String,
    /// Current lifecycle status.
    pub status: AdrStatus,
    /// Graph edges to [`Evidence`] records that validate this decision.
    pub evidence: Vec<String>,
}

// ---------------------------------------------------------------------------
// EvidenceResult
// ---------------------------------------------------------------------------

/// Outcome of an evidence test run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceResult {
    /// The check passed.
    Passed,
    /// The check failed.
    Failed,
    /// The check has not been run yet or the result is indeterminate.
    Unknown,
}

// ---------------------------------------------------------------------------
// Evidence
// ---------------------------------------------------------------------------

/// A structured evidence record linked to an [`Adr`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Unique identifier.
    pub id: String,
    /// When this evidence was collected.
    pub tested_at: DateTime<Utc>,
    /// The runtime condition snapshot recorded at test time.
    pub condition: HashMap<String, serde_json::Value>,
    /// Outcome of the test.
    pub result: EvidenceResult,
    /// Canonical URL for the test run (issue, PR, CI run, etc.).
    pub reference: String,
}

// ---------------------------------------------------------------------------
// SessionType
// ---------------------------------------------------------------------------

/// The kind of agent session that produced an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionType {
    /// Top-level orchestrator session.
    Main,
    /// Group-scoped session.
    Group,
    /// Delegated sub-agent session.
    SubAgent,
}

// ---------------------------------------------------------------------------
// AgentContext
// ---------------------------------------------------------------------------

/// Runtime context passed to every praxis procedure call.
///
/// This is the primary input to the `evaluate` procedure and is used to match
/// constraints against the current action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    /// The type of action being evaluated (e.g. `"write_file"`, `"spawn_agent"`).
    pub action_type: String,
    /// The resource or entity the action targets.
    pub target: String,
    /// Arbitrary key/value metadata provided by the orchestration layer.
    pub metadata: HashMap<String, serde_json::Value>,
    /// The session context from which this action was initiated.
    pub session_type: SessionType,
}

impl AgentContext {
    /// Convenience constructor with an empty metadata map.
    pub fn new(
        action_type: impl Into<String>,
        target: impl Into<String>,
        session_type: SessionType,
    ) -> Self {
        Self {
            action_type: action_type.into(),
            target: target.into(),
            metadata: HashMap::new(),
            session_type,
        }
    }

    /// Builder-style setter for a metadata field.
    pub fn with_meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(action: &str, meta: HashMap<String, serde_json::Value>) -> AgentContext {
        AgentContext {
            action_type: action.into(),
            target: "test-target".into(),
            metadata: meta,
            session_type: SessionType::Main,
        }
    }

    #[test]
    fn condition_always_is_true() {
        let c = Condition::Always;
        assert!(c.evaluate(&ctx("anything", HashMap::new())));
    }

    #[test]
    fn condition_field_eq() {
        let c = Condition::FieldEq {
            field: "env".into(),
            value: json!("prod"),
        };
        let mut meta = HashMap::new();
        meta.insert("env".into(), json!("prod"));
        assert!(c.evaluate(&ctx("deploy", meta.clone())));
        meta.insert("env".into(), json!("staging"));
        assert!(!c.evaluate(&ctx("deploy", meta)));
    }

    #[test]
    fn condition_action_type_eq() {
        let c = Condition::ActionTypeEq {
            value: "write_file".into(),
        };
        assert!(c.evaluate(&ctx("write_file", HashMap::new())));
        assert!(!c.evaluate(&ctx("read_file", HashMap::new())));
    }

    #[test]
    fn condition_field_exists() {
        let c = Condition::FieldExists {
            field: "resource_owner".into(),
        };
        let mut meta = HashMap::new();
        meta.insert("resource_owner".into(), json!("team-1"));
        assert!(c.evaluate(&ctx("write_file", meta)));
        assert!(!c.evaluate(&ctx("write_file", HashMap::new())));
    }

    #[test]
    fn condition_field_gt() {
        let c = Condition::FieldGt {
            field: "privilege_level".into(),
            threshold: 2.0,
        };
        let meta_high = [("privilege_level".into(), json!(3))].into_iter().collect();
        let meta_low = [("privilege_level".into(), json!(1))].into_iter().collect();
        assert!(c.evaluate(&ctx("admin", meta_high)));
        assert!(!c.evaluate(&ctx("admin", meta_low)));
    }

    #[test]
    fn condition_action_starts_with() {
        let c = Condition::ActionStartsWith {
            prefix: "write_".into(),
        };
        assert!(c.evaluate(&ctx("write_file", HashMap::new())));
        assert!(!c.evaluate(&ctx("read_file", HashMap::new())));
    }

    #[test]
    fn condition_not() {
        let c = Condition::Not {
            condition: Box::new(Condition::Always),
        };
        assert!(!c.evaluate(&ctx("x", HashMap::new())));
    }

    #[test]
    fn condition_all_requires_all() {
        let c = Condition::All {
            conditions: vec![
                Condition::Always,
                Condition::ActionStartsWith {
                    prefix: "write_".into(),
                },
            ],
        };
        assert!(c.evaluate(&ctx("write_file", HashMap::new())));
        assert!(!c.evaluate(&ctx("read_file", HashMap::new())));
    }

    #[test]
    fn condition_any_requires_one() {
        let c = Condition::Any {
            conditions: vec![
                Condition::ActionStartsWith {
                    prefix: "write_".into(),
                },
                Condition::ActionStartsWith {
                    prefix: "delete_".into(),
                },
            ],
        };
        assert!(c.evaluate(&ctx("write_file", HashMap::new())));
        assert!(c.evaluate(&ctx("delete_file", HashMap::new())));
        assert!(!c.evaluate(&ctx("read_file", HashMap::new())));
    }

    #[test]
    fn condition_serde_roundtrip() {
        let c = Condition::All {
            conditions: vec![
                Condition::FieldEq {
                    field: "x".into(),
                    value: json!(1),
                },
                Condition::Not {
                    condition: Box::new(Condition::Always),
                },
            ],
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Condition = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn agent_context_with_meta_builder() {
        let ctx = AgentContext::new("write_file", "config.toml", SessionType::Main)
            .with_meta("resource_owner", json!("user-1"))
            .with_meta("privilege_level", json!(3));
        assert_eq!(ctx.metadata["resource_owner"], json!("user-1"));
        assert_eq!(ctx.metadata["privilege_level"], json!(3));
    }
}
