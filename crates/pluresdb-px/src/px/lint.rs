//! Lint pass for .px documents — detects potential issues before execution.
//!
//! Produces warnings/errors for patterns that may cause runtime failures:
//! - Non-exhaustive match steps (no wildcard `_` arm)
//! - Empty procedure bodies
//! - Unreachable code after unconditional match arms

use super::{PxDocument, PxMatchArm, PxProcedure, PxStep};

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

/// Run all lint passes on a parsed document.
pub fn lint(doc: &PxDocument) -> Vec<LintDiagnostic> {
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
fn lint_procedure(proc: &PxProcedure, diags: &mut Vec<LintDiagnostic>) {
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
fn lint_step(step: &PxStep, proc_name: &str, idx: usize, diags: &mut Vec<LintDiagnostic>) {
    match step {
        PxStep::Match { arms } => {
            lint_match_exhaustiveness(arms, proc_name, idx, diags);
            lint_match_unreachable(arms, proc_name, idx, diags);
            lint_match_duplicate_conditions(arms, proc_name, idx, diags);
        }
        PxStep::Loop {
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
        PxStep::When { steps, .. } => {
            for (sub_idx, sub_step) in steps.iter().enumerate() {
                lint_step(sub_step, proc_name, sub_idx, diags);
            }
        }
        PxStep::Try { steps, catch, .. } => {
            lint_empty_catch(catch, proc_name, idx, diags);
            for (sub_idx, sub_step) in steps.iter().enumerate() {
                lint_step(sub_step, proc_name, sub_idx, diags);
            }
            for (sub_idx, sub_step) in catch.iter().enumerate() {
                lint_step(sub_step, proc_name, sub_idx, diags);
            }
        }
        PxStep::Parallel { branches, .. } => {
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
    arms: &[PxMatchArm],
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
    arms: &[PxMatchArm],
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
fn lint_unused_output_vars(proc: &PxProcedure, diags: &mut Vec<LintDiagnostic>) {
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
fn step_output_var(step: &PxStep) -> Option<&str> {
    match step {
        PxStep::Call { output_var, .. } => output_var.as_deref(),
        PxStep::Loop { output_var, .. } => output_var.as_deref(),
        PxStep::Parallel { output_var, .. } => output_var.as_deref(),
        _ => None,
    }
}

/// Recursively collect all `$variable` references from a step.
fn collect_var_references(step: &PxStep, refs: &mut std::collections::HashSet<String>) {
    match step {
        PxStep::Call { params, .. } => {
            collect_refs_from_value(params, refs);
        }
        PxStep::Match { arms } => {
            for arm in arms {
                collect_refs_from_str(&arm.condition, refs);
                collect_refs_from_str(&arm.result, refs);
            }
        }
        PxStep::When { condition, steps } => {
            collect_refs_from_str(condition, refs);
            for s in steps {
                collect_var_references(s, refs);
            }
        }
        PxStep::Loop { over, steps, .. } => {
            if let Some(over_expr) = over {
                collect_refs_from_str(over_expr, refs);
            }
            for s in steps {
                collect_var_references(s, refs);
            }
        }
        PxStep::Emit { event } => {
            collect_refs_from_value(event, refs);
        }
        PxStep::Try { steps, catch, .. } => {
            for s in steps {
                collect_var_references(s, refs);
            }
            for s in catch {
                collect_var_references(s, refs);
            }
        }
        PxStep::Parallel { branches, .. } => {
            for branch in branches {
                for s in &branch.steps {
                    collect_var_references(s, refs);
                }
            }
        }
        PxStep::Return { value } => {
            if let Some(v) = value {
                collect_refs_from_value(v, refs);
            }
        }
        PxStep::Abort { value } => {
            if let Some(v) = value {
                collect_refs_from_value(v, refs);
            }
        }
        PxStep::Assign { value, .. } => {
            collect_refs_from_str(value, refs);
        }
        PxStep::If { condition, then_steps, else_steps } => {
            collect_refs_from_str(condition, refs);
            for s in then_steps {
                collect_var_references(s, refs);
            }
            for s in else_steps {
                collect_var_references(s, refs);
            }
        }
        PxStep::For { iterable, steps, .. } => {
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
    steps: &[PxStep],
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
    catch: &[PxStep],
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
    arms: &[PxMatchArm],
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
fn lint_shadowed_output_vars(proc: &PxProcedure, diags: &mut Vec<LintDiagnostic>) {
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
fn lint_unreachable_after_terminal(proc: &PxProcedure, diags: &mut Vec<LintDiagnostic>) {
    check_steps_for_unreachable(&proc.steps, &proc.name, diags);
}

/// Check a step list for terminal steps followed by unreachable code.
fn check_steps_for_unreachable(
    steps: &[PxStep],
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
            PxStep::Return { .. } => {
                found_terminal = Some((idx, "return"));
            }
            PxStep::Abort { .. } => {
                found_terminal = Some((idx, "abort"));
            }
            // Recurse into nested blocks
            PxStep::When { steps: inner, .. } => {
                check_steps_for_unreachable(inner, proc_name, diags);
            }
            PxStep::Loop { steps: inner, .. } => {
                check_steps_for_unreachable(inner, proc_name, diags);
            }
            PxStep::Try { steps: try_steps, catch, .. } => {
                check_steps_for_unreachable(try_steps, proc_name, diags);
                check_steps_for_unreachable(catch, proc_name, diags);
            }
            PxStep::Parallel { branches, .. } => {
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
fn lint_unused_procedure_params(proc: &PxProcedure, diags: &mut Vec<LintDiagnostic>) {
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
fn lint_undefined_calls(doc: &PxDocument, diags: &mut Vec<LintDiagnostic>) {
    let known_procedures: std::collections::HashSet<&str> = doc
        .procedures
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    // Also consider functions as callable (they share the call namespace)
    let known_functions: std::collections::HashSet<&str> = doc
        .functions
        .iter()
        .map(|f| f.name.as_str())
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
    steps: &[PxStep],
    proc_name: &str,
    known_procs: &std::collections::HashSet<&str>,
    known_fns: &std::collections::HashSet<&str>,
    diags: &mut Vec<LintDiagnostic>,
) {
    for (idx, step) in steps.iter().enumerate() {
        match step {
            PxStep::Call { name, .. } => {
                if !known_procs.contains(name.as_str())
                    && !known_fns.contains(name.as_str())
                {
                    diags.push(LintDiagnostic {
                        code: "PX-L011",
                        message: format!(
                            "call to undefined procedure or function `{}`",
                            name
                        ),
                        severity: LintSeverity::Error,
                        procedure: Some(proc_name.to_string()),
                        step_index: Some(idx),
                    });
                }
            }
            PxStep::When { steps: nested, .. } => {
                collect_undefined_calls_in_steps(
                    nested, proc_name, known_procs, known_fns, diags,
                );
            }
            PxStep::Loop { steps: nested, .. } => {
                collect_undefined_calls_in_steps(
                    nested, proc_name, known_procs, known_fns, diags,
                );
            }
            PxStep::Try { steps, catch, .. } => {
                collect_undefined_calls_in_steps(
                    steps, proc_name, known_procs, known_fns, diags,
                );
                collect_undefined_calls_in_steps(
                    catch, proc_name, known_procs, known_fns, diags,
                );
            }
            PxStep::Parallel { branches, .. } => {
                for branch in branches {
                    collect_undefined_calls_in_steps(
                        &branch.steps, proc_name, known_procs, known_fns, diags,
                    );
                }
            }
            PxStep::Match { arms: _ }
            | PxStep::Emit { .. }
            | PxStep::Return { .. }
            | PxStep::Abort { .. }
            | PxStep::Assign { .. } => {}
            PxStep::If { then_steps, else_steps, .. } => {
                collect_undefined_calls_in_steps(
                    then_steps, proc_name, known_procs, known_fns, diags,
                );
                collect_undefined_calls_in_steps(
                    else_steps, proc_name, known_procs, known_fns, diags,
                );
            }
            PxStep::For { steps: nested, .. } => {
                collect_undefined_calls_in_steps(
                    nested, proc_name, known_procs, known_fns, diags,
                );
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
fn lint_arity_mismatch(doc: &PxDocument, diags: &mut Vec<LintDiagnostic>) {
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
        .map(|f| {
            let keys: HashSet<&str> = f.params.iter().map(|p| p.name.as_str()).collect();
            (f.name.as_str(), keys)
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
    steps: &[PxStep],
    proc_name: &str,
    proc_params: &std::collections::HashMap<&str, std::collections::HashSet<&str>>,
    fn_params: &std::collections::HashMap<&str, std::collections::HashSet<&str>>,
    diags: &mut Vec<LintDiagnostic>,
) {
    for (idx, step) in steps.iter().enumerate() {
        match step {
            PxStep::Call { name, params, .. } => {
                // Find the target's declared params
                let declared = proc_params.get(name.as_str())
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
                                missing.iter().map(|s| format!("`{}`", s)).collect::<Vec<_>>().join(", ")
                            ),
                            severity: LintSeverity::Warning,
                            procedure: Some(proc_name.to_string()),
                            step_index: Some(idx),
                        });
                    }
                }
            }
            PxStep::When { steps: nested, .. } => {
                check_arity_in_steps(nested, proc_name, proc_params, fn_params, diags);
            }
            PxStep::Loop { steps: nested, .. } => {
                check_arity_in_steps(nested, proc_name, proc_params, fn_params, diags);
            }
            PxStep::Try { steps, catch, .. } => {
                check_arity_in_steps(steps, proc_name, proc_params, fn_params, diags);
                check_arity_in_steps(catch, proc_name, proc_params, fn_params, diags);
            }
            PxStep::Parallel { branches, .. } => {
                for branch in branches {
                    check_arity_in_steps(&branch.steps, proc_name, proc_params, fn_params, diags);
                }
            }
            PxStep::Match { arms: _ }
            | PxStep::Emit { .. }
            | PxStep::Return { .. }
            | PxStep::Abort { .. }
            | PxStep::Assign { .. } => {}
            PxStep::If { then_steps, else_steps, .. } => {
                check_arity_in_steps(then_steps, proc_name, proc_params, fn_params, diags);
                check_arity_in_steps(else_steps, proc_name, proc_params, fn_params, diags);
            }
            PxStep::For { steps: nested, .. } => {
                check_arity_in_steps(nested, proc_name, proc_params, fn_params, diags);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::px::{PxDocument, PxMatchArm, PxProcedure, PxProcedureTrigger, PxStep};

    fn empty_doc() -> PxDocument {
        PxDocument {
            imports: vec![],
            configs: vec![],
            entities: vec![],
            facts: vec![],
            rules: vec![],
            constraints: vec![],
            contracts: vec![],
            functions: vec![],
            triggers: vec![],
            procedures: vec![],
            dataflow_procedures: vec![],
            scenarios: vec![],
        }
    }

    fn make_proc(name: &str, steps: Vec<PxStep>) -> PxProcedure {
        PxProcedure {
            name: name.to_string(),
            trigger: Some(PxProcedureTrigger {
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

        let diags = lint(&doc);
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
            vec![PxStep::Match {
                arms: vec![
                    PxMatchArm {
                        condition: "status == \"active\"".to_string(),
                        result: "active".to_string(),
                    },
                    PxMatchArm {
                        condition: "status == \"inactive\"".to_string(),
                        result: "inactive".to_string(),
                    },
                ],
            }],
        ));

        let diags = lint(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "PX-L002");
        assert!(diags[0].message.contains("no wildcard"));
    }

    #[test]
    fn l002_exhaustive_match_no_warning() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "handler",
            vec![PxStep::Match {
                arms: vec![
                    PxMatchArm {
                        condition: "status == \"active\"".to_string(),
                        result: "active".to_string(),
                    },
                    PxMatchArm {
                        condition: "_".to_string(),
                        result: "unknown".to_string(),
                    },
                ],
            }],
        ));

        let diags = lint(&doc);
        assert!(diags.is_empty());
    }

    #[test]
    fn l003_unreachable_after_wildcard() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "handler",
            vec![PxStep::Match {
                arms: vec![
                    PxMatchArm {
                        condition: "status == \"active\"".to_string(),
                        result: "active".to_string(),
                    },
                    PxMatchArm {
                        condition: "_".to_string(),
                        result: "default".to_string(),
                    },
                    PxMatchArm {
                        condition: "status == \"pending\"".to_string(),
                        result: "pending".to_string(),
                    },
                ],
            }],
        ));

        let diags = lint(&doc);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "PX-L003");
        assert!(diags[0].message.contains("unreachable"));
    }

    #[test]
    fn lint_nested_match_in_loop() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "processor",
            vec![PxStep::Loop {
                over: Some("$items".to_string()),
                times: None,
                item_var: "item".to_string(),
                key_var: None,
                steps: vec![PxStep::Match {
                    arms: vec![PxMatchArm {
                        condition: "item.type == \"a\"".to_string(),
                        result: "handled".to_string(),
                    }],
                }],
                output_var: None,
            }],
        ));

        let diags = lint(&doc);
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
            vec![PxStep::Emit {
                event: serde_json::json!({"type": "hello"}),
            }],
        ));
        doc.procedures.push(make_proc(
            "simple",
            vec![
                PxStep::Call {
                    name: "greet".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
                PxStep::Emit {
                    event: serde_json::json!({"type": "done"}),
                },
            ],
        ));

        let diags = lint(&doc);
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
            vec![PxStep::Match {
                arms: vec![
                    PxMatchArm {
                        condition: "status == \"active\"".to_string(),
                        result: "first".to_string(),
                    },
                    PxMatchArm {
                        condition: "status == \"pending\"".to_string(),
                        result: "second".to_string(),
                    },
                    PxMatchArm {
                        condition: "status == \"active\"".to_string(),
                        result: "duplicate".to_string(),
                    },
                    PxMatchArm {
                        condition: "_".to_string(),
                        result: "default".to_string(),
                    },
                ],
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L004").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("same condition as arm 1"));
    }

    #[test]
    fn l004_no_false_positive_for_unique_arms() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "handler",
            vec![PxStep::Match {
                arms: vec![
                    PxMatchArm {
                        condition: "status == \"a\"".to_string(),
                        result: "a".to_string(),
                    },
                    PxMatchArm {
                        condition: "status == \"b\"".to_string(),
                        result: "b".to_string(),
                    },
                    PxMatchArm {
                        condition: "_".to_string(),
                        result: "default".to_string(),
                    },
                ],
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L004").collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l005_unused_output_var() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                PxStep::Call {
                    name: "fetch_data".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("data".to_string()),
                },
                PxStep::Emit {
                    event: serde_json::json!({"type": "done"}),
                },
            ],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L005").collect();
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
                PxStep::Call {
                    name: "fetch_data".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("data".to_string()),
                },
                PxStep::Call {
                    name: "process".to_string(),
                    params: serde_json::json!({"input": "$data"}),
                    output_var: None,
                },
            ],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L005").collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l005_var_used_in_loop_over() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                PxStep::Call {
                    name: "get_items".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("items".to_string()),
                },
                PxStep::Loop {
                    over: Some("$items".to_string()),
                    times: None,
                    item_var: "item".to_string(),
                    key_var: None,
                    steps: vec![PxStep::Emit {
                        event: serde_json::json!({"item": "$item"}),
                    }],
                    output_var: None,
                },
            ],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L005").collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l005_var_used_in_when_condition() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                PxStep::Call {
                    name: "check".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("result".to_string()),
                },
                PxStep::When {
                    condition: "$result == true".to_string(),
                    steps: vec![PxStep::Emit {
                        event: serde_json::json!({"status": "ok"}),
                    }],
                },
            ],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L005").collect();
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
            vec![PxStep::Loop {
                over: Some("$items".to_string()),
                times: None,
                item_var: "item".to_string(),
                key_var: None,
                steps: vec![PxStep::Call {
                    name: "increment".to_string(),
                    params: serde_json::json!({"value": 1}),
                    output_var: None,
                }],
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L006").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("$item"));
        assert!(diags[0].message.contains("never referenced"));
    }

    #[test]
    fn l006_no_warning_when_item_used() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "processor",
            vec![PxStep::Loop {
                over: Some("$items".to_string()),
                times: None,
                item_var: "item".to_string(),
                key_var: None,
                steps: vec![PxStep::Call {
                    name: "process".to_string(),
                    params: serde_json::json!({"data": "$item"}),
                    output_var: None,
                }],
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L006").collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l006_unused_key_var() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "mapper",
            vec![PxStep::Loop {
                over: Some("$map".to_string()),
                times: None,
                item_var: "val".to_string(),
                key_var: Some("key".to_string()),
                steps: vec![PxStep::Call {
                    name: "process".to_string(),
                    params: serde_json::json!({"data": "$val"}),
                    output_var: None,
                }],
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L006").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("$key"));
    }

    #[test]
    fn l006_no_warning_for_times_loop() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "repeater",
            vec![PxStep::Loop {
                over: None,
                times: Some(5),
                item_var: "i".to_string(),
                key_var: None,
                steps: vec![PxStep::Call {
                    name: "ping".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                }],
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L006").collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l007_empty_catch_block() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "risky",
            vec![PxStep::Try {
                steps: vec![PxStep::Call {
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

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L007").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("silently swallowed"));
    }

    #[test]
    fn l007_no_warning_with_catch_steps() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "safe",
            vec![PxStep::Try {
                steps: vec![PxStep::Call {
                    name: "risky_op".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                }],
                catch: vec![PxStep::Emit {
                    event: serde_json::json!({"error": "handled"}),
                }],
                retry: None,
                retry_delay_ms: None,
                retry_backoff: None,
                retry_max_delay_ms: None,
                retry_jitter: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L007").collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l008_shadowed_output_var() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                PxStep::Call {
                    name: "fetch_data".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("result".to_string()),
                },
                PxStep::Call {
                    name: "transform_data".to_string(),
                    params: serde_json::json!({"input": "$result"}),
                    output_var: Some("result".to_string()),
                },
            ],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L008").collect();
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
                PxStep::Call {
                    name: "fetch_data".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("data".to_string()),
                },
                PxStep::Call {
                    name: "transform".to_string(),
                    params: serde_json::json!({"input": "$data"}),
                    output_var: Some("transformed".to_string()),
                },
            ],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L008").collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l008_multiple_shadows() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "pipeline",
            vec![
                PxStep::Call {
                    name: "step1".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("x".to_string()),
                },
                PxStep::Call {
                    name: "step2".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("x".to_string()),
                },
                PxStep::Call {
                    name: "step3".to_string(),
                    params: serde_json::json!({}),
                    output_var: Some("x".to_string()),
                },
            ],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L008").collect();
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
                PxStep::Call {
                    name: "setup".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
                PxStep::Return { value: Some(serde_json::json!("done")) },
                PxStep::Call {
                    name: "cleanup".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
            ],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L009").collect();
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
                PxStep::Abort { value: Some(serde_json::json!("fatal error")) },
                PxStep::Call {
                    name: "never_reached".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
                PxStep::Call {
                    name: "also_unreachable".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
            ],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L009").collect();
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
                PxStep::Call {
                    name: "work".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
                PxStep::Return { value: None },
            ],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L009").collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l009_unreachable_in_nested_when_block() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "nested",
            vec![PxStep::When {
                condition: "$flag == true".to_string(),
                steps: vec![
                    PxStep::Return { value: None },
                    PxStep::Call {
                        name: "dead_code".to_string(),
                        params: serde_json::json!({}),
                        output_var: None,
                    },
                ],
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L009").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("return"));
    }

    // === PX-L010: Unused procedure parameters ===

    #[test]
    fn l010_unused_trigger_param() {
        let mut doc = empty_doc();
        doc.procedures.push(PxProcedure {
            name: "handler".to_string(),
            trigger: Some(PxProcedureTrigger {
                kind: "on_event".to_string(),
                params: Some(serde_json::json!({"channel": "string", "message": "string"})),
            }),
            given: None,
            steps: vec![
                PxStep::Call {
                    name: "process".to_string(),
                    params: serde_json::json!({"msg": "$message"}),
                    output_var: None,
                },
            ],
        });

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L010").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("channel"));
        assert!(diags[0].message.contains("never referenced"));
    }

    #[test]
    fn l010_no_warning_when_all_params_used() {
        let mut doc = empty_doc();
        doc.procedures.push(PxProcedure {
            name: "handler".to_string(),
            trigger: Some(PxProcedureTrigger {
                kind: "on_event".to_string(),
                params: Some(serde_json::json!({"channel": "string", "message": "string"})),
            }),
            given: None,
            steps: vec![
                PxStep::Call {
                    name: "send".to_string(),
                    params: serde_json::json!({"to": "$channel", "text": "$message"}),
                    output_var: None,
                },
            ],
        });

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L010").collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l010_no_warning_without_trigger_params() {
        let mut doc = empty_doc();
        doc.procedures.push(PxProcedure {
            name: "handler".to_string(),
            trigger: Some(PxProcedureTrigger {
                kind: "manual".to_string(),
                params: None,
            }),
            given: None,
            steps: vec![
                PxStep::Call {
                    name: "work".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
            ],
        });

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L010").collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l010_param_used_in_given_clause() {
        let mut doc = empty_doc();
        doc.procedures.push(PxProcedure {
            name: "handler".to_string(),
            trigger: Some(PxProcedureTrigger {
                kind: "on_event".to_string(),
                params: Some(serde_json::json!({"priority": "string"})),
            }),
            given: Some("$priority == \"high\"".to_string()),
            steps: vec![
                PxStep::Call {
                    name: "alert".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                },
            ],
        });

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L010").collect();
        assert!(diags.is_empty());
    }

    #[test]
    fn l010_multiple_unused_params() {
        let mut doc = empty_doc();
        doc.procedures.push(PxProcedure {
            name: "handler".to_string(),
            trigger: Some(PxProcedureTrigger {
                kind: "webhook".to_string(),
                params: Some(serde_json::json!({"url": "string", "method": "string", "body": "string"})),
            }),
            given: None,
            steps: vec![
                PxStep::Emit {
                    event: serde_json::json!({"type": "received"}),
                },
            ],
        });

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L010").collect();
        assert_eq!(diags.len(), 3);
    }

    // === PX-L011: Undefined procedure calls ===

    #[test]
    fn lint_l011_undefined_call() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "caller",
            vec![PxStep::Call {
                name: "nonexistent_proc".to_string(),
                params: serde_json::json!({}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L011").collect();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, LintSeverity::Error);
        assert!(diags[0].message.contains("nonexistent_proc"));
    }

    #[test]
    fn lint_l011_defined_call_no_diagnostic() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "helper",
            vec![PxStep::Emit {
                event: serde_json::json!({"done": true}),
            }],
        ));
        doc.procedures.push(make_proc(
            "caller",
            vec![PxStep::Call {
                name: "helper".to_string(),
                params: serde_json::json!({}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L011").collect();
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn lint_l011_call_to_function_no_diagnostic() {
        let mut doc = empty_doc();
        doc.functions.push(crate::px::PxFunction {
            name: "compute_hash".to_string(),
            params: vec![],
            return_type: "string".to_string(),
            mode: crate::px::FunctionMode::Deterministic,
            docstring: String::new(),
        });
        doc.procedures.push(make_proc(
            "caller",
            vec![PxStep::Call {
                name: "compute_hash".to_string(),
                params: serde_json::json!({"input": "data"}),
                output_var: Some("hash".to_string()),
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L011").collect();
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn lint_l011_nested_undefined_call_in_when() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "outer",
            vec![PxStep::When {
                condition: "$x == true".to_string(),
                steps: vec![PxStep::Call {
                    name: "missing_fn".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                }],
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L011").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("missing_fn"));
    }

    #[test]
    fn lint_l011_nested_undefined_call_in_try_catch() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "handler",
            vec![PxStep::Try {
                steps: vec![PxStep::Call {
                    name: "ok_proc".to_string(),
                    params: serde_json::json!({}),
                    output_var: None,
                }],
                catch: vec![PxStep::Call {
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

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L011").collect();
        // Both ok_proc and fallback_missing are undefined
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn lint_l011_self_recursive_call_no_diagnostic() {
        let mut doc = empty_doc();
        doc.procedures.push(make_proc(
            "recursive",
            vec![PxStep::Call {
                name: "recursive".to_string(),
                params: serde_json::json!({"depth": 1}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L011").collect();
        assert_eq!(diags.len(), 0);
    }

    // === PX-L012: Arity mismatch ===

    #[test]
    fn lint_l012_extra_param_in_call() {
        let mut doc = empty_doc();
        // Target procedure declares {x, y}
        doc.procedures.push(PxProcedure {
            name: "target".to_string(),
            trigger: Some(PxProcedureTrigger {
                kind: "manual".to_string(),
                params: Some(serde_json::json!({"x": "number", "y": "number"})),
            }),
            given: None,
            steps: vec![PxStep::Emit { event: serde_json::json!({"done": true}) }],
        });
        // Caller passes {x, y, z} — z is extra
        doc.procedures.push(make_proc(
            "caller",
            vec![PxStep::Call {
                name: "target".to_string(),
                params: serde_json::json!({"x": 1, "y": 2, "z": 3}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L012").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`z`"));
        assert!(diags[0].message.contains("unexpected"));
        assert_eq!(diags[0].severity, LintSeverity::Warning);
    }

    #[test]
    fn lint_l012_missing_param_in_call() {
        let mut doc = empty_doc();
        // Target declares {x, y}
        doc.procedures.push(PxProcedure {
            name: "target".to_string(),
            trigger: Some(PxProcedureTrigger {
                kind: "manual".to_string(),
                params: Some(serde_json::json!({"x": "number", "y": "number"})),
            }),
            given: None,
            steps: vec![PxStep::Emit { event: serde_json::json!({"done": true}) }],
        });
        // Caller passes {x} only — y is missing
        doc.procedures.push(make_proc(
            "caller",
            vec![PxStep::Call {
                name: "target".to_string(),
                params: serde_json::json!({"x": 1}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L012").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`y`"));
        assert!(diags[0].message.contains("missing"));
    }

    #[test]
    fn lint_l012_exact_match_no_diagnostic() {
        let mut doc = empty_doc();
        doc.procedures.push(PxProcedure {
            name: "target".to_string(),
            trigger: Some(PxProcedureTrigger {
                kind: "manual".to_string(),
                params: Some(serde_json::json!({"x": "number", "y": "number"})),
            }),
            given: None,
            steps: vec![PxStep::Emit { event: serde_json::json!({"done": true}) }],
        });
        doc.procedures.push(make_proc(
            "caller",
            vec![PxStep::Call {
                name: "target".to_string(),
                params: serde_json::json!({"x": 1, "y": 2}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L012").collect();
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn lint_l012_function_arity_mismatch() {
        let mut doc = empty_doc();
        doc.functions.push(crate::px::PxFunction {
            name: "compute".to_string(),
            params: vec![
                crate::px::PxField { name: "input".to_string(), type_expr: "string".to_string() },
                crate::px::PxField { name: "mode".to_string(), type_expr: "string".to_string() },
            ],
            return_type: "string".to_string(),
            mode: crate::px::FunctionMode::Deterministic,
            docstring: String::new(),
        });
        // Call passes {input, mode, extra}
        doc.procedures.push(make_proc(
            "caller",
            vec![PxStep::Call {
                name: "compute".to_string(),
                params: serde_json::json!({"input": "data", "mode": "fast", "extra": true}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L012").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`extra`"));
    }

    #[test]
    fn lint_l012_no_trigger_params_no_diagnostic() {
        let mut doc = empty_doc();
        // Target has no declared params (trigger without params)
        doc.procedures.push(PxProcedure {
            name: "target".to_string(),
            trigger: Some(PxProcedureTrigger {
                kind: "manual".to_string(),
                params: None,
            }),
            given: None,
            steps: vec![PxStep::Emit { event: serde_json::json!({"done": true}) }],
        });
        // Caller passes params — target has no signature so we can't check
        doc.procedures.push(make_proc(
            "caller",
            vec![PxStep::Call {
                name: "target".to_string(),
                params: serde_json::json!({"anything": 1}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L012").collect();
        assert_eq!(diags.len(), 0);
    }

    #[test]
    fn lint_l012_null_params_with_declared_params() {
        let mut doc = empty_doc();
        doc.procedures.push(PxProcedure {
            name: "target".to_string(),
            trigger: Some(PxProcedureTrigger {
                kind: "manual".to_string(),
                params: Some(serde_json::json!({"required_param": "string"})),
            }),
            given: None,
            steps: vec![PxStep::Emit { event: serde_json::json!({"done": true}) }],
        });
        // Caller passes null (no object)
        doc.procedures.push(make_proc(
            "caller",
            vec![PxStep::Call {
                name: "target".to_string(),
                params: serde_json::Value::Null,
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L012").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("no parameters"));
        assert!(diags[0].message.contains("`required_param`"));
    }

    #[test]
    fn lint_l012_nested_call_in_when() {
        let mut doc = empty_doc();
        doc.procedures.push(PxProcedure {
            name: "target".to_string(),
            trigger: Some(PxProcedureTrigger {
                kind: "manual".to_string(),
                params: Some(serde_json::json!({"a": "number"})),
            }),
            given: None,
            steps: vec![PxStep::Emit { event: serde_json::json!({"done": true}) }],
        });
        doc.procedures.push(make_proc(
            "caller",
            vec![PxStep::When {
                condition: "$x == true".to_string(),
                steps: vec![PxStep::Call {
                    name: "target".to_string(),
                    params: serde_json::json!({"a": 1, "b": 2}),
                    output_var: None,
                }],
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L012").collect();
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("`b`"));
    }

    #[test]
    fn lint_l012_external_call_no_diagnostic() {
        let mut doc = empty_doc();
        // Call to a procedure NOT in the document — L012 shouldn't fire (only L011 handles that)
        doc.procedures.push(make_proc(
            "caller",
            vec![PxStep::Call {
                name: "external_api".to_string(),
                params: serde_json::json!({"any": "thing"}),
                output_var: None,
            }],
        ));

        let diags: Vec<_> = lint(&doc).into_iter().filter(|d| d.code == "PX-L012").collect();
        assert_eq!(diags.len(), 0);
    }
}
