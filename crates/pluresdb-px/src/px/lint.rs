//! Lint pass for .px documents — detects potential issues before execution.
//!
//! Produces warnings/errors for patterns that may cause runtime failures:
//! - Non-exhaustive match steps (no wildcard `_` arm)
//! - Empty procedure bodies
//! - Unreachable code after unconditional match arms
//!
//! ## M6 (praxis-lang import)
//!
//! The linter's rules were authored against the **string-form** projection of a
//! procedure (conditions as `String`, params as JSON, `over`/`item_var` as
//! `String`) — exactly the shape the executor consumes. Rather than re-derive
//! that projection from px-ast a second time, the public [`lint`] entry accepts a
//! [`px_ast::PxDocument`] and **lowers** each procedure to a small lint-local view
//! ([`LintDoc`]) using the compiler's record renderer
//! ([`super::compiler::step_to_json`]) as the single source of truth. All rule
//! logic then runs unchanged over that view. The view types are lint's own
//! concern (they outlive the deleted local flat AST).

use serde_json::Value as Json;

// ───────────────────────────────────────────────────────────────────
// Lint-local view of a document (string-form projection the rules operate on).
// Byte-identical in shape to the v1 flat AST the rule logic + tests were built
// against. Populated from px-ast via `LintDoc::from_px_ast`.
// ───────────────────────────────────────────────────────────────────

/// A procedure's trigger, in lint-view form.
#[derive(Debug, Clone)]
pub struct LintTrigger {
    pub kind: String,
    pub params: Option<Json>,
}

/// A procedure, in lint-view form.
#[derive(Debug, Clone)]
pub struct LintProc {
    pub name: String,
    pub trigger: Option<LintTrigger>,
    pub given: Option<String>,
    pub steps: Vec<LintStep>,
}

/// A match arm, in lint-view form (condition + result rendered as strings).
#[derive(Debug, Clone)]
pub struct LintMatchArm {
    pub condition: String,
    pub result: String,
}

/// A parallel branch, in lint-view form.
#[derive(Debug, Clone)]
pub struct LintBranch {
    pub name: String,
    pub steps: Vec<LintStep>,
    pub retry: Option<u64>,
    pub retry_delay_ms: Option<u64>,
    pub retry_backoff: Option<String>,
    pub retry_max_delay_ms: Option<u64>,
    pub retry_jitter: Option<bool>,
}

/// A single step, in lint-view form (string-form conditions, JSON params).
#[derive(Debug, Clone)]
pub enum LintStep {
    Call {
        name: String,
        params: Json,
        output_var: Option<String>,
    },
    Match {
        arms: Vec<LintMatchArm>,
    },
    When {
        condition: String,
        steps: Vec<LintStep>,
    },
    Loop {
        over: Option<String>,
        times: Option<u64>,
        item_var: String,
        key_var: Option<String>,
        steps: Vec<LintStep>,
        output_var: Option<String>,
    },
    Emit {
        event: Json,
    },
    Try {
        steps: Vec<LintStep>,
        catch: Vec<LintStep>,
        retry: Option<u64>,
        retry_delay_ms: Option<u64>,
        retry_backoff: Option<String>,
        retry_max_delay_ms: Option<u64>,
        retry_jitter: Option<bool>,
    },
    Parallel {
        branches: Vec<LintBranch>,
        output_var: Option<String>,
    },
    Return {
        value: Option<Json>,
    },
    Abort {
        value: Option<Json>,
    },
    Assign {
        var: String,
        value: String,
    },
    If {
        condition: String,
        then_steps: Vec<LintStep>,
        else_steps: Vec<LintStep>,
    },
    For {
        var: String,
        iterable: String,
        steps: Vec<LintStep>,
    },
}

/// A document, in lint-view form. Only the parts the linter inspects are kept
/// (procedures + functions); everything else is irrelevant to the lint rules.
#[derive(Debug, Clone, Default)]
pub struct LintDoc {
    pub procedures: Vec<LintProc>,
    /// Function signatures (name + declared param names) for call-arity checks.
    pub functions: Vec<(String, Vec<String>)>,
}

// ───────────────────────────────────────────────────────────────────
// px-ast → lint-view lowering (reuses compiler::step_to_json as SSOT).
// ───────────────────────────────────────────────────────────────────

fn steps_from_records(records: &[Json]) -> Vec<LintStep> {
    records.iter().filter_map(LintStep::from_record).collect()
}

impl LintStep {
    /// Build a lint-view step from the executor record JSON emitted by
    /// `compiler::step_to_json`. Returns `None` for record kinds the linter does
    /// not analyze (e.g. `define`, `code`), which are intentionally skipped —
    /// they are NOT dropped from compilation (the record compiler preserves
    /// them); the linter simply has no rule for them yet.
    fn from_record(rec: &Json) -> Option<LintStep> {
        let kind = rec.get("kind").and_then(|k| k.as_str())?;
        let s = |k: &str| rec.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
        let u = |k: &str| rec.get(k).and_then(|v| v.as_u64());
        let b = |k: &str| rec.get(k).and_then(|v| v.as_bool());
        let steps_at = |k: &str| {
            rec.get(k)
                .and_then(|v| v.as_array())
                .map(|a| steps_from_records(a))
                .unwrap_or_default()
        };
        Some(match kind {
            "call" => LintStep::Call {
                name: s("name").unwrap_or_default(),
                params: rec.get("params").cloned().unwrap_or(Json::Null),
                output_var: s("output_var"),
            },
            "match" => LintStep::Match {
                arms: rec
                    .get("arms")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .map(|arm| LintMatchArm {
                                condition: arm
                                    .get("condition")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                result: arm
                                    .get("result")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            },
            "when" => LintStep::When {
                condition: s("condition").unwrap_or_default(),
                steps: steps_at("steps"),
            },
            "loop" => LintStep::Loop {
                over: s("over"),
                times: u("times"),
                item_var: s("as").unwrap_or_else(|| "item".to_string()),
                key_var: s("key_as"),
                steps: steps_at("steps"),
                output_var: s("output_var"),
            },
            "emit" => LintStep::Emit {
                event: rec.get("event").cloned().unwrap_or(Json::Null),
            },
            "try" => LintStep::Try {
                steps: steps_at("steps"),
                catch: steps_at("catch"),
                retry: u("retry"),
                retry_delay_ms: u("retry_delay_ms"),
                retry_backoff: s("retry_backoff"),
                retry_max_delay_ms: u("retry_max_delay_ms"),
                retry_jitter: b("retry_jitter"),
            },
            "parallel" => LintStep::Parallel {
                branches: rec
                    .get("branches")
                    .and_then(|v| v.as_array())
                    .map(|a| {
                        a.iter()
                            .map(|br| LintBranch {
                                name: br
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                steps: br
                                    .get("steps")
                                    .and_then(|v| v.as_array())
                                    .map(|a| steps_from_records(a))
                                    .unwrap_or_default(),
                                retry: br.get("retry").and_then(|v| v.as_u64()),
                                retry_delay_ms: br.get("retry_delay_ms").and_then(|v| v.as_u64()),
                                retry_backoff: br
                                    .get("retry_backoff")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                retry_max_delay_ms: br
                                    .get("retry_max_delay_ms")
                                    .and_then(|v| v.as_u64()),
                                retry_jitter: br.get("retry_jitter").and_then(|v| v.as_bool()),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                output_var: s("output_var"),
            },
            "return" => LintStep::Return {
                value: rec.get("value").cloned(),
            },
            "abort" => LintStep::Abort {
                value: rec.get("value").cloned(),
            },
            "assign" => LintStep::Assign {
                var: s("var").unwrap_or_default(),
                value: s("value").unwrap_or_default(),
            },
            "if" => LintStep::If {
                condition: s("condition").unwrap_or_default(),
                then_steps: steps_at("then"),
                else_steps: steps_at("else"),
            },
            "for" => LintStep::For {
                var: s("var").unwrap_or_default(),
                iterable: s("iterable").unwrap_or_default(),
                steps: steps_at("steps"),
            },
            // `define`, `code`, and any future kinds have no lint rule yet.
            _ => return None,
        })
    }
}

impl LintDoc {
    /// Lower a px-ast document to the lint view. Only legacy (step-list)
    /// procedures are analyzed by the current rules; dataflow procedures and
    /// code-block bodies carry no v1 step list to lint (documented follow-up).
    pub fn from_px_ast(doc: &px_ast::PxDocument) -> LintDoc {
        use px_ast::{ProcedureBody, ProcedureTrigger, Statement};

        let trigger_kind = |t: &ProcedureTrigger| -> &'static str {
            match t {
                ProcedureTrigger::Periodic { .. } => "periodic",
                ProcedureTrigger::OnWrite { .. } => "on_write",
                ProcedureTrigger::OnEvent(_) => "on_event",
                ProcedureTrigger::Startup => "startup",
                ProcedureTrigger::BeforeResponse => "before_response",
                ProcedureTrigger::AfterResponse => "after_response",
                ProcedureTrigger::Cron { .. } => "cron",
                ProcedureTrigger::Manual => "manual",
            }
        };

        // Extract declared trigger parameter names from a legacy trigger.
        // `on_write {k: v, ...}`, `periodic {every: ...}`, and `cron {...}`
        // carry their params as a `Value::Map`; the top-level keys are the
        // parameter names the procedure body is expected to reference.
        fn trigger_param_names(t: &ProcedureTrigger) -> Vec<String> {
            let args: Option<&px_ast::Value> = match t {
                ProcedureTrigger::OnWrite { args, .. } => args.as_ref(),
                ProcedureTrigger::Periodic { interval } => interval.as_ref(),
                ProcedureTrigger::Cron { schedule } => schedule.as_ref(),
                _ => None,
            };
            match args {
                Some(px_ast::Value::Map(pairs)) => {
                    pairs.iter().map(|(k, _)| k.name.clone()).collect()
                }
                _ => Vec::new(),
            }
        }

        let mut procedures = Vec::new();
        let mut functions = Vec::new();

        for stmt in &doc.statements {
            match stmt {
                Statement::LegacyProcedure(p) => {
                    let steps = match &p.body {
                        ProcedureBody::Steps(s) => {
                            let records: Vec<Json> =
                                s.iter().map(super::compiler::step_to_json).collect();
                            steps_from_records(&records)
                        }
                        // A code-block body has no v1 steps to lint.
                        ProcedureBody::Code(_) => Vec::new(),
                    };
                    // The param-hygiene rules (L010/L012) key off an object of
                    // trigger parameter names (e.g. `trigger: on_write {channel:
                    // "string", message: "string"}`), so synthesize one.
                    //
                    // px-ast represents those params in TWO possible places:
                    //  1. the flat `Vec<Ident>` `p.params` (v1 `params: [$a, $b]` form), and
                    //  2. the trigger's `args` map for `on_write` / `periodic` / `cron`
                    //     (the `trigger: on_write {k: v, ...}` form the fixtures and most
                    //     real procedures use).
                    // The old in-tree engine sourced them from the trigger args; reading
                    // only `p.params` silently dropped every trigger-declared param, so
                    // L010/L012 never fired. Union both sources here.
                    let mut param_map = serde_json::Map::new();
                    for id in &p.params {
                        param_map.insert(id.name.clone(), Json::String(String::new()));
                    }
                    if let Some(trigger) = &p.trigger {
                        for name in trigger_param_names(trigger) {
                            param_map
                                .entry(name)
                                .or_insert_with(|| Json::String(String::new()));
                        }
                    }
                    let params_obj = if param_map.is_empty() {
                        None
                    } else {
                        Some(Json::Object(param_map))
                    };
                    let trigger = Some(LintTrigger {
                        kind: p
                            .trigger
                            .as_ref()
                            .map(|t| trigger_kind(t).to_string())
                            .unwrap_or_else(|| "manual".to_string()),
                        params: params_obj,
                    });
                    procedures.push(LintProc {
                        name: p.name.name.clone(),
                        trigger,
                        given: p.given.as_ref().map(|g| g.value.clone()),
                        steps,
                    });
                }
                Statement::Function(f) => {
                    let param_names = f
                        .params
                        .iter()
                        .map(|p| p.name.name.clone())
                        .collect::<Vec<_>>();
                    functions.push((f.name.name.clone(), param_names));
                }
                _ => {}
            }
        }

        LintDoc {
            procedures,
            functions,
        }
    }
}

/// Severity of a lint diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum LintSeverity {
    Warning,
    Error,
}

/// A lint diagnostic produced by the lint pass.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LintDiagnostic {
    /// Which lint rule triggered this.
    pub code: &'static str,
    /// Human-readable message.
    pub message: String,
    /// Severity level.
    pub severity: LintSeverity,
    /// Name of the procedure (if applicable).
    pub procedure: Option<String>,
    /// Step index within the procedure (0-based, if applicable).
    pub step_index: Option<usize>,
}

impl std::fmt::Display for LintDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sev = match self.severity {
            LintSeverity::Warning => "warning",
            LintSeverity::Error => "error",
        };
        let location = match (&self.procedure, self.step_index) {
            (Some(proc), Some(idx)) => format!(" in `{}` step {}", proc, idx + 1),
            (Some(proc), None) => format!(" in `{}`", proc),
            _ => String::new(),
        };
        write!(f, "[{}] {}{}: {}", self.code, sev, location, self.message)
    }
}

/// Run all lint passes on a parsed px-ast document.
///
/// Lowers the document to the string-form lint view, then runs every rule.
pub fn lint(doc: &px_ast::PxDocument) -> Vec<LintDiagnostic> {
    let view = LintDoc::from_px_ast(doc);
    lint_view(&view)
}

/// Run all lint passes over an already-lowered lint view.
fn lint_view(doc: &LintDoc) -> Vec<LintDiagnostic> {
    let mut diagnostics = Vec::new();

    for procedure in &doc.procedures {
        lint_procedure(procedure, &mut diagnostics);
    }

    // Document-level lints (cross-procedure analysis)
    lint_undefined_calls(doc, &mut diagnostics);
    lint_arity_mismatch(doc, &mut diagnostics);

    diagnostics
}

/// Lint a single procedure.
fn lint_procedure(proc: &LintProc, diags: &mut Vec<LintDiagnostic>) {
    // L001: Empty procedure body
    if proc.steps.is_empty() {
        diags.push(LintDiagnostic {
            code: "PX-L001",
            message: "procedure has no steps".to_string(),
            severity: LintSeverity::Warning,
            procedure: Some(proc.name.clone()),
            step_index: None,
        });
        return;
    }

    for (idx, step) in proc.steps.iter().enumerate() {
        lint_step(step, &proc.name, idx, diags);
    }

    // L005: Unused output variables (procedure-level analysis)
    lint_unused_output_vars(proc, diags);

    // L008: Shadowed output variables (same name bound by multiple steps)
    lint_shadowed_output_vars(proc, diags);

    // L009: Unreachable steps after return/abort
    lint_unreachable_after_terminal(proc, diags);

    // L010: Unused procedure parameters (declared in trigger but never referenced)
    lint_unused_procedure_params(proc, diags);
}

/// Lint a single step (recursing into nested structures).
fn lint_step(step: &LintStep, proc_name: &str, idx: usize, diags: &mut Vec<LintDiagnostic>) {
    match step {
        LintStep::Match { arms } => {
            lint_match_exhaustiveness(arms, proc_name, idx, diags);
            lint_match_unreachable(arms, proc_name, idx, diags);
            lint_match_duplicate_conditions(arms, proc_name, idx, diags);
        }
        LintStep::Loop {
            over,
            item_var,
            key_var,
            steps,
            ..
        } => {
            lint_unused_loop_item_var(over, item_var, key_var, steps, proc_name, idx, diags);
            for (sub_idx, sub_step) in steps.iter().enumerate() {
                lint_step(sub_step, proc_name, sub_idx, diags);
            }
        }
        LintStep::When { steps, .. } => {
            for (sub_idx, sub_step) in steps.iter().enumerate() {
                lint_step(sub_step, proc_name, sub_idx, diags);
            }
        }
        LintStep::Try { steps, catch, .. } => {
            lint_empty_catch(catch, proc_name, idx, diags);
            for (sub_idx, sub_step) in steps.iter().enumerate() {
                lint_step(sub_step, proc_name, sub_idx, diags);
            }
            for (sub_idx, sub_step) in catch.iter().enumerate() {
                lint_step(sub_step, proc_name, sub_idx, diags);
            }
        }
        LintStep::Parallel { branches, .. } => {
            for branch in branches {
                for (sub_idx, sub_step) in branch.steps.iter().enumerate() {
                    lint_step(sub_step, proc_name, sub_idx, diags);
                }
            }
        }
        _ => {}
    }
}

/// PX-L002: Non-exhaustive match — no wildcard `_` arm present.
fn lint_match_exhaustiveness(
    arms: &[LintMatchArm],
    proc_name: &str,
    idx: usize,
    diags: &mut Vec<LintDiagnostic>,
) {
    let has_wildcard = arms.iter().any(|arm| {
        let cond = arm.condition.trim();
        cond == "_" || cond == "_ =>" || cond.starts_with("_ ")
    });

    if !has_wildcard {
        diags.push(LintDiagnostic {
            code: "PX-L002",
            message: format!(
                "match step has {} arm(s) but no wildcard `_` — may fail at runtime if no arm matches",
                arms.len()
            ),
            severity: LintSeverity::Warning,
            procedure: Some(proc_name.to_string()),
            step_index: Some(idx),
        });
    }
}

/// PX-L004: Duplicate arm conditions in a match.
fn lint_match_duplicate_conditions(
    arms: &[LintMatchArm],
    proc_name: &str,
    idx: usize,
    diags: &mut Vec<LintDiagnostic>,
) {
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (arm_idx, arm) in arms.iter().enumerate() {
        let cond = arm.condition.trim();
        if cond == "_" {
            continue; // wildcard is a special case, not a duplicate
        }
        if let Some(&first_idx) = seen.get(cond) {
            diags.push(LintDiagnostic {
                code: "PX-L004",
                message: format!(
                    "arm {} has the same condition as arm {} (`{}`) — only the first will ever match",
                    arm_idx + 1,
                    first_idx + 1,
                    cond
                ),
                severity: LintSeverity::Warning,
                procedure: Some(proc_name.to_string()),
                step_index: Some(idx),
            });
        } else {
            seen.insert(cond, arm_idx);
        }
    }
}

/// PX-L005: Unused output variables — bound but never referenced in subsequent steps.
fn lint_unused_output_vars(proc: &LintProc, diags: &mut Vec<LintDiagnostic>) {
    // Collect all output_var bindings with their step index
    let mut bindings: Vec<(usize, &str)> = Vec::new();
    for (idx, step) in proc.steps.iter().enumerate() {
        if let Some(var) = step_output_var(step) {
            bindings.push((idx, var));
        }
    }

    if bindings.is_empty() {
        return;
    }

    // Collect all variable references across the procedure
    let mut references: std::collections::HashSet<String> = std::collections::HashSet::new();
    for step in &proc.steps {
        collect_var_references(step, &mut references);
    }

    // Check each binding against references
    for (idx, var_name) in bindings {
        if !references.contains(&format!("${}", var_name)) {
            diags.push(LintDiagnostic {
                code: "PX-L005",
                message: format!(
                    "output variable `${}` is bound but never referenced in subsequent steps",
                    var_name
                ),
                severity: LintSeverity::Warning,
                procedure: Some(proc.name.clone()),
                step_index: Some(idx),
            });
        }
    }
}

/// Extract the output_var from a step, if any.
fn step_output_var(step: &LintStep) -> Option<&str> {
    match step {
        LintStep::Call { output_var, .. } => output_var.as_deref(),
        LintStep::Loop { output_var, .. } => output_var.as_deref(),
        LintStep::Parallel { output_var, .. } => output_var.as_deref(),
        _ => None,
    }
}

/// Recursively collect all `$variable` references from a step.
fn collect_var_references(step: &LintStep, refs: &mut std::collections::HashSet<String>) {
    match step {
        LintStep::Call { params, .. } => {
            collect_refs_from_value(params, refs);
        }
        LintStep::Match { arms } => {
            for arm in arms {
                collect_refs_from_str(&arm.condition, refs);
                collect_refs_from_str(&arm.result, refs);
            }
        }
        LintStep::When { condition, steps } => {
            collect_refs_from_str(condition, refs);
            for s in steps {
                collect_var_references(s, refs);
            }
        }
        LintStep::Loop { over, steps, .. } => {
            if let Some(over_expr) = over {
                collect_refs_from_str(over_expr, refs);
            }
            for s in steps {
                collect_var_references(s, refs);
            }
        }
        LintStep::Emit { event } => {
            collect_refs_from_value(event, refs);
        }
        LintStep::Try { steps, catch, .. } => {
            for s in steps {
                collect_var_references(s, refs);
            }
            for s in catch {
                collect_var_references(s, refs);
            }
        }
        LintStep::Parallel { branches, .. } => {
            for branch in branches {
                for s in &branch.steps {
                    collect_var_references(s, refs);
                }
            }
        }
        LintStep::Return { value } => {
            if let Some(v) = value {
                collect_refs_from_value(v, refs);
            }
        }
        LintStep::Abort { value } => {
            if let Some(v) = value {
                collect_refs_from_value(v, refs);
            }
        }
        LintStep::Assign { value, .. } => {
            collect_refs_from_str(value, refs);
        }
        LintStep::If {
            condition,
            then_steps,
            else_steps,
        } => {
            collect_refs_from_str(condition, refs);
            for s in then_steps {
                collect_var_references(s, refs);
            }
            for s in else_steps {
                collect_var_references(s, refs);
            }
        }
        LintStep::For {
            iterable, steps, ..
        } => {
            collect_refs_from_str(iterable, refs);
            for s in steps {
                collect_var_references(s, refs);
            }
        }
    }
}

/// Extract `$identifier` patterns from a string.
fn collect_refs_from_str(s: &str, refs: &mut std::collections::HashSet<String>) {
    // Match $identifier patterns (alphanumeric + underscore, starting with $)
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '$' {
            let mut var = String::from("$");
            while let Some(&next) = chars.peek() {
                if next.is_alphanumeric() || next == '_' {
                    var.push(next);
                    chars.next();
                } else {
                    break;
                }
            }
            if var.len() > 1 {
                refs.insert(var);
            }
        }
    }
}

/// Extract `$identifier` patterns from a JSON value (recursing into objects/arrays/strings).
fn collect_refs_from_value(val: &serde_json::Value, refs: &mut std::collections::HashSet<String>) {
    match val {
        serde_json::Value::String(s) => collect_refs_from_str(s, refs),
        serde_json::Value::Array(arr) => {
            for v in arr {
                collect_refs_from_value(v, refs);
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values() {
                collect_refs_from_value(v, refs);
            }
        }
        _ => {}
    }
}

/// PX-L006: Unused loop item variable — loop iterates but never references the item.
fn lint_unused_loop_item_var(
    over: &Option<String>,
    item_var: &str,
    key_var: &Option<String>,
    steps: &[LintStep],
    proc_name: &str,
    idx: usize,
    diags: &mut Vec<LintDiagnostic>,
) {
    // Only applies to `over` loops (not `times` loops which may just repeat N times)
    if over.is_none() {
        return;
    }

    let mut refs: std::collections::HashSet<String> = std::collections::HashSet::new();
    for step in steps {
        collect_var_references(step, &mut refs);
    }

    let item_ref = format!("${}", item_var);
    if !refs.contains(&item_ref) {
        diags.push(LintDiagnostic {
            code: "PX-L006",
            message: format!(
                "loop item variable `${}` is never referenced in loop body — consider using `times` instead of `over`",
                item_var
            ),
            severity: LintSeverity::Warning,
            procedure: Some(proc_name.to_string()),
            step_index: Some(idx),
        });
    }

    // Also check key_var if declared
    if let Some(kv) = key_var {
        let key_ref = format!("${}", kv);
        if !refs.contains(&key_ref) {
            diags.push(LintDiagnostic {
                code: "PX-L006",
                message: format!(
                    "loop key variable `${}` is declared but never referenced in loop body",
                    kv
                ),
                severity: LintSeverity::Warning,
                procedure: Some(proc_name.to_string()),
                step_index: Some(idx),
            });
        }
    }
}

/// PX-L007: Empty catch block — errors are silently swallowed.
fn lint_empty_catch(
    catch: &[LintStep],
    proc_name: &str,
    idx: usize,
    diags: &mut Vec<LintDiagnostic>,
) {
    if catch.is_empty() {
        diags.push(LintDiagnostic {
            code: "PX-L007",
            message: "try step has an empty catch block — errors will be silently swallowed"
                .to_string(),
            severity: LintSeverity::Warning,
            procedure: Some(proc_name.to_string()),
            step_index: Some(idx),
        });
    }
}

/// PX-L003: Unreachable arms after a wildcard `_`.
fn lint_match_unreachable(
    arms: &[LintMatchArm],
    proc_name: &str,
    idx: usize,
    diags: &mut Vec<LintDiagnostic>,
) {
    let mut wildcard_seen = false;
    for (arm_idx, arm) in arms.iter().enumerate() {
        let cond = arm.condition.trim();
        if wildcard_seen {
            diags.push(LintDiagnostic {
                code: "PX-L003",
                message: format!(
                    "arm {} is unreachable — wildcard `_` already covers all cases (arm {})",
                    arm_idx + 1,
                    arm_idx
                ),
                severity: LintSeverity::Warning,
                procedure: Some(proc_name.to_string()),
                step_index: Some(idx),
            });
        }
        if cond == "_" || cond.starts_with("_ ") {
            wildcard_seen = true;
        }
    }
}

/// PX-L008: Shadowed output variables — multiple steps bind to the same output_var name.
///
/// The later binding overwrites the earlier one, making the first call's output
/// inaccessible. This is usually a copy-paste bug.
fn lint_shadowed_output_vars(proc: &LintProc, diags: &mut Vec<LintDiagnostic>) {
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

    for (idx, step) in proc.steps.iter().enumerate() {
        if let Some(var) = step_output_var(step) {
            if let Some(&first_idx) = seen.get(var) {
                diags.push(LintDiagnostic {
                    code: "PX-L008",
                    message: format!(
                        "output variable `${}` is already bound by step {} — this binding shadows it",
                        var,
                        first_idx + 1
                    ),
                    severity: LintSeverity::Warning,
                    procedure: Some(proc.name.clone()),
                    step_index: Some(idx),
                });
            } else {
                seen.insert(var, idx);
            }
        }
    }
}

/// PX-L009: Detect unreachable steps after return/abort in procedure body.
///
/// A `return` or `abort` step unconditionally terminates execution.
/// Any subsequent steps at the same nesting level are unreachable.
fn lint_unreachable_after_terminal(proc: &LintProc, diags: &mut Vec<LintDiagnostic>) {
    check_steps_for_unreachable(&proc.steps, &proc.name, diags);
}

/// Check a step list for terminal steps followed by unreachable code.
fn check_steps_for_unreachable(
    steps: &[LintStep],
    proc_name: &str,
    diags: &mut Vec<LintDiagnostic>,
) {
    let mut found_terminal: Option<(usize, &'static str)> = None;

    for (idx, step) in steps.iter().enumerate() {
        if let Some((term_idx, term_kind)) = found_terminal {
            diags.push(LintDiagnostic {
                code: "PX-L009",
                message: format!(
                    "unreachable step after `{}` at step {}",
                    term_kind,
                    term_idx + 1
                ),
                severity: LintSeverity::Warning,
                procedure: Some(proc_name.to_string()),
                step_index: Some(idx),
            });
            continue;
        }

        match step {
            LintStep::Return { .. } => {
                found_terminal = Some((idx, "return"));
            }
            LintStep::Abort { .. } => {
                found_terminal = Some((idx, "abort"));
            }
            // Recurse into nested blocks
            LintStep::When { steps: inner, .. } => {
                check_steps_for_unreachable(inner, proc_name, diags);
            }
            LintStep::Loop { steps: inner, .. } => {
                check_steps_for_unreachable(inner, proc_name, diags);
            }
            LintStep::Try {
                steps: try_steps,
                catch,
                ..
            } => {
                check_steps_for_unreachable(try_steps, proc_name, diags);
                check_steps_for_unreachable(catch, proc_name, diags);
            }
            LintStep::Parallel { branches, .. } => {
                for branch in branches {
                    check_steps_for_unreachable(&branch.steps, proc_name, diags);
                }
            }
            _ => {}
        }
    }
}

/// PX-L010: Unused procedure parameters — declared in trigger params but never referenced.
///
/// When a procedure's trigger declares parameters (e.g., `trigger: on_event {channel: "string", message: "string"}`),
/// each param key should be referenced as `$key` somewhere in the procedure body.
/// Unreferenced params are likely dead code or indicate a typo.
fn lint_unused_procedure_params(proc: &LintProc, diags: &mut Vec<LintDiagnostic>) {
    // Extract parameter names from trigger params (if it's an object)
    let param_names: Vec<String> = match &proc.trigger {
        Some(trigger) => match &trigger.params {
            Some(serde_json::Value::Object(map)) => map.keys().cloned().collect(),
            _ => return,
        },
        None => return,
    };

    if param_names.is_empty() {
        return;
    }

    // Collect all variable references across the procedure body
    let mut references: std::collections::HashSet<String> = std::collections::HashSet::new();
    for step in &proc.steps {
        collect_var_references(step, &mut references);
    }

    // Also check the `given` clause for references
    if let Some(given) = &proc.given {
        collect_refs_from_str(given, &mut references);
    }

    // Check each param against references
    for param_name in &param_names {
        let var_ref = format!("${}", param_name);
        if !references.contains(&var_ref) {
            diags.push(LintDiagnostic {
                code: "PX-L010",
                message: format!(
                    "trigger parameter `{}` is declared but never referenced as `${}` in the procedure body",
                    param_name, param_name
                ),
                severity: LintSeverity::Warning,
                procedure: Some(proc.name.clone()),
                step_index: None,
            });
        }
    }
}

/// PX-L011: Undefined procedure calls — a Call step references a procedure not defined in this document.
///
/// Collects all procedure names, then walks all Call steps to check if their `name` matches
/// a known procedure. Unresolved calls likely indicate typos or missing imports.
fn lint_undefined_calls(doc: &LintDoc, diags: &mut Vec<LintDiagnostic>) {
    let known_procedures: std::collections::HashSet<&str> =
        doc.procedures.iter().map(|p| p.name.as_str()).collect();

    // Also consider functions as callable (they share the call namespace)
    let known_functions: std::collections::HashSet<&str> = doc
        .functions
        .iter()
        .map(|(name, _params)| name.as_str())
        .collect();

    for procedure in &doc.procedures {
        collect_undefined_calls_in_steps(
            &procedure.steps,
            &procedure.name,
            &known_procedures,
            &known_functions,
            diags,
        );
    }
}

/// Recursively walk steps looking for Call steps with undefined targets.
fn collect_undefined_calls_in_steps(
    steps: &[LintStep],
    proc_name: &str,
    known_procs: &std::collections::HashSet<&str>,
    known_fns: &std::collections::HashSet<&str>,
    diags: &mut Vec<LintDiagnostic>,
) {
    for (idx, step) in steps.iter().enumerate() {
        match step {
            LintStep::Call { name, .. } => {
                if !known_procs.contains(name.as_str()) && !known_fns.contains(name.as_str()) {
                    diags.push(LintDiagnostic {
                        code: "PX-L011",
                        message: format!("call to undefined procedure or function `{}`", name),
                        severity: LintSeverity::Error,
                        procedure: Some(proc_name.to_string()),
                        step_index: Some(idx),
                    });
                }
            }
            LintStep::When { steps: nested, .. } => {
                collect_undefined_calls_in_steps(nested, proc_name, known_procs, known_fns, diags);
            }
            LintStep::Loop { steps: nested, .. } => {
                collect_undefined_calls_in_steps(nested, proc_name, known_procs, known_fns, diags);
            }
            LintStep::Try { steps, catch, .. } => {
                collect_undefined_calls_in_steps(steps, proc_name, known_procs, known_fns, diags);
                collect_undefined_calls_in_steps(catch, proc_name, known_procs, known_fns, diags);
            }
            LintStep::Parallel { branches, .. } => {
                for branch in branches {
                    collect_undefined_calls_in_steps(
                        &branch.steps,
                        proc_name,
                        known_procs,
                        known_fns,
                        diags,
                    );
                }
            }
            LintStep::Match { arms: _ }
            | LintStep::Emit { .. }
            | LintStep::Return { .. }
            | LintStep::Abort { .. }
            | LintStep::Assign { .. } => {}
            LintStep::If {
                then_steps,
                else_steps,
                ..
            } => {
                collect_undefined_calls_in_steps(
                    then_steps,
                    proc_name,
                    known_procs,
                    known_fns,
                    diags,
                );
                collect_undefined_calls_in_steps(
                    else_steps,
                    proc_name,
                    known_procs,
                    known_fns,
                    diags,
                );
            }
            LintStep::For { steps: nested, .. } => {
                collect_undefined_calls_in_steps(nested, proc_name, known_procs, known_fns, diags);
            }
        }
    }
}

/// PX-L012: Arity mismatch — a Call step passes parameters not declared by the target procedure/function,
/// or the target declares parameters not provided by the call.
///
/// For intra-document calls only (targets that resolve to a procedure or function in this document).
/// - Extra params (passed but not declared): Warning — likely a typo or stale param.
/// - Missing params (declared but not passed): Warning — target may expect this value.
fn lint_arity_mismatch(doc: &LintDoc, diags: &mut Vec<LintDiagnostic>) {
    use std::collections::{HashMap, HashSet};

    // Build signature maps: name → set of declared param names
    let proc_params: HashMap<&str, HashSet<&str>> = doc
        .procedures
        .iter()
        .filter_map(|p| {
            let trigger = p.trigger.as_ref()?;
            let obj = trigger.params.as_ref()?.as_object()?;
            let keys: HashSet<&str> = obj.keys().map(|k| k.as_str()).collect();
            Some((p.name.as_str(), keys))
        })
        .collect();

    let fn_params: HashMap<&str, HashSet<&str>> = doc
        .functions
        .iter()
        .map(|(name, params)| {
            let keys: HashSet<&str> = params.iter().map(|p| p.as_str()).collect();
            (name.as_str(), keys)
        })
        .collect();

    for procedure in &doc.procedures {
        check_arity_in_steps(
            &procedure.steps,
            &procedure.name,
            &proc_params,
            &fn_params,
            diags,
        );
    }
}

/// Recursively walk steps checking arity for Call steps with known targets.
fn check_arity_in_steps(
    steps: &[LintStep],
    proc_name: &str,
    proc_params: &std::collections::HashMap<&str, std::collections::HashSet<&str>>,
    fn_params: &std::collections::HashMap<&str, std::collections::HashSet<&str>>,
    diags: &mut Vec<LintDiagnostic>,
) {
    for (idx, step) in steps.iter().enumerate() {
        match step {
            LintStep::Call { name, params, .. } => {
                // Find the target's declared params
                let declared = proc_params
                    .get(name.as_str())
                    .or_else(|| fn_params.get(name.as_str()));

                if let Some(declared_keys) = declared {
                    // Get the call's param keys (skip if params isn't an object)
                    if let Some(call_obj) = params.as_object() {
                        let call_keys: std::collections::HashSet<&str> =
                            call_obj.keys().map(|k| k.as_str()).collect();

                        // Extra params: in call but not in declaration
                        for extra in call_keys.difference(declared_keys) {
                            diags.push(LintDiagnostic {
                                code: "PX-L012",
                                message: format!(
                                    "call to `{}` passes unexpected parameter `{}` (not declared by target)",
                                    name, extra
                                ),
                                severity: LintSeverity::Warning,
                                procedure: Some(proc_name.to_string()),
                                step_index: Some(idx),
                            });
                        }

                        // Missing params: in declaration but not in call
                        for missing in declared_keys.difference(&call_keys) {
                            diags.push(LintDiagnostic {
                                code: "PX-L012",
                                message: format!(
                                    "call to `{}` is missing parameter `{}` (declared by target)",
                                    name, missing
                                ),
                                severity: LintSeverity::Warning,
                                procedure: Some(proc_name.to_string()),
                                step_index: Some(idx),
                            });
                        }
                    } else if !declared_keys.is_empty() && params.is_null() {
                        // Call passes no params (null) but target expects some
                        let missing: Vec<_> = declared_keys.iter().collect();
                        diags.push(LintDiagnostic {
                            code: "PX-L012",
                            message: format!(
                                "call to `{}` passes no parameters but target declares: {}",
                                name,
                                missing
                                    .iter()
                                    .map(|s| format!("`{}`", s))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                            severity: LintSeverity::Warning,
                            procedure: Some(proc_name.to_string()),
                            step_index: Some(idx),
                        });
                    }
                }
            }
            LintStep::When { steps: nested, .. } => {
                check_arity_in_steps(nested, proc_name, proc_params, fn_params, diags);
            }
            LintStep::Loop { steps: nested, .. } => {
                check_arity_in_steps(nested, proc_name, proc_params, fn_params, diags);
            }
            LintStep::Try { steps, catch, .. } => {
                check_arity_in_steps(steps, proc_name, proc_params, fn_params, diags);
                check_arity_in_steps(catch, proc_name, proc_params, fn_params, diags);
            }
            LintStep::Parallel { branches, .. } => {
                for branch in branches {
                    check_arity_in_steps(&branch.steps, proc_name, proc_params, fn_params, diags);
                }
            }
            LintStep::Match { arms: _ }
            | LintStep::Emit { .. }
            | LintStep::Return { .. }
            | LintStep::Abort { .. }
            | LintStep::Assign { .. } => {}
            LintStep::If {
                then_steps,
                else_steps,
                ..
            } => {
                check_arity_in_steps(then_steps, proc_name, proc_params, fn_params, diags);
                check_arity_in_steps(else_steps, proc_name, proc_params, fn_params, diags);
            }
            LintStep::For { steps: nested, .. } => {
                check_arity_in_steps(nested, proc_name, proc_params, fn_params, diags);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_doc() -> LintDoc {
        LintDoc::default()
    }

    fn make_proc(name: &str, steps: Vec<LintStep>) -> LintProc {
        LintProc {
            name: name.to_string(),
            trigger: Some(LintTrigger {
                kind: "manual".to_string(),
                params: None,
            }),
            given: None,
            steps,
        }
    }

    #[test]
    fn l001_empty_procedure() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc("empty", vec![]));

        let diags = lint_view(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "PX-L001");
        assert_eq!(diags[0].severity, LintSeverity::Warning);
        assert_eq!(diags[0].procedure.as_deref(), Some("empty"));
    }

    #[test]
    fn l002_non_exhaustive_match() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "handler",
            vec![LintStep::Match {
                arms: vec![
                    LintMatchArm {
                        condition: "status == \"active\"".to_string(),
                        result: "active".to_string(),
                    },
                    LintMatchArm {
                        condition: "status == \"inactive\"".to_string(),
                        result: "inactive".to_string(),
                    },
                ],
            }],
        ));

        let diags = lint_view(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "PX-L002");
        assert!(diags[0].message.contains("no wildcard"));
    }

    #[test]
    fn l002_exhaustive_match_no_warning() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "handler",
            vec![LintStep::Match {
                arms: vec![
                    LintMatchArm {
                        condition: "status == \"active\"".to_string(),
                        result: "active".to_string(),
                    },
                    LintMatchArm {
                        condition: "_".to_string(),
                        result: "unknown".to_string(),
                    },
                ],
            }],
        ));

        let diags = lint_view(&doc);
        assert!(diags.is_empty());
    }

    #[test]
    fn l003_unreachable_after_wildcard() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "handler",
            vec![LintStep::Match {
                arms: vec![
                    LintMatchArm {
                        condition: "status == \"active\"".to_string(),
                        result: "active".to_string(),
                    },
                    LintMatchArm {
                        condition: "_".to_string(),
                        result: "default".to_string(),
                    },
                    LintMatchArm {
                        condition: "status == \"pending\"".to_string(),
                        result: "pending".to_string(),
                    },
                ],
            }],
        ));

        let diags = lint_view(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "PX-L003");
        assert!(diags[0].message.contains("unreachable"));
    }

    #[test]
    fn lint_nested_match_in_loop() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "processor",
            vec![LintStep::Loop {
                over: Some("$items".to_string()),
                times: None,
                item_var: "item".to_string(),
                key_var: None,
                steps: vec![LintStep::Match {
                    arms: vec![LintMatchArm {
                        condition: "item.type == \"a\"".to_string(),
                        result: "handled".to_string(),
                    }],
                }],
                output_var: None,
            }],
        ));

        let diags = lint_view(&doc);
        // L002 for non-exhaustive match + L006 for unused $item (condition uses bare `item.type` not `$item`)
        assert_eq!(diags.len(), 2);
        assert!(diags.iter().any(|d| d.code == "PX-L002"));
        assert!(diags.iter().any(|d| d.code == "PX-L006"));
    }

    #[test]
    fn lint_no_issues_for_simple_procedure() {
        let mut doc = empty_doc();
        // Add target so L011 (undefined call) doesn't fire
        doc.procedures.push(make_proc(
            "greet",
            vec![LintStep::Emit {
                event: serde_json::json!({"type": "hello"}),
            }],
        ));
        doc.procedures.push(make_proc(
            "simple",
            vec![
                LintStep::Call {
                    name: "greet".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
                LintStep::Emit {
                    event: serde_json::json!({"type": "done"}),
                },
            ],
        ));

        let diags = lint_view(&doc);
        assert!(diags.is_empty());
    }

    #[test]
    fn display_format() {
        let diag = LintDiagnostic {
            code: "PX-L002",
            message: "match step has 2 arm(s) but no wildcard `_`".to_string(),
            severity: LintSeverity::Warning,
            procedure: Some("handler".to_string()),
            step_index: Some(0),
        };
        let s = format!("{}", diag);
        assert!(s.contains("[PX-L002]"));
        assert!(s.contains("warning"));
        assert!(s.contains("handler"));
        assert!(s.contains("step 1"));
    }

    #[test]
    fn l004_duplicate_arm_conditions() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "handler",
            vec![LintStep::Match {
                arms: vec![
                    LintMatchArm {
                        condition: "status == \"active\"".to_string(),
                        result: "first".to_string(),
                    },
                    LintMatchArm {
                        condition: "status == \"pending\"".to_string(),
                        result: "second".to_string(),
                    },
                    LintMatchArm {
                        condition: "status == \"active\"".to_string(),
                        result: "duplicate".to_string(),
                    },
                    LintMatchArm {
                        condition: "_".to_string(),
                        result: "default".to_string(),
                    },
                ],
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L004")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("same condition as arm 1"));
    }

    #[test]
    fn l004_no_false_positive_for_unique_arms() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "handler",
            vec![LintStep::Match {
                arms: vec![
                    LintMatchArm {
                        condition: "status == \"a\"".to_string(),
                        result: "a".to_string(),
                    },
                    LintMatchArm {
                        condition: "status == \"b\"".to_string(),
                        result: "b".to_string(),
                    },
                    LintMatchArm {
                        condition: "_".to_string(),
                        result: "default".to_string(),
                    },
                ],
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L004")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l005_unused_output_var() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                LintStep::Call {
                    name: "fetch_data".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("data".to_string()),
                },
                LintStep::Emit {
                    event: serde_json::json!({"type": "done"}),
                },
            ],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L005")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("$data"));
        assert!(diags[0].message.contains("never referenced"));
    }

    #[test]
    fn l005_no_warning_when_var_is_used() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                LintStep::Call {
                    name: "fetch_data".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("data".to_string()),
                },
                LintStep::Call {
                    name: "process".to_string(),
                    params: serde_json::json!({"input": "$data"}),
                    output_var: None,
                },
            ],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L005")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l005_var_used_in_loop_over() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                LintStep::Call {
                    name: "get_items".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("items".to_string()),
                },
                LintStep::Loop {
                    over: Some("$items".to_string()),
                    times: None,
                    item_var: "item".to_string(),
                    key_var: None,
                    steps: vec![LintStep::Emit {
                        event: serde_json::json!({"item": "$item"}),
                    }],
                    output_var: None,
                },
            ],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L005")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l005_var_used_in_when_condition() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                LintStep::Call {
                    name: "check".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("result".to_string()),
                },
                LintStep::When {
                    condition: "$result == true".to_string(),
                    steps: vec![LintStep::Emit {
                        event: serde_json::json!({"status": "ok"}),
                    }],
                },
            ],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L005")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn collect_refs_from_str_works() {
        let mut refs = std::collections::HashSet::new();
        collect_refs_from_str("hello $world and $foo_bar", &mut refs);
        assert!(refs.contains("$world"));
        assert!(refs.contains("$foo_bar"));
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn collect_refs_from_str_no_bare_dollar() {
        let mut refs = std::collections::HashSet::new();
        collect_refs_from_str("cost is $5 or $ nothing", &mut refs);
        // $5 starts with digit after $ but 5 is alphanumeric so it matches
        assert!(refs.contains("$5"));
        assert_eq!(refs.len(), 1);
    }

    #[test]
    fn l006_unused_loop_item_var() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "counter",
            vec![LintStep::Loop {
                over: Some("$items".to_string()),
                times: None,
                item_var: "item".to_string(),
                key_var: None,
                steps: vec![LintStep::Call {
                    name: "increment".to_string(),
                    params: serde_json::json!({"value": 1}),
                    output_var: None,
                }],
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L006")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("$item"));
        assert!(diags[0].message.contains("never referenced"));
    }

    #[test]
    fn l006_no_warning_when_item_used() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "processor",
            vec![LintStep::Loop {
                over: Some("$items".to_string()),
                times: None,
                item_var: "item".to_string(),
                key_var: None,
                steps: vec![LintStep::Call {
                    name: "process".to_string(),
                    params: serde_json::json!({"data": "$item"}),
                    output_var: None,
                }],
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L006")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l006_unused_key_var() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "mapper",
            vec![LintStep::Loop {
                over: Some("$map".to_string()),
                times: None,
                item_var: "val".to_string(),
                key_var: Some("key".to_string()),
                steps: vec![LintStep::Call {
                    name: "process".to_string(),
                    params: serde_json::json!({"data": "$val"}),
                    output_var: None,
                }],
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L006")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("$key"));
    }

    #[test]
    fn l006_no_warning_for_times_loop() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "repeater",
            vec![LintStep::Loop {
                over: None,
                times: Some(5),
                item_var: "i".to_string(),
                key_var: None,
                steps: vec![LintStep::Call {
                    name: "ping".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                }],
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L006")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l007_empty_catch_block() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "risky",
            vec![LintStep::Try {
                steps: vec![LintStep::Call {
                    name: "risky_op".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                }],
                catch: vec![],
                retry: None,
                retry_delay_ms: None,
                retry_backoff: None,
                retry_max_delay_ms: None,
                retry_jitter: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L007")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("silently swallowed"));
    }

    #[test]
    fn l007_no_warning_with_catch_steps() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "safe",
            vec![LintStep::Try {
                steps: vec![LintStep::Call {
                    name: "risky_op".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                }],
                catch: vec![LintStep::Emit {
                    event: serde_json::json!({"error": "handled"}),
                }],
                retry: None,
                retry_delay_ms: None,
                retry_backoff: None,
                retry_max_delay_ms: None,
                retry_jitter: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L007")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l008_shadowed_output_var() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                LintStep::Call {
                    name: "fetch_data".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("result".to_string()),
                },
                LintStep::Call {
                    name: "transform_data".to_string(),
                    params: serde_json::json!({"input": "$result"}),
                    output_var: Some("result".to_string()),
                },
            ],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L008")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("$result"));
        assert!(diags[0].message.contains("step 1"));
        assert_eq!(diags[0].step_index, Some(1));
    }

    #[test]
    fn l008_no_warning_for_unique_output_vars() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                LintStep::Call {
                    name: "fetch_data".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("data".to_string()),
                },
                LintStep::Call {
                    name: "transform".to_string(),
                    params: serde_json::json!({"input": "$data"}),
                    output_var: Some("transformed".to_string()),
                },
            ],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L008")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l008_multiple_shadows() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                LintStep::Call {
                    name: "step1".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("x".to_string()),
                },
                LintStep::Call {
                    name: "step2".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("x".to_string()),
                },
                LintStep::Call {
                    name: "step3".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("x".to_string()),
                },
            ],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L008")
            .collect();
        // Two shadows: step 2 shadows step 1, step 3 shadows step 1
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].step_index, Some(1));
        assert_eq!(diags[1].step_index, Some(2));
    }

    // === PX-L009: Unreachable steps after return/abort ===

    #[test]
    fn l009_unreachable_after_return() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "early_exit",
            vec![
                LintStep::Call {
                    name: "setup".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
                LintStep::Return {
                    value: Some(serde_json::json!("done")),
                },
                LintStep::Call {
                    name: "cleanup".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
            ],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L009")
            .collect();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].step_index, Some(2));
        assert!(diags[0].message.contains("return"));
        assert!(diags[0].message.contains("step 2"));
    }

    #[test]
    fn l009_unreachable_after_abort() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "fail_fast",
            vec![
                LintStep::Abort {
                    value: Some(serde_json::json!("fatal error")),
                },
                LintStep::Call {
                    name: "never_reached".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
                LintStep::Call {
                    name: "also_unreachable".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
            ],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L009")
            .collect();
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].step_index, Some(1));
        assert_eq!(diags[1].step_index, Some(2));
    }

    #[test]
    fn l009_no_warning_when_return_is_last() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "clean_exit",
            vec![
                LintStep::Call {
                    name: "work".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
                LintStep::Return { value: None },
            ],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L009")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l009_unreachable_in_nested_when_block() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "nested",
            vec![LintStep::When {
                condition: "$flag == true".to_string(),
                steps: vec![
                    LintStep::Return { value: None },
                    LintStep::Call {
                        name: "dead_code".to_string(),
                        params: serde_json::json!({}),
                        output_var: None,
                    },
                ],
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L009")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("return"));
    }

    // === PX-L010: Unused procedure parameters ===

    #[test]
    fn l010_unused_trigger_param() {
        let mut doc = empty_doc();
        doc.procedures.push(LintProc {
            name: "handler".to_string(),
            trigger: Some(LintTrigger {
                kind: "on_event".to_string(),
                params: Some(serde_json::json!({"channel": "string", "message": "string"})),
            }),
            given: None,
            steps: vec![LintStep::Call {
                name: "process".to_string(),
                params: serde_json::json!({"msg": "$message"}),
                output_var: None,
            }],
        });

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L010")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("channel"));
        assert!(diags[0].message.contains("never referenced"));
    }

    #[test]
    fn l010_no_warning_when_all_params_used() {
        let mut doc = empty_doc();
        doc.procedures.push(LintProc {
            name: "handler".to_string(),
            trigger: Some(LintTrigger {
                kind: "on_event".to_string(),
                params: Some(serde_json::json!({"channel": "string", "message": "string"})),
            }),
            given: None,
            steps: vec![LintStep::Call {
                name: "send".to_string(),
                params: serde_json::json!({"to": "$channel", "text": "$message"}),
                output_var: None,
            }],
        });

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L010")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l010_no_warning_without_trigger_params() {
        let mut doc = empty_doc();
        doc.procedures.push(LintProc {
            name: "handler".to_string(),
            trigger: Some(LintTrigger {
                kind: "manual".to_string(),
                params: None,
            }),
            given: None,
            steps: vec![LintStep::Call {
                name: "work".to_string(),
                params: serde_json::json!({}),
                output_var: None,
            }],
        });

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L010")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l010_param_used_in_given_clause() {
        let mut doc = empty_doc();
        doc.procedures.push(LintProc {
            name: "handler".to_string(),
            trigger: Some(LintTrigger {
                kind: "on_event".to_string(),
                params: Some(serde_json::json!({"priority": "string"})),
            }),
            given: Some("$priority == \"high\"".to_string()),
            steps: vec![LintStep::Call {
                name: "alert".to_string(),
                params: serde_json::json!({}),
                output_var: None,
            }],
        });

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L010")
            .collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l010_multiple_unused_params() {
        let mut doc = empty_doc();
        doc.procedures.push(LintProc {
            name: "handler".to_string(),
            trigger: Some(LintTrigger {
                kind: "webhook".to_string(),
                params: Some(
                    serde_json::json!({"url": "string", "method": "string", "body": "string"}),
                ),
            }),
            given: None,
            steps: vec![LintStep::Emit {
                event: serde_json::json!({"type": "received"}),
            }],
        });

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L010")
            .collect();
        assert_eq!(diags.len(), 3);
    }

    // === PX-L011: Undefined procedure calls ===

    #[test]
    fn lint_l011_undefined_call() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "caller",
            vec![LintStep::Call {
                name: "nonexistent_proc".to_string(),
                params: serde_json::json!({}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L011")
            .collect();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, LintSeverity::Error);
        assert!(diags[0].message.contains("nonexistent_proc"));
    }

    #[test]
    fn lint_l011_defined_call_no_diagnostic() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "helper",
            vec![LintStep::Emit {
                event: serde_json::json!({"done": true}),
            }],
        ));
        doc.procedures.push(make_proc(
            "caller",
            vec![LintStep::Call {
                name: "helper".to_string(),
                params: serde_json::json!({}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L011")
            .collect();
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn lint_l011_call_to_function_no_diagnostic() {
        let mut doc = empty_doc();
        doc.functions.push(("compute_hash".to_string(), Vec::new()));
        doc.procedures.push(make_proc(
            "caller",
            vec![LintStep::Call {
                name: "compute_hash".to_string(),
                params: serde_json::json!({"input": "data"}),
                output_var: Some("hash".to_string()),
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L011")
            .collect();
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn lint_l011_nested_undefined_call_in_when() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "outer",
            vec![LintStep::When {
                condition: "$x == true".to_string(),
                steps: vec![LintStep::Call {
                    name: "missing_fn".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                }],
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L011")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("missing_fn"));
    }

    #[test]
    fn lint_l011_nested_undefined_call_in_try_catch() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "handler",
            vec![LintStep::Try {
                steps: vec![LintStep::Call {
                    name: "ok_proc".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                }],
                catch: vec![LintStep::Call {
                    name: "fallback_missing".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                }],
                retry: None,
                retry_delay_ms: None,
                retry_backoff: None,
                retry_max_delay_ms: None,
                retry_jitter: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L011")
            .collect();
        // Both ok_proc and fallback_missing are undefined
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn lint_l011_self_recursive_call_no_diagnostic() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "recursive",
            vec![LintStep::Call {
                name: "recursive".to_string(),
                params: serde_json::json!({"depth": 1}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L011")
            .collect();
        assert_eq!(diags.len(), 0);
    }

    // === PX-L012: Arity mismatch ===

    #[test]
    fn lint_l012_extra_param_in_call() {
        let mut doc = empty_doc();
        // Target procedure declares {x, y}
        doc.procedures.push(LintProc {
            name: "target".to_string(),
            trigger: Some(LintTrigger {
                kind: "manual".to_string(),
                params: Some(serde_json::json!({"x": "number", "y": "number"})),
            }),
            given: None,
            steps: vec![LintStep::Emit {
                event: serde_json::json!({"done": true}),
            }],
        });
        // Caller passes {x, y, z} — z is extra
        doc.procedures.push(make_proc(
            "caller",
            vec![LintStep::Call {
                name: "target".to_string(),
                params: serde_json::json!({"x": 1, "y": 2, "z": 3}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L012")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`z`"));
        assert!(diags[0].message.contains("unexpected"));
        assert_eq!(diags[0].severity, LintSeverity::Warning);
    }

    #[test]
    fn lint_l012_missing_param_in_call() {
        let mut doc = empty_doc();
        // Target declares {x, y}
        doc.procedures.push(LintProc {
            name: "target".to_string(),
            trigger: Some(LintTrigger {
                kind: "manual".to_string(),
                params: Some(serde_json::json!({"x": "number", "y": "number"})),
            }),
            given: None,
            steps: vec![LintStep::Emit {
                event: serde_json::json!({"done": true}),
            }],
        });
        // Caller passes {x} only — y is missing
        doc.procedures.push(make_proc(
            "caller",
            vec![LintStep::Call {
                name: "target".to_string(),
                params: serde_json::json!({"x": 1}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L012")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`y`"));
        assert!(diags[0].message.contains("missing"));
    }

    #[test]
    fn lint_l012_exact_match_no_diagnostic() {
        let mut doc = empty_doc();
        doc.procedures.push(LintProc {
            name: "target".to_string(),
            trigger: Some(LintTrigger {
                kind: "manual".to_string(),
                params: Some(serde_json::json!({"x": "number", "y": "number"})),
            }),
            given: None,
            steps: vec![LintStep::Emit {
                event: serde_json::json!({"done": true}),
            }],
        });
        doc.procedures.push(make_proc(
            "caller",
            vec![LintStep::Call {
                name: "target".to_string(),
                params: serde_json::json!({"x": 1, "y": 2}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L012")
            .collect();
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn lint_l012_function_arity_mismatch() {
        let mut doc = empty_doc();
        doc.functions.push((
            "compute".to_string(),
            vec!["input".to_string(), "mode".to_string()],
        ));
        // Call passes {input, mode, extra}
        doc.procedures.push(make_proc(
            "caller",
            vec![LintStep::Call {
                name: "compute".to_string(),
                params: serde_json::json!({"input": "data", "mode": "fast", "extra": true}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L012")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`extra`"));
    }

    #[test]
    fn lint_l012_no_trigger_params_no_diagnostic() {
        let mut doc = empty_doc();
        // Target has no declared params (trigger without params)
        doc.procedures.push(LintProc {
            name: "target".to_string(),
            trigger: Some(LintTrigger {
                kind: "manual".to_string(),
                params: None,
            }),
            given: None,
            steps: vec![LintStep::Emit {
                event: serde_json::json!({"done": true}),
            }],
        });
        // Caller passes params — target has no signature so we can't check
        doc.procedures.push(make_proc(
            "caller",
            vec![LintStep::Call {
                name: "target".to_string(),
                params: serde_json::json!({"anything": 1}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L012")
            .collect();
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn lint_l012_null_params_with_declared_params() {
        let mut doc = empty_doc();
        doc.procedures.push(LintProc {
            name: "target".to_string(),
            trigger: Some(LintTrigger {
                kind: "manual".to_string(),
                params: Some(serde_json::json!({"required_param": "string"})),
            }),
            given: None,
            steps: vec![LintStep::Emit {
                event: serde_json::json!({"done": true}),
            }],
        });
        // Caller passes null (no object)
        doc.procedures.push(make_proc(
            "caller",
            vec![LintStep::Call {
                name: "target".to_string(),
                params: serde_json::Value::Null,
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L012")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("no parameters"));
        assert!(diags[0].message.contains("`required_param`"));
    }

    #[test]
    fn lint_l012_nested_call_in_when() {
        let mut doc = empty_doc();
        doc.procedures.push(LintProc {
            name: "target".to_string(),
            trigger: Some(LintTrigger {
                kind: "manual".to_string(),
                params: Some(serde_json::json!({"a": "number"})),
            }),
            given: None,
            steps: vec![LintStep::Emit {
                event: serde_json::json!({"done": true}),
            }],
        });
        doc.procedures.push(make_proc(
            "caller",
            vec![LintStep::When {
                condition: "$x == true".to_string(),
                steps: vec![LintStep::Call {
                    name: "target".to_string(),
                    params: serde_json::json!({"a": 1, "b": 2}),
                    output_var: None,
                }],
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L012")
            .collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`b`"));
    }

    #[test]
    fn lint_l012_external_call_no_diagnostic() {
        let mut doc = empty_doc();
        // Call to a procedure NOT in the document — L012 shouldn't fire (only L011 handles that)
        doc.procedures.push(make_proc(
            "caller",
            vec![LintStep::Call {
                name: "external_api".to_string(),
                params: serde_json::json!({"any": "thing"}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint_view(&doc)
            .into_iter()
            .filter(|d| d.code == "PX-L012")
            .collect();
        assert_eq!(diags.len(), 0);
    }
}
