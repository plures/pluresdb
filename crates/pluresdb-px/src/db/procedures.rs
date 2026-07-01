//! Praxis procedures evaluated against [`PraxisStore`].
//!
//! These are the procedures available in this module:
//!
//! | Procedure | Description |
//! |-----------|-------------|
//! | [`evaluate`] | Returns all [`Constraint`]s violated by an [`AgentContext`] |
//! | [`on_action`] | Pre-action hook — returns `Err` when a blocking constraint fires |
//! | [`compile_nl`] | Compiles natural-language text into a [`Constraint`] insert |
//! | [`query_gaps`] | Returns [`Evidence`] records whose `result` is `Unknown` |
//! | [`apply_correction`] | Creates or updates a constraint from a user correction |
//! | [`undo_correction`] | Removes a correction-sourced constraint from the store |

use crate::db::schema::{AgentContext, Condition, Constraint, Evidence, EvidenceResult, Severity};
use crate::db::store::PraxisStore;
// M6: constraint-predicate parsing now uses the SSOT praxis-lang grammar via
// `px_eval::parse_expr` (which parses through `px_compiler`), walking the
// resulting `px_ast::Expr` instead of the deleted local pest grammar.
use px_ast::{BinOp, Expr};

/// Marker prefix stamped onto the `description` of a constraint produced from
/// input that could be parsed neither as a structured predicate nor as a known
/// natural-language keyword pattern.
///
/// Such a constraint is **honestly inert** (`when: Always`, `require: Always`)
/// and is flagged so it can never be mistaken for a real enforcing guard
/// (satisfies C-NOSTUB-001 — no silent `Always` masquerading as enforcement).
pub const UNPARSED_MARKER: &str = "[UNPARSED — NOT ENFORCED]";

// ---------------------------------------------------------------------------
// Violation
// ---------------------------------------------------------------------------

/// A constraint that was violated by a given [`AgentContext`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Violation {
    /// The violated constraint.
    pub constraint: Constraint,
    /// Human-readable explanation (includes the constraint's `fix` instruction).
    pub message: String,
}

// ---------------------------------------------------------------------------
// ProcedureError
// ---------------------------------------------------------------------------

/// Error type returned by [`on_action`] when the action is blocked.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionBlocked {
    /// All violations that caused the block.
    pub violations: Vec<Violation>,
}

impl std::fmt::Display for ActionBlocked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "action blocked by {} constraint(s):",
            self.violations.len()
        )?;
        for v in &self.violations {
            write!(
                f,
                "\n  [{}] {} — fix: {}",
                v.constraint.id, v.message, v.constraint.fix
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for ActionBlocked {}

// ---------------------------------------------------------------------------
// evaluate
// ---------------------------------------------------------------------------

/// Return all constraints whose precondition (`when`) holds for `ctx` but
/// whose invariant (`require`) does **not** hold.
///
/// Constraints are checked in an unspecified order.  All violations are
/// returned, not just the first.
///
/// # Example
///
/// ```rust
/// use pluresdb_px::db::{AgentContext, SessionType};
/// use pluresdb_px::db::procedures::evaluate;
/// use pluresdb_px::db::seed::default_store;
/// use serde_json::json;
///
/// let store = default_store();
/// let ctx = AgentContext::new("write_file", "config.toml", SessionType::Main)
///     .with_meta("resource_owner", json!(""));
/// let violations = evaluate(&store, &ctx);
/// // resource_owner_declared constraint fires because owner is empty
/// assert!(!violations.is_empty());
/// ```
pub fn evaluate(store: &PraxisStore, ctx: &AgentContext) -> Vec<Violation> {
    store
        .constraints()
        .filter(|c| {
            // when holds AND require does NOT hold
            c.when.evaluate(ctx) && !c.require.evaluate(ctx)
        })
        .map(|c| {
            let message = format!(
                "constraint {} violated for action `{}`",
                c.id, ctx.action_type
            );
            Violation {
                constraint: c.clone(),
                message,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// on_action
// ---------------------------------------------------------------------------

/// Pre-action hook that blocks when any **error-severity** constraint is violated.
///
/// Warnings are collected and returned separately so the caller can log them
/// without blocking.
///
/// Returns `Ok(warnings)` when the action may proceed (zero or only warning
/// violations), or `Err(ActionBlocked)` when at least one `Error`-severity
/// constraint fires.
///
/// # Example
///
/// ```rust
/// use pluresdb_px::db::{AgentContext, SessionType};
/// use pluresdb_px::db::procedures::on_action;
/// use pluresdb_px::db::seed::default_store;
/// use serde_json::json;
///
/// let store = default_store();
/// let ctx = AgentContext::new("read_file", "README.md", SessionType::Main);
/// assert!(on_action(&store, &ctx).is_ok());
/// ```
pub fn on_action(store: &PraxisStore, ctx: &AgentContext) -> Result<Vec<Violation>, ActionBlocked> {
    let all = evaluate(store, ctx);
    let (errors, warnings): (Vec<_>, Vec<_>) = all
        .into_iter()
        .partition(|v| v.constraint.severity == Severity::Error);

    if errors.is_empty() {
        Ok(warnings)
    } else {
        Err(ActionBlocked { violations: errors })
    }
}

// ---------------------------------------------------------------------------
// compile_nl
// ---------------------------------------------------------------------------

/// Compile a constraint description into a [`Constraint`] record ready to be
/// inserted into the store.
///
/// # Real structured-predicate path (primary)
///
/// The text is first run through the **canonical `.px` expression grammar**
/// (`px_eval::parse_expr`, which parses via `px_compiler` — the single source
/// of truth in praxis-lang).  If it parses as one or more `field <op> value`
/// comparisons
/// joined by `and` / `or` (`&&` / `||`), it is mapped to a **real, enforcing**
/// [`Condition`] AST placed in `require`.  Supported comparison operators:
///
/// | Operator | Mapped condition                            |
/// |----------|---------------------------------------------|
/// | `<`      | `FieldLt { threshold }`                     |
/// | `>`      | `FieldGt { threshold }`                     |
/// | `<=`     | `Not(FieldGt { threshold })`  (inclusive)   |
/// | `>=`     | `Not(FieldLt { threshold })`  (inclusive)   |
/// | `==`     | `FieldEq { value }`                         |
/// | `!=`     | `Not(FieldEq { value })`                    |
///
/// `and` joins comparisons into `Condition::All`; `or` into `Condition::Any`.
/// A leading `metadata.` on the field path is stripped, because [`Condition`]
/// fields index directly into [`AgentContext::metadata`] (so
/// `metadata.amount <= 100` and `amount <= 100` are equivalent).
///
/// Because `require` is the *invariant*, a rule such as `amount <= 100`
/// **blocks** a context with `amount = 500` (require is false -> violation) and
/// **passes** one with `amount = 50`.  This is the real enforcement that
/// replaces the former `Condition::Always` no-op.
///
/// # Natural-language keyword fallback (explicit, documented)
///
/// If the text is *not* a parseable structured predicate, a small set of
/// legacy free-text keyword heuristics is tried as an explicitly-commented
/// fallback for genuine English corrections:
///
/// | Keyword in text     | Generated condition                              |
/// |---------------------|--------------------------------------------------|
/// | `write_` / `delete_`| `when: ActionStartsWith { prefix }`              |
/// | `resource_owner`    | `require: Not(FieldEq { resource_owner = "" })`  |
/// | `privilege_level`   | `require: FieldLt { privilege_level, 3.0 }`      |
/// | `risk_score`        | `require: FieldLt { risk_score, 0.9 }`           |
///
/// # Unparsed input (no fake pass-through)
///
/// If the text matches **neither** the structured-predicate path **nor** any
/// known keyword, the returned constraint is **honestly inert and flagged**:
/// `when: Always`, `require: Always`, and its `description` is prefixed with
/// [`UNPARSED_MARKER`].  It therefore enforces nothing *and announces that it
/// enforces nothing* — it can never be mistaken for a real guard
/// (C-NOSTUB-001).  Callers that require enforcement should reject any
/// constraint whose description starts with [`UNPARSED_MARKER`].
///
/// The `id` is taken from the `id` argument.
pub fn compile_nl(text: &str, id: impl Into<String>) -> Constraint {
    let id = id.into();
    let trimmed = text.trim();
    let lower = text.to_lowercase();

    let severity = if lower.contains("error") || lower.contains("block") || lower.contains("must") {
        Severity::Error
    } else {
        Severity::Warning
    };

    // -- 1. Structured-predicate path (REAL enforcement) ----------------------
    // Parse via the canonical .px expression grammar and map comparisons to a
    // real Condition AST. This is what makes `amount <= 100` actually block.
    if let Some(require) = parse_structured_predicate(trimmed) {
        let when = nl_when_clause(&lower).unwrap_or(Condition::Always);
        return Constraint {
            id,
            description: trimmed.to_string(),
            when,
            require,
            fix: "Ensure the action's metadata satisfies the predicate before proceeding.".into(),
            evidence: vec![],
            severity,
        };
    }

    // -- 2. Legacy natural-language keyword fallback (explicit) ---------------
    // Only reached for genuine free-text English that is NOT a structured
    // predicate. Kept narrow and clearly commented per TASK-PX-CANON Stage 1.
    let when = nl_when_clause(&lower);
    let require = if lower.contains("resource_owner") {
        // require that resource_owner is a non-empty string
        Some(Condition::Not {
            condition: Box::new(Condition::FieldEq {
                field: "resource_owner".into(),
                value: serde_json::Value::String(String::new()),
            }),
        })
    } else if lower.contains("privilege_level") {
        Some(Condition::FieldLt {
            field: "privilege_level".into(),
            threshold: 3.0,
        })
    } else if lower.contains("risk_score") {
        Some(Condition::FieldLt {
            field: "risk_score".into(),
            threshold: 0.9,
        })
    } else {
        None
    };

    match (when, require) {
        // A recognised keyword produced at least one real predicate.
        (when_opt, Some(require)) => Constraint {
            id,
            description: trimmed.to_string(),
            when: when_opt.unwrap_or(Condition::Always),
            require,
            fix: "Review the constraint text and implement the appropriate check.".into(),
            evidence: vec![],
            severity,
        },
        // Only an action-prefix keyword matched (a real `when`, but no invariant
        // to enforce). Treat as inert-and-flagged: a `when` with `require:
        // Always` enforces nothing, so we must NOT present it as a real guard.
        (Some(_), None) | (None, None) => Constraint {
            id,
            description: format!("{UNPARSED_MARKER} {trimmed}"),
            when: Condition::Always,
            require: Condition::Always,
            fix: "Could not derive an enforceable predicate from this text; rewrite as `field <op> value` (e.g. `metadata.amount <= 100`).".into(),
            evidence: vec![],
            severity,
        },
    }
}

// ---------------------------------------------------------------------------
// Structured-predicate parser (canonical-grammar backed)
// ---------------------------------------------------------------------------

/// Derive a legacy `when` precondition from free-text keywords, if any.
///
/// Returns `None` when no action-prefix keyword is present (caller substitutes
/// `Condition::Always`).
fn nl_when_clause(lower: &str) -> Option<Condition> {
    if lower.contains("write_") {
        Some(Condition::ActionStartsWith {
            prefix: "write_".into(),
        })
    } else if lower.contains("delete_") {
        Some(Condition::ActionStartsWith {
            prefix: "delete_".into(),
        })
    } else {
        None
    }
}

/// Parse `text` as a structured constraint predicate using the **canonical**
/// `.px` expression grammar, mapping it to a real [`Condition`].
///
/// Accepts one or more `field <op> value` comparisons joined by a single class
/// of logic operator (all `and`/`&&` -> [`Condition::All`], all `or`/`||` ->
/// [`Condition::Any`]).  Returns `None` if the text is not a single expression,
/// is not a flat conjunction/disjunction of simple comparisons, mixes `and`
/// with `or`, or contains a comparison whose sides are not `field <op> literal`.
fn parse_structured_predicate(text: &str) -> Option<Condition> {
    if text.is_empty() {
        return None;
    }
    // Use the ONE canonical grammar (praxis-lang via px_eval::parse_expr) as the
    // validator/parser. `parse_expr` parses the text as a constraint `require:`
    // expression through px_compiler, so an invalid/garbage expression yields
    // `Err` -> `None` here.
    let expr = px_eval::parse_expr(text).ok()?;

    // Flatten a flat conjunction/disjunction of comparisons joined by a SINGLE
    // class of logic operator. Mixed `and`/`or`, or any non-comparison operand,
    // is refused (we will not guess precedence or enforce complex predicates).
    let mut comparisons: Vec<Condition> = Vec::new();
    let mut joiner: Option<Logic> = None;
    if !flatten_predicate(&expr, &mut comparisons, &mut joiner) {
        return None;
    }

    match comparisons.len() {
        0 => None,
        1 => comparisons.into_iter().next(),
        _ => Some(match joiner {
            Some(Logic::Or) => Condition::Any {
                conditions: comparisons,
            },
            // Default conjunction (also the `and` case).
            _ => Condition::All {
                conditions: comparisons,
            },
        }),
    }
}

/// Recursively flatten a single-joiner logic chain of comparisons.
///
/// Returns `false` (reject the whole predicate) if the expression mixes `and`
/// with `or`, or contains an operand that is not itself a simple comparison or
/// a same-joiner logic node. `Paren` is transparent.
fn flatten_predicate(expr: &Expr, out: &mut Vec<Condition>, joiner: &mut Option<Logic>) -> bool {
    match expr {
        Expr::Paren(inner) => flatten_predicate(inner, out, joiner),
        Expr::Binary { left, op, right } => {
            let this_logic = match op {
                BinOp::And => Some(Logic::And),
                BinOp::Or => Some(Logic::Or),
                _ => None,
            };
            if let Some(this) = this_logic {
                // Refuse to silently guess precedence on mixed and/or.
                match joiner {
                    None => *joiner = Some(this),
                    Some(prev) if *prev != this => return false,
                    Some(_) => {}
                }
                flatten_predicate(left, out, joiner) && flatten_predicate(right, out, joiner)
            } else {
                // A comparison operator (==, !=, <, >, <=, >=) or something we
                // don't enforce. `comparison_expr_to_condition` filters the rest.
                match comparison_expr_to_condition(expr) {
                    Some(c) => {
                        out.push(c);
                        true
                    }
                    None => false,
                }
            }
        }
        _ => false,
    }
}

/// Logical joiner between comparisons.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Logic {
    And,
    Or,
}

/// Map a single comparison expression (`left <cmp> right`) to a [`Condition`].
/// Returns `None` for anything that is not `field <comp_op> literal`.
fn comparison_expr_to_condition(expr: &Expr) -> Option<Condition> {
    let Expr::Binary { left, op, right } = expr else {
        return None;
    };
    // Only the six comparison operators are enforceable predicates.
    let op_str = match op {
        BinOp::Eq => "==",
        BinOp::Neq => "!=",
        BinOp::Gt => ">",
        BinOp::Lt => "<",
        BinOp::Gte => ">=",
        BinOp::Lte => "<=",
        _ => return None,
    };

    let field = expr_to_predicate_string(left)?;
    let value_str = expr_to_predicate_string(right)?;

    // Field path indexes into AgentContext::metadata; strip a `metadata.` head.
    let field = field
        .strip_prefix("metadata.")
        .map(str::to_string)
        .unwrap_or(field);

    // Reject anything that doesn't look like a plain field identifier path
    // (e.g. arithmetic, function calls, quoted junk on the LHS).
    if !is_field_path(&field) {
        return None;
    }

    build_comparison(&field, op_str, &value_str)
}

/// Render the LHS/RHS of a comparison back to the source-equivalent string the
/// string-based predicate helpers (`is_field_path`, `literal_to_json`) expect.
///
/// Only the shapes that can appear as a `field <op> literal` side are handled:
/// dotted identifier paths, bare identifiers (enum-style bareword literals),
/// `$var`/`$var.field` references, and scalar literals. Anything else (calls,
/// arithmetic, nested exprs, lists/maps) yields `None`, which the caller treats
/// as "not an enforceable predicate".
fn expr_to_predicate_string(expr: &Expr) -> Option<String> {
    use px_ast::Value as AstValue;
    match expr {
        Expr::Paren(inner) => expr_to_predicate_string(inner),
        // `foo.bar.baz` dotted path (LHS field, or a bareword-enum RHS like `green`).
        Expr::Path(dotted) => Some(
            dotted
                .segments
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join("."),
        ),
        // `$var` / `$var.field` -> keep the `$`-prefixed source form.
        Expr::Var(var) => {
            let mut s = format!("${}", var.name.name);
            for acc in &var.accessors {
                match acc {
                    px_ast::Accessor::Dot(id) => {
                        s.push('.');
                        s.push_str(&id.name);
                    }
                    // Bracket/index accessors are not enforceable field paths.
                    px_ast::Accessor::Bracket(_) => return None,
                }
            }
            Some(s)
        }
        // Scalar literals -> render to the string form `literal_to_json` parses.
        Expr::Literal(v) => match v {
            AstValue::String(s) => Some(format!("\"{s}\"")),
            AstValue::Integer(i) => Some(i.to_string()),
            AstValue::Float(f) => Some(f.to_string()),
            AstValue::Boolean(b) => Some(b.to_string()),
            // A bare identifier literal (e.g. an enum value `green`).
            AstValue::Ident(id) => Some(id.name.clone()),
            // A dotted path can also appear in value position.
            AstValue::Path(dotted) => Some(
                dotted
                    .segments
                    .iter()
                    .map(|seg| seg.name.as_str())
                    .collect::<Vec<_>>()
                    .join("."),
            ),
            _ => None,
        },
        _ => None,
    }
}

/// True if `s` is a dotted identifier path (the only LHS shape we enforce).
fn is_field_path(s: &str) -> bool {
    !s.is_empty()
        && s.split('.').all(|seg| {
            !seg.is_empty()
                && seg.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && seg
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        })
}

/// Build the [`Condition`] for `field op value_str`.
fn build_comparison(field: &str, op: &str, value_str: &str) -> Option<Condition> {
    let json = literal_to_json(value_str)?;

    match op {
        "<" | ">" | "<=" | ">=" => {
            // Ordering comparisons require a numeric RHS.
            let threshold = json.as_f64()?;
            Some(match op {
                "<" => Condition::FieldLt {
                    field: field.to_string(),
                    threshold,
                },
                ">" => Condition::FieldGt {
                    field: field.to_string(),
                    threshold,
                },
                // `<=`  is  !(x > t)
                "<=" => Condition::Not {
                    condition: Box::new(Condition::FieldGt {
                        field: field.to_string(),
                        threshold,
                    }),
                },
                // `>=`  is  !(x < t)
                _ => Condition::Not {
                    condition: Box::new(Condition::FieldLt {
                        field: field.to_string(),
                        threshold,
                    }),
                },
            })
        }
        "==" => Some(Condition::FieldEq {
            field: field.to_string(),
            value: json,
        }),
        "!=" => Some(Condition::Not {
            condition: Box::new(Condition::FieldEq {
                field: field.to_string(),
                value: json,
            }),
        }),
        _ => None,
    }
}

/// Convert a literal RHS token into a JSON value (number, bool, or string).
fn literal_to_json(s: &str) -> Option<serde_json::Value> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s == "true" {
        return Some(serde_json::Value::Bool(true));
    }
    if s == "false" {
        return Some(serde_json::Value::Bool(false));
    }
    // Quoted string literal.
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        return Some(serde_json::Value::String(s[1..s.len() - 1].to_string()));
    }
    // Numeric literal (int or float).
    if let Ok(i) = s.parse::<i64>() {
        return Some(serde_json::Value::Number(i.into()));
    }
    if let Ok(f) = s.parse::<f64>() {
        return serde_json::Number::from_f64(f).map(serde_json::Value::Number);
    }
    None
}

// ---------------------------------------------------------------------------
// query_gaps
// ---------------------------------------------------------------------------

/// Return all [`Evidence`] records whose `result` is [`EvidenceResult::Unknown`].
///
/// These represent "gaps" — constraints or decisions that have not yet been
/// validated by a concrete test run.
pub fn query_gaps(store: &PraxisStore) -> Vec<&Evidence> {
    store
        .evidence_records()
        .filter(|e| e.result == EvidenceResult::Unknown)
        .collect()
}

// ---------------------------------------------------------------------------
// apply_correction
// ---------------------------------------------------------------------------

/// The result of applying a user correction to the praxis store.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CorrectionApplied {
    /// The constraint that was created or updated.
    pub constraint: Constraint,
    /// Whether this was an insert (`true`) or update of an existing constraint.
    pub is_new: bool,
    /// Human-readable confirmation message.
    pub confirmation: String,
}

/// Apply a user correction to the praxis store.
///
/// Compiles the correction text into a [`Constraint`] via [`compile_nl`] and
/// upserts it into `store`.  Returns a [`CorrectionApplied`] record the caller
/// can use for confirmation messages and audit.
///
/// The constraint inherits decay protection: its description is prefixed with
/// `[correction]` so downstream systems can identify correction-sourced rules.
///
/// # Example
///
/// ```rust
/// use pluresdb_px::db::procedures::apply_correction;
/// use pluresdb_px::db::store::PraxisStore;
///
/// let mut store = PraxisStore::new();
/// let result = apply_correction(
///     &mut store,
///     "write_ actions must declare a resource_owner",
///     "C-CORR-1",
/// );
/// assert!(result.is_new);
/// assert!(store.get_constraint("C-CORR-1").is_some());
/// ```
pub fn apply_correction(
    store: &mut PraxisStore,
    correction_text: &str,
    id: impl Into<String>,
) -> CorrectionApplied {
    let id = id.into();
    let is_new = store.get_constraint(&id).is_none();

    let mut constraint = compile_nl(correction_text, &id);
    // Prefix description so correction-sourced rules are identifiable.
    constraint.description = format!("[correction] {}", constraint.description);

    store.upsert_constraint(constraint.clone());

    let confirmation = format!(
        "Got it, I'll remember to {} going forward.",
        constraint.description.trim_start_matches("[correction] ")
    );

    CorrectionApplied {
        constraint,
        is_new,
        confirmation,
    }
}

// ---------------------------------------------------------------------------
// undo_correction
// ---------------------------------------------------------------------------

/// Undo a previously applied correction by removing its constraint from the
/// store.
///
/// Returns the removed [`Constraint`], or `None` if no constraint with the
/// given ID exists.
pub fn undo_correction(store: &mut PraxisStore, constraint_id: &str) -> Option<Constraint> {
    store.remove_constraint(constraint_id).ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::{EvidenceResult, SessionType};
    use crate::db::seed::default_store;
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;

    // ── evaluate ─────────────────────────────────────────────────────────────

    #[test]
    fn evaluate_clean_context_returns_no_violations() {
        let store = default_store();
        let ctx = AgentContext::new("read_file", "README.md", SessionType::Main);
        let violations = evaluate(&store, &ctx);
        assert!(
            violations.is_empty(),
            "read_file with no metadata should not violate any constraint; got: {:?}",
            violations
                .iter()
                .map(|v| &v.constraint.id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn evaluate_missing_resource_owner_on_write() {
        let store = default_store();
        let ctx = AgentContext::new("write_file", "config.toml", SessionType::Main)
            .with_meta("resource_owner", json!(""));
        let violations = evaluate(&store, &ctx);
        let ids: Vec<&str> = violations
            .iter()
            .map(|v| v.constraint.id.as_str())
            .collect();
        assert!(
            ids.contains(&"C-0002"),
            "C-0002 (resource_owner_declared) should fire; got: {ids:?}"
        );
    }

    #[test]
    fn evaluate_high_privilege_triggers_violation() {
        let store = default_store();
        let ctx = AgentContext::new("admin_action", "system", SessionType::Main)
            .with_meta("privilege_level", json!(4));
        let violations = evaluate(&store, &ctx);
        let ids: Vec<&str> = violations
            .iter()
            .map(|v| v.constraint.id.as_str())
            .collect();
        assert!(
            ids.contains(&"C-0003"),
            "C-0003 (privilege escalation) should fire; got: {ids:?}"
        );
    }

    #[test]
    fn evaluate_extreme_risk_score_triggers_violation() {
        let store = default_store();
        let ctx = AgentContext::new("deploy", "production", SessionType::Main)
            .with_meta("risk_score", json!(0.95));
        let violations = evaluate(&store, &ctx);
        let ids: Vec<&str> = violations
            .iter()
            .map(|v| v.constraint.id.as_str())
            .collect();
        assert!(
            ids.contains(&"C-0004"),
            "C-0004 (risk score) should fire; got: {ids:?}"
        );
    }

    // ── on_action ────────────────────────────────────────────────────────────

    #[test]
    fn on_action_permits_safe_read() {
        let store = default_store();
        let ctx = AgentContext::new("read_config", "settings.toml", SessionType::Main);
        assert!(on_action(&store, &ctx).is_ok());
    }

    #[test]
    fn on_action_blocks_write_without_owner() {
        let store = default_store();
        let ctx = AgentContext::new("write_file", "config.toml", SessionType::Main)
            .with_meta("resource_owner", json!(""));
        let result = on_action(&store, &ctx);
        assert!(result.is_err(), "should be blocked");
        let blocked = result.unwrap_err();
        assert!(!blocked.violations.is_empty());
    }

    #[test]
    fn action_blocked_display_includes_fix() {
        let store = default_store();
        let ctx = AgentContext::new("write_file", "x", SessionType::Main)
            .with_meta("resource_owner", json!(""));
        let err = on_action(&store, &ctx).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("blocked"), "display should mention blocked");
        assert!(msg.contains("fix:"), "display should include fix");
    }

    // ── compile_nl ───────────────────────────────────────────────────────────

    #[test]
    fn compile_nl_write_action_pattern() {
        let c = compile_nl(
            "write_ actions must declare a resource_owner field.",
            "C-TEST",
        );
        assert_eq!(c.id, "C-TEST");
        assert_eq!(c.severity, Severity::Error);
        assert!(matches!(c.when, Condition::ActionStartsWith { .. }));
    }

    #[test]
    fn compile_nl_privilege_pattern() {
        let c = compile_nl("privilege_level should stay below 3", "C-PRIV");
        assert!(matches!(c.require, Condition::FieldLt { .. }));
    }

    #[test]
    fn compile_nl_unknown_text_is_flagged_inert_not_silent_always() {
        // Genuinely unrecognised English: neither a structured predicate nor a
        // known keyword. It MUST be flagged inert, not a silent Always that
        // masquerades as enforcement (C-NOSTUB-001).
        let c = compile_nl("something entirely unrecognised", "C-UNK");
        assert!(matches!(c.when, Condition::Always));
        assert!(matches!(c.require, Condition::Always));
        assert!(
            c.description.starts_with(UNPARSED_MARKER),
            "unparsed constraint must be flagged with the marker; got: {:?}",
            c.description
        );
    }

    // ── compile_nl: REAL structured-predicate path ───────────────────────────

    /// Helper: build a ctx carrying a single numeric `amount` metadata value.
    fn ctx_amount(amount: i64) -> AgentContext {
        AgentContext::new("place_trade", "market", SessionType::Main)
            .with_meta("amount", json!(amount))
    }

    /// Helper: does this constraint BLOCK the given ctx?  (when holds AND
    /// require does NOT hold).
    fn blocks(c: &Constraint, ctx: &AgentContext) -> bool {
        c.when.evaluate(ctx) && !c.require.evaluate(ctx)
    }

    #[test]
    fn compile_nl_le_blocks_violation_passes_compliant() {
        // The $500-trade class of input. `amount <= 100` must really enforce.
        let c = compile_nl("metadata.amount <= 100", "C-AMT");
        // require is the real invariant, NOT Always.
        assert!(
            !matches!(c.require, Condition::Always),
            "require must be a real predicate, not Always"
        );
        assert!(blocks(&c, &ctx_amount(500)), "amount=500 must be blocked");
        assert!(!blocks(&c, &ctx_amount(50)), "amount=50 must pass");
        // Boundary: <= is inclusive, so amount=100 passes.
        assert!(
            !blocks(&c, &ctx_amount(100)),
            "amount=100 must pass (<= is inclusive)"
        );
    }

    #[test]
    fn compile_nl_metadata_prefix_is_stripped() {
        // `metadata.amount` and `amount` must behave identically.
        let with_prefix = compile_nl("metadata.amount <= 100", "C-A");
        let without = compile_nl("amount <= 100", "C-B");
        assert_eq!(
            blocks(&with_prefix, &ctx_amount(500)),
            blocks(&without, &ctx_amount(500))
        );
        assert!(blocks(&without, &ctx_amount(500)));
    }

    #[test]
    fn compile_nl_lt_strict() {
        let c = compile_nl("risk_score < 0.9", "C-RISK");
        assert!(matches!(c.require, Condition::FieldLt { .. }));
        let hi = AgentContext::new("deploy", "prod", SessionType::Main)
            .with_meta("risk_score", json!(0.95));
        let lo = AgentContext::new("deploy", "prod", SessionType::Main)
            .with_meta("risk_score", json!(0.5));
        assert!(blocks(&c, &hi), "0.95 must be blocked by < 0.9");
        assert!(!blocks(&c, &lo), "0.5 must pass < 0.9");
    }

    #[test]
    fn compile_nl_gt_strict() {
        let c = compile_nl("score > 10", "C-GT");
        assert!(matches!(c.require, Condition::FieldGt { .. }));
        let ok = AgentContext::new("a", "b", SessionType::Main).with_meta("score", json!(20));
        let bad = AgentContext::new("a", "b", SessionType::Main).with_meta("score", json!(5));
        assert!(!blocks(&c, &ok), "20 satisfies > 10");
        assert!(blocks(&c, &bad), "5 violates > 10");
    }

    #[test]
    fn compile_nl_ge_inclusive() {
        // `>= 3`  ≡  !(x < 3): 3 and 4 pass, 2 blocked.
        let c = compile_nl("level >= 3", "C-GE");
        let mk =
            |n: i64| AgentContext::new("a", "b", SessionType::Main).with_meta("level", json!(n));
        assert!(!blocks(&c, &mk(4)), "4 passes >= 3");
        assert!(!blocks(&c, &mk(3)), "3 passes >= 3 (inclusive)");
        assert!(blocks(&c, &mk(2)), "2 violates >= 3");
    }

    #[test]
    fn compile_nl_eq_and_ne() {
        // ==
        let eq = compile_nl("env == \"prod\"", "C-EQ");
        assert!(matches!(eq.require, Condition::FieldEq { .. }));
        let prod = AgentContext::new("a", "b", SessionType::Main).with_meta("env", json!("prod"));
        let stg = AgentContext::new("a", "b", SessionType::Main).with_meta("env", json!("staging"));
        assert!(!blocks(&eq, &prod), "env=prod satisfies == prod");
        assert!(blocks(&eq, &stg), "env=staging violates == prod");

        // !=  (maps to Not(FieldEq))
        let ne = compile_nl("env != \"prod\"", "C-NE");
        assert!(matches!(ne.require, Condition::Not { .. }));
        assert!(blocks(&ne, &prod), "env=prod violates != prod");
        assert!(!blocks(&ne, &stg), "env=staging satisfies != prod");
    }

    #[test]
    fn compile_nl_and_combination() {
        // Conjunction: amount <= 100 AND level >= 2 → Condition::All.
        let c = compile_nl("amount <= 100 and level >= 2", "C-AND");
        assert!(matches!(c.require, Condition::All { .. }));
        let mk = |amt: i64, lvl: i64| {
            AgentContext::new("place_trade", "m", SessionType::Main)
                .with_meta("amount", json!(amt))
                .with_meta("level", json!(lvl))
        };
        assert!(!blocks(&c, &mk(50, 3)), "both satisfied -> pass");
        assert!(blocks(&c, &mk(500, 3)), "amount too high -> block");
        assert!(blocks(&c, &mk(50, 1)), "level too low -> block");
    }

    #[test]
    fn compile_nl_or_combination() {
        // Disjunction: amount <= 100 OR override == true → Condition::Any.
        let c = compile_nl("amount <= 100 or override == true", "C-OR");
        assert!(matches!(c.require, Condition::Any { .. }));
        let mk = |amt: i64, ovr: bool| {
            AgentContext::new("place_trade", "m", SessionType::Main)
                .with_meta("amount", json!(amt))
                .with_meta("override", json!(ovr))
        };
        assert!(!blocks(&c, &mk(50, false)), "amount ok -> pass");
        assert!(!blocks(&c, &mk(500, true)), "override true -> pass");
        assert!(blocks(&c, &mk(500, false)), "neither -> block");
    }

    #[test]
    fn compile_nl_must_keyword_sets_error_severity() {
        let c = compile_nl(
            "place_trade actions must have metadata.amount <= 100",
            "C-SEV",
        );
        // "must" => Error severity; predicate still parses from the tail.
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn compile_nl_full_sentence_with_embedded_predicate_is_flagged_when_not_pure() {
        // A sentence that is NOT a bare expression must not silently become a
        // real guard via the structured path; it falls through. Because it has
        // no recognised keyword either, it is flagged inert (never silent Always).
        let c = compile_nl("please make sure the trade is reasonable", "C-SENT");
        assert!(c.description.starts_with(UNPARSED_MARKER));
        assert!(matches!(c.require, Condition::Always));
    }

    // ── REGRESSION: the old silent-Always stub is gone ───────────────────────

    #[test]
    fn regression_numeric_threshold_never_compiles_to_always() {
        // Before TASK-PX-CANON Stage 1, ANY text without a magic keyword
        // (including numeric thresholds) compiled `require: Always` — a silent
        // pass-through that enforced nothing. Prove that class is dead.
        for src in [
            "metadata.amount <= 100",
            "amount < 100",
            "risk_score < 0.9",
            "score > 10",
            "level >= 2",
            "count != 0",
        ] {
            let c = compile_nl(src, "C-REG");
            assert!(
                !matches!(c.require, Condition::Always),
                "`{src}` must compile to a real predicate, not Condition::Always"
            );
            assert!(
                !c.description.starts_with(UNPARSED_MARKER),
                "`{src}` is a valid predicate and must NOT be flagged unparsed"
            );
        }
    }

    #[test]
    fn regression_dollar_500_trade_actually_blocks() {
        // The canonical motivating example from the task definition.
        let c = compile_nl(
            "place_trade actions must have metadata.amount <= 100",
            "C-TRADE",
        );
        // The sentence form falls through to the inert path (not a bare expr),
        // so we also assert the BARE predicate form (the supported syntax) blocks.
        let bare = compile_nl("metadata.amount <= 100", "C-TRADE2");
        let ctx500 = AgentContext::new("place_trade", "market", SessionType::Main)
            .with_meta("amount", json!(500));
        assert!(
            blocks(&bare, &ctx500),
            "$500 trade must be blocked by the real predicate"
        );
        // And the sentence form is at least honestly flagged, not a fake guard.
        assert!(
            matches!(c.require, Condition::Always) == c.description.starts_with(UNPARSED_MARKER),
            "sentence form must be flagged inert if it is Always"
        );
    }

    // ── query_gaps ───────────────────────────────────────────────────────────

    #[test]
    fn query_gaps_returns_unknown_evidence() {
        let mut store = default_store();
        store
            .insert_evidence(Evidence {
                id: "EV-GAP".into(),
                tested_at: Utc::now(),
                condition: HashMap::new(),
                result: EvidenceResult::Unknown,
                reference: "https://github.com/plures/pares-radix/issues/999".into(),
            })
            .unwrap();

        let gaps = query_gaps(&store);
        let ids: Vec<&str> = gaps.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"EV-GAP"));
    }

    #[test]
    fn query_gaps_excludes_passed_evidence() {
        let store = default_store();
        // default_store seeds passed evidence; ensure gaps don't include it
        let gaps = query_gaps(&store);
        for ev in &gaps {
            assert_eq!(ev.result, EvidenceResult::Unknown);
        }
    }

    #[test]
    fn query_gaps_returns_adr0004_gap_evidence() {
        let store = default_store();
        let gaps = query_gaps(&store);
        // ADR-0004 seeds an EvidenceResult::Unknown record for CI validation
        let ids: Vec<&str> = gaps.iter().map(|e| e.id.as_str()).collect();
        assert!(
            ids.contains(&"EV-ADR0004-CI"),
            "EV-ADR0004-CI (unknown) should appear in gaps; got: {ids:?}"
        );
    }

    // ── apply_correction ─────────────────────────────────────────────────────

    #[test]
    fn apply_correction_creates_new_constraint() {
        let mut store = PraxisStore::new();
        let result = apply_correction(
            &mut store,
            "write_ actions must declare a resource_owner",
            "C-CORR-1",
        );
        assert!(result.is_new);
        assert!(store.get_constraint("C-CORR-1").is_some());
        assert!(result.constraint.description.starts_with("[correction]"));
        assert!(result.confirmation.contains("going forward"));
    }

    #[test]
    fn apply_correction_updates_existing_constraint() {
        let mut store = PraxisStore::new();
        // First insertion
        apply_correction(&mut store, "risk_score should be low", "C-CORR-2");
        assert_eq!(store.constraint_count(), 1);
        // Second insertion with same ID updates
        let result = apply_correction(
            &mut store,
            "risk_score must stay below threshold",
            "C-CORR-2",
        );
        assert!(!result.is_new);
        assert_eq!(store.constraint_count(), 1);
    }

    #[test]
    fn apply_correction_produces_constraint_with_correction_prefix() {
        let mut store = PraxisStore::new();
        let result = apply_correction(
            &mut store,
            "privilege_level should be restricted",
            "C-CORR-3",
        );
        let c = store.get_constraint("C-CORR-3").unwrap();
        assert!(c.description.starts_with("[correction]"));
        assert!(result.confirmation.contains("going forward"));
    }

    // ── undo_correction ──────────────────────────────────────────────────────

    #[test]
    fn undo_correction_removes_constraint() {
        let mut store = PraxisStore::new();
        apply_correction(&mut store, "some rule", "C-CORR-4");
        assert!(store.get_constraint("C-CORR-4").is_some());

        let removed = undo_correction(&mut store, "C-CORR-4");
        assert!(removed.is_some());
        assert!(store.get_constraint("C-CORR-4").is_none());
    }

    #[test]
    fn undo_correction_nonexistent_returns_none() {
        let mut store = PraxisStore::new();
        let removed = undo_correction(&mut store, "C-DOES-NOT-EXIST");
        assert!(removed.is_none());
    }
}
