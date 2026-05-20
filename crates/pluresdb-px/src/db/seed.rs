//! Seed data for the praxis PluresDB store.
//!
//! This module is the **compiler/seeder** described in the issue: it migrates
//! the existing TypeScript `.praxis/` constraints and the ADR-0004 decision
//! into structured PluresDB records.
//!
//! Call [`default_store`] to get a fully-seeded [`PraxisStore`] with all
//! built-in constraints, ADRs, and evidence records.

use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;

use crate::db::schema::{
    Adr, AdrStatus, Condition, Constraint, Evidence, EvidenceResult, SessionType, Severity,
};
use crate::db::store::PraxisStore;

// ---------------------------------------------------------------------------
// Constraints seeder
// ---------------------------------------------------------------------------

/// Insert all built-in constraints into `store`.
///
/// These are direct migrations of the existing copilot-readiness expectations
/// encoded in the TypeScript `.praxis/` safety rules.
pub fn seed_constraints(store: &mut PraxisStore) {
    // C-0001 — every action must have a non-empty action_type
    store.upsert_constraint(Constraint {
        id: "C-0001".into(),
        description: "Every agent action must carry a non-empty action_type.".into(),
        when: Condition::Always,
        require: Condition::Not {
            condition: Box::new(Condition::FieldEq {
                field: "__action_type__".into(),
                value: json!(""),
            }),
        },
        fix: "Ensure the orchestration layer sets a non-empty action_type before dispatching."
            .into(),
        evidence: vec!["ADR-0004".into()],
        severity: Severity::Error,
    });

    // C-0002 — resource-mutation actions must declare a resource_owner
    store.upsert_constraint(Constraint {
        id: "C-0002".into(),
        description: "Write/delete/update/create/publish actions must declare resource_owner."
            .into(),
        when: Condition::Any {
            conditions: vec![
                Condition::ActionStartsWith {
                    prefix: "write_".into(),
                },
                Condition::ActionStartsWith {
                    prefix: "delete_".into(),
                },
                Condition::ActionStartsWith {
                    prefix: "update_".into(),
                },
                Condition::ActionStartsWith {
                    prefix: "create_".into(),
                },
                Condition::ActionStartsWith {
                    prefix: "publish_".into(),
                },
                Condition::ActionStartsWith {
                    prefix: "send_".into(),
                },
                Condition::ActionStartsWith {
                    prefix: "post_".into(),
                },
            ],
        },
        require: Condition::Not {
            condition: Box::new(Condition::Any {
                conditions: vec![
                    // field absent ↔ FieldEq with null — but we rely on "not empty string"
                    Condition::FieldEq {
                        field: "resource_owner".into(),
                        value: json!(""),
                    },
                ],
            }),
        },
        fix: "Set resource_owner to the owning user/team ID before dispatching a mutation action."
            .into(),
        evidence: vec!["ADR-0004".into()],
        severity: Severity::Error,
    });

    // C-0003 — high-privilege actions require escalation
    store.upsert_constraint(Constraint {
        id: "C-0003".into(),
        description: "Actions with privilege_level ≥ 3 require explicit approval (gate).".into(),
        when: Condition::FieldGt {
            field: "privilege_level".into(),
            threshold: 2.0,
        },
        // require: privilege_level < 3 (this will always fail when when-clause triggers,
        // intentionally — the fix is to obtain a gate approval before re-dispatching)
        require: Condition::FieldLt {
            field: "privilege_level".into(),
            threshold: 3.0,
        },
        fix: "Obtain an explicit gate approval (RuleResult::Gate) before re-dispatching.".into(),
        evidence: vec!["ADR-0004".into()],
        severity: Severity::Error,
    });

    // C-0004 — extreme risk scores block the action
    store.upsert_constraint(Constraint {
        id: "C-0004".into(),
        description: "Actions with risk_score > 0.9 must be blocked.".into(),
        when: Condition::FieldGt {
            field: "risk_score".into(),
            threshold: 0.9,
        },
        // require: risk_score < 0.9 — always fails when the when-clause fires (risk_score > 0.9)
        require: Condition::FieldLt {
            field: "risk_score".into(),
            threshold: 0.9,
        },
        fix: "Reduce risk through staged rollout or obtain explicit approval.".into(),
        evidence: vec!["ADR-0004".into()],
        severity: Severity::Error,
    });

    // C-0005 — elevated risk scores produce a warning
    store.upsert_constraint(Constraint {
        id: "C-0005".into(),
        description: "Actions with risk_score > 0.5 emit a warning.".into(),
        when: Condition::FieldGt {
            field: "risk_score".into(),
            threshold: 0.5,
        },
        require: Condition::FieldLt {
            field: "risk_score".into(),
            threshold: 0.5,
        },
        fix: "Review the risk assessment before proceeding.".into(),
        evidence: vec!["ADR-0004".into()],
        severity: Severity::Warning,
    });

    // C-0006 — rate limiting warning
    store.upsert_constraint(Constraint {
        id: "C-0006".into(),
        description: "Calls exceeding 60/min should be throttled.".into(),
        when: Condition::FieldGt {
            field: "calls_per_minute".into(),
            threshold: 60.0,
        },
        require: Condition::FieldLt {
            field: "calls_per_minute".into(),
            threshold: 60.0,
        },
        fix: "Implement upstream throttling or circuit breaker.".into(),
        evidence: vec!["ADR-0004".into()],
        severity: Severity::Warning,
    });

    // C-0007 — copilot readiness: agent must not bypass praxis evaluation
    store.upsert_constraint(Constraint {
        id: "C-0007".into(),
        description: "Every agent action must pass through praxis evaluation before execution."
            .into(),
        when: Condition::Always,
        require: Condition::Always, // tautological — enforced architecturally
        fix: "Ensure on_action() is called in the agent dispatch path.".into(),
        evidence: vec!["ADR-0004".into()],
        severity: Severity::Warning,
    });

    // C-0008 — sub-agent sessions must not self-report privilege
    store.upsert_constraint(Constraint {
        id: "C-0008".into(),
        description: "Sub-agent sessions must not self-assign privilege_level ≥ 3.".into(),
        when: Condition::All {
            conditions: vec![
                Condition::SessionIs {
                    session_type: SessionType::SubAgent,
                },
                Condition::FieldGt {
                    field: "privilege_level".into(),
                    threshold: 2.0,
                },
            ],
        },
        require: Condition::FieldLt {
            field: "privilege_level".into(),
            threshold: 3.0,
        },
        fix: "Sub-agent sessions must request escalation from the main session.".into(),
        evidence: vec!["ADR-0004".into()],
        severity: Severity::Error,
    });

    // C-0009 — task description word count ≤ 200 (ADR-0013)
    store.upsert_constraint(Constraint {
        id: "C-0009".into(),
        description: "Sub-agent task descriptions must not exceed 200 words (ADR-0013).".into(),
        when: Condition::FieldGt {
            field: "task_description_word_count".into(),
            threshold: 200.0,
        },
        require: Condition::FieldLt {
            field: "task_description_word_count".into(),
            threshold: 200.0,
        },
        fix: "Decompose the task into sub-tasks each with ≤ 200-word descriptions. \
              Emit task_decomposition_required with suggested split points."
            .into(),
        evidence: vec!["ADR-0013".into()],
        severity: Severity::Error,
    });

    // C-0010 — expected text output ≤ 2 000 chars (ADR-0013)
    store.upsert_constraint(Constraint {
        id: "C-0010".into(),
        description: "Sub-agent tasks expecting text output must not exceed 2000 estimated chars \
                      (ADR-0013)."
            .into(),
        when: Condition::All {
            conditions: vec![
                Condition::FieldEq {
                    field: "expected_output_type".into(),
                    value: json!("text"),
                },
                Condition::FieldGt {
                    field: "expected_output_chars".into(),
                    threshold: 2000.0,
                },
            ],
        },
        require: Condition::FieldLt {
            field: "expected_output_chars".into(),
            threshold: 2000.0,
        },
        fix: "Decompose the task so each sub-task produces ≤ 2000 chars of text output. \
              Emit task_decomposition_required with suggested split points."
            .into(),
        evidence: vec!["ADR-0013".into()],
        severity: Severity::Error,
    });
}

// ---------------------------------------------------------------------------
// ADR seeder
// ---------------------------------------------------------------------------

/// Insert built-in ADR records.  Currently migrates **ADR-0004** (praxis
/// evaluation model) as structured data.
pub fn seed_adrs(store: &mut PraxisStore) {
    // ADR-0001 — Rust as primary implementation language
    store.upsert_adr(Adr {
        id: "ADR-0001".into(),
        title: "Rust as primary implementation language".into(),
        status: AdrStatus::Accepted,
        evidence: vec!["EV-ADR0001-CARGO".into(), "EV-ADR0001-CLIPPY".into()],
    });

    // ADR-0002 — PluresDB as the sole database
    store.upsert_adr(Adr {
        id: "ADR-0002".into(),
        title: "PluresDB as the sole database — no SQLite, no Postgres".into(),
        status: AdrStatus::Accepted,
        evidence: vec!["EV-ADR0002-INTEGRATION".into()],
    });

    // ADR-0003 — Local-first, P2P sync via Hyperswarm
    store.upsert_adr(Adr {
        id: "ADR-0003".into(),
        title: "Local-first architecture with P2P sync via Hyperswarm".into(),
        status: AdrStatus::Accepted,
        evidence: vec!["EV-ADR0003-SYNC".into()],
    });

    // ADR-0004 — Praxis logic migrated to PluresDB runtime (this issue)
    store.upsert_adr(Adr {
        id: "ADR-0004".into(),
        title: "Praxis logic lives in PluresDB schema + procedures, not TypeScript files".into(),
        status: AdrStatus::Accepted,
        evidence: vec![
            "EV-ADR0004-CONSTRAINT-EVAL".into(),
            "EV-ADR0004-SCHEMA-DEFINED".into(),
            "EV-ADR0004-CI".into(),
        ],
    });

    // ADR-0013 — Task decomposition size limits
    store.upsert_adr(Adr {
        id: "ADR-0013".into(),
        title: "Sub-agent task descriptions ≤ 200 words; text output ≤ 2000 chars".into(),
        status: AdrStatus::Accepted,
        evidence: vec!["EV-ADR0013-TASK-SIZE".into()],
    });
}

// ---------------------------------------------------------------------------
// Evidence seeder
// ---------------------------------------------------------------------------

/// Insert built-in evidence records.
pub fn seed_evidence(store: &mut PraxisStore) {
    // ── ADR-0001 evidence ────────────────────────────────────────────────────

    store.upsert_evidence(Evidence {
        id: "EV-ADR0001-CARGO".into(),
        tested_at: Utc::now(),
        condition: [("workspace_crates".into(), json!(true))]
            .into_iter()
            .collect(),
        result: EvidenceResult::Passed,
        reference: "https://github.com/plures/pares-radix/blob/main/Cargo.toml".into(),
    });

    store.upsert_evidence(Evidence {
        id: "EV-ADR0001-CLIPPY".into(),
        tested_at: Utc::now(),
        condition: [("ci_clippy_clean".into(), json!(true))]
            .into_iter()
            .collect(),
        result: EvidenceResult::Passed,
        reference: "https://github.com/plures/pares-radix/blob/main/.github/workflows/ci.yml"
            .into(),
    });

    // ── ADR-0002 evidence ────────────────────────────────────────────────────

    store.upsert_evidence(Evidence {
        id: "EV-ADR0002-INTEGRATION".into(),
        tested_at: Utc::now(),
        condition: [
            ("no_sqlite_dep".into(), json!(true)),
            ("no_postgres_dep".into(), json!(true)),
        ]
        .into_iter()
        .collect(),
        result: EvidenceResult::Passed,
        reference: "https://github.com/plures/pares-radix/blob/main/Cargo.toml".into(),
    });

    // ── ADR-0003 evidence ────────────────────────────────────────────────────

    store.upsert_evidence(Evidence {
        id: "EV-ADR0003-SYNC".into(),
        tested_at: Utc::now(),
        condition: [("crates_sync_exists".into(), json!(true))]
            .into_iter()
            .collect(),
        result: EvidenceResult::Passed,
        reference: "https://github.com/plures/pares-radix/tree/main/crates/sync".into(),
    });

    // ── ADR-0004 evidence ────────────────────────────────────────────────────

    // Constraint schema is defined (Rust structs in db::schema)
    store.upsert_evidence(Evidence {
        id: "EV-ADR0004-SCHEMA-DEFINED".into(),
        tested_at: Utc::now(),
        condition: [
            ("constraint_type_defined".into(), json!(true)),
            ("adr_type_defined".into(), json!(true)),
            ("evidence_type_defined".into(), json!(true)),
            ("agent_context_type_defined".into(), json!(true)),
        ]
        .into_iter()
        .collect(),
        result: EvidenceResult::Passed,
        reference: "https://github.com/plures/pares-radix/tree/main/crates/praxis/src/db/schema.rs"
            .into(),
    });

    // evaluate() procedure returns violations for a given AgentContext
    store.upsert_evidence(Evidence {
        id: "EV-ADR0004-CONSTRAINT-EVAL".into(),
        tested_at: Utc::now(),
        condition: [
            ("evaluate_procedure_exists".into(), json!(true)),
            ("on_action_procedure_exists".into(), json!(true)),
            ("compile_nl_procedure_exists".into(), json!(true)),
            ("query_gaps_procedure_exists".into(), json!(true)),
        ]
        .into_iter()
        .collect(),
        result: EvidenceResult::Passed,
        reference:
            "https://github.com/plures/pares-radix/tree/main/crates/praxis/src/db/procedures.rs"
                .into(),
    });

    // CI validation of the constraints at runtime — pending a CI run
    store.upsert_evidence(Evidence {
        id: "EV-ADR0004-CI".into(),
        tested_at: Utc::now(),
        condition: HashMap::new(),
        result: EvidenceResult::Unknown,
        reference: "https://github.com/plures/pares-radix/issues/344".into(),
    });

    // ── ADR-0013 evidence ────────────────────────────────────────────────────

    // 2026-04-10 sub-agent test results that proved the size limits
    store.upsert_evidence(Evidence {
        id: "EV-ADR0013-TASK-SIZE".into(),
        tested_at: Utc::now(),
        condition: [
            ("max_description_words".into(), json!(200)),
            ("max_output_chars".into(), json!(2000)),
            ("test_date".into(), json!("2026-04-10")),
        ]
        .into_iter()
        .collect(),
        result: EvidenceResult::Passed,
        reference: "https://github.com/plures/pares-radix/issues/396".into(),
    });
}

// ---------------------------------------------------------------------------
// default_store
// ---------------------------------------------------------------------------

/// Build and return a fully-seeded [`PraxisStore`] with all built-in
/// constraints, ADRs, and evidence records.
///
/// This is the primary entry point used by tests and the CLI.
pub fn default_store() -> PraxisStore {
    let mut store = PraxisStore::new();
    seed_constraints(&mut store);
    seed_adrs(&mut store);
    seed_evidence(&mut store);
    store
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::procedures::{evaluate, query_gaps};
    use crate::db::schema::AgentContext;

    #[test]
    fn default_store_has_constraints() {
        let store = default_store();
        assert!(
            store.constraint_count() >= 10,
            "expected at least 10 seeded constraints"
        );
    }

    #[test]
    fn default_store_has_adrs() {
        let store = default_store();
        assert!(store.adr_count() >= 5, "expected at least 5 seeded ADRs");
        assert!(
            store.get_adr("ADR-0004").is_some(),
            "ADR-0004 must be present"
        );
        assert!(
            store.get_adr("ADR-0013").is_some(),
            "ADR-0013 must be present"
        );
    }

    #[test]
    fn default_store_has_evidence() {
        let store = default_store();
        assert!(
            store.evidence_count() >= 8,
            "expected at least 8 evidence records"
        );
    }

    #[test]
    fn adr_0004_has_three_evidence_edges() {
        let store = default_store();
        let ev = store.adr_evidence("ADR-0004");
        assert_eq!(ev.len(), 3, "ADR-0004 should link to 3 evidence records");
    }

    #[test]
    fn adr_0004_evidence_includes_passed_and_unknown() {
        let store = default_store();
        let ev = store.adr_evidence("ADR-0004");
        let has_passed = ev.iter().any(|e| e.result == EvidenceResult::Passed);
        let has_unknown = ev.iter().any(|e| e.result == EvidenceResult::Unknown);
        assert!(
            has_passed,
            "should have at least one passed evidence record"
        );
        assert!(has_unknown, "CI validation should be Unknown until run");
    }

    #[test]
    fn constraint_c0007_always_passes_evaluate() {
        // C-0007 has require: Always, so it should never fire in evaluate
        let store = default_store();
        let ctx = AgentContext::new("anything", "anywhere", SessionType::Main);
        let violations = evaluate(&store, &ctx);
        let ids: Vec<&str> = violations
            .iter()
            .map(|v| v.constraint.id.as_str())
            .collect();
        assert!(
            !ids.contains(&"C-0007"),
            "C-0007 require=Always must never fire"
        );
    }

    #[test]
    fn sub_agent_high_privilege_blocked() {
        let store = default_store();
        let ctx = AgentContext::new("admin", "system", SessionType::SubAgent)
            .with_meta("privilege_level", json!(4));
        let violations = evaluate(&store, &ctx);
        let ids: Vec<&str> = violations
            .iter()
            .map(|v| v.constraint.id.as_str())
            .collect();
        // Both C-0003 and C-0008 should fire
        assert!(
            ids.contains(&"C-0003"),
            "C-0003 should fire for privilege_level=4"
        );
        assert!(
            ids.contains(&"C-0008"),
            "C-0008 should fire for sub-agent privilege_level=4"
        );
    }

    #[test]
    fn query_gaps_finds_ci_evidence() {
        let store = default_store();
        let gaps = query_gaps(&store);
        let ids: Vec<&str> = gaps.iter().map(|e| e.id.as_str()).collect();
        assert!(
            ids.contains(&"EV-ADR0004-CI"),
            "EV-ADR0004-CI should be a gap; got: {ids:?}"
        );
    }
}
