//! Procedure executor — runs compiled `.px` procedures step by step.
//!
//! The executor takes a compiled procedure (as a `serde_json::Value` record
//! from the compiler) and walks its steps, resolving calls through a
//! pluggable [`ActionHandler`] trait, evaluating `when` guards, and
//! matching on conditions.
//!
//! # Architecture
//!
//! ```text
//! PxDocument ──► compiler ──► CompiledRecord (JSON) ──► Executor
//!                                                          │
//!                                                    ActionHandler
//!                                                    (pluggable)
//! ```
//!
//! The executor is intentionally model-agnostic: it doesn't know about LLMs,
//! HTTP, or any specific runtime. The [`ActionHandler`] trait is the
//! integration point where the host system (pares-radix core, MCP server,
//! etc.) provides concrete implementations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Action Handler Trait ──────────────────────────────────────────────────────

/// Trait for handling procedure step calls.
///
/// Implementors provide the actual side-effects (API calls, tool invocations,
/// state mutations) that procedure steps reference by name.
pub trait ActionHandler: Send + Sync {
    /// Execute a named action with the given parameters.
    ///
    /// Returns a JSON value representing the result, which may be bound to
    /// an output variable for subsequent steps.
    fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError>;

    /// Evaluate a condition expression against the current execution context.
    ///
    /// Returns `true` if the condition is satisfied. The default implementation
    /// does simple equality checks against the variable bindings passed in
    /// `vars`. Override for richer expression evaluation.
    fn evaluate_condition(&self, expr: &str, vars: &HashMap<String, Value>) -> bool {
        default_evaluate_condition(expr, vars)
    }
}

// ── Execution Types ───────────────────────────────────────────────────────────

/// The result of executing a procedure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Name of the procedure that was executed.
    pub procedure_name: String,
    /// Results of each step, in order.
    pub step_results: Vec<StepResult>,
    /// Final variable bindings after execution.
    pub variables: HashMap<String, Value>,
    /// Whether the procedure completed successfully.
    pub success: bool,
    /// Error message if the procedure failed.
    pub error: Option<String>,
}

/// The result of executing a single step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Which step index this corresponds to.
    pub index: usize,
    /// The kind of step that was executed.
    pub kind: String,
    /// The output value (if any).
    pub output: Option<Value>,
    /// Whether this step was skipped (e.g., `when` guard failed).
    pub skipped: bool,
}

/// Errors that can occur during procedure execution.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ExecutionError {
    /// A called action is not registered in the handler.
    #[error("unknown action: {0}")]
    UnknownAction(String),

    /// A called action failed.
    #[error("action '{action}' failed: {message}")]
    ActionFailed { action: String, message: String },

    /// The procedure record has an invalid structure.
    #[error("invalid procedure structure: {0}")]
    InvalidStructure(String),

    /// A variable referenced in a step was not bound.
    #[error("unbound variable: {0}")]
    UnboundVariable(String),

    /// A match step had no matching arm.
    #[error("no matching arm in match step")]
    NoMatchingArm,

    /// The procedure was explicitly aborted.
    #[error("aborted: {0}")]
    Aborted(String),
}

// ── Executor ──────────────────────────────────────────────────────────────────

/// Executes a compiled procedure record.
///
/// The `record_data` parameter is the `data` field of a `CompiledRecord`
/// with `type: "procedure"`.
pub fn execute(
    record_data: &Value,
    handler: &dyn ActionHandler,
) -> Result<ExecutionResult, ExecutionError> {
    execute_with_vars(record_data, handler, HashMap::new())
}

/// Executes a compiled procedure record with pre-seeded variables.
///
/// Use this when the procedure is triggered with parameters that should
/// be available as variable bindings during execution.
pub fn execute_with_vars(
    record_data: &Value,
    handler: &dyn ActionHandler,
    initial_vars: HashMap<String, Value>,
) -> Result<ExecutionResult, ExecutionError> {
    let procedure_name = record_data
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let steps = record_data
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            ExecutionError::InvalidStructure("missing or non-array 'steps' field".into())
        })?;

    let mut vars = initial_vars;
    let mut step_results = Vec::new();

    for (index, step) in steps.iter().enumerate() {
        let result = execute_step(step, index, &mut vars, handler)?;
        let is_return = result.kind == "return";
        step_results.push(result);
        if is_return {
            break;
        }
    }

    Ok(ExecutionResult {
        procedure_name,
        step_results,
        variables: vars,
        success: true,
        error: None,
    })
}

/// Execute a single step within a procedure.
fn execute_step(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn ActionHandler,
) -> Result<StepResult, ExecutionError> {
    let kind = step
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExecutionError::InvalidStructure("step missing 'kind'".into()))?;

    match kind {
        "call" => execute_call(step, index, vars, handler),
        "match" => execute_match(step, index, vars, handler),
        "when" => execute_when(step, index, vars, handler),
        "loop" => execute_loop(step, index, vars, handler),
        "emit" => execute_emit(step, index, vars, handler),
        "try" => execute_try(step, index, vars, handler),
        "parallel" => execute_parallel(step, index, vars, handler),
        "return" => {
            let value = step.get("value").cloned();
            Ok(StepResult {
                index,
                kind: "return".to_string(),
                output: value,
                skipped: false,
            })
        }
        "abort" => {
            let reason = step
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("procedure aborted")
                .to_string();
            Err(ExecutionError::Aborted(reason))
        }
        other => Err(ExecutionError::InvalidStructure(format!(
            "unknown step kind: {other}"
        ))),
    }
}

/// Execute a `call` step: invoke an action and optionally bind the result.
fn execute_call(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn ActionHandler,
) -> Result<StepResult, ExecutionError> {
    let name = step
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExecutionError::InvalidStructure("call step missing 'name'".into()))?;

    let params = step.get("params").cloned().unwrap_or(Value::Null);

    // Resolve variable references in params
    let resolved_params = resolve_vars(&params, vars);

    let output = handler.call(name, &resolved_params)?;

    // Bind output to variable if specified
    if let Some(output_var) = step.get("output_var").and_then(|v| v.as_str()) {
        if !output_var.is_empty() {
            vars.insert(output_var.to_string(), output.clone());
        }
    }

    Ok(StepResult {
        index,
        kind: "call".into(),
        output: Some(output),
        skipped: false,
    })
}

/// Execute a `match` step: find the first arm whose condition is true.
///
/// Supports two modes:
/// 1. Condition-based: each arm has a `condition` that is evaluated as a boolean expression.
/// 2. Subject-based: the step has a `subject` field, and each arm has a `pattern` field
///    that is matched against the resolved subject value (supports literals, variables,
///    multi-pattern `|`, range patterns, and `_` wildcard).
fn execute_match(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn ActionHandler,
) -> Result<StepResult, ExecutionError> {
    let arms = step
        .get("arms")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ExecutionError::InvalidStructure("match step missing 'arms'".into()))?;

    // Check for subject-based matching
    if let Some(subject_expr) = step.get("subject").and_then(|v| v.as_str()) {
        return execute_match_subject(subject_expr, arms, index, vars);
    }

    for arm in arms {
        let condition = arm
            .get("condition")
            .and_then(|v| v.as_str())
            .unwrap_or("true");

        if handler.evaluate_condition(condition, vars) {
            let result_val = arm.get("result").cloned().unwrap_or(Value::Null);
            return Ok(StepResult {
                index,
                kind: "match".into(),
                output: Some(result_val),
                skipped: false,
            });
        }
    }

    // No arm matched — this is not necessarily an error for match steps.
    // Return a skipped result rather than failing hard.
    Ok(StepResult {
        index,
        kind: "match".into(),
        output: None,
        skipped: true,
    })
}

/// Execute a subject-based match step: resolve subject, match patterns in each arm.
fn execute_match_subject(
    subject_expr: &str,
    arms: &[Value],
    index: usize,
    vars: &mut HashMap<String, Value>,
) -> Result<StepResult, ExecutionError> {
    // Resolve the subject as both string and raw Value (for tuple matching)
    let (subject_val, subject_raw) = if (subject_expr.starts_with('"') && subject_expr.ends_with('"'))
        || (subject_expr.starts_with('\'') && subject_expr.ends_with('\''))
    {
        let s = subject_expr[1..subject_expr.len() - 1].to_string();
        (s.clone(), Value::String(s))
    } else if let Some(val) = resolve_var(subject_expr, vars) {
        (value_to_interpolation_string(&val), val)
    } else {
        (subject_expr.to_string(), Value::String(subject_expr.to_string()))
    };

    for arm in arms {
        let raw_pattern = arm
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("_");

        // Extract optional guard: `"active" if score > 50` → ("active", Some("score > 50"))
        let (pattern, guard) = extract_guard(raw_pattern);

        if pattern == "_" {
            // Default arm: check guard if present
            if let Some(guard_expr) = guard {
                if !default_evaluate_condition(guard_expr, vars) {
                    continue;
                }
            }
            let result_val = arm.get("result").cloned().unwrap_or(Value::Null);
            let resolved = resolve_vars(&result_val, vars);
            return Ok(StepResult {
                index,
                kind: "match".into(),
                output: Some(resolved),
                skipped: false,
            });
        }

        // Multi-pattern support
        let alternatives = split_pattern_alternatives(pattern);
        let mut arm_bindings: MatchBindings = HashMap::new();
        let matched = alternatives.iter().any(|alt| {
            let alt = alt.trim();
            // Tuple pattern: ("error", 500, _)
            if let Some((tuple_match, bindings)) = try_tuple_pattern(alt, &subject_raw, vars) {
                if tuple_match {
                    arm_bindings = bindings;
                }
                return tuple_match;
            }
            // Struct pattern: {kind: "error", code: 500}
            if let Some((struct_match, bindings)) = try_struct_pattern(alt, &subject_raw, vars) {
                if struct_match {
                    arm_bindings = bindings;
                }
                return struct_match;
            }
            // Range pattern
            if let Some(range_match) = try_range_pattern(alt, &subject_val) {
                return range_match;
            }
            let pattern_val = if (alt.starts_with('"') && alt.ends_with('"'))
                || (alt.starts_with('\'') && alt.ends_with('\''))
            {
                alt[1..alt.len() - 1].to_string()
            } else if let Some(val) = resolve_var(alt, vars) {
                value_to_interpolation_string(&val)
            } else {
                alt.to_string()
            };
            subject_val == pattern_val
        });

        if matched {
            // If there's a guard, evaluate it (with bindings available)
            let guard_vars: HashMap<String, Value> = vars.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .chain(arm_bindings.iter().map(|(k, v)| (k.clone(), v.clone())))
                .collect();
            if let Some(guard_expr) = guard {
                if !default_evaluate_condition(guard_expr, &guard_vars) {
                    continue; // Pattern matched but guard failed
                }
            }
            // Inject bindings into vars for result resolution
            vars.extend(arm_bindings);
            let result_val = arm.get("result").cloned().unwrap_or(Value::Null);
            let resolved = resolve_vars(&result_val, vars);
            return Ok(StepResult {
                index,
                kind: "match".into(),
                output: Some(resolved),
                skipped: false,
            });
        }
    }

    Ok(StepResult {
        index,
        kind: "match".into(),
        output: None,
        skipped: true,
    })
}

/// Execute a `when` step: run nested steps only if the condition holds.
fn execute_when(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn ActionHandler,
) -> Result<StepResult, ExecutionError> {
    let condition = step
        .get("condition")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExecutionError::InvalidStructure("when step missing 'condition'".into()))?;

    if !handler.evaluate_condition(condition, vars) {
        return Ok(StepResult {
            index,
            kind: "when".into(),
            output: None,
            skipped: true,
        });
    }

    // Condition met — execute nested steps
    let nested_steps = step
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ExecutionError::InvalidStructure("when step missing 'steps'".into()))?;

    let mut nested_results = Vec::new();
    for (i, nested) in nested_steps.iter().enumerate() {
        let result = execute_step(nested, i, vars, handler)?;
        nested_results.push(result);
    }

    // Return the last nested result as the when step's output
    let last_output = nested_results.last().and_then(|r| r.output.clone());

    Ok(StepResult {
        index,
        kind: "when".into(),
        output: last_output,
        skipped: false,
    })
}

/// Execute a `loop` step: iterate over an array or repeat N times.
///
/// Supports two modes:
/// - `over`: a `$variable` reference to an array; iterates each element
/// - `times`: a number; repeats nested steps that many times
///
/// The current item is bound to `$item` (configurable via `as` field),
/// and the index is bound to `$index`.
fn execute_loop(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn ActionHandler,
) -> Result<StepResult, ExecutionError> {
    let nested_steps = step
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ExecutionError::InvalidStructure("loop step missing 'steps'".into()))?;

    let item_var = step.get("as").and_then(|v| v.as_str()).unwrap_or("item");

    let output_var = step.get("output_var").and_then(|v| v.as_str());

    // Determine iteration source
    // For map/object iteration, we track keys separately
    let is_map_iteration;
    let iterations: Vec<Value>;
    let map_keys: Vec<String>;

    if let Some(over_ref) = step.get("over").and_then(|v| v.as_str()) {
        // Resolve variable reference
        let var_name = over_ref.strip_prefix('$').unwrap_or(over_ref);
        match vars.get(var_name) {
            Some(Value::Array(arr)) => {
                iterations = arr.clone();
                map_keys = Vec::new();
                is_map_iteration = false;
            }
            Some(Value::Object(map)) => {
                // Map iteration: iterate over key-value pairs
                map_keys = map.keys().cloned().collect();
                iterations = map.values().cloned().collect();
                is_map_iteration = true;
            }
            Some(other) => {
                iterations = vec![other.clone()]; // single-item iteration
                map_keys = Vec::new();
                is_map_iteration = false;
            }
            None => {
                return Ok(StepResult {
                    index,
                    kind: "loop".into(),
                    output: None,
                    skipped: true,
                })
            }
        }
    } else if let Some(times) = step.get("times").and_then(|v| v.as_u64()) {
        iterations = (0..times).map(|i| Value::Number(i.into())).collect();
        map_keys = Vec::new();
        is_map_iteration = false;
    } else {
        return Err(ExecutionError::InvalidStructure(
            "loop step requires 'over' or 'times'".into(),
        ));
    };

    // key_var defaults to "key" but can be overridden with "key_as"
    let key_var = step.get("key_as").and_then(|v| v.as_str()).unwrap_or("key");

    let mut results: Vec<Value> = Vec::new();

    for (iter_index, item) in iterations.into_iter().enumerate() {
        vars.insert(item_var.to_string(), item);
        vars.insert("index".to_string(), Value::Number(iter_index.into()));

        // For map iteration, bind the key variable
        if is_map_iteration {
            if let Some(k) = map_keys.get(iter_index) {
                vars.insert(key_var.to_string(), Value::String(k.clone()));
            }
        }

        for nested in nested_steps {
            let result = execute_step(nested, iter_index, vars, handler)?;
            if let Some(output) = &result.output {
                results.push(output.clone());
            }
        }
    }

    // Clean up loop variables
    vars.remove(item_var);
    vars.remove("index");
    if is_map_iteration {
        vars.remove(key_var);
    }

    let output = Value::Array(results);

    // Bind collected results if output_var specified
    if let Some(out_var) = output_var {
        if !out_var.is_empty() {
            vars.insert(out_var.to_string(), output.clone());
        }
    }

    Ok(StepResult {
        index,
        kind: "loop".into(),
        output: Some(output),
        skipped: false,
    })
}

/// Execute an `emit` step: produce events for the event loop.
///
/// The emitted value is appended to the `$emit` variable (an array).
/// The adapter reads this variable after execution to dispatch events.
fn execute_emit(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    _handler: &dyn ActionHandler,
) -> Result<StepResult, ExecutionError> {
    let event_data = step
        .get("event")
        .cloned()
        .ok_or_else(|| ExecutionError::InvalidStructure("emit step missing 'event'".into()))?;

    // Resolve variable references in the event data
    let resolved = resolve_vars(&event_data, vars);

    // Append to $emit array
    let emit_arr = vars
        .entry("emit".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));

    if let Value::Array(arr) = emit_arr {
        arr.push(resolved.clone());
    }

    Ok(StepResult {
        index,
        kind: "emit".into(),
        output: Some(resolved),
        skipped: false,
    })
}

/// Execute a `try` step: run nested steps with error recovery.
///
/// If any nested step fails, the `catch` steps are executed instead.
/// The error is bound to `$error` for use in catch steps.
fn execute_try(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn ActionHandler,
) -> Result<StepResult, ExecutionError> {
    let try_steps = step
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ExecutionError::InvalidStructure("try step missing 'steps'".into()))?;

    let catch_steps = step.get("catch").and_then(|v| v.as_array());

    // Retry configuration: retry=N means up to N additional attempts after the first.
    let max_retries = step
        .get("retry")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let retry_delay_ms = step
        .get("retry_delay_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let retry_backoff = step
        .get("retry_backoff")
        .and_then(|v| v.as_str())
        .unwrap_or("fixed");

    let retry_max_delay_ms = step
        .get("retry_max_delay_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(u64::MAX);

    let retry_jitter = step
        .get("retry_jitter")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let mut last_err: Option<ExecutionError> = None;

    for attempt in 0..=max_retries {
        vars.insert("retry_count".to_string(), Value::Number(attempt.into()));

        // Delay before retry (not before the first attempt)
        if attempt > 0 && retry_delay_ms > 0 {
            let base_delay = match retry_backoff {
                "exponential" => {
                    let exp_delay = retry_delay_ms.saturating_mul(1u64 << (attempt as u64 - 1));
                    exp_delay.min(retry_max_delay_ms)
                }
                _ => retry_delay_ms.min(retry_max_delay_ms), // "fixed" or unknown
            };
            let delay = if retry_jitter && base_delay > 0 {
                // Full jitter: uniform random in [0, base_delay]
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                attempt.hash(&mut hasher);
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos()
                    .hash(&mut hasher);
                let h = hasher.finish();
                h % (base_delay + 1)
            } else {
                base_delay
            };
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }

        // Attempt the try block
        let mut last_output = None;
        let mut failed = false;

        for (i, nested) in try_steps.iter().enumerate() {
            match execute_step(nested, i, vars, handler) {
                Ok(result) => {
                    last_output = result.output;
                }
                Err(err) => {
                    last_err = Some(err);
                    failed = true;
                    break;
                }
            }
        }

        if !failed {
            // All try steps succeeded
            vars.remove("error");
            vars.remove("retry_count");
            return Ok(StepResult {
                index,
                kind: "try".into(),
                output: last_output,
                skipped: false,
            });
        }

        // If we have retries left, continue the loop
        if attempt < max_retries {
            continue;
        }

        // All retries exhausted — run catch or return error
        let err = last_err.take().unwrap();
        vars.insert("error".to_string(), Value::String(err.to_string()));

        if let Some(catch) = catch_steps {
            let mut catch_output = None;
            for (j, catch_step) in catch.iter().enumerate() {
                let result = execute_step(catch_step, j, vars, handler)?;
                catch_output = result.output;
            }
            vars.remove("retry_count");
            return Ok(StepResult {
                index,
                kind: "try".into(),
                output: catch_output,
                skipped: false,
            });
        }

        vars.remove("retry_count");
        return Ok(StepResult {
            index,
            kind: "try".into(),
            output: Some(Value::String(err.to_string())),
            skipped: false,
        });
    }

    // Unreachable but satisfies the compiler
    vars.remove("retry_count");
    Ok(StepResult {
        index,
        kind: "try".into(),
        output: None,
        skipped: false,
    })
}

/// Execute a `parallel` step: run named branches.
///
/// In the synchronous executor, branches are executed sequentially (no true
/// parallelism). Each branch gets its own copy of the variables, and the
/// results are collected into a map keyed by branch name.
///
/// The async executor provides true concurrent execution via `tokio::join!`.
fn execute_parallel(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn ActionHandler,
) -> Result<StepResult, ExecutionError> {
    let branches = step
        .get("branches")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            ExecutionError::InvalidStructure("parallel step missing 'branches'".into())
        })?;

    let output_var = step.get("output_var").and_then(|v| v.as_str());

    let mut results_map = serde_json::Map::new();

    for branch in branches {
        let branch_name = branch
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ExecutionError::InvalidStructure("parallel branch missing 'name'".into())
            })?;

        let branch_steps = branch
            .get("steps")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                ExecutionError::InvalidStructure("parallel branch missing 'steps'".into())
            })?;

        // Per-branch retry configuration
        let branch_retry = branch
            .get("retry")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let branch_retry_delay_ms = branch
            .get("retry_delay_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let branch_retry_backoff = branch
            .get("retry_backoff")
            .and_then(|v| v.as_str())
            .unwrap_or("fixed");
        let branch_retry_max_delay_ms = branch
            .get("retry_max_delay_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(30_000);
        let branch_retry_jitter = branch
            .get("retry_jitter")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Each branch gets a snapshot of vars (isolation)
        let mut branch_vars = vars.clone();
        let mut last_err = None;
        let mut branch_succeeded = false;

        for attempt in 0..=(branch_retry) {
            // Delay before retry (not before first attempt)
            if attempt > 0 && branch_retry_delay_ms > 0 {
                let base_delay = match branch_retry_backoff {
                    "exponential" => {
                        let exp_delay = branch_retry_delay_ms
                            .saturating_mul(1u64 << (attempt as u64 - 1));
                        exp_delay.min(branch_retry_max_delay_ms)
                    }
                    _ => branch_retry_delay_ms.min(branch_retry_max_delay_ms),
                };
                let delay = if branch_retry_jitter && base_delay > 0 {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = DefaultHasher::new();
                    attempt.hash(&mut hasher);
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos()
                        .hash(&mut hasher);
                    let h = hasher.finish();
                    h % (base_delay + 1)
                } else {
                    base_delay
                };
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }

            branch_vars.insert(
                "retry_count".to_string(),
                Value::Number(attempt.into()),
            );

            let mut attempt_vars = branch_vars.clone();
            let mut last_output = Value::Null;
            let mut success = true;

            for (i, nested) in branch_steps.iter().enumerate() {
                match execute_step(nested, i, &mut attempt_vars, handler) {
                    Ok(result) => {
                        if let Some(output) = result.output {
                            last_output = output;
                        }
                    }
                    Err(e) => {
                        last_err = Some(e);
                        success = false;
                        break;
                    }
                }
            }

            if success {
                results_map.insert(branch_name.to_string(), last_output);
                branch_succeeded = true;
                break;
            }
        }

        if !branch_succeeded {
            return Err(last_err.unwrap_or_else(|| ExecutionError::ActionFailed {
                action: branch_name.to_string(),
                message: "branch failed after retries".into(),
            }));
        }
    }

    let output = Value::Object(results_map);

    if let Some(out_var) = output_var {
        if !out_var.is_empty() {
            vars.insert(out_var.to_string(), output.clone());
        }
    }

    Ok(StepResult {
        index,
        kind: "parallel".into(),
        output: Some(output),
        skipped: false,
    })
}

// ── Variable Resolution ───────────────────────────────────────────────────────

/// Resolve variable references (`$var_name`) in a JSON value tree.
///
/// Strings starting with `$` are looked up in the variables map. If not
/// found, the original string is preserved (no error — allows literal `$`
/// in params for forward-compatible use).
fn resolve_vars(value: &Value, vars: &HashMap<String, Value>) -> Value {
    match value {
        Value::String(s) if s.starts_with('$') && !s.contains("${") => {
            // Whole-string variable reference: "$name" → value of name
            let var_name = &s[1..];
            vars.get(var_name).cloned().unwrap_or_else(|| value.clone())
        }
        Value::String(s) if s.contains("${") => {
            // String interpolation: "Hello, ${name}!" → "Hello, world!"
            Value::String(interpolate_string(s, vars))
        }
        Value::Object(map) => {
            let resolved: serde_json::Map<String, Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), resolve_vars(v, vars)))
                .collect();
            Value::Object(resolved)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(|v| resolve_vars(v, vars)).collect()),
        other => other.clone(),
    }
}

/// Interpolate `${var}` and `${var.field}` references within a string.
///
/// Supports:
/// - `${name}` — simple variable lookup
/// - `${result.field}` — dotted path access
/// - `${count + 1}` — simple arithmetic (add/subtract with integer literals)
/// - Unresolved references are left as-is
fn interpolate_string(s: &str, vars: &HashMap<String, Value>) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut expr = String::new();
            let mut depth = 1;
            for c in chars.by_ref() {
                if c == '{' {
                    depth += 1;
                    expr.push(c);
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    expr.push(c);
                } else {
                    expr.push(c);
                }
            }
            if depth != 0 {
                // Unterminated — emit raw
                result.push('$');
                result.push('{');
                result.push_str(&expr);
            } else {
                // Try to resolve the expression
                let resolved = resolve_interpolation_expr(expr.trim(), vars);
                result.push_str(&resolved);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Resolve an expression inside `${}`. Supports:
/// - Simple variable: `name`
/// - Dotted access: `result.status`
/// - Arithmetic: `count + 1`, `total - 5`
/// - Pipe filters: `name | uppercase`, `value | trim | lowercase`
fn resolve_interpolation_expr(expr: &str, vars: &HashMap<String, Value>) -> String {
    // Check for pipe operator: `value | filter`
    // Must distinguish from logical OR `||` in ternary conditions
    if let Some(result) = try_pipe_expr(expr, vars) {
        return result;
    }

    // Try arithmetic: var +/- literal
    if let Some(val) = try_arithmetic_expr(expr, vars) {
        return val;
    }

    // Try variable/dotted resolution
    if let Some(val) = resolve_var(expr, vars) {
        return value_to_interpolation_string(&val);
    }

    // Unresolved — return original
    format!("${{{}}}", expr)
}

/// Apply pipe filters to a value expression.
/// Syntax: `expr | filter1 | filter2 | filter3(arg)`
/// Supported filters:
/// - `uppercase` / `upper` — convert to UPPERCASE
/// - `lowercase` / `lower` — convert to lowercase
/// - `trim` — strip leading/trailing whitespace
/// - `capitalize` — capitalize first character
/// - `reverse` — reverse the string
/// - `length` / `len` — return string length (or array length)
/// - `default(value)` — use value if the expression is empty/null
/// - `replace(old, new)` — replace occurrences
/// - `truncate(n)` — truncate to n characters
/// - `split(sep)` — split and return JSON array
fn try_pipe_expr(expr: &str, vars: &HashMap<String, Value>) -> Option<String> {
    // Find the first single `|` that isn't part of `||`
    let pipe_idx = find_pipe_operator(expr)?;

    let value_expr = expr[..pipe_idx].trim();
    let filters_str = expr[pipe_idx + 1..].trim();

    // Resolve the base value
    let base_value = if let Some(val) = try_arithmetic_expr(value_expr, vars) {
        val
    } else if let Some(val) = resolve_var(value_expr, vars) {
        value_to_interpolation_string(&val)
    } else if value_expr.starts_with('"') && value_expr.ends_with('"') {
        // String literal
        value_expr[1..value_expr.len() - 1].to_string()
    } else {
        // Can't resolve base — not a pipe expression we can handle
        return None;
    };

    // Split remaining filters by `|` (again avoiding `||`)
    let filters = split_pipe_filters(filters_str);

    let mut result = base_value;
    for filter in &filters {
        result = apply_pipe_filter(filter.trim(), &result, vars)?;
    }

    Some(result)
}

/// Find the index of the first single `|` (not `||`) in the expression.
/// Returns None if no pipe operator is found.
fn find_pipe_operator(expr: &str) -> Option<usize> {
    let bytes = expr.as_bytes();
    let mut i = 0;
    let mut depth = 0; // track parentheses depth
    let mut in_string = false;
    let mut string_char = b'\0';

    while i < bytes.len() {
        let b = bytes[i];

        if in_string {
            if b == string_char && (i == 0 || bytes[i - 1] != b'\\') {
                in_string = false;
            }
            i += 1;
            continue;
        }

        match b {
            b'"' | b'\'' => {
                in_string = true;
                string_char = b;
            }
            b'(' => depth += 1,
            b')' if depth > 0 => depth -= 1,
            b'|' if depth == 0 => {
                // Check it's not `||`
                if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    i += 2; // skip `||`
                    continue;
                }
                // Check it's not preceded by `|` (part of `||`)
                if i > 0 && bytes[i - 1] == b'|' {
                    i += 1;
                    continue;
                }
                return Some(i);
            }
            b'?' => {
                // If there's a ternary, pipe detection should not grab it
                // Only detect pipes before any ternary operator
                return None;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Split a filter chain by `|` separators (respecting parentheses).
fn split_pipe_filters(s: &str) -> Vec<&str> {
    let mut filters = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    let mut string_char = b'\0';

    while i < bytes.len() {
        let b = bytes[i];

        if in_string {
            if b == string_char && (i == 0 || bytes[i - 1] != b'\\') {
                in_string = false;
            }
            i += 1;
            continue;
        }

        match b {
            b'"' | b'\'' => {
                in_string = true;
                string_char = b;
            }
            b'(' => depth += 1,
            b')' if depth > 0 => depth -= 1,
            b'|' if depth == 0 => {
                // Skip `||`
                if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    i += 2;
                    continue;
                }
                filters.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    filters.push(&s[start..]);
    filters
}

/// Apply a single pipe filter to a string value.
fn apply_pipe_filter(filter: &str, value: &str, _vars: &HashMap<String, Value>) -> Option<String> {
    // Check for filter with arguments: filter(arg1, arg2)
    if let Some(paren_idx) = filter.find('(') {
        if filter.ends_with(')') {
            let name = filter[..paren_idx].trim();
            let args_str = &filter[paren_idx + 1..filter.len() - 1];
            return apply_filter_with_args(name, value, args_str);
        }
    }

    // Simple filters (no args)
    match filter {
        "uppercase" | "upper" => Some(value.to_uppercase()),
        "lowercase" | "lower" => Some(value.to_lowercase()),
        "trim" => Some(value.trim().to_string()),
        "capitalize" | "cap" => {
            let mut chars = value.chars();
            match chars.next() {
                None => Some(String::new()),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    Some(format!("{}{}", upper, chars.as_str()))
                }
            }
        }
        "reverse" | "rev" => Some(value.chars().rev().collect()),
        "length" | "len" => Some(value.len().to_string()),
        "snake_case" | "snake" => Some(to_snake_case(value)),
        "camel_case" | "camel" => Some(to_camel_case(value)),
        "kebab_case" | "kebab" => Some(to_kebab_case(value)),
        _ => None, // Unknown filter — signal failure
    }
}

/// Apply a filter that takes arguments.
fn apply_filter_with_args(name: &str, value: &str, args_str: &str) -> Option<String> {
    match name {
        "default" | "fallback" => {
            let default_val = args_str.trim().trim_matches('"').trim_matches('\'');
            if value.is_empty() || value == "null" || value == "undefined" {
                Some(default_val.to_string())
            } else {
                Some(value.to_string())
            }
        }
        "truncate" | "trunc" => {
            let n: usize = args_str.trim().parse().ok()?;
            if value.len() <= n {
                Some(value.to_string())
            } else {
                Some(value.chars().take(n).collect())
            }
        }
        "replace" => {
            // replace(old, new)
            let parts = split_filter_args(args_str);
            if parts.len() != 2 {
                return None;
            }
            let old = parts[0].trim().trim_matches('"').trim_matches('\'');
            let new = parts[1].trim().trim_matches('"').trim_matches('\'');
            Some(value.replace(old, new))
        }
        "split" => {
            let sep = args_str.trim().trim_matches('"').trim_matches('\'');
            let parts: Vec<&str> = value.split(sep).collect();
            // Return as JSON array
            let json_parts: Vec<String> = parts.iter().map(|p| format!("\"{}\"" , p)).collect();
            Some(format!("[{}]", json_parts.join(", ")))
        }
        "pad_left" | "pad_start" => {
            let parts = split_filter_args(args_str);
            let width: usize = parts.first()?.trim().parse().ok()?;
            let pad_char = parts.get(1)
                .map(|s| s.trim().trim_matches('"').trim_matches('\''))
                .and_then(|s| s.chars().next())
                .unwrap_or(' ');
            if value.len() >= width {
                Some(value.to_string())
            } else {
                let padding: String = std::iter::repeat_n(pad_char, width - value.len()).collect();
                Some(format!("{}{}", padding, value))
            }
        }
        "pad_right" | "pad_end" => {
            let parts = split_filter_args(args_str);
            let width: usize = parts.first()?.trim().parse().ok()?;
            let pad_char = parts.get(1)
                .map(|s| s.trim().trim_matches('"').trim_matches('\''))
                .and_then(|s| s.chars().next())
                .unwrap_or(' ');
            if value.len() >= width {
                Some(value.to_string())
            } else {
                let padding: String = std::iter::repeat_n(pad_char, width - value.len()).collect();
                Some(format!("{}{}", value, padding))
            }
        }
        "repeat" => {
            let n: usize = args_str.trim().parse().ok()?;
            Some(value.repeat(n))
        }
        "slice" => {
            // slice(start, end) or slice(start)
            let parts = split_filter_args(args_str);
            let start: usize = parts.first()?.trim().parse().ok()?;
            let chars: Vec<char> = value.chars().collect();
            if start >= chars.len() {
                return Some(String::new());
            }
            if let Some(end_str) = parts.get(1) {
                let end: usize = end_str.trim().parse().ok()?;
                let end = end.min(chars.len());
                Some(chars[start..end].iter().collect())
            } else {
                Some(chars[start..].iter().collect())
            }
        }
        _ => None,
    }
}

/// Split filter arguments by comma, respecting quotes.
fn split_filter_args(s: &str) -> Vec<&str> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut in_string = false;
    let mut string_char = '\0';
    let chars: Vec<char> = s.chars().collect();

    for (i, &c) in chars.iter().enumerate() {
        if in_string {
            if c == string_char && (i == 0 || chars[i - 1] != '\\') {
                in_string = false;
            }
        } else {
            match c {
                '"' | '\'' => {
                    in_string = true;
                    string_char = c;
                }
                ',' => {
                    let byte_start = s.char_indices().nth(start).map(|(idx, _)| idx).unwrap_or(0);
                    let byte_end = s.char_indices().nth(i).map(|(idx, _)| idx).unwrap_or(s.len());
                    args.push(&s[byte_start..byte_end]);
                    start = i + 1;
                }
                _ => {}
            }
        }
    }
    let byte_start = s.char_indices().nth(start).map(|(idx, _)| idx).unwrap_or(0);
    args.push(&s[byte_start..]);
    args
}

/// Convert a string to snake_case.
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' {
            result.push('_');
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert a string to camelCase.
fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for (i, c) in s.chars().enumerate() {
        if c == '_' || c == '-' || c == ' ' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(c.to_uppercase());
            capitalize_next = false;
        } else if i == 0 {
            result.extend(c.to_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert a string to kebab-case.
fn to_kebab_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('-');
            }
            result.extend(c.to_lowercase());
        } else if c == '_' || c == ' ' {
            result.push('-');
        } else {
            result.push(c);
        }
    }
    result
}

/// Attempt to evaluate simple arithmetic: `var + N` or `var - N`
fn try_arithmetic_expr(expr: &str, vars: &HashMap<String, Value>) -> Option<String> {
    // Try match expression: match var { "a" => expr1, "b" => expr2, _ => default }
    if let Some(result) = try_match_expr(expr, vars) {
        return Some(result);
    }

    // Try ternary: condition ? trueVal : falseVal
    if let Some(result) = try_ternary_expr(expr, vars) {
        return Some(result);
    }

    // Look for +, -, *, /, % operators (not at position 0 for - which could be negative)
    // Lower precedence ops first: + and -
    for op in ['+', '-'] {
        if let Some(idx) = expr[1..].find(op).map(|i| i + 1) {
            let lhs = expr[..idx].trim();
            let rhs = expr[idx + 1..].trim();

            let lhs_num = interp_eval_numeric(lhs, vars)?;
            let rhs_num = interp_eval_numeric(rhs, vars)?;

            let result = match op {
                '+' => lhs_num + rhs_num,
                '-' => lhs_num - rhs_num,
                _ => return None,
            };

            return Some(format_numeric_result(result));
        }
    }

    // Higher precedence: *, /, %
    for op in ['*', '/', '%'] {
        if let Some(idx) = expr.find(op) {
            let lhs = expr[..idx].trim();
            let rhs = expr[idx + 1..].trim();

            let lhs_num = interp_eval_numeric(lhs, vars)?;
            let rhs_num = interp_eval_numeric(rhs, vars)?;

            if (op == '/' || op == '%') && rhs_num == 0.0 {
                return None;
            }

            let result = match op {
                '*' => lhs_num * rhs_num,
                '/' => lhs_num / rhs_num,
                '%' => lhs_num % rhs_num,
                _ => return None,
            };

            return Some(format_numeric_result(result));
        }
    }

    None
}

/// Evaluate a numeric value for interpolation arithmetic (variable or literal).
fn interp_eval_numeric(s: &str, vars: &HashMap<String, Value>) -> Option<f64> {
    // Try as literal number first
    if let Ok(n) = s.parse::<f64>() {
        return Some(n);
    }
    // Try as variable reference
    let val = resolve_var(s, vars)?;
    val.as_f64()
}

/// Format a numeric result: integer if whole, float otherwise.
fn format_numeric_result(result: f64) -> String {
    if result.fract() == 0.0 && result.abs() < i64::MAX as f64 {
        (result as i64).to_string()
    } else {
        result.to_string()
    }
}

/// Try to evaluate a match expression: `match expr { "val1" => result1, "val2" => result2, _ => default }`
///
/// Provides inline pattern matching similar to Rust's match.
/// Arms are separated by commas. The wildcard `_` matches anything.
/// Values and results can be quoted strings, numbers, variables, or booleans.
fn try_match_expr(expr: &str, vars: &HashMap<String, Value>) -> Option<String> {
    // Must start with `match `
    let trimmed = expr.trim();
    if !trimmed.starts_with("match ") {
        return None;
    }

    // Find the opening `{`
    let brace_start = trimmed.find('{')?;
    let subject_str = trimmed[6..brace_start].trim(); // between "match " and "{"

    // Find the closing `}` — track nesting
    let inner = &trimmed[brace_start + 1..];
    let brace_end = find_matching_brace(inner)?;
    let arms_str = inner[..brace_end].trim();

    // Resolve the subject as both a raw Value (for tuple matching) and a string
    let (subject_val, subject_raw) = if (subject_str.starts_with('"') && subject_str.ends_with('"'))
        || (subject_str.starts_with('\'') && subject_str.ends_with('\''))
    {
        let s = subject_str[1..subject_str.len() - 1].to_string();
        (s.clone(), Value::String(s))
    } else if let Some(val) = resolve_var(subject_str, vars) {
        (value_to_interpolation_string(&val), val)
    } else {
        (subject_str.to_string(), Value::String(subject_str.to_string()))
    };

    // Parse arms: split by top-level commas
    let arms = split_match_arms(arms_str);
    let mut default_result: Option<String> = None;

    for arm in &arms {
        let arm = arm.trim();
        if arm.is_empty() {
            continue;
        }

        // Split on `=>`
        let arrow_idx = arm.find("=>")?;
        let pattern_with_guard = arm[..arrow_idx].trim();
        let result_expr = arm[arrow_idx + 2..].trim();

        // Extract optional guard: `pattern if condition` → (pattern, Some(condition))
        let (pattern, guard) = extract_guard(pattern_with_guard);

        if pattern == "_" {
            // Default arm: guard still applies if present
            if let Some(guard_expr) = &guard {
                if !default_evaluate_condition(guard_expr, vars) {
                    continue;
                }
            }
            default_result = Some(resolve_match_result(result_expr, vars));
            continue;
        }

        // Multi-pattern support: "a" | "b" | "c" => result
        let alternatives = split_pattern_alternatives(pattern);
        let mut arm_bindings: MatchBindings = HashMap::new();
        let matched = alternatives.iter().any(|alt| {
            let alt = alt.trim();
            // Tuple pattern: ("error", 500, _)
            if let Some((tuple_match, bindings)) = try_tuple_pattern(alt, &subject_raw, vars) {
                if tuple_match {
                    arm_bindings = bindings;
                }
                return tuple_match;
            }
            // Struct pattern: {kind: "error", code: 500}
            if let Some((struct_match, bindings)) = try_struct_pattern(alt, &subject_raw, vars) {
                if struct_match {
                    arm_bindings = bindings;
                }
                return struct_match;
            }
            // Range pattern: 1..5 (exclusive end) or 1..=5 (inclusive end)
            if let Some(range_match) = try_range_pattern(alt, &subject_val) {
                return range_match;
            }
            let pattern_val = if (alt.starts_with('"') && alt.ends_with('"'))
                || (alt.starts_with('\'') && alt.ends_with('\''))
            {
                alt[1..alt.len() - 1].to_string()
            } else if let Some(val) = resolve_var(alt, vars) {
                value_to_interpolation_string(&val)
            } else {
                alt.to_string()
            };
            subject_val == pattern_val
        });

        if matched {
            // If there's a guard, evaluate it — with bindings available
            let merged_vars: HashMap<String, Value> = vars.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .chain(arm_bindings.iter().map(|(k, v)| (k.clone(), v.clone())))
                .collect();
            if let Some(guard_expr) = &guard {
                if !default_evaluate_condition(guard_expr, &merged_vars) {
                    continue; // Pattern matched but guard failed, try next arm
                }
            }
            return Some(resolve_match_result(result_expr, &merged_vars));
        }
    }

    // No arm matched — use default or return None
    default_result
}

/// Extract an optional guard from a pattern string.
/// `"active" if score > 50` → (`"active"`, Some("score > 50"))
/// `"active"` → (`"active"`, None)
/// Handles quoted strings: won't split on `if` inside quotes.
fn extract_guard(pattern_with_guard: &str) -> (&str, Option<&str>) {
    // Find ` if ` that's not inside quotes
    let bytes = pattern_with_guard.as_bytes();
    let mut in_double = false;
    let mut in_single = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' if !in_single => in_double = !in_double,
            b'\'' if !in_double => in_single = !in_single,
            b' ' if !in_double && !in_single && i + 3 < bytes.len() && bytes[i + 1] == b'i' && bytes[i + 2] == b'f' && bytes[i + 3] == b' ' => {
                let pattern = pattern_with_guard[..i].trim();
                let guard = pattern_with_guard[i + 4..].trim();
                if !guard.is_empty() {
                    return (pattern, Some(guard));
                }
            }
            _ => {}
        }
        i += 1;
    }
    (pattern_with_guard, None)
}

/// Split multi-pattern alternatives by top-level `|` (respecting quotes)
/// e.g., `"a" | "b" | var` → [`"a"`, `"b"`, `var`]
fn split_pattern_alternatives(pattern: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut in_double = false;
    let mut in_single = false;
    let mut start = 0;

    for (i, c) in pattern.char_indices() {
        match c {
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            '|' if !in_double && !in_single => {
                parts.push(&pattern[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < pattern.len() {
        parts.push(&pattern[start..]);
    }
    // If no `|` found, return the whole pattern as a single alternative
    if parts.is_empty() {
        vec![pattern]
    } else {
        parts
    }
}

/// Try to match a range pattern like `1..5` (exclusive) or `1..=5` (inclusive)
/// Returns Some(true) if the subject matches, Some(false) if it's a valid range but doesn't match,
/// None if the pattern isn't a range at all.
fn try_range_pattern(pattern: &str, subject: &str) -> Option<bool> {
    // Check for inclusive range first: `start..=end`
    if let Some(dot_idx) = pattern.find("..=") {
        let start_str = pattern[..dot_idx].trim();
        let end_str = pattern[dot_idx + 3..].trim();
        let start: f64 = start_str.parse().ok()?;
        let end: f64 = end_str.parse().ok()?;
        let subject_num: f64 = subject.parse().ok()?;
        return Some(subject_num >= start && subject_num <= end);
    }
    // Check for exclusive range: `start..end`
    if let Some(dot_idx) = pattern.find("..") {
        // Make sure it's not something like `..` alone or a float
        let start_str = pattern[..dot_idx].trim();
        let end_str = pattern[dot_idx + 2..].trim();
        if start_str.is_empty() || end_str.is_empty() {
            return None;
        }
        let start: f64 = start_str.parse().ok()?;
        let end: f64 = end_str.parse().ok()?;
        let subject_num: f64 = subject.parse().ok()?;
        return Some(subject_num >= start && subject_num < end);
    }
    None
}

/// Result of a pattern match attempt: matched flag + captured bindings.
type MatchBindings = HashMap<String, Value>;

/// Try to match a tuple pattern like `("error", 500, _)` against a JSON array subject.
/// Returns `Some((true, bindings))` if the pattern matches, `Some((false, empty))` if it's a valid
/// tuple pattern that doesn't match, or `None` if the pattern is not a tuple pattern.
///
/// Elements are compared positionally:
/// - `_` is a wildcard (matches anything)
/// - `$name` where name is NOT in vars captures the value as a binding
/// - `$name` where name IS in vars compares against the stored value
/// - Quoted strings compare as strings
/// - Numbers compare as numbers
/// - Tuple length must match array length
fn try_tuple_pattern(pattern: &str, subject_val: &Value, vars: &HashMap<String, Value>) -> Option<(bool, MatchBindings)> {
    let trimmed = pattern.trim();
    if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return None;
    }

    // Extract the subject as a JSON array
    let subject_arr = match subject_val {
        Value::Array(arr) => arr,
        _ => return Some((false, HashMap::new())), // Tuple pattern against non-array = no match
    };

    // Parse the tuple elements (split by top-level commas within the parens)
    let inner = &trimmed[1..trimmed.len() - 1];
    let elements = split_tuple_elements(inner);

    // Length must match
    if elements.len() != subject_arr.len() {
        return Some((false, HashMap::new()));
    }

    let mut bindings: MatchBindings = HashMap::new();

    // Compare element by element
    for (elem_pat, subject_elem) in elements.iter().zip(subject_arr.iter()) {
        let elem_pat = elem_pat.trim();

        // Wildcard
        if elem_pat == "_" {
            continue;
        }

        // Binding variable: $name where name is NOT in vars → capture
        if let Some(var_name) = elem_pat.strip_prefix('$') {
            if !vars.contains_key(var_name) {
                bindings.insert(var_name.to_string(), subject_elem.clone());
                continue;
            }
        }

        // Quoted string
        if (elem_pat.starts_with('"') && elem_pat.ends_with('"'))
            || (elem_pat.starts_with('\'') && elem_pat.ends_with('\''))
        {
            let pat_str = &elem_pat[1..elem_pat.len() - 1];
            match subject_elem {
                Value::String(s) if s == pat_str => continue,
                _ => return Some((false, HashMap::new())),
            }
        }

        // Boolean
        if elem_pat == "true" {
            if subject_elem == &Value::Bool(true) {
                continue;
            }
            return Some((false, HashMap::new()));
        }
        if elem_pat == "false" {
            if subject_elem == &Value::Bool(false) {
                continue;
            }
            return Some((false, HashMap::new()));
        }

        // Null
        if elem_pat == "null" {
            if subject_elem.is_null() {
                continue;
            }
            return Some((false, HashMap::new()));
        }

        // Number
        if let Ok(n) = elem_pat.parse::<f64>() {
            match subject_elem {
                Value::Number(num) => {
                    if let Some(sv) = num.as_f64() {
                        if (sv - n).abs() < f64::EPSILON {
                            continue;
                        }
                    }
                    return Some((false, HashMap::new()));
                }
                _ => return Some((false, HashMap::new())),
            }
        }

        // Variable reference ($name or bare ident) — only for vars that EXIST
        if let Some(resolved) = resolve_var(elem_pat, vars) {
            let subject_str = value_to_interpolation_string(subject_elem);
            let resolved_str = value_to_interpolation_string(&resolved);
            if subject_str == resolved_str {
                continue;
            }
            return Some((false, HashMap::new()));
        }

        // Bare string comparison (unquoted literal)
        let subject_str = value_to_interpolation_string(subject_elem);
        if subject_str == elem_pat {
            continue;
        }
        return Some((false, HashMap::new()));
    }

    Some((true, bindings))
}

/// Try to match a struct pattern like `{kind: "error", code: 500}` against a JSON object subject.
/// Returns `Some(true)` if the pattern matches, `Some(false)` if it's a valid struct pattern
/// that doesn't match, or `None` if the pattern is not a struct pattern.
///
/// Field matching rules:
/// - `_` is a wildcard (matches any value for that field)
/// - Quoted strings compare as strings
/// - Numbers compare as numbers
/// - Booleans and null compare as their types
/// - Variables (`$name` or bare `ident` starting with `$`) are resolved from vars
/// - The pattern does NOT require exhaustive fields — unmentioned fields are ignored
/// - All mentioned fields must exist in the subject and match their patterns
fn try_struct_pattern(pattern: &str, subject_val: &Value, vars: &HashMap<String, Value>) -> Option<(bool, MatchBindings)> {
    let trimmed = pattern.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }

    // Extract the subject as a JSON object
    let subject_obj = match subject_val {
        Value::Object(obj) => obj,
        _ => return Some((false, HashMap::new())), // Struct pattern against non-object = no match
    };

    // Parse the field patterns (split by top-level commas within the braces)
    let inner = &trimmed[1..trimmed.len() - 1];
    let inner_trimmed = inner.trim();
    if inner_trimmed.is_empty() {
        // Empty struct pattern `{}` matches any object
        return Some((true, HashMap::new()));
    }

    let fields = split_struct_fields(inner);
    let mut bindings: MatchBindings = HashMap::new();

    // Check each field pattern against the subject
    for field_pat in &fields {
        let field_pat = field_pat.trim();
        if field_pat.is_empty() {
            continue;
        }

        // Split on first `:` to get field_name: value_pattern
        let colon_idx = match find_field_colon(field_pat) {
            Some(idx) => idx,
            None => {
                // No colon — could be a shorthand like `{active}` meaning field must exist and be truthy
                // For now, check field exists in subject
                let field_name = field_pat.trim();
                if !subject_obj.contains_key(field_name) {
                    return Some((false, HashMap::new()));
                }
                continue;
            }
        };

        let field_name = field_pat[..colon_idx].trim();
        let value_pattern = field_pat[colon_idx + 1..].trim();

        // Look up the field in the subject
        let subject_field = match subject_obj.get(field_name) {
            Some(val) => val,
            None => return Some((false, HashMap::new())), // Required field missing
        };

        // Match the value pattern against the subject field
        let (field_matched, field_bindings) = struct_field_matches(value_pattern, subject_field, vars);
        if !field_matched {
            return Some((false, HashMap::new()));
        }
        bindings.extend(field_bindings);
    }

    Some((true, bindings))
}

/// Check if a single struct field value matches the expected pattern.
/// Returns (matched, bindings) where bindings are variables captured during matching.
fn struct_field_matches(pattern: &str, subject: &Value, vars: &HashMap<String, Value>) -> (bool, MatchBindings) {
    let pat = pattern.trim();

    // Wildcard
    if pat == "_" {
        return (true, HashMap::new());
    }

    // Binding variable: $name where name is NOT in vars → capture
    if let Some(var_name) = pat.strip_prefix('$') {
        if !vars.contains_key(var_name) {
            let mut bindings = HashMap::new();
            bindings.insert(var_name.to_string(), subject.clone());
            return (true, bindings);
        }
    }

    // Quoted string
    if (pat.starts_with('"') && pat.ends_with('"'))
        || (pat.starts_with('\'') && pat.ends_with('\''))
    {
        let pat_str = &pat[1..pat.len() - 1];
        return (matches!(subject, Value::String(s) if s == pat_str), HashMap::new());
    }

    // Boolean
    if pat == "true" {
        return (subject == &Value::Bool(true), HashMap::new());
    }
    if pat == "false" {
        return (subject == &Value::Bool(false), HashMap::new());
    }

    // Null
    if pat == "null" {
        return (subject.is_null(), HashMap::new());
    }

    // Nested struct pattern
    if pat.starts_with('{') && pat.ends_with('}') {
        if let Some((result, nested_bindings)) = try_struct_pattern(pat, subject, vars) {
            return (result, nested_bindings);
        }
    }

    // Nested tuple pattern
    if pat.starts_with('(') && pat.ends_with(')') {
        if let Some((result, nested_bindings)) = try_tuple_pattern(pat, subject, vars) {
            return (result, nested_bindings);
        }
    }

    // Number
    if let Ok(n) = pat.parse::<f64>() {
        if let Some(sv) = subject.as_f64() {
            return ((sv - n).abs() < f64::EPSILON, HashMap::new());
        }
        return (false, HashMap::new());
    }

    // Variable reference ($name) — only for vars that EXIST
    if let Some(resolved) = resolve_var(pat, vars) {
        let subject_str = value_to_interpolation_string(subject);
        let resolved_str = value_to_interpolation_string(&resolved);
        return (subject_str == resolved_str, HashMap::new());
    }

    // Bare string comparison (unquoted literal)
    let subject_str = value_to_interpolation_string(subject);
    (subject_str == pat, HashMap::new())
}

/// Split struct fields by top-level commas (respecting nested braces, parens, and quotes).
fn split_struct_fields(s: &str) -> Vec<&str> {
    let mut elements = Vec::new();
    let mut brace_depth = 0;
    let mut paren_depth = 0;
    let mut in_double = false;
    let mut in_single = false;
    let mut start = 0;
    let bytes = s.as_bytes();

    for i in 0..bytes.len() {
        match bytes[i] {
            b'"' if !in_single => in_double = !in_double,
            b'\'' if !in_double => in_single = !in_single,
            b'{' if !in_double && !in_single => brace_depth += 1,
            b'}' if !in_double && !in_single => brace_depth -= 1,
            b'(' if !in_double && !in_single => paren_depth += 1,
            b')' if !in_double && !in_single => paren_depth -= 1,
            b',' if !in_double && !in_single && brace_depth == 0 && paren_depth == 0 => {
                elements.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start <= s.len() {
        let last = s[start..].trim();
        if !last.is_empty() {
            elements.push(&s[start..]);
        }
    }
    elements
}

/// Find the first colon in a field pattern that separates field name from value pattern.
/// Respects nested structures and quotes.
fn find_field_colon(s: &str) -> Option<usize> {
    let mut brace_depth = 0;
    let mut paren_depth = 0;
    let mut in_double = false;
    let mut in_single = false;
    let bytes = s.as_bytes();

    for (i, &byte) in bytes.iter().enumerate() {
        match byte {
            b'"' if !in_single => in_double = !in_double,
            b'\'' if !in_double => in_single = !in_single,
            b'{' if !in_double && !in_single => brace_depth += 1,
            b'}' if !in_double && !in_single => brace_depth -= 1,
            b'(' if !in_double && !in_single => paren_depth += 1,
            b')' if !in_double && !in_single => paren_depth -= 1,
            b':' if !in_double && !in_single && brace_depth == 0 && paren_depth == 0 => {
                return Some(i);
            }
            _ => {}
        }
    }
    None
}

/// Split tuple elements by top-level commas (respecting nested parens and quotes).
fn split_tuple_elements(s: &str) -> Vec<&str> {
    let mut elements = Vec::new();
    let mut depth = 0;
    let mut in_double = false;
    let mut in_single = false;
    let mut start = 0;
    let bytes = s.as_bytes();

    for i in 0..bytes.len() {
        match bytes[i] {
            b'"' if !in_single => in_double = !in_double,
            b'\'' if !in_double => in_single = !in_single,
            b'(' if !in_double && !in_single => depth += 1,
            b')' if !in_double && !in_single => depth -= 1,
            b',' if !in_double && !in_single && depth == 0 => {
                elements.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start <= s.len() {
        let last = s[start..].trim();
        if !last.is_empty() {
            elements.push(&s[start..]);
        }
    }
    elements
}

/// Split match arms by top-level commas (respecting braces, parens, and quotes)
fn split_match_arms(s: &str) -> Vec<&str> {
    let mut arms = Vec::new();
    let mut depth = 0;
    let mut in_double = false;
    let mut in_single = false;
    let mut start = 0;

    for (i, c) in s.char_indices() {
        match c {
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            '{' | '(' if !in_double && !in_single => depth += 1,
            '}' | ')' if !in_double && !in_single => depth -= 1,
            ',' if !in_double && !in_single && depth == 0 => {
                arms.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < s.len() {
        arms.push(&s[start..]);
    }
    arms
}

/// Find the matching closing `}` from position 0, tracking nesting
fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_double = false;
    let mut in_single = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            '{' if !in_double && !in_single => depth += 1,
            '}' if !in_double && !in_single => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Resolve a match arm result: could be a quoted string, variable, number, or nested expression
fn resolve_match_result(expr: &str, vars: &HashMap<String, Value>) -> String {
    let expr = expr.trim();

    // Quoted string — supports ${...} interpolation inside
    if (expr.starts_with('"') && expr.ends_with('"'))
        || (expr.starts_with('\'') && expr.ends_with('\''))
    {
        let inner = &expr[1..expr.len() - 1];
        return interpolate_string(inner, vars);
    }

    // Variable reference
    if let Some(val) = resolve_var(expr, vars) {
        return value_to_interpolation_string(&val);
    }

    // Try nested arithmetic/ternary
    if let Some(val) = try_arithmetic_expr(expr, vars) {
        return val;
    }

    // Return as literal
    expr.to_string()
}

/// Try to evaluate a ternary expression: `condition ? trueExpr : falseExpr`
///
/// Supports nested ternaries in both branches:
///   `a > 0 ? b > 1 ? "deep" : "shallow" : "negative"`
/// The `?` and `:` are matched by tracking nesting depth.
fn try_ternary_expr(expr: &str, vars: &HashMap<String, Value>) -> Option<String> {
    // Find the first top-level `?` (outside quotes)
    let q_idx = find_top_level_char(expr, '?')?;
    let condition_str = expr[..q_idx].trim();
    let branches = &expr[q_idx + 1..];

    // Find the matching `:` — for every nested `?` we see, skip one `:`.
    let colon_idx = find_matching_colon(branches)?;
    let true_expr = branches[..colon_idx].trim();
    let false_expr = branches[colon_idx + 1..].trim();

    // Evaluate the condition using the same condition evaluator
    let condition_result = default_evaluate_condition(condition_str, vars);

    let chosen = if condition_result { true_expr } else { false_expr };

    // Recursively evaluate if the chosen branch is itself a ternary
    if chosen.contains('?') && chosen.contains(':') {
        if let Some(nested) = try_ternary_expr(chosen, vars) {
            return Some(nested);
        }
    }

    // The chosen branch can be: a quoted string, a variable reference, or a number
    if (chosen.starts_with('"') && chosen.ends_with('"'))
        || (chosen.starts_with('\'') && chosen.ends_with('\''))
    {
        // Quoted string literal — strip quotes, apply interpolation
        let inner = &chosen[1..chosen.len() - 1];
        return Some(interpolate_string(inner, vars));
    }

    // Try as a variable
    if let Some(val) = resolve_var(chosen, vars) {
        return Some(value_to_interpolation_string(&val));
    }

    // Return as literal
    Some(chosen.to_string())
}

/// Find the first occurrence of `ch` at the top level (not inside quotes).
fn find_top_level_char(expr: &str, ch: char) -> Option<usize> {
    let mut in_double = false;
    let mut in_single = false;
    for (i, c) in expr.char_indices() {
        match c {
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            _ if c == ch && !in_double && !in_single => return Some(i),
            _ => {}
        }
    }
    None
}

/// Find the `:` that matches the outermost `?` in a branch string.
/// For every nested `?` encountered (outside quotes), we must skip one `:`.
fn find_matching_colon(branches: &str) -> Option<usize> {
    let mut depth: usize = 0;
    let mut in_double = false;
    let mut in_single = false;
    for (i, c) in branches.char_indices() {
        match c {
            '"' if !in_single => in_double = !in_double,
            '\'' if !in_double => in_single = !in_single,
            '?' if !in_double && !in_single => depth += 1,
            ':' if !in_double && !in_single => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Convert a Value to a string suitable for interpolation.
fn value_to_interpolation_string(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

// ── Default Condition Evaluator ───────────────────────────────────────────────

/// Evaluate a condition expression against variable bindings.
///
/// Supports:
/// - `true`, `false`, `_`, `default`, `else` — literals
/// - `var == value` — equality checks against bound variables
/// - `var != value` — inequality checks
/// - Dotted access (`result.status == ok`)
/// - Bare truthiness checks
pub fn default_evaluate_condition(expr: &str, vars: &HashMap<String, Value>) -> bool {
    let expr = expr.trim();
    eval_or(expr, vars)
}

// ── Expression Parser (recursive descent) ─────────────────────────────────────
//
// Grammar (lowest to highest precedence):
//   or_expr   := and_expr ( "||" and_expr )*
//   and_expr  := unary ( "&&" unary )*
//   unary     := "!" unary | atom
//   atom      := "(" or_expr ")" | comparison | literal | truthy_var
//   comparison := var ("==" | "!=" | ">=" | "<=" | ">" | "<") value
//
// We split on logical operators first (outside parentheses), then evaluate atoms.

/// Evaluate an OR expression: `a || b || c`
fn eval_or(expr: &str, vars: &HashMap<String, Value>) -> bool {
    let parts = split_logical(expr, "||");
    if parts.len() > 1 {
        return parts.iter().any(|part| eval_and(part.trim(), vars));
    }
    eval_and(expr, vars)
}

/// Evaluate an AND expression: `a && b && c`
fn eval_and(expr: &str, vars: &HashMap<String, Value>) -> bool {
    let parts = split_logical(expr, "&&");
    if parts.len() > 1 {
        return parts.iter().all(|part| eval_unary(part.trim(), vars));
    }
    eval_unary(expr, vars)
}

/// Evaluate a unary expression: `!expr` or just `atom`
fn eval_unary(expr: &str, vars: &HashMap<String, Value>) -> bool {
    let expr = expr.trim();
    if let Some(rest) = expr.strip_prefix('!') {
        let rest = rest.trim();
        // Handle `!(...)` or `!var`
        return !eval_unary(rest, vars);
    }
    eval_atom(expr, vars)
}

/// Evaluate an atom: parenthesized expression, comparison, literal, or truthy variable.
fn eval_atom(expr: &str, vars: &HashMap<String, Value>) -> bool {
    let expr = expr.trim();

    // Parenthesized expression
    if expr.starts_with('(') && matching_close_paren(expr) == Some(expr.len() - 1) {
        return eval_or(&expr[1..expr.len() - 1], vars);
    }

    // Literals
    match expr {
        "true" | "_" | "default" | "else" => return true,
        "false" => return false,
        _ => {}
    }

    // Try comparison operators (order matters: >= before >, <= before <, == and != before others)
    // == comparison
    if let Some((lhs, rhs)) = split_comparison(expr, "==") {
        return compare_eq(lhs, rhs, vars);
    }
    // != comparison
    if let Some((lhs, rhs)) = split_comparison(expr, "!=") {
        return !compare_eq(lhs, rhs, vars);
    }
    // >= comparison
    if let Some((lhs, rhs)) = split_comparison(expr, ">=") {
        return compare_ord(lhs, rhs, vars, |a, b| a >= b);
    }
    // <= comparison
    if let Some((lhs, rhs)) = split_comparison(expr, "<=") {
        return compare_ord(lhs, rhs, vars, |a, b| a <= b);
    }
    // > comparison
    if let Some((lhs, rhs)) = split_comparison(expr, ">") {
        return compare_ord(lhs, rhs, vars, |a, b| a > b);
    }
    // < comparison
    if let Some((lhs, rhs)) = split_comparison(expr, "<") {
        return compare_ord(lhs, rhs, vars, |a, b| a < b);
    }

    // `contains` operator: `list contains "value"` or `str contains "sub"`
    if let Some((lhs, rhs)) = split_contains(expr) {
        return eval_contains(lhs, rhs, vars);
    }

    // `in` operator: `"value" in list`
    if let Some((item, collection)) = split_in(expr) {
        return eval_contains(collection, item, vars);
    }

    // `matches` operator: `var matches "pattern"` (regex)
    if let Some((lhs, rhs)) = split_keyword_op(expr, " matches ") {
        return eval_matches(lhs, rhs, vars);
    }

    // `starts_with` operator: `var starts_with "prefix"`
    if let Some((lhs, rhs)) = split_keyword_op(expr, " starts_with ") {
        return eval_starts_with(lhs, rhs, vars);
    }

    // `ends_with` operator: `var ends_with "suffix"`
    if let Some((lhs, rhs)) = split_keyword_op(expr, " ends_with ") {
        return eval_ends_with(lhs, rhs, vars);
    }

    // Match expression (standalone): `match var { "a" => true, _ => false }`
    // Evaluate inline and check truthiness of result.
    if expr.starts_with("match ") {
        if let Some(result) = try_match_expr(expr, vars) {
            let r = result.trim();
            return match r {
                "true" | "1" | "yes" => true,
                "false" | "0" | "no" | "" => false,
                _ => !r.is_empty(),
            };
        }
        return false;
    }

    // Function-call syntax: `len(items)`, `is_empty(name)`, etc.
    if let Some(result) = resolve_function_call(expr, vars) {
        return is_truthy(&result);
    }

    // Bare variable name — truthy check
    // Also support pipes: `name | trim` evaluates as truthy if result is non-empty
    if let Some(filtered) = resolve_value_with_pipes(expr, vars) {
        return !filtered.is_empty();
    }
    if let Some(val) = resolve_var(expr, vars) {
        return is_truthy(&val);
    }

    false
}

/// Split an expression on a logical operator (`&&` or `||`), respecting parentheses and braces.
fn split_logical<'a>(expr: &'a str, op: &str) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut brace_depth = 0i32;
    let mut last = 0;
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();
    let op_len = op_bytes.len();

    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'{' => brace_depth += 1,
            b'}' => brace_depth -= 1,
            b'"' => {
                // Skip quoted strings
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ if depth == 0 && brace_depth == 0 && i + op_len <= bytes.len() && &bytes[i..i + op_len] == op_bytes => {
                parts.push(&expr[last..i]);
                i += op_len;
                last = i;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&expr[last..]);
    parts
}

/// Find the index of the matching close parenthesis for a leading `(`.
fn matching_close_paren(expr: &str) -> Option<usize> {
    if !expr.starts_with('(') {
        return None;
    }
    let mut depth = 0i32;
    for (i, ch) in expr.chars().enumerate() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split a simple comparison expression on an operator, ensuring we don't confuse
/// `>=` with `>` followed by `=`.
fn split_comparison<'a>(expr: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    // For multi-char ops, find the first occurrence outside parens and braces
    let bytes = expr.as_bytes();
    let op_bytes = op.as_bytes();
    let op_len = op_bytes.len();
    let mut depth = 0i32;
    let mut brace_depth = 0i32;

    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'{' => brace_depth += 1,
            b'}' => brace_depth -= 1,
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ if depth == 0 && brace_depth == 0 && i + op_len <= bytes.len() && &bytes[i..i + op_len] == op_bytes => {
                // For single-char ops (> or <), ensure they're not part of >= or <= or =>
                if op_len == 1 && i + 1 < bytes.len() && (bytes[i + 1] == b'=' || bytes[i + 1] == b'>') {
                    i += 1;
                    continue;
                }
                // For `==`, ensure it's not part of `=>`
                if op == "==" && i > 0 && bytes[i] == b'=' && i + 1 < bytes.len() && bytes[i + 1] == b'>' {
                    i += 1;
                    continue;
                }
                let lhs = expr[..i].trim();
                let rhs = expr[i + op_len..].trim().trim_matches('"');
                if !lhs.is_empty() && !rhs.is_empty() {
                    return Some((lhs, rhs));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Resolve a variable (direct lookup or dotted path).
/// Supports `$variable` prefix notation — the `$` is stripped before lookup.
fn resolve_var(name: &str, vars: &HashMap<String, Value>) -> Option<Value> {
    // Strip leading `$` if present (common in .px procedure expressions)
    let name = name.strip_prefix('$').unwrap_or(name);
    if let Some(val) = vars.get(name) {
        return Some(val.clone());
    }
    resolve_dotted(name, vars)
}

/// Resolve a value expression that may contain pipe filters.
/// Used in when-guard conditions to support syntax like:
///   `name | uppercase == "HELLO"`
///   `input | trim != ""`
///   `path | lowercase | trim starts_with "/api"`
///
/// Returns the resolved string value after applying all filters,
/// or None if the expression can't be resolved.
fn resolve_value_with_pipes(expr: &str, vars: &HashMap<String, Value>) -> Option<String> {
    // Check if there's a pipe operator in this expression
    if let Some(pipe_idx) = find_pipe_operator(expr) {
        let value_expr = expr[..pipe_idx].trim();
        let filters_str = expr[pipe_idx + 1..].trim();

        // Resolve the base value
        let base_value = if let Some(val) = resolve_var(value_expr, vars) {
            value_to_interpolation_string(&val)
        } else if value_expr.starts_with('"') && value_expr.ends_with('"') {
            value_expr[1..value_expr.len() - 1].to_string()
        } else {
            return None;
        };

        // Apply filters
        let filters = split_pipe_filters(filters_str);
        let mut result = base_value;
        for filter in &filters {
            result = apply_pipe_filter(filter.trim(), &result, vars)?;
        }
        return Some(result);
    }
    None
}

/// Compare for equality. Supports arithmetic expressions.
fn compare_eq(lhs: &str, rhs: &str, vars: &HashMap<String, Value>) -> bool {
    // Try function-call syntax on lhs: `trim(name) == "hello"`, `upper(x) == "FOO"`
    if let Some(lhs_val) = resolve_function_call(lhs, vars) {
        let rhs_val = resolve_function_call(rhs, vars)
            .map(|v| value_to_interpolation_string(&v))
            .or_else(|| resolve_value_with_pipes(rhs, vars))
            .unwrap_or_else(|| rhs.trim_matches('"').to_string());
        return value_to_interpolation_string(&lhs_val) == rhs_val;
    }

    // Try pipe filters on lhs: `name | uppercase == "HELLO"`
    if let Some(filtered) = resolve_value_with_pipes(lhs, vars) {
        // Also resolve rhs pipes if present
        let rhs_val = resolve_value_with_pipes(rhs, vars)
            .unwrap_or_else(|| rhs.trim_matches('"').to_string());
        return filtered == rhs_val;
    }

    // Match expression on lhs: `match status { ... } == "value"`
    if lhs.trim().starts_with("match ") {
        if let Some(lhs_val) = try_match_expr(lhs.trim(), vars) {
            let rhs_val = if rhs.trim().starts_with("match ") {
                try_match_expr(rhs.trim(), vars).unwrap_or_default()
            } else {
                rhs.trim_matches('"').to_string()
            };
            return lhs_val == rhs_val;
        }
        return false;
    }
    // Match expression on rhs: `var == match level { ... }`
    if rhs.trim().starts_with("match ") {
        if let Some(rhs_val) = try_match_expr(rhs.trim(), vars) {
            if let Some(val) = resolve_var(lhs, vars) {
                return value_to_interpolation_string(&val) == rhs_val;
            }
            return lhs == rhs_val;
        }
        return false;
    }

    // Try arithmetic on lhs first
    if let Some(lhs_num) = eval_numeric_expr(lhs, vars) {
        if let Ok(rhs_num) = rhs.parse::<f64>() {
            return lhs_num == rhs_num;
        }
        // lhs is numeric but rhs isn't — compare as strings
        if lhs_num.fract() == 0.0 {
            return (lhs_num as i64).to_string() == rhs;
        }
        return lhs_num.to_string() == rhs;
    }

    if let Some(val) = resolve_var(lhs, vars) {
        return match &val {
            Value::String(s) => s.as_str() == rhs,
            Value::Number(n) => {
                // Try numeric comparison first
                if let (Some(a), Ok(b)) = (n.as_f64(), rhs.parse::<f64>()) {
                    a == b
                } else {
                    n.to_string() == rhs
                }
            }
            Value::Bool(b) => b.to_string() == rhs,
            Value::Null => rhs == "null",
            _ => false,
        };
    }
    false
}

/// Compare using an ordering function.
/// Supports arithmetic expressions on either side: `count + 1 > threshold`
fn compare_ord(
    lhs: &str,
    rhs: &str,
    vars: &HashMap<String, Value>,
    cmp: impl Fn(f64, f64) -> bool,
) -> bool {
    let lhs_num = eval_numeric_expr(lhs, vars);
    let rhs_num = eval_numeric_expr(rhs, vars);

    match (lhs_num, rhs_num) {
        (Some(a), Some(b)) => cmp(a, b),
        _ => false,
    }
}

/// Evaluate a numeric expression that may contain simple arithmetic.
/// Supports: literal numbers, variable references, and `var +/- N` or `N +/- var`.
fn eval_numeric_expr(expr: &str, vars: &HashMap<String, Value>) -> Option<f64> {
    let expr = expr.trim();

    // Try as a plain number literal first
    if let Ok(n) = expr.parse::<f64>() {
        return Some(n);
    }

    // Try function-call syntax: len(items), length(name), etc.
    if let Some(result) = resolve_function_call(expr, vars) {
        return match &result {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.parse::<f64>().ok(),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        };
    }

    // Try as a variable reference
    if let Some(val) = resolve_var(expr, vars) {
        return match &val {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.parse::<f64>().ok(),
            _ => None,
        };
    }

    // Try arithmetic: look for + or - (not at position 0, which could be a negative sign)
    for op in ['+', '-'] {
        // Find operator outside any leading negative sign
        if let Some(idx) = expr[1..].find(op).map(|i| i + 1) {
            let lhs_part = expr[..idx].trim();
            let rhs_part = expr[idx + 1..].trim();

            // Recursively evaluate both sides (handles var + var, N + var, var + N)
            let a = eval_numeric_expr(lhs_part, vars)?;
            let b = eval_numeric_expr(rhs_part, vars)?;

            return Some(match op {
                '+' => a + b,
                '-' => a - b,
                _ => unreachable!(),
            });
        }
    }

    // Try multiplication, division, and modulo
    for op in ['*', '/', '%'] {
        if let Some(idx) = expr.find(op) {
            let lhs_part = expr[..idx].trim();
            let rhs_part = expr[idx + 1..].trim();

            let a = eval_numeric_expr(lhs_part, vars)?;
            let b = eval_numeric_expr(rhs_part, vars)?;

            return Some(match op {
                '*' => a * b,
                '/' => {
                    if b == 0.0 {
                        return None;
                    }
                    a / b
                }
                '%' => {
                    if b == 0.0 {
                        return None;
                    }
                    a % b
                }
                _ => unreachable!(),
            });
        }
    }

    None
}

/// Resolve a function argument: try nested function call first, then variable lookup, then literal.
/// This enables expressions like `len(trim(name))` where inner functions resolve first.
fn resolve_arg(arg: &str, vars: &HashMap<String, Value>) -> Option<Value> {
    let arg = arg.trim();
    // Try as nested function call first
    if let Some(val) = resolve_function_call(arg, vars) {
        return Some(val);
    }
    // Try as variable
    if let Some(val) = resolve_var(arg, vars) {
        return Some(val);
    }
    // Try as string literal (with interpolation support)
    if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
        let raw = &arg[1..arg.len() - 1];
        // If the string contains ${...}, interpolate variables
        if raw.contains("${") {
            return Some(Value::String(interpolate_string(raw, vars)));
        }
        return Some(Value::String(raw.to_string()));
    }
    // Try as numeric literal
    if let Ok(n) = arg.parse::<i64>() {
        return Some(Value::Number(serde_json::Number::from(n)));
    }
    if let Ok(n) = arg.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return Some(Value::Number(num));
        }
    }
    // Try as boolean literal
    match arg {
        "true" => return Some(Value::Bool(true)),
        "false" => return Some(Value::Bool(false)),
        "null" | "nil" => return Some(Value::Null),
        _ => {}
    }
    None
}

/// Resolve a function-call expression like `len(items)`, `is_empty(name)`, `trim(value)`, etc.
/// Supports nested calls like `len(trim(name))` via recursive `resolve_arg`.
/// Returns the result as a JSON Value, or None if not a recognized function call.
fn resolve_function_call(expr: &str, vars: &HashMap<String, Value>) -> Option<Value> {
    let expr = expr.trim();
    // Match pattern: identifier( ... )
    // Use rfind-based matching to handle nested parens: find the FIRST '(' that matches the last ')'
    let paren_open = expr.find('(')?;
    if !expr.ends_with(')') {
        return None;
    }
    let fn_name = expr[..paren_open].trim();
    // Validate that fn_name is a simple identifier (letters, digits, underscores)
    if fn_name.is_empty() || !fn_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    let args_str = &expr[paren_open + 1..expr.len() - 1];

    // Parse arguments (simple comma split, respecting quotes and nested parens)
    let args = split_function_args(args_str);

    match fn_name {
        // len(var) — returns length of string or array
        // Supports nested calls: len(trim(name))
        "len" | "length" => {
            if args.len() != 1 {
                return None;
            }
            let arg = args[0].trim();
            if let Some(val) = resolve_arg(arg, vars) {
                let n = match &val {
                    Value::String(s) => s.len(),
                    Value::Array(a) => a.len(),
                    Value::Object(o) => o.len(),
                    Value::Null => 0,
                    _ => value_to_interpolation_string(&val).len(),
                };
                return Some(Value::Number(serde_json::Number::from(n)));
            }
            None
        }

        // is_empty(var) — returns true if null, empty string, empty array, or empty object
        "is_empty" | "empty" => {
            if args.len() != 1 {
                return None;
            }
            let arg = args[0].trim();
            if let Some(val) = resolve_arg(arg, vars) {
                let empty = match &val {
                    Value::Null => true,
                    Value::String(s) => s.is_empty(),
                    Value::Array(a) => a.is_empty(),
                    Value::Object(o) => o.is_empty(),
                    Value::Bool(b) => !b,
                    Value::Number(_) => false,
                };
                return Some(Value::Bool(empty));
            }
            // Unresolved var treated as empty
            Some(Value::Bool(true))
        }

        // not_empty(var) — opposite of is_empty
        "not_empty" => {
            if args.len() != 1 {
                return None;
            }
            let arg = args[0].trim();
            if let Some(val) = resolve_arg(arg, vars) {
                let empty = match &val {
                    Value::Null => true,
                    Value::String(s) => s.is_empty(),
                    Value::Array(a) => a.is_empty(),
                    Value::Object(o) => o.is_empty(),
                    Value::Bool(b) => !b,
                    Value::Number(_) => false,
                };
                return Some(Value::Bool(!empty));
            }
            Some(Value::Bool(false))
        }

        // contains(haystack, needle) — check if string/array contains value
        "contains" => {
            if args.len() != 2 {
                return None;
            }
            let haystack_arg = args[0].trim();
            let needle_arg = args[1].trim();
            let haystack_val = resolve_arg(haystack_arg, vars)?;
            // Resolve needle: try as arg first, fall back to raw string
            let needle_str = if let Some(nval) = resolve_arg(needle_arg, vars) {
                value_to_interpolation_string(&nval)
            } else {
                needle_arg.trim_matches('"').to_string()
            };
            let result = match &haystack_val {
                Value::String(s) => s.contains(needle_str.as_str()),
                Value::Array(a) => a.iter().any(|item| {
                    value_to_interpolation_string(item) == needle_str
                }),
                _ => false,
            };
            Some(Value::Bool(result))
        }

        // starts_with(var, prefix)
        "starts_with" => {
            if args.len() != 2 {
                return None;
            }
            let var_arg = args[0].trim();
            let prefix_arg = args[1].trim();
            let val = resolve_arg(var_arg, vars)?;
            let s = value_to_interpolation_string(&val);
            let prefix = if let Some(pval) = resolve_arg(prefix_arg, vars) {
                value_to_interpolation_string(&pval)
            } else {
                prefix_arg.trim_matches('"').to_string()
            };
            Some(Value::Bool(s.starts_with(prefix.as_str())))
        }

        // ends_with(var, suffix)
        "ends_with" => {
            if args.len() != 2 {
                return None;
            }
            let var_arg = args[0].trim();
            let suffix_arg = args[1].trim();
            let val = resolve_arg(var_arg, vars)?;
            let s = value_to_interpolation_string(&val);
            let suffix = if let Some(sval) = resolve_arg(suffix_arg, vars) {
                value_to_interpolation_string(&sval)
            } else {
                suffix_arg.trim_matches('"').to_string()
            };
            Some(Value::Bool(s.ends_with(suffix.as_str())))
        }

        // trim(var) — returns trimmed string
        "trim" => {
            if args.len() != 1 {
                return None;
            }
            let arg = args[0].trim();
            let val = resolve_arg(arg, vars)?;
            let s = value_to_interpolation_string(&val);
            Some(Value::String(s.trim().to_string()))
        }

        // uppercase(var) / upper(var)
        "uppercase" | "upper" => {
            if args.len() != 1 {
                return None;
            }
            let arg = args[0].trim();
            let val = resolve_arg(arg, vars)?;
            let s = value_to_interpolation_string(&val);
            Some(Value::String(s.to_uppercase()))
        }

        // lowercase(var) / lower(var)
        "lowercase" | "lower" => {
            if args.len() != 1 {
                return None;
            }
            let arg = args[0].trim();
            let val = resolve_arg(arg, vars)?;
            let s = value_to_interpolation_string(&val);
            Some(Value::String(s.to_lowercase()))
        }

        // capitalize(var)
        "capitalize" | "cap" => {
            if args.len() != 1 {
                return None;
            }
            let arg = args[0].trim();
            let val = resolve_arg(arg, vars)?;
            let s = value_to_interpolation_string(&val);
            let mut chars = s.chars();
            let result = match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    format!("{}{}", upper, chars.as_str())
                }
            };
            Some(Value::String(result))
        }

        // reverse(var)
        "reverse" | "rev" => {
            if args.len() != 1 {
                return None;
            }
            let arg = args[0].trim();
            let val = resolve_arg(arg, vars)?;
            let s = value_to_interpolation_string(&val);
            Some(Value::String(s.chars().rev().collect()))
        }

        // default(var, fallback) — return var value if truthy, otherwise fallback
        "default" | "fallback" => {
            if args.len() != 2 {
                return None;
            }
            let var_arg = args[0].trim();
            let fallback_arg = args[1].trim();
            if let Some(val) = resolve_arg(var_arg, vars) {
                if is_truthy(&val) {
                    return Some(val);
                }
            }
            // Resolve fallback as arg too
            if let Some(fb) = resolve_arg(fallback_arg, vars) {
                return Some(fb);
            }
            Some(Value::String(fallback_arg.trim_matches('"').to_string()))
        }

        // type_of(var) — returns the JSON type as a string
        "type_of" | "typeof" => {
            if args.len() != 1 {
                return None;
            }
            let arg = args[0].trim();
            if let Some(val) = resolve_arg(arg, vars) {
                let type_name = match &val {
                    Value::Null => "null",
                    Value::Bool(_) => "boolean",
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                    Value::Array(_) => "array",
                    Value::Object(_) => "object",
                };
                return Some(Value::String(type_name.to_string()));
            }
            Some(Value::String("null".to_string()))
        }

        // one_of(var, val1, val2, ...) — returns true if var equals any of the values
        // Like SQL IN: `one_of(status, "active", "pending", "review")`
        "one_of" | "any_of" | "in_list" => {
            if args.len() < 2 {
                return None;
            }
            let var_arg = args[0].trim();
            let var_val = resolve_arg(var_arg, vars)?;
            let var_str = value_to_interpolation_string(&var_val);
            for candidate in &args[1..] {
                let candidate = candidate.trim();
                if let Some(cval) = resolve_arg(candidate, vars) {
                    if value_to_interpolation_string(&cval) == var_str {
                        return Some(Value::Bool(true));
                    }
                } else {
                    // Raw string fallback
                    let raw = candidate.trim_matches('"');
                    if raw == var_str {
                        return Some(Value::Bool(true));
                    }
                }
            }
            Some(Value::Bool(false))
        }

        // none_of(var, val1, val2, ...) — returns true if var does NOT equal any of the values
        "none_of" | "not_in" => {
            if args.len() < 2 {
                return None;
            }
            let var_arg = args[0].trim();
            let var_val = resolve_arg(var_arg, vars)?;
            let var_str = value_to_interpolation_string(&var_val);
            for candidate in &args[1..] {
                let candidate = candidate.trim();
                if let Some(cval) = resolve_arg(candidate, vars) {
                    if value_to_interpolation_string(&cval) == var_str {
                        return Some(Value::Bool(false));
                    }
                } else {
                    let raw = candidate.trim_matches('"');
                    if raw == var_str {
                        return Some(Value::Bool(false));
                    }
                }
            }
            Some(Value::Bool(true))
        }

        // if_else(condition, then_value, else_value) — conditional expression
        // Example: `if_else(is_admin, "full", "limited")`
        "if_else" | "ternary" | "cond" => {
            if args.len() != 3 {
                return None;
            }
            let cond_arg = args[0].trim();
            let then_arg = args[1].trim();
            let else_arg = args[2].trim();
            // Evaluate condition: try as function call, then as variable truthy check
            let cond_result = if let Some(val) = resolve_arg(cond_arg, vars) {
                is_truthy(&val)
            } else {
                // Try evaluating as a sub-expression
                default_evaluate_condition(cond_arg, vars)
            };
            if cond_result {
                resolve_arg(then_arg, vars).or_else(|| {
                    Some(Value::String(then_arg.trim_matches('"').to_string()))
                })
            } else {
                resolve_arg(else_arg, vars).or_else(|| {
                    Some(Value::String(else_arg.trim_matches('"').to_string()))
                })
            }
        }

        // coalesce(val1, val2, ...) — returns the first non-null, non-empty value
        // Example: `coalesce(display_name, username, "anonymous")`
        "coalesce" | "first_of" => {
            if args.is_empty() {
                return None;
            }
            for arg in &args {
                let arg = arg.trim();
                if let Some(val) = resolve_arg(arg, vars) {
                    if is_truthy(&val) {
                        return Some(val);
                    }
                }
            }
            // All were null/empty — return null
            Some(Value::Null)
        }

        // min(a, b, ...) — returns the minimum numeric value
        "min" => {
            if args.is_empty() {
                return None;
            }
            let mut min_val: Option<f64> = None;
            for arg in &args {
                let arg = arg.trim();
                if let Some(val) = resolve_arg(arg, vars) {
                    if let Some(n) = value_to_f64(&val) {
                        min_val = Some(match min_val {
                            None => n,
                            Some(current) => current.min(n),
                        });
                    }
                }
            }
            min_val.and_then(|n| {
                if n == n.floor() && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
                    Some(Value::Number(serde_json::Number::from(n as i64)))
                } else {
                    serde_json::Number::from_f64(n).map(Value::Number)
                }
            })
        }

        // max(a, b, ...) — returns the maximum numeric value
        "max" => {
            if args.is_empty() {
                return None;
            }
            let mut max_val: Option<f64> = None;
            for arg in &args {
                let arg = arg.trim();
                if let Some(val) = resolve_arg(arg, vars) {
                    if let Some(n) = value_to_f64(&val) {
                        max_val = Some(match max_val {
                            None => n,
                            Some(current) => current.max(n),
                        });
                    }
                }
            }
            max_val.and_then(|n| {
                if n == n.floor() && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
                    Some(Value::Number(serde_json::Number::from(n as i64)))
                } else {
                    serde_json::Number::from_f64(n).map(Value::Number)
                }
            })
        }

        // abs(var) — returns absolute value
        "abs" => {
            if args.len() != 1 {
                return None;
            }
            let arg = args[0].trim();
            let val = resolve_arg(arg, vars)?;
            let n = value_to_f64(&val)?;
            let abs_n = n.abs();
            if abs_n == abs_n.floor() && abs_n >= 0.0 && abs_n <= i64::MAX as f64 {
                Some(Value::Number(serde_json::Number::from(abs_n as i64)))
            } else {
                serde_json::Number::from_f64(abs_n).map(Value::Number)
            }
        }

        // clamp(val, min, max) — clamps val between min and max
        "clamp" => {
            if args.len() != 3 {
                return None;
            }
            let val = resolve_arg(args[0].trim(), vars)?;
            let min_val = resolve_arg(args[1].trim(), vars)?;
            let max_val = resolve_arg(args[2].trim(), vars)?;
            let n = value_to_f64(&val)?;
            let lo = value_to_f64(&min_val)?;
            let hi = value_to_f64(&max_val)?;
            let clamped = n.max(lo).min(hi);
            if clamped == clamped.floor() && clamped >= i64::MIN as f64 && clamped <= i64::MAX as f64
            {
                Some(Value::Number(serde_json::Number::from(clamped as i64)))
            } else {
                serde_json::Number::from_f64(clamped).map(Value::Number)
            }
        }

        _ => None,
    }
}

/// Split function arguments, respecting nested parens and quoted strings.
fn split_function_args(args_str: &str) -> Vec<&str> {
    let args_str = args_str.trim();
    if args_str.is_empty() {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_quotes = false;
    let mut last = 0;
    let bytes = args_str.as_bytes();

    for i in 0..bytes.len() {
        match bytes[i] {
            b'"' if !in_quotes => in_quotes = true,
            b'"' if in_quotes => in_quotes = false,
            b'(' if !in_quotes => depth += 1,
            b')' if !in_quotes => depth -= 1,
            b',' if !in_quotes && depth == 0 => {
                parts.push(&args_str[last..i]);
                last = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&args_str[last..]);
    parts
}

/// Check if a JSON value is truthy.
fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        _ => true,
    }
}

/// Convert a JSON value to f64 (for numeric functions).
fn value_to_f64(val: &Value) -> Option<f64> {
    match val {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        Value::Bool(true) => Some(1.0),
        Value::Bool(false) => Some(0.0),
        _ => None,
    }
}

/// Split a `contains` expression: `lhs contains rhs`
fn split_contains(expr: &str) -> Option<(&str, &str)> {
    // Find " contains " token outside parens/quotes
    let needle = " contains ";
    let idx = find_keyword_outside_parens(expr, needle)?;
    let lhs = expr[..idx].trim();
    let rhs = expr[idx + needle.len()..].trim().trim_matches('"');
    if !lhs.is_empty() && !rhs.is_empty() {
        Some((lhs, rhs))
    } else {
        None
    }
}

/// Split an `in` expression: `item in collection`
fn split_in(expr: &str) -> Option<(&str, &str)> {
    let needle = " in ";
    let idx = find_keyword_outside_parens(expr, needle)?;
    let item = expr[..idx].trim().trim_matches('"');
    let collection = expr[idx + needle.len()..].trim();
    if !item.is_empty() && !collection.is_empty() {
        Some((item, collection))
    } else {
        None
    }
}

/// Find the position of a keyword token in an expression, respecting parens and quotes.
fn find_keyword_outside_parens(expr: &str, keyword: &str) -> Option<usize> {
    let bytes = expr.as_bytes();
    let kw_bytes = keyword.as_bytes();
    let kw_len = kw_bytes.len();
    let mut depth = 0i32;
    let mut in_quotes = false;

    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            in_quotes = !in_quotes;
        } else if !in_quotes {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ if depth == 0
                    && i + kw_len <= bytes.len()
                    && &bytes[i..i + kw_len] == kw_bytes =>
                {
                    return Some(i);
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

/// Evaluate a `contains` check. Works for:
/// - Arrays: checks if the array contains the value
/// - Strings: checks if the string contains the substring
fn eval_contains(collection_expr: &str, item_expr: &str, vars: &HashMap<String, Value>) -> bool {
    // Try pipe filters on collection_expr for string containment
    if let Some(filtered_collection) = resolve_value_with_pipes(collection_expr, vars) {
        let sub = if let Some(filtered_item) = resolve_value_with_pipes(item_expr, vars) {
            filtered_item
        } else {
            match resolve_var(item_expr, vars) {
                Some(Value::String(v)) => v,
                _ => item_expr.to_string(),
            }
        };
        return filtered_collection.contains(&sub);
    }

    let collection = resolve_var(collection_expr, vars);
    match collection {
        Some(Value::Array(arr)) => {
            // Check if any element matches the item
            let item_val = resolve_var(item_expr, vars);
            match item_val {
                Some(val) => arr.contains(&val),
                None => {
                    // Treat as literal string
                    let item_as_value = Value::String(item_expr.to_string());
                    arr.contains(&item_as_value)
                        || arr.iter().any(|el| match el {
                            Value::Number(n) => n.to_string() == item_expr,
                            _ => false,
                        })
                }
            }
        }
        Some(Value::String(s)) => {
            // String containment
            let sub = match resolve_var(item_expr, vars) {
                Some(Value::String(v)) => v,
                _ => item_expr.to_string(),
            };
            s.contains(&sub)
        }
        _ => false,
    }
}

/// Split a keyword operator expression generically.
fn split_keyword_op<'a>(expr: &'a str, keyword: &str) -> Option<(&'a str, &'a str)> {
    let idx = find_keyword_outside_parens(expr, keyword)?;
    let lhs = expr[..idx].trim();
    let rhs = expr[idx + keyword.len()..].trim().trim_matches('"');
    if !lhs.is_empty() && !rhs.is_empty() {
        Some((lhs, rhs))
    } else {
        None
    }
}

/// Evaluate a `matches` (regex) check.
fn eval_matches(var_expr: &str, pattern: &str, vars: &HashMap<String, Value>) -> bool {
    // Try pipe filters first
    let haystack = if let Some(filtered) = resolve_value_with_pipes(var_expr, vars) {
        filtered
    } else {
        let val = resolve_var(var_expr, vars);
        match &val {
            Some(Value::String(s)) => s.as_str().to_owned(),
            Some(Value::Number(n)) => n.to_string(),
            _ => return false,
        }
    };
    match regex::Regex::new(pattern) {
        Ok(re) => re.is_match(&haystack),
        Err(_) => false,
    }
}

/// Evaluate a `starts_with` check. Supports pipe filters: `name | trim starts_with "prefix"`
fn eval_starts_with(var_expr: &str, prefix: &str, vars: &HashMap<String, Value>) -> bool {
    // Try pipe filters first
    if let Some(filtered) = resolve_value_with_pipes(var_expr, vars) {
        return filtered.starts_with(prefix);
    }
    let val = resolve_var(var_expr, vars);
    match &val {
        Some(Value::String(s)) => s.starts_with(prefix),
        _ => false,
    }
}

/// Evaluate an `ends_with` check. Supports pipe filters: `name | trim ends_with "suffix"`
fn eval_ends_with(var_expr: &str, suffix: &str, vars: &HashMap<String, Value>) -> bool {
    // Try pipe filters first
    if let Some(filtered) = resolve_value_with_pipes(var_expr, vars) {
        return filtered.ends_with(suffix);
    }
    let val = resolve_var(var_expr, vars);
    match &val {
        Some(Value::String(s)) => s.ends_with(suffix),
        _ => false,
    }
}
///
/// Supports paths like:
/// - `result.status` — nested object access
/// - `items[0]` — array index
/// - `items[0].name` — array index then object access
/// - `data["key"]` — bracket key access on objects
/// - `response.data.items[2].id` — mixed paths
fn resolve_dotted(path: &str, vars: &HashMap<String, Value>) -> Option<Value> {
    let segments = parse_path_segments(path);
    if segments.len() < 2 {
        return None;
    }

    // First segment must be a key (root variable name)
    let root_key = segments[0].as_str()?;
    let root = vars.get(root_key)?;
    let mut current = root;

    for segment in &segments[1..] {
        current = resolve_segment(current, segment)?;
    }

    Some(current.clone())
}

/// A path segment: either a string key or a numeric index.
#[derive(Debug, Clone, PartialEq)]
enum PathSegment {
    Key(String),
    Index(usize),
}

impl PathSegment {
    fn as_str(&self) -> Option<&str> {
        match self {
            PathSegment::Key(s) => Some(s.as_str()),
            PathSegment::Index(_) => None,
        }
    }
}

/// Parse a path string into segments.
///
/// Examples:
/// - `"foo.bar"` → `[Key("foo"), Key("bar")]`
/// - `"items[0]"` → `[Key("items"), Index(0)]`
/// - `"items[0].name"` → `[Key("items"), Index(0), Key("name")]`
/// - `"data[\"key\"]"` → `[Key("data"), Key("key")]`
fn parse_path_segments(path: &str) -> Vec<PathSegment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = path.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '.' => {
                if !current.is_empty() {
                    segments.push(PathSegment::Key(current.clone()));
                    current.clear();
                }
            }
            '[' => {
                if !current.is_empty() {
                    segments.push(PathSegment::Key(current.clone()));
                    current.clear();
                }
                // Parse bracket content
                i += 1;
                let mut bracket_content = String::new();
                while i < chars.len() && chars[i] != ']' {
                    bracket_content.push(chars[i]);
                    i += 1;
                }
                // Determine if it's a numeric index or a string key
                let trimmed = bracket_content.trim();
                if let Ok(idx) = trimmed.parse::<usize>() {
                    segments.push(PathSegment::Index(idx));
                } else {
                    // Strip quotes if present: ["key"] or ['key']
                    let key = trimmed
                        .trim_start_matches('"')
                        .trim_end_matches('"')
                        .trim_start_matches('\'')
                        .trim_end_matches('\'');
                    segments.push(PathSegment::Key(key.to_string()));
                }
            }
            ']' => {
                // Already consumed by '[' handler
            }
            c => {
                current.push(c);
            }
        }
        i += 1;
    }

    if !current.is_empty() {
        segments.push(PathSegment::Key(current));
    }

    segments
}

/// Resolve a single segment against a JSON value.
fn resolve_segment<'a>(value: &'a Value, segment: &PathSegment) -> Option<&'a Value> {
    match segment {
        PathSegment::Key(key) => value.get(key.as_str()),
        PathSegment::Index(idx) => value.get(*idx),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A test handler that records calls and returns configurable results.
    struct MockHandler {
        results: HashMap<String, Value>,
    }

    impl MockHandler {
        fn new() -> Self {
            Self {
                results: HashMap::new(),
            }
        }

        fn with_result(mut self, action: &str, result: Value) -> Self {
            self.results.insert(action.to_string(), result);
            self
        }
    }

    impl ActionHandler for MockHandler {
        fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
            self.results
                .get(name)
                .cloned()
                .ok_or_else(|| ExecutionError::UnknownAction(name.to_string()))
        }
    }

    #[test]
    fn execute_simple_call() {
        let handler = MockHandler::new().with_result("greet", json!("hello"));

        let procedure = json!({
            "type": "procedure",
            "name": "test_proc",
            "steps": [
                { "kind": "call", "name": "greet", "params": {}, "output_var": "greeting" }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        assert_eq!(result.procedure_name, "test_proc");
        assert_eq!(result.step_results.len(), 1);
        assert_eq!(result.step_results[0].output, Some(json!("hello")));
        assert_eq!(result.variables.get("greeting"), Some(&json!("hello")));
    }

    #[test]
    fn execute_call_chain_with_var_passing() {
        let handler = MockHandler::new()
            .with_result("fetch_data", json!({"status": "ok", "count": 42}))
            .with_result("process", json!("done"));

        let procedure = json!({
            "type": "procedure",
            "name": "chain",
            "steps": [
                { "kind": "call", "name": "fetch_data", "params": {}, "output_var": "data" },
                { "kind": "call", "name": "process", "params": { "input": "$data" }, "output_var": "result" }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        assert_eq!(
            result.variables.get("data"),
            Some(&json!({"status": "ok", "count": 42}))
        );
        assert_eq!(result.variables.get("result"), Some(&json!("done")));
    }

    #[test]
    fn execute_when_condition_true() {
        let handler = MockHandler::new().with_result("notify", json!("notified"));

        let procedure = json!({
            "type": "procedure",
            "name": "conditional",
            "steps": [
                {
                    "kind": "when",
                    "condition": "should_notify",
                    "steps": [
                        { "kind": "call", "name": "notify", "params": {} }
                    ]
                }
            ]
        });

        let vars = HashMap::from([("should_notify".to_string(), json!(true))]);
        let result = execute_with_vars(&procedure, &handler, vars).unwrap();
        assert!(!result.step_results[0].skipped);
        assert_eq!(result.step_results[0].output, Some(json!("notified")));
    }

    #[test]
    fn execute_when_condition_false_skips() {
        let handler = MockHandler::new();

        let procedure = json!({
            "type": "procedure",
            "name": "conditional",
            "steps": [
                {
                    "kind": "when",
                    "condition": "should_notify",
                    "steps": [
                        { "kind": "call", "name": "notify", "params": {} }
                    ]
                }
            ]
        });

        let vars = HashMap::from([("should_notify".to_string(), json!(false))]);
        let result = execute_with_vars(&procedure, &handler, vars).unwrap();
        assert!(result.step_results[0].skipped);
    }

    #[test]
    fn execute_match_selects_first_true_arm() {
        let handler = MockHandler::new();

        let procedure = json!({
            "type": "procedure",
            "name": "matcher",
            "steps": [
                {
                    "kind": "match",
                    "arms": [
                        { "condition": "status == error", "result": "handle_error" },
                        { "condition": "status == ok", "result": "handle_ok" },
                        { "condition": "default", "result": "handle_default" }
                    ]
                }
            ]
        });

        let vars = HashMap::from([("status".to_string(), json!("ok"))]);
        let result = execute_with_vars(&procedure, &handler, vars).unwrap();
        assert_eq!(result.step_results[0].output, Some(json!("handle_ok")));
        assert!(!result.step_results[0].skipped);
    }

    #[test]
    fn execute_match_falls_through_to_default() {
        let handler = MockHandler::new();

        let procedure = json!({
            "type": "procedure",
            "name": "matcher",
            "steps": [
                {
                    "kind": "match",
                    "arms": [
                        { "condition": "status == error", "result": "handle_error" },
                        { "condition": "default", "result": "fallback" }
                    ]
                }
            ]
        });

        let vars = HashMap::from([("status".to_string(), json!("ok"))]);
        let result = execute_with_vars(&procedure, &handler, vars).unwrap();
        assert_eq!(result.step_results[0].output, Some(json!("fallback")));
    }

    #[test]
    fn execute_match_no_match_skips() {
        let handler = MockHandler::new();

        let procedure = json!({
            "type": "procedure",
            "name": "matcher",
            "steps": [
                {
                    "kind": "match",
                    "arms": [
                        { "condition": "status == error", "result": "handle_error" }
                    ]
                }
            ]
        });

        let vars = HashMap::from([("status".to_string(), json!("ok"))]);
        let result = execute_with_vars(&procedure, &handler, vars).unwrap();
        assert!(result.step_results[0].skipped);
    }

    #[test]
    fn execute_unknown_action_errors() {
        let handler = MockHandler::new();

        let procedure = json!({
            "type": "procedure",
            "name": "bad",
            "steps": [
                { "kind": "call", "name": "nonexistent", "params": {} }
            ]
        });

        let result = execute(&procedure, &handler);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExecutionError::UnknownAction(_)
        ));
    }

    #[test]
    fn resolve_vars_in_params() {
        let vars = HashMap::from([
            ("name".to_string(), json!("world")),
            ("count".to_string(), json!(42)),
        ]);

        let params = json!({
            "greeting": "$name",
            "times": "$count",
            "literal": "hello",
            "nested": { "ref": "$name" }
        });

        let resolved = resolve_vars(&params, &vars);
        assert_eq!(resolved["greeting"], json!("world"));
        assert_eq!(resolved["times"], json!(42));
        assert_eq!(resolved["literal"], json!("hello"));
        assert_eq!(resolved["nested"]["ref"], json!("world"));
    }

    #[test]
    fn default_condition_evaluator() {
        let vars = HashMap::from([
            ("status".to_string(), json!("ok")),
            ("count".to_string(), json!(5)),
            ("flag".to_string(), json!(true)),
        ]);

        assert!(default_evaluate_condition("true", &vars));
        assert!(!default_evaluate_condition("false", &vars));
        assert!(default_evaluate_condition("_", &vars));
        assert!(default_evaluate_condition("default", &vars));
        assert!(default_evaluate_condition("status == ok", &vars));
        assert!(!default_evaluate_condition("status == error", &vars));
        assert!(default_evaluate_condition("status != error", &vars));
        assert!(default_evaluate_condition("count == 5", &vars));
        assert!(default_evaluate_condition("flag", &vars));
        assert!(!default_evaluate_condition("nonexistent", &vars));
    }

    #[test]
    fn dotted_access_in_conditions() {
        let vars = HashMap::from([("result".to_string(), json!({"status": "green", "count": 3}))]);

        assert!(default_evaluate_condition("result.status == green", &vars));
        assert!(!default_evaluate_condition("result.status == red", &vars));
        assert!(default_evaluate_condition("result.count == 3", &vars));
    }

    #[test]
    fn end_to_end_compiled_procedure() {
        // Test the executor against compiler output format directly.
        // This validates that the executor correctly handles the JSON
        // structure that the compiler produces.
        let proc_data = json!({
            "type": "procedure",
            "name": "deploy_check",
            "trigger": { "kind": "manual" },
            "steps": [
                { "kind": "call", "name": "check_window", "params": {}, "output_var": "window_status" },
                {
                    "kind": "match",
                    "arms": [
                        { "condition": "window_status == blocked", "result": "abort" },
                        { "condition": "_", "result": "proceed" }
                    ]
                }
            ]
        });

        let handler = MockHandler::new().with_result("check_window", json!("open"));

        let result = execute(&proc_data, &handler).unwrap();
        assert!(result.success);
        assert_eq!(result.procedure_name, "deploy_check");
        assert_eq!(result.variables.get("window_status"), Some(&json!("open")));
        // "open" != "blocked", so match falls to default "_"
        assert_eq!(result.step_results[1].output, Some(json!("proceed")));
    }

    #[test]
    fn execute_loop_over_array() {
        let handler = MockHandler::new().with_result("process_item", json!("processed"));

        let procedure = json!({
            "type": "procedure",
            "name": "batch",
            "steps": [
                {
                    "kind": "loop",
                    "over": "items",
                    "as": "item",
                    "steps": [
                        { "kind": "call", "name": "process_item", "params": { "val": "$item" } }
                    ],
                    "output_var": "results"
                }
            ]
        });

        let vars = HashMap::from([("items".to_string(), json!(["a", "b", "c"]))]);
        let result = execute_with_vars(&procedure, &handler, vars).unwrap();
        assert!(result.success);
        assert_eq!(
            result.variables.get("results"),
            Some(&json!(["processed", "processed", "processed"]))
        );
    }

    #[test]
    fn execute_loop_times() {
        let handler = MockHandler::new().with_result("tick", json!("tock"));

        let procedure = json!({
            "type": "procedure",
            "name": "repeat",
            "steps": [
                {
                    "kind": "loop",
                    "times": 3,
                    "steps": [
                        { "kind": "call", "name": "tick", "params": {} }
                    ],
                    "output_var": "ticks"
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        assert_eq!(
            result.variables.get("ticks"),
            Some(&json!(["tock", "tock", "tock"]))
        );
    }

    #[test]
    fn execute_loop_missing_var_skips() {
        let handler = MockHandler::new();

        let procedure = json!({
            "type": "procedure",
            "name": "skip",
            "steps": [
                {
                    "kind": "loop",
                    "over": "nonexistent",
                    "steps": [
                        { "kind": "call", "name": "noop", "params": {} }
                    ]
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.step_results[0].skipped);
    }

    #[test]
    fn execute_loop_over_map() {
        let handler = MockHandler::new()
            .with_result("process", json!("ok"));

        let procedure = json!({
            "type": "procedure",
            "name": "map_iter",
            "steps": [
                {
                    "kind": "loop",
                    "over": "config",
                    "as": "value",
                    "output_var": "results",
                    "steps": [
                        { "kind": "call", "name": "process", "params": {} }
                    ]
                }
            ]
        });

        let vars = HashMap::from([("config".to_string(), json!({ "host": "localhost", "port": "8080", "debug": "true" }))]);
        let result = execute_with_vars(&procedure, &handler, vars).unwrap();
        assert!(result.success);
        // 3 entries in the map = 3 call results collected
        let results = result.variables.get("results").unwrap();
        assert_eq!(results.as_array().unwrap().len(), 3);
    }

    #[test]
    fn execute_loop_over_map_binds_key() {
        let handler = MockHandler::new()
            .with_result("noop", json!("done"));

        let procedure = json!({
            "type": "procedure",
            "name": "map_kv",
            "steps": [
                {
                    "kind": "loop",
                    "over": "settings",
                    "as": "val",
                    "key_as": "k",
                    "output_var": "out",
                    "steps": [
                        { "kind": "call", "name": "noop", "params": {} }
                    ]
                }
            ]
        });

        let vars = HashMap::from([("settings".to_string(), json!({ "alpha": "1", "beta": "2" }))]);
        let result = execute_with_vars(&procedure, &handler, vars).unwrap();
        assert!(result.success);
        // key and value vars should be cleaned up after loop
        assert!(!result.variables.contains_key("k"));
        assert!(!result.variables.contains_key("val"));
        // output collected
        let out = result.variables.get("out").unwrap();
        assert_eq!(out.as_array().unwrap().len(), 2);
    }

    #[test]
    fn execute_loop_over_map_collects_output() {
        let handler = MockHandler::new()
            .with_result("double", json!("doubled"));

        let procedure = json!({
            "type": "procedure",
            "name": "map_collect",
            "steps": [
                {
                    "kind": "loop",
                    "over": "items",
                    "output_var": "collected",
                    "steps": [
                        { "kind": "call", "name": "double", "params": {} }
                    ]
                }
            ]
        });

        let vars = HashMap::from([("items".to_string(), json!({ "x": "1", "y": "2", "z": "3" }))]);
        let result = execute_with_vars(&procedure, &handler, vars).unwrap();
        assert!(result.success);
        // output_var should contain 3 "doubled" results
        let collected = result.variables.get("collected").unwrap();
        assert_eq!(collected.as_array().unwrap().len(), 3);
    }

    #[test]
    fn execute_emit_appends_to_emit_var() {
        let handler = MockHandler::new();

        let procedure = json!({
            "type": "procedure",
            "name": "emitter",
            "steps": [
                {
                    "kind": "emit",
                    "event": { "type": "timer", "id": "t1", "name": "check", "recurring": false }
                },
                {
                    "kind": "emit",
                    "event": { "type": "timer", "id": "t2", "name": "backup", "recurring": true }
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        let emit = result.variables.get("emit").unwrap();
        let arr = emit.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "check");
        assert_eq!(arr[1]["name"], "backup");
    }

    #[test]
    fn execute_emit_resolves_vars() {
        let handler = MockHandler::new();

        let procedure = json!({
            "type": "procedure",
            "name": "emitter",
            "steps": [
                {
                    "kind": "emit",
                    "event": { "type": "state_change", "key": "$target_key", "new_value": "done" }
                }
            ]
        });

        let vars = HashMap::from([("target_key".to_string(), json!("build_status"))]);
        let result = execute_with_vars(&procedure, &handler, vars).unwrap();
        let emit = result.variables.get("emit").unwrap();
        assert_eq!(emit[0]["key"], "build_status");
    }

    #[test]
    fn execute_try_catches_error() {
        let handler = MockHandler::new().with_result("fallback", json!("recovered"));

        let procedure = json!({
            "type": "procedure",
            "name": "resilient",
            "steps": [
                {
                    "kind": "try",
                    "steps": [
                        { "kind": "call", "name": "failing_action", "params": {} }
                    ],
                    "catch": [
                        { "kind": "call", "name": "fallback", "params": {} }
                    ]
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        // The error was caught and fallback ran
        assert!(!result.step_results[0].skipped);
        assert_eq!(result.step_results[0].output, Some(json!("recovered")));
        // Error variable should be set
        assert!(result.variables.get("error").is_some());
    }

    #[test]
    fn execute_try_no_error_clears_error_var() {
        let handler = MockHandler::new().with_result("safe_action", json!("ok"));

        let procedure = json!({
            "type": "procedure",
            "name": "safe",
            "steps": [
                {
                    "kind": "try",
                    "steps": [
                        { "kind": "call", "name": "safe_action", "params": {} }
                    ],
                    "catch": [
                        { "kind": "call", "name": "fallback", "params": {} }
                    ]
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        assert!(result.variables.get("error").is_none());
    }

    #[test]
    fn execute_try_without_catch_still_succeeds() {
        let handler = MockHandler::new();

        let procedure = json!({
            "type": "procedure",
            "name": "no_catch",
            "steps": [
                {
                    "kind": "try",
                    "steps": [
                        { "kind": "call", "name": "failing_action", "params": {} }
                    ]
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        // Error info in output but procedure continues
        assert!(result.step_results[0].output.is_some());
    }

    #[test]
    fn end_to_end_parse_compile_execute() {
        // Full pipeline: parse .px source → compile → execute
        use crate::px::{compiler::compile, parse};

        // Use valid .px grammar syntax
        let source = "procedure greet_user:\n  trigger: manual\n  say_hello {} -> $greeting\n";

        let doc = parse(source).expect("parse failed");
        let records = compile(&doc);
        assert_eq!(records.len(), 1);

        let handler = MockHandler::new().with_result("say_hello", json!("hello world"));

        let result = execute(&records[0].data, &handler).unwrap();
        assert!(result.success);
        assert_eq!(result.procedure_name, "greet_user");
        assert_eq!(
            result.variables.get("greeting"),
            Some(&json!("hello world"))
        );
    }

    // ── Logical operator tests ────────────────────────────────────────────────

    #[test]
    fn condition_and_operator() {
        let vars = HashMap::from([
            ("status".to_string(), json!("ok")),
            ("count".to_string(), json!(5)),
            ("flag".to_string(), json!(true)),
        ]);

        // Both true
        assert!(default_evaluate_condition("status == ok && flag", &vars));
        // First false
        assert!(!default_evaluate_condition(
            "status == error && flag",
            &vars
        ));
        // Second false
        assert!(!default_evaluate_condition(
            "flag && status == error",
            &vars
        ));
        // Triple AND
        assert!(default_evaluate_condition(
            "status == ok && flag && count == 5",
            &vars
        ));
        assert!(!default_evaluate_condition(
            "status == ok && flag && count == 99",
            &vars
        ));
    }

    #[test]
    fn condition_or_operator() {
        let vars = HashMap::from([
            ("status".to_string(), json!("ok")),
            ("flag".to_string(), json!(false)),
        ]);

        // First true
        assert!(default_evaluate_condition("status == ok || flag", &vars));
        // Second true (first false)
        assert!(default_evaluate_condition(
            "status == error || status == ok",
            &vars
        ));
        // Both false
        assert!(!default_evaluate_condition(
            "status == error || flag",
            &vars
        ));
    }

    #[test]
    fn condition_not_operator() {
        let vars = HashMap::from([
            ("flag".to_string(), json!(true)),
            ("empty".to_string(), json!(false)),
        ]);

        assert!(!default_evaluate_condition("!flag", &vars));
        assert!(default_evaluate_condition("!empty", &vars));
        assert!(default_evaluate_condition("!nonexistent", &vars));
        // Double negation
        assert!(default_evaluate_condition("!!flag", &vars));
    }

    #[test]
    fn condition_comparison_operators() {
        let vars = HashMap::from([
            ("count".to_string(), json!(5)),
            ("score".to_string(), json!(85.5)),
        ]);

        // Greater than
        assert!(default_evaluate_condition("count > 3", &vars));
        assert!(!default_evaluate_condition("count > 5", &vars));
        assert!(!default_evaluate_condition("count > 10", &vars));

        // Less than
        assert!(default_evaluate_condition("count < 10", &vars));
        assert!(!default_evaluate_condition("count < 5", &vars));
        assert!(!default_evaluate_condition("count < 3", &vars));

        // Greater or equal
        assert!(default_evaluate_condition("count >= 5", &vars));
        assert!(default_evaluate_condition("count >= 4", &vars));
        assert!(!default_evaluate_condition("count >= 6", &vars));

        // Less or equal
        assert!(default_evaluate_condition("count <= 5", &vars));
        assert!(default_evaluate_condition("count <= 6", &vars));
        assert!(!default_evaluate_condition("count <= 4", &vars));

        // Float comparisons
        assert!(default_evaluate_condition("score > 80", &vars));
        assert!(default_evaluate_condition("score < 90", &vars));
        assert!(default_evaluate_condition("score >= 85.5", &vars));
    }

    #[test]
    fn condition_combined_logical_and_comparison() {
        let vars = HashMap::from([
            ("status".to_string(), json!("open")),
            ("priority".to_string(), json!(3)),
            ("assigned".to_string(), json!(true)),
        ]);

        // AND with comparison
        assert!(default_evaluate_condition(
            "status == open && priority > 2",
            &vars
        ));
        assert!(!default_evaluate_condition(
            "status == open && priority > 5",
            &vars
        ));

        // OR with comparison
        assert!(default_evaluate_condition(
            "priority > 10 || assigned",
            &vars
        ));

        // Mixed
        assert!(default_evaluate_condition(
            "status == open && (priority > 2 || !assigned)",
            &vars
        ));
        assert!(!default_evaluate_condition(
            "status == closed && (priority > 2 || assigned)",
            &vars
        ));
    }

    #[test]
    fn condition_parentheses() {
        let vars = HashMap::from([
            ("a".to_string(), json!(true)),
            ("b".to_string(), json!(false)),
            ("c".to_string(), json!(true)),
        ]);

        // Without parens: a && b || c => (a && b) || c => false || true => true
        assert!(default_evaluate_condition("a && b || c", &vars));
        // With parens: a && (b || c) => true && (false || true) => true && true => true
        assert!(default_evaluate_condition("a && (b || c)", &vars));
        // a && (b || !c) => true && (false || false) => false
        assert!(!default_evaluate_condition("a && (b || !c)", &vars));
    }

    #[test]
    fn deep_dotted_path_resolution() {
        let vars = HashMap::from([
            ("response".to_string(), json!({
                "data": {
                    "items": [1, 2, 3],
                    "meta": {
                        "count": 3,
                        "status": "ok"
                    }
                },
                "status": 200
            })),
        ]);

        // Two levels deep
        assert!(default_evaluate_condition("response.data.meta.status == ok", &vars));
        assert!(default_evaluate_condition("response.data.meta.count == 3", &vars));
        assert!(default_evaluate_condition("response.status == 200", &vars));
        assert!(!default_evaluate_condition("response.data.meta.status == error", &vars));
    }

    #[test]
    fn contains_operator_array() {
        let vars = HashMap::from([
            ("tags".to_string(), json!(["rust", "wasm", "praxis"])),
            ("numbers".to_string(), json!([1, 2, 3, 5, 8])),
            ("needle".to_string(), json!("rust")),
        ]);

        // Array contains string literal
        assert!(default_evaluate_condition("tags contains \"rust\"", &vars));
        assert!(default_evaluate_condition("tags contains \"praxis\"", &vars));
        assert!(!default_evaluate_condition("tags contains \"python\"", &vars));

        // Array contains via variable reference
        assert!(default_evaluate_condition("tags contains needle", &vars));

        // Array contains number
        assert!(default_evaluate_condition("numbers contains 3", &vars));
        assert!(!default_evaluate_condition("numbers contains 4", &vars));
    }

    #[test]
    fn contains_operator_string() {
        let vars = HashMap::from([
            ("message".to_string(), json!("hello world, welcome!")),
            ("sub".to_string(), json!("world")),
        ]);

        assert!(default_evaluate_condition("message contains \"world\"", &vars));
        assert!(default_evaluate_condition("message contains \"hello\"", &vars));
        assert!(!default_evaluate_condition("message contains \"goodbye\"", &vars));

        // Contains with variable as substring
        assert!(default_evaluate_condition("message contains sub", &vars));
    }

    #[test]
    fn in_operator() {
        let vars = HashMap::from([
            ("roles".to_string(), json!(["admin", "editor", "viewer"])),
            ("role".to_string(), json!("editor")),
        ]);

        // "value" in collection
        assert!(default_evaluate_condition("\"admin\" in roles", &vars));
        assert!(!default_evaluate_condition("\"superuser\" in roles", &vars));

        // Variable in collection
        assert!(default_evaluate_condition("role in roles", &vars));
    }

    #[test]
    fn contains_with_logical_operators() {
        let vars = HashMap::from([
            ("tags".to_string(), json!(["rust", "async"])),
            ("status".to_string(), json!("active")),
        ]);

        assert!(default_evaluate_condition(
            "tags contains \"rust\" && status == active",
            &vars
        ));
        assert!(!default_evaluate_condition(
            "tags contains \"python\" && status == active",
            &vars
        ));
        assert!(default_evaluate_condition(
            "tags contains \"python\" || status == active",
            &vars
        ));
    }

    #[test]
    fn dollar_prefix_variable_resolution() {
        let vars = HashMap::from([
            ("status".to_string(), json!("ok")),
            ("count".to_string(), json!(42)),
            ("result".to_string(), json!({"code": 200})),
        ]);

        // $variable resolves to same as bare variable
        assert!(default_evaluate_condition("$status == ok", &vars));
        assert!(!default_evaluate_condition("$status == error", &vars));
        assert!(default_evaluate_condition("$count == 42", &vars));
        assert!(default_evaluate_condition("$count > 10", &vars));
        // Truthy check with $prefix
        assert!(default_evaluate_condition("$status", &vars));
        assert!(!default_evaluate_condition("$nonexistent", &vars));
        // Dotted access with $prefix
        assert!(default_evaluate_condition("$result.code == 200", &vars));
    }

    #[test]
    fn matches_operator_regex() {
        let vars = HashMap::from([
            ("email".to_string(), json!("user@example.com")),
            ("version".to_string(), json!("2.14.3")),
            ("name".to_string(), json!("hello-world")),
        ]);

        // Basic regex match
        assert!(default_evaluate_condition(r#"email matches ".*@example\.com""#, &vars));
        assert!(!default_evaluate_condition(r#"email matches ".*@other\.com""#, &vars));
        // Version pattern
        assert!(default_evaluate_condition(r#"version matches "^\d+\.\d+\.\d+$""#, &vars));
        // Character class
        assert!(default_evaluate_condition(r#"name matches "^[a-z-]+$""#, &vars));
        // Invalid regex returns false (no panic)
        assert!(!default_evaluate_condition(r#"name matches "[invalid""#, &vars));
    }

    #[test]
    fn starts_with_operator() {
        let vars = HashMap::from([
            ("path".to_string(), json!("/api/v2/users")),
            ("status".to_string(), json!("error_timeout")),
        ]);

        assert!(default_evaluate_condition(r#"path starts_with "/api/v2""#, &vars));
        assert!(!default_evaluate_condition(r#"path starts_with "/admin""#, &vars));
        assert!(default_evaluate_condition(r#"status starts_with "error""#, &vars));
    }

    #[test]
    fn ends_with_operator() {
        let vars = HashMap::from([
            ("filename".to_string(), json!("report.pdf")),
            ("url".to_string(), json!("https://example.com/api")),
        ]);

        assert!(default_evaluate_condition(r#"filename ends_with ".pdf""#, &vars));
        assert!(!default_evaluate_condition(r#"filename ends_with ".txt""#, &vars));
        assert!(default_evaluate_condition(r#"url ends_with "/api""#, &vars));
    }

    #[test]
    fn combined_new_operators_with_logic() {
        let vars = HashMap::from([
            ("path".to_string(), json!("/api/v2/users")),
            ("method".to_string(), json!("POST")),
            ("tags".to_string(), json!(["auth", "rate-limited"])),
        ]);

        // Combined with && and ||
        assert!(default_evaluate_condition(
            r#"path starts_with "/api" && method == POST"#,
            &vars
        ));
        assert!(default_evaluate_condition(
            r#"path ends_with "/users" || method == GET"#,
            &vars
        ));
        // $prefix with new operators
        assert!(default_evaluate_condition(
            r#"$path starts_with "/api""#,
            &vars
        ));
        assert!(default_evaluate_condition(
            r#"$tags contains "auth" && $method == POST"#,
            &vars
        ));
    }

    #[test]
    fn bracket_array_indexing() {
        let vars = HashMap::from([
            ("items".to_string(), json!(["alpha", "beta", "gamma"])),
            (
                "users".to_string(),
                json!([{"name": "alice", "age": 30}, {"name": "bob", "age": 25}]),
            ),
        ]);

        // Simple array index
        assert!(default_evaluate_condition("items[0] == alpha", &vars));
        assert!(default_evaluate_condition("items[1] == beta", &vars));
        assert!(default_evaluate_condition("items[2] == gamma", &vars));
        assert!(!default_evaluate_condition("items[0] == beta", &vars));

        // Array index with nested object access
        assert!(default_evaluate_condition("users[0].name == alice", &vars));
        assert!(default_evaluate_condition("users[1].name == bob", &vars));
        assert!(default_evaluate_condition("users[0].age == 30", &vars));
        assert!(default_evaluate_condition("users[1].age == 25", &vars));
        assert!(!default_evaluate_condition("users[0].name == bob", &vars));
    }

    #[test]
    fn bracket_object_key_access() {
        let vars = HashMap::from([(
            "headers".to_string(),
            json!({"content-type": "application/json", "x-request-id": "abc123"}),
        )]);

        // Bracket key access (for keys with hyphens that can't use dot notation)
        assert!(default_evaluate_condition(
            "headers[\"content-type\"] == application/json",
            &vars
        ));
        assert!(default_evaluate_condition(
            "headers[\"x-request-id\"] == abc123",
            &vars
        ));
    }

    #[test]
    fn mixed_dot_and_bracket_paths() {
        let vars = HashMap::from([(
            "response".to_string(),
            json!({
                "data": {
                    "items": [
                        {"id": 1, "status": "active"},
                        {"id": 2, "status": "inactive"}
                    ],
                    "meta": {"total": 2}
                }
            }),
        )]);

        // Deep mixed path: dot, bracket index, dot
        assert!(default_evaluate_condition(
            "response.data.items[0].status == active",
            &vars
        ));
        assert!(default_evaluate_condition(
            "response.data.items[1].status == inactive",
            &vars
        ));
        assert!(default_evaluate_condition(
            "response.data.items[0].id == 1",
            &vars
        ));
        assert!(default_evaluate_condition(
            "response.data.meta.total == 2",
            &vars
        ));
    }

    #[test]
    fn bracket_index_out_of_bounds() {
        let vars = HashMap::from([("items".to_string(), json!(["a", "b"]))]);

        // Out-of-bounds returns false (not found)
        assert!(!default_evaluate_condition("items[5] == a", &vars));
        // Truthy check on out-of-bounds
        assert!(!default_evaluate_condition("items[99]", &vars));
        // In-bounds truthy check
        assert!(default_evaluate_condition("items[0]", &vars));
    }

    #[test]
    fn bracket_indexing_with_operators() {
        let vars = HashMap::from([(
            "scores".to_string(),
            json!([85, 92, 78, 95]),
        )]);

        // Comparison operators with bracket indexing
        assert!(default_evaluate_condition("scores[0] > 80", &vars));
        assert!(default_evaluate_condition("scores[1] >= 92", &vars));
        assert!(default_evaluate_condition("scores[2] < 80", &vars));
        assert!(default_evaluate_condition("scores[3] <= 95", &vars));
        assert!(!default_evaluate_condition("scores[0] > 90", &vars));
    }

    #[test]
    fn parse_path_segments_unit() {
        use super::PathSegment::*;

        assert_eq!(
            super::parse_path_segments("foo.bar"),
            vec![Key("foo".into()), Key("bar".into())]
        );
        assert_eq!(
            super::parse_path_segments("items[0]"),
            vec![Key("items".into()), Index(0)]
        );
        assert_eq!(
            super::parse_path_segments("items[0].name"),
            vec![Key("items".into()), Index(0), Key("name".into())]
        );
        assert_eq!(
            super::parse_path_segments("data[\"key\"]"),
            vec![Key("data".into()), Key("key".into())]
        );
        assert_eq!(
            super::parse_path_segments("a.b[2].c.d[0]"),
            vec![
                Key("a".into()),
                Key("b".into()),
                Index(2),
                Key("c".into()),
                Key("d".into()),
                Index(0)
            ]
        );
    }

    #[test]
    fn execute_parallel_branches() {
        let handler = MockHandler::new()
            .with_result("fetch_a", json!("result_a"))
            .with_result("fetch_b", json!("result_b"))
            .with_result("fetch_c", json!("result_c"));

        let procedure = json!({
            "type": "procedure",
            "name": "parallel_test",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        {
                            "name": "alpha",
                            "steps": [
                                { "kind": "call", "name": "fetch_a", "params": {} }
                            ]
                        },
                        {
                            "name": "beta",
                            "steps": [
                                { "kind": "call", "name": "fetch_b", "params": {} }
                            ]
                        },
                        {
                            "name": "gamma",
                            "steps": [
                                { "kind": "call", "name": "fetch_c", "params": {} }
                            ]
                        }
                    ],
                    "output_var": "results"
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        let results = result.variables.get("results").unwrap();
        assert_eq!(results["alpha"], json!("result_a"));
        assert_eq!(results["beta"], json!("result_b"));
        assert_eq!(results["gamma"], json!("result_c"));
    }

    #[test]
    fn execute_parallel_branch_isolation() {
        // Branches should not see each other's variable mutations
        let handler = MockHandler::new()
            .with_result("set_val", json!("modified"))
            .with_result("read_val", json!("original"));

        let procedure = json!({
            "type": "procedure",
            "name": "isolation_test",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        {
                            "name": "writer",
                            "steps": [
                                { "kind": "call", "name": "set_val", "params": {}, "output_var": "shared" }
                            ]
                        },
                        {
                            "name": "reader",
                            "steps": [
                                { "kind": "call", "name": "read_val", "params": { "ref": "$shared" } }
                            ]
                        }
                    ],
                    "output_var": "par_results"
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        // The "shared" var set by writer should NOT be in the parent scope
        assert!(result.variables.get("shared").is_none());
        // But output_var should have the map
        assert!(result.variables.get("par_results").is_some());
    }

    #[test]
    fn execute_parallel_error_propagates() {
        // If a branch fails, the whole parallel step fails
        let handler = MockHandler::new().with_result("ok_action", json!("fine"));

        let procedure = json!({
            "type": "procedure",
            "name": "error_test",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        {
                            "name": "good",
                            "steps": [
                                { "kind": "call", "name": "ok_action", "params": {} }
                            ]
                        },
                        {
                            "name": "bad",
                            "steps": [
                                { "kind": "call", "name": "nonexistent", "params": {} }
                            ]
                        }
                    ]
                }
            ]
        });

        let result = execute(&procedure, &handler);
        assert!(result.is_err());
    }

    #[test]
    fn try_retry_succeeds_on_second_attempt() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct FlakeHandler {
            call_count: AtomicUsize,
        }

        impl ActionHandler for FlakeHandler {
            fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                match name {
                    "flaky" => {
                        if count == 0 {
                            Err(ExecutionError::ActionFailed {
                                action: "flaky".into(),
                                message: "transient error".into(),
                            })
                        } else {
                            Ok(json!("success_on_retry"))
                        }
                    }
                    _ => Err(ExecutionError::UnknownAction(name.into())),
                }
            }
        }

        let handler = FlakeHandler {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "retry_test",
            "steps": [
                {
                    "kind": "try",
                    "retry": 2,
                    "steps": [
                        { "kind": "call", "name": "flaky", "params": {}, "output_var": "result" }
                    ],
                    "catch": [
                        { "kind": "emit", "event": "should_not_reach" }
                    ]
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        // Succeeded on retry — catch was not reached
        assert_eq!(result.variables.get("result"), Some(&json!("success_on_retry")));
        // retry_count cleaned up
        assert!(result.variables.get("retry_count").is_none());
        // error cleared
        assert!(result.variables.get("error").is_none());
        // Was called exactly 2 times (first fail + second success)
        assert_eq!(handler.call_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn try_retry_exhausted_runs_catch() {
        let handler = MockHandler::new().with_result("fallback", json!("caught_after_retries"));

        let procedure = json!({
            "type": "procedure",
            "name": "retry_exhausted",
            "steps": [
                {
                    "kind": "try",
                    "retry": 3,
                    "steps": [
                        { "kind": "call", "name": "always_fails", "params": {} }
                    ],
                    "catch": [
                        { "kind": "call", "name": "fallback", "params": {} }
                    ]
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        assert_eq!(result.step_results[0].output, Some(json!("caught_after_retries")));
        // error variable was set before catch ran
        assert!(result.variables.get("error").is_some());
    }

    #[test]
    fn try_retry_zero_is_default_no_retry() {
        let handler = MockHandler::new().with_result("fallback", json!("immediate_catch"));

        let procedure = json!({
            "type": "procedure",
            "name": "no_retry",
            "steps": [
                {
                    "kind": "try",
                    "steps": [
                        { "kind": "call", "name": "always_fails", "params": {} }
                    ],
                    "catch": [
                        { "kind": "call", "name": "fallback", "params": {} }
                    ]
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        // Without retry field, catch runs immediately
        assert_eq!(result.step_results[0].output, Some(json!("immediate_catch")));
    }

    #[test]
    fn try_retry_with_fixed_delay() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Instant;

        struct FlakeHandler {
            call_count: AtomicUsize,
        }

        impl ActionHandler for FlakeHandler {
            fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(ExecutionError::ActionFailed {
                        action: "flaky".into(),
                        message: format!("fail #{}", count),
                    })
                } else {
                    Ok(json!("ok"))
                }
            }
        }

        let handler = FlakeHandler {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "fixed_delay_test",
            "steps": [
                {
                    "kind": "try",
                    "retry": 3,
                    "retry_delay_ms": 10,
                    "steps": [
                        { "kind": "call", "name": "flaky", "params": {} }
                    ]
                }
            ]
        });

        let start = Instant::now();
        let result = execute(&procedure, &handler).unwrap();
        let elapsed = start.elapsed();

        assert!(result.success);
        assert_eq!(result.step_results[0].output, Some(json!("ok")));
        // Should have at least 20ms of delay (2 retries × 10ms each)
        assert!(elapsed.as_millis() >= 18, "Expected >= 18ms, got {}ms", elapsed.as_millis());
    }

    #[test]
    fn try_retry_with_exponential_backoff() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Instant;

        struct FlakeHandler {
            call_count: AtomicUsize,
        }

        impl ActionHandler for FlakeHandler {
            fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count < 3 {
                    Err(ExecutionError::ActionFailed {
                        action: "flaky".into(),
                        message: format!("fail #{}", count),
                    })
                } else {
                    Ok(json!("recovered"))
                }
            }
        }

        let handler = FlakeHandler {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "exp_backoff_test",
            "steps": [
                {
                    "kind": "try",
                    "retry": 4,
                    "retry_delay_ms": 10,
                    "retry_backoff": "exponential",
                    "steps": [
                        { "kind": "call", "name": "flaky", "params": {} }
                    ]
                }
            ]
        });

        let start = Instant::now();
        let result = execute(&procedure, &handler).unwrap();
        let elapsed = start.elapsed();

        assert!(result.success);
        assert_eq!(result.step_results[0].output, Some(json!("recovered")));
        // Exponential: 10 + 20 + 40 = 70ms for 3 retries
        assert!(elapsed.as_millis() >= 60, "Expected >= 60ms exponential delay, got {}ms", elapsed.as_millis());
    }

    #[test]
    fn try_retry_exponential_backoff_with_max_delay() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Instant;

        struct FlakeHandler {
            call_count: AtomicUsize,
        }

        impl ActionHandler for FlakeHandler {
            fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count < 3 {
                    Err(ExecutionError::ActionFailed {
                        action: "flaky".into(),
                        message: format!("fail #{}", count),
                    })
                } else {
                    Ok(json!("capped"))
                }
            }
        }

        let handler = FlakeHandler {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "max_delay_test",
            "steps": [
                {
                    "kind": "try",
                    "retry": 4,
                    "retry_delay_ms": 10,
                    "retry_backoff": "exponential",
                    "retry_max_delay_ms": 25,
                    "steps": [
                        { "kind": "call", "name": "flaky", "params": {} }
                    ]
                }
            ]
        });

        let start = Instant::now();
        let result = execute(&procedure, &handler).unwrap();
        let elapsed = start.elapsed();

        assert!(result.success);
        assert_eq!(result.step_results[0].output, Some(json!("capped")));
        // Capped: 10 + 20 + 25 = 55ms (3rd would be 40 but capped at 25)
        assert!(elapsed.as_millis() >= 45, "Expected >= 45ms capped delay, got {}ms", elapsed.as_millis());
        // Should NOT be as long as uncapped (10 + 20 + 40 = 70ms)
        assert!(elapsed.as_millis() < 100, "Delay too long ({}ms) — max cap not working?", elapsed.as_millis());
    }

    #[test]
    fn try_retry_with_jitter() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct FlakeHandler {
            call_count: AtomicUsize,
        }

        impl ActionHandler for FlakeHandler {
            fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(ExecutionError::ActionFailed {
                        action: "flaky".into(),
                        message: format!("fail #{}", count),
                    })
                } else {
                    Ok(json!("jittered_success"))
                }
            }
        }

        let handler = FlakeHandler {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "jitter_test",
            "steps": [
                {
                    "kind": "try",
                    "retry": 3,
                    "retry_delay_ms": 50,
                    "retry_backoff": "exponential",
                    "retry_jitter": true,
                    "steps": [
                        { "kind": "call", "name": "flaky", "params": {} }
                    ]
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        assert_eq!(result.step_results[0].output, Some(json!("jittered_success")));
    }

    #[test]
    fn try_retry_jitter_reduces_total_delay() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Instant;

        struct AlwaysFail {
            call_count: AtomicUsize,
        }

        impl ActionHandler for AlwaysFail {
            fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                Err(ExecutionError::ActionFailed {
                    action: "always_fail".into(),
                    message: "nope".into(),
                })
            }
        }

        let handler = AlwaysFail {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "jitter_timing_test",
            "steps": [
                {
                    "kind": "try",
                    "retry": 3,
                    "retry_delay_ms": 30,
                    "retry_backoff": "exponential",
                    "retry_jitter": true,
                    "retry_max_delay_ms": 200,
                    "steps": [
                        { "kind": "call", "name": "always_fail", "params": {} }
                    ],
                    "catch": [
                        { "kind": "emit", "event": "caught" }
                    ]
                }
            ]
        });

        let start = Instant::now();
        let result = execute(&procedure, &handler).unwrap();
        let elapsed = start.elapsed();

        assert!(result.success);
        // Without jitter: 30 + 60 + 120 = 210ms. With jitter: [0,30] + [0,60] + [0,120] <= 210ms
        assert!(elapsed.as_millis() <= 250, "Jitter delay too long: {}ms", elapsed.as_millis());
    }

    #[test]
    fn parallel_branch_retry_sync() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct FlakySyncHandler {
            fail_count: AtomicUsize,
            fail_until: usize,
        }

        impl ActionHandler for FlakySyncHandler {
            fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                if name == "flaky" {
                    let count = self.fail_count.fetch_add(1, Ordering::SeqCst);
                    if count < self.fail_until {
                        return Err(ExecutionError::ActionFailed {
                            action: "flaky".into(),
                            message: format!("fail #{}", count),
                        });
                    }
                }
                Ok(json!(format!("{}_done", name)))
            }
        }

        let handler = FlakySyncHandler {
            fail_count: AtomicUsize::new(0),
            fail_until: 2,
        };

        let procedure = json!({
            "type": "procedure",
            "name": "sync_branch_retry",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        {
                            "name": "stable",
                            "steps": [
                                { "kind": "call", "name": "ok", "params": {} }
                            ]
                        },
                        {
                            "name": "retried",
                            "retry": 3,
                            "retry_delay_ms": 1,
                            "steps": [
                                { "kind": "call", "name": "flaky", "params": {} }
                            ]
                        }
                    ],
                    "output_var": "out"
                }
            ]
        });

        let result = execute(&procedure, &handler).unwrap();
        assert!(result.success);
        let out = result.variables.get("out").unwrap();
        assert_eq!(out["stable"], json!("ok_done"));
        assert_eq!(out["retried"], json!("flaky_done"));
        assert_eq!(handler.fail_count.load(Ordering::SeqCst), 3);
    }

    // ── String Interpolation Tests ────────────────────────────────────────────

    #[test]
    fn test_interpolate_simple_variable() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("world"));
        let result = interpolate_string("Hello, ${name}!", &vars);
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_interpolate_multiple_variables() {
        let mut vars = HashMap::new();
        vars.insert("first".to_string(), json!("Alice"));
        vars.insert("last".to_string(), json!("Smith"));
        let result = interpolate_string("${first} ${last}", &vars);
        assert_eq!(result, "Alice Smith");
    }

    #[test]
    fn test_interpolate_numeric_variable() {
        let mut vars = HashMap::new();
        vars.insert("count".to_string(), json!(42));
        let result = interpolate_string("There are ${count} items", &vars);
        assert_eq!(result, "There are 42 items");
    }

    #[test]
    fn test_interpolate_dotted_path() {
        let mut vars = HashMap::new();
        vars.insert("result".to_string(), json!({"status": "ok", "code": 200}));
        let result = interpolate_string("Status: ${result.status} (${result.code})", &vars);
        assert_eq!(result, "Status: ok (200)");
    }

    #[test]
    fn test_interpolate_arithmetic() {
        let mut vars = HashMap::new();
        vars.insert("count".to_string(), json!(5));
        let result = interpolate_string("Next: ${count + 1}, Prev: ${count - 1}", &vars);
        assert_eq!(result, "Next: 6, Prev: 4");
    }

    #[test]
    fn test_interpolate_unresolved_kept_as_is() {
        let vars = HashMap::new();
        let result = interpolate_string("Hello, ${unknown}!", &vars);
        assert_eq!(result, "Hello, ${unknown}!");
    }

    #[test]
    fn test_interpolate_no_placeholders() {
        let vars = HashMap::new();
        let result = interpolate_string("plain text", &vars);
        assert_eq!(result, "plain text");
    }

    #[test]
    fn test_resolve_vars_interpolation_in_params() {
        let mut vars = HashMap::new();
        vars.insert("user".to_string(), json!("alice"));
        vars.insert("host".to_string(), json!("example.com"));

        let params = json!({"url": "https://${host}/api/users/${user}"});
        let resolved = resolve_vars(&params, &vars);
        assert_eq!(resolved["url"], "https://example.com/api/users/alice");
    }

    #[test]
    fn test_resolve_vars_whole_string_still_works() {
        let mut vars = HashMap::new();
        vars.insert("data".to_string(), json!({"nested": true}));

        // Whole-string $var should return the actual value (object), not a string
        let params = json!({"payload": "$data"});
        let resolved = resolve_vars(&params, &vars);
        assert_eq!(resolved["payload"], json!({"nested": true}));
    }

    // ── Arithmetic in When-Guards Tests ───────────────────────────────────────

    #[test]
    fn test_arithmetic_in_comparison_add() {
        let mut vars = HashMap::new();
        vars.insert("count".to_string(), json!(5));
        // count + 1 > 5 → 6 > 5 → true
        assert!(default_evaluate_condition("count + 1 > 5", &vars));
        // count + 1 > 6 → 6 > 6 → false
        assert!(!default_evaluate_condition("count + 1 > 6", &vars));
    }

    #[test]
    fn test_arithmetic_in_comparison_subtract() {
        let mut vars = HashMap::new();
        vars.insert("total".to_string(), json!(10));
        // total - 3 >= 7 → 7 >= 7 → true
        assert!(default_evaluate_condition("total - 3 >= 7", &vars));
        // total - 3 >= 8 → 7 >= 8 → false
        assert!(!default_evaluate_condition("total - 3 >= 8", &vars));
    }

    #[test]
    fn test_arithmetic_equality() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!(4));
        // x + 1 == 5 → true
        assert!(default_evaluate_condition("x + 1 == 5", &vars));
        // x + 1 == 6 → false
        assert!(!default_evaluate_condition("x + 1 == 6", &vars));
    }

    #[test]
    fn test_arithmetic_both_sides() {
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!(3));
        vars.insert("b".to_string(), json!(5));
        // a + 2 >= b → 5 >= 5 → true
        assert!(default_evaluate_condition("a + 2 >= b", &vars));
        // a > b - 3 → 3 > 2 → true
        assert!(default_evaluate_condition("a > b - 3", &vars));
    }

    #[test]
    fn test_arithmetic_multiply() {
        let mut vars = HashMap::new();
        vars.insert("rate".to_string(), json!(10));
        // rate * 2 == 20 → true
        assert!(default_evaluate_condition("rate * 2 == 20", &vars));
    }

    #[test]
    fn test_arithmetic_divide() {
        let mut vars = HashMap::new();
        vars.insert("total".to_string(), json!(100));
        // total / 4 == 25 → true
        assert!(default_evaluate_condition("total / 4 == 25", &vars));
    }

    #[test]
    fn test_arithmetic_modulo_comparison() {
        let mut vars = HashMap::new();
        vars.insert("count".to_string(), json!(10));
        // 10 % 3 == 1
        assert!(default_evaluate_condition("count % 3 == 1", &vars));
        // 10 % 5 == 0
        assert!(default_evaluate_condition("count % 5 == 0", &vars));
        // 10 % 4 != 0
        assert!(default_evaluate_condition("count % 4 != 0", &vars));
    }

    #[test]
    fn test_arithmetic_modulo_division_by_zero() {
        let mut vars = HashMap::new();
        vars.insert("count".to_string(), json!(10));
        // Division by zero should not crash, condition should be false
        assert!(!default_evaluate_condition("count % 0 == 0", &vars));
        assert!(!default_evaluate_condition("count / 0 == 0", &vars));
    }

    #[test]
    fn test_interpolate_modulo() {
        let mut vars = HashMap::new();
        vars.insert("count".to_string(), json!(17));
        let result = interpolate_string("Remainder: ${count % 5}", &vars);
        assert_eq!(result, "Remainder: 2");
    }

    #[test]
    fn test_interpolate_multiply() {
        let mut vars = HashMap::new();
        vars.insert("price".to_string(), json!(5));
        let result = interpolate_string("Total: ${price * 3}", &vars);
        assert_eq!(result, "Total: 15");
    }

    #[test]
    fn test_interpolate_divide() {
        let mut vars = HashMap::new();
        vars.insert("total".to_string(), json!(100));
        let result = interpolate_string("Half: ${total / 2}", &vars);
        assert_eq!(result, "Half: 50");
    }

    #[test]
    fn test_interpolate_ternary_true() {
        let mut vars = HashMap::new();
        vars.insert("enabled".to_string(), json!(true));
        let result = interpolate_string("Status: ${enabled ? 'on' : 'off'}", &vars);
        assert_eq!(result, "Status: on");
    }

    #[test]
    fn test_interpolate_ternary_false() {
        let mut vars = HashMap::new();
        vars.insert("enabled".to_string(), json!(false));
        let result = interpolate_string("Status: ${enabled ? 'on' : 'off'}", &vars);
        assert_eq!(result, "Status: off");
    }

    #[test]
    fn test_interpolate_ternary_with_comparison() {
        let mut vars = HashMap::new();
        vars.insert("count".to_string(), json!(5));
        let result = interpolate_string("${count > 3 ? 'many' : 'few'}", &vars);
        assert_eq!(result, "many");
    }

    #[test]
    fn test_interpolate_ternary_with_variable_branch() {
        let mut vars = HashMap::new();
        vars.insert("active".to_string(), json!(true));
        vars.insert("name".to_string(), json!("Alice"));
        let result = interpolate_string("User: ${active ? name : 'unknown'}", &vars);
        assert_eq!(result, "User: Alice");
    }

    #[test]
    fn test_interpolate_ternary_numeric_branch() {
        let mut vars = HashMap::new();
        vars.insert("premium".to_string(), json!(false));
        let result = interpolate_string("Limit: ${premium ? 100 : 10}", &vars);
        assert_eq!(result, "Limit: 10");
    }

    #[test]
    fn test_interpolate_nested_ternary_inner_true() {
        // a > 0 ? (b > 1 ? "deep" : "shallow") : "negative"
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!(5));
        vars.insert("b".to_string(), json!(10));
        let result = interpolate_string("${a > 0 ? b > 1 ? \"deep\" : \"shallow\" : \"negative\"}", &vars);
        assert_eq!(result, "deep");
    }

    #[test]
    fn test_interpolate_nested_ternary_inner_false() {
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!(5));
        vars.insert("b".to_string(), json!(0));
        let result = interpolate_string("${a > 0 ? b > 1 ? \"deep\" : \"shallow\" : \"negative\"}", &vars);
        assert_eq!(result, "shallow");
    }

    #[test]
    fn test_interpolate_nested_ternary_outer_false() {
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!(-1));
        vars.insert("b".to_string(), json!(10));
        let result = interpolate_string("${a > 0 ? b > 1 ? \"deep\" : \"shallow\" : \"negative\"}", &vars);
        assert_eq!(result, "negative");
    }

    #[test]
    fn test_interpolate_nested_ternary_in_false_branch() {
        // Nesting in the false branch: a > 0 ? "positive" : b > 5 ? "big-neg" : "small-neg"
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!(-1));
        vars.insert("b".to_string(), json!(10));
        let result = interpolate_string("${a > 0 ? \"positive\" : b > 5 ? \"big-neg\" : \"small-neg\"}", &vars);
        assert_eq!(result, "big-neg");
    }

    #[test]
    fn test_interpolate_nested_ternary_false_branch_inner_false() {
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!(-1));
        vars.insert("b".to_string(), json!(2));
        let result = interpolate_string("${a > 0 ? \"positive\" : b > 5 ? \"big-neg\" : \"small-neg\"}", &vars);
        assert_eq!(result, "small-neg");
    }

    #[test]
    fn test_ternary_with_quoted_colon_in_string() {
        // Colons inside quoted strings should not confuse the parser
        let mut vars = HashMap::new();
        vars.insert("ok".to_string(), json!(true));
        let result = interpolate_string("${ok ? \"time: now\" : \"time: later\"}", &vars);
        assert_eq!(result, "time: now");
    }

    #[test]
    fn test_find_matching_colon_simple() {
        assert_eq!(find_matching_colon(" true : false"), Some(6));
    }

    #[test]
    fn test_find_matching_colon_nested() {
        // "b > 1 ? deep : shallow : negative" — first : is inside nested ternary
        let branches = " b > 1 ? \"deep\" : \"shallow\" : \"negative\"";
        let colon_idx = find_matching_colon(branches).unwrap();
        let false_part = branches[colon_idx + 1..].trim();
        assert_eq!(false_part, "\"negative\"");
    }

    // === Pipe operator tests ===

    #[test]
    fn test_pipe_uppercase() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("hello"));
        let result = interpolate_string("${name | uppercase}", &vars);
        assert_eq!(result, "HELLO");
    }

    #[test]
    fn test_pipe_lowercase() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("WORLD"));
        let result = interpolate_string("${name | lowercase}", &vars);
        assert_eq!(result, "world");
    }

    #[test]
    fn test_pipe_trim() {
        let mut vars = HashMap::new();
        vars.insert("msg".to_string(), json!("  hello  "));
        let result = interpolate_string("${msg | trim}", &vars);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_pipe_capitalize() {
        let mut vars = HashMap::new();
        vars.insert("word".to_string(), json!("hello world"));
        let result = interpolate_string("${word | capitalize}", &vars);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_pipe_reverse() {
        let mut vars = HashMap::new();
        vars.insert("word".to_string(), json!("abc"));
        let result = interpolate_string("${word | reverse}", &vars);
        assert_eq!(result, "cba");
    }

    #[test]
    fn test_pipe_length() {
        let mut vars = HashMap::new();
        vars.insert("word".to_string(), json!("hello"));
        let result = interpolate_string("${word | length}", &vars);
        assert_eq!(result, "5");
    }

    #[test]
    fn test_pipe_chained_filters() {
        let mut vars = HashMap::new();
        vars.insert("msg".to_string(), json!("  Hello World  "));
        let result = interpolate_string("${msg | trim | lowercase}", &vars);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_pipe_default_empty() {
        let mut vars = HashMap::new();
        vars.insert("val".to_string(), json!(""));
        let result = interpolate_string("${val | default('fallback')}", &vars);
        assert_eq!(result, "fallback");
    }

    #[test]
    fn test_pipe_default_nonempty() {
        let mut vars = HashMap::new();
        vars.insert("val".to_string(), json!("exists"));
        let result = interpolate_string("${val | default('fallback')}", &vars);
        assert_eq!(result, "exists");
    }

    #[test]
    fn test_pipe_truncate() {
        let mut vars = HashMap::new();
        vars.insert("text".to_string(), json!("hello world"));
        let result = interpolate_string("${text | truncate(5)}", &vars);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_pipe_replace() {
        let mut vars = HashMap::new();
        vars.insert("text".to_string(), json!("foo bar foo"));
        let result = interpolate_string("${text | replace('foo', 'baz')}", &vars);
        assert_eq!(result, "baz bar baz");
    }

    #[test]
    fn test_pipe_snake_case() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("HelloWorld"));
        let result = interpolate_string("${name | snake_case}", &vars);
        assert_eq!(result, "hello_world");
    }

    #[test]
    fn test_pipe_camel_case() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("hello_world"));
        let result = interpolate_string("${name | camel_case}", &vars);
        assert_eq!(result, "helloWorld");
    }

    #[test]
    fn test_pipe_kebab_case() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("HelloWorld"));
        let result = interpolate_string("${name | kebab_case}", &vars);
        assert_eq!(result, "hello-world");
    }

    #[test]
    fn test_pipe_split() {
        let mut vars = HashMap::new();
        vars.insert("csv".to_string(), json!("a,b,c"));
        let result = interpolate_string("${csv | split(',')}", &vars);
        assert_eq!(result, r#"["a", "b", "c"]"#);
    }

    #[test]
    fn test_pipe_pad_left() {
        let mut vars = HashMap::new();
        vars.insert("num".to_string(), json!("42"));
        let result = interpolate_string("${num | pad_left(5, '0')}", &vars);
        assert_eq!(result, "00042");
    }

    #[test]
    fn test_pipe_repeat() {
        let mut vars = HashMap::new();
        vars.insert("s".to_string(), json!("ab"));
        let result = interpolate_string("${s | repeat(3)}", &vars);
        assert_eq!(result, "ababab");
    }

    #[test]
    fn test_pipe_slice() {
        let mut vars = HashMap::new();
        vars.insert("text".to_string(), json!("hello world"));
        let result = interpolate_string("${text | slice(0, 5)}", &vars);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_pipe_does_not_break_ternary() {
        // Ternary expressions with || should still work
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!(false));
        vars.insert("b".to_string(), json!(true));
        let result = interpolate_string("${a ? 'yes' : 'no'}", &vars);
        assert_eq!(result, "no");
    }

    #[test]
    fn test_pipe_with_surrounding_text() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("alice"));
        let result = interpolate_string("Hello, ${name | uppercase}!", &vars);
        assert_eq!(result, "Hello, ALICE!");
    }

    // ── Pipe filters in when-guard conditions ───────────────────────────────────

    #[test]
    fn test_condition_pipe_eq_uppercase() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("hello"));
        assert!(default_evaluate_condition("name | uppercase == HELLO", &vars));
        assert!(!default_evaluate_condition("name | uppercase == hello", &vars));
    }

    #[test]
    fn test_condition_pipe_eq_lowercase() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("ACTIVE"));
        assert!(default_evaluate_condition("status | lowercase == active", &vars));
    }

    #[test]
    fn test_condition_pipe_eq_trim() {
        let mut vars = HashMap::new();
        vars.insert("input".to_string(), json!("  hello  "));
        assert!(default_evaluate_condition("input | trim == hello", &vars));
    }

    #[test]
    fn test_condition_pipe_ne() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("hello"));
        assert!(default_evaluate_condition("name | uppercase != hello", &vars));
        assert!(!default_evaluate_condition("name | uppercase != HELLO", &vars));
    }

    #[test]
    fn test_condition_pipe_chained() {
        let mut vars = HashMap::new();
        vars.insert("msg".to_string(), json!("  Hello World  "));
        assert!(default_evaluate_condition(
            "msg | trim | lowercase == hello world",
            &vars
        ));
    }

    #[test]
    fn test_condition_pipe_starts_with() {
        let mut vars = HashMap::new();
        vars.insert("path".to_string(), json!("/API/v2/users"));
        assert!(default_evaluate_condition(
            r#"path | lowercase starts_with "/api""#,
            &vars
        ));
        assert!(!default_evaluate_condition(
            r#"path | lowercase starts_with "/admin""#,
            &vars
        ));
    }

    #[test]
    fn test_condition_pipe_ends_with() {
        let mut vars = HashMap::new();
        vars.insert("file".to_string(), json!("README.MD"));
        assert!(default_evaluate_condition(
            r#"file | lowercase ends_with ".md""#,
            &vars
        ));
    }

    #[test]
    fn test_condition_pipe_contains() {
        let mut vars = HashMap::new();
        vars.insert("message".to_string(), json!("Hello World"));
        assert!(default_evaluate_condition(
            r#"message | lowercase contains "world""#,
            &vars
        ));
        assert!(!default_evaluate_condition(
            r#"message | lowercase contains "WORLD""#,
            &vars
        ));
    }

    #[test]
    fn test_condition_pipe_matches() {
        let mut vars = HashMap::new();
        vars.insert("email".to_string(), json!("USER@Example.Com"));
        assert!(default_evaluate_condition(
            r#"email | lowercase matches ".*@example\.com""#,
            &vars
        ));
    }

    #[test]
    fn test_condition_pipe_truthy() {
        let mut vars = HashMap::new();
        vars.insert("input".to_string(), json!("  "));
        // " " trimmed is empty → falsy
        assert!(!default_evaluate_condition("input | trim", &vars));
        vars.insert("input".to_string(), json!("  hi  "));
        // "hi" is non-empty → truthy
        assert!(default_evaluate_condition("input | trim", &vars));
    }

    #[test]
    fn test_condition_pipe_with_logical_ops() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("hello"));
        vars.insert("flag".to_string(), json!(true));
        assert!(default_evaluate_condition(
            "name | uppercase == HELLO && flag",
            &vars
        ));
        assert!(!default_evaluate_condition(
            "name | uppercase == WORLD && flag",
            &vars
        ));
    }

    #[test]
    fn test_condition_pipe_capitalize() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("hello"));
        assert!(default_evaluate_condition("name | capitalize == Hello", &vars));
    }

    #[test]
    fn test_condition_pipe_does_not_break_existing() {
        // Ensure plain variable comparisons still work
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("ok"));
        vars.insert("count".to_string(), json!(5));
        assert!(default_evaluate_condition("status == ok", &vars));
        assert!(default_evaluate_condition("count == 5", &vars));
        assert!(default_evaluate_condition("count > 3", &vars));
    }

    // ── Function-call syntax tests ────────────────────────────────────────

    #[test]
    fn test_fn_len_array_comparison() {
        let mut vars = HashMap::new();
        vars.insert("items".to_string(), json!(["a", "b", "c", "d", "e"]));
        assert!(default_evaluate_condition("len(items) > 3", &vars));
        assert!(default_evaluate_condition("len(items) == 5", &vars));
        assert!(!default_evaluate_condition("len(items) < 3", &vars));
        assert!(default_evaluate_condition("length(items) >= 5", &vars));
    }

    #[test]
    fn test_fn_len_string() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("hello"));
        assert!(default_evaluate_condition("len(name) == 5", &vars));
        assert!(default_evaluate_condition("len(name) > 2", &vars));
    }

    #[test]
    fn test_fn_len_object() {
        let mut vars = HashMap::new();
        vars.insert("config".to_string(), json!({"a": 1, "b": 2}));
        assert!(default_evaluate_condition("len(config) == 2", &vars));
    }

    #[test]
    fn test_fn_is_empty_truthy() {
        let mut vars = HashMap::new();
        vars.insert("empty_str".to_string(), json!(""));
        vars.insert("empty_arr".to_string(), json!([]));
        vars.insert("full_str".to_string(), json!("hello"));
        vars.insert("null_val".to_string(), Value::Null);

        // is_empty returns true for empty values → truthy in condition
        assert!(default_evaluate_condition("is_empty(empty_str)", &vars));
        assert!(default_evaluate_condition("is_empty(empty_arr)", &vars));
        assert!(default_evaluate_condition("is_empty(null_val)", &vars));
        assert!(default_evaluate_condition("is_empty(nonexistent)", &vars));

        // is_empty returns false for non-empty → falsy in condition
        assert!(!default_evaluate_condition("is_empty(full_str)", &vars));
    }

    #[test]
    fn test_fn_not_empty() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("world"));
        vars.insert("empty".to_string(), json!(""));

        assert!(default_evaluate_condition("not_empty(name)", &vars));
        assert!(!default_evaluate_condition("not_empty(empty)", &vars));
    }

    #[test]
    fn test_fn_contains() {
        let mut vars = HashMap::new();
        vars.insert("greeting".to_string(), json!("hello world"));
        vars.insert("tags".to_string(), json!(["rust", "praxis", "radix"]));

        assert!(default_evaluate_condition("contains(greeting, \"world\")", &vars));
        assert!(!default_evaluate_condition("contains(greeting, \"foo\")", &vars));
        assert!(default_evaluate_condition("contains(tags, \"rust\")", &vars));
        assert!(!default_evaluate_condition("contains(tags, \"python\")", &vars));
    }

    #[test]
    fn test_fn_starts_with_ends_with() {
        let mut vars = HashMap::new();
        vars.insert("path".to_string(), json!("/api/v2/users"));

        assert!(default_evaluate_condition("starts_with(path, \"/api\")", &vars));
        assert!(!default_evaluate_condition("starts_with(path, \"/web\")", &vars));
        assert!(default_evaluate_condition("ends_with(path, \"users\")", &vars));
        assert!(!default_evaluate_condition("ends_with(path, \"posts\")", &vars));
    }

    #[test]
    fn test_fn_trim_comparison() {
        let mut vars = HashMap::new();
        vars.insert("input".to_string(), json!("  hello  "));

        assert!(default_evaluate_condition("trim(input) == hello", &vars));
        assert!(!default_evaluate_condition("trim(input) == \"  hello  \"", &vars));
    }

    #[test]
    fn test_fn_uppercase_lowercase() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("Hello"));

        assert!(default_evaluate_condition("upper(name) == HELLO", &vars));
        assert!(default_evaluate_condition("lowercase(name) == hello", &vars));
    }

    #[test]
    fn test_fn_type_of() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("hello"));
        vars.insert("count".to_string(), json!(42));
        vars.insert("flag".to_string(), json!(true));
        vars.insert("items".to_string(), json!([1, 2, 3]));

        assert!(default_evaluate_condition("type_of(name) == string", &vars));
        assert!(default_evaluate_condition("type_of(count) == number", &vars));
        assert!(default_evaluate_condition("type_of(flag) == boolean", &vars));
        assert!(default_evaluate_condition("type_of(items) == array", &vars));
        assert!(default_evaluate_condition("type_of(missing) == null", &vars));
    }

    #[test]
    fn test_fn_default_fallback() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("world"));
        vars.insert("empty".to_string(), json!(""));

        assert!(default_evaluate_condition("default(name, \"anon\") == world", &vars));
        assert!(default_evaluate_condition("default(empty, \"anon\") == anon", &vars));
        assert!(default_evaluate_condition("default(missing, \"anon\") == anon", &vars));
    }

    #[test]
    fn test_fn_in_logical_expressions() {
        let mut vars = HashMap::new();
        vars.insert("items".to_string(), json!(["a", "b", "c"]));
        vars.insert("name".to_string(), json!("admin"));

        // Combined with && and ||
        assert!(default_evaluate_condition("len(items) > 2 && not_empty(name)", &vars));
        assert!(default_evaluate_condition("is_empty(missing) || len(items) == 3", &vars));
        assert!(!default_evaluate_condition("len(items) > 5 && not_empty(name)", &vars));
    }

    #[test]
    fn test_fn_with_negation() {
        let mut vars = HashMap::new();
        vars.insert("items".to_string(), json!(["a", "b"]));
        vars.insert("name".to_string(), json!("hello"));

        assert!(default_evaluate_condition("!is_empty(name)", &vars));
        assert!(!default_evaluate_condition("!not_empty(name)", &vars));
    }

    #[test]
    fn test_fn_capitalize_reverse() {
        let mut vars = HashMap::new();
        vars.insert("word".to_string(), json!("hello"));

        assert!(default_evaluate_condition("capitalize(word) == Hello", &vars));
        assert!(default_evaluate_condition("reverse(word) == olleh", &vars));
    }

    #[test]
    fn test_fn_len_arithmetic() {
        let mut vars = HashMap::new();
        vars.insert("items".to_string(), json!(["a", "b", "c"]));
        vars.insert("offset".to_string(), json!(2));

        // len(items) + offset == 5
        assert!(default_evaluate_condition("len(items) + offset == 5", &vars));
    }

    // ===== Nested function call tests =====

    #[test]
    fn test_nested_len_trim() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("  hello  "));
        // len(trim(name)) should be 5 ("hello")
        assert!(default_evaluate_condition("len(trim(name)) == 5", &vars));
    }

    #[test]
    fn test_nested_len_upper() {
        let mut vars = HashMap::new();
        vars.insert("greeting".to_string(), json!("hi"));
        // len(upper(greeting)) == 2
        assert!(default_evaluate_condition("len(upper(greeting)) == 2", &vars));
    }

    #[test]
    fn test_nested_is_empty_trim() {
        let mut vars = HashMap::new();
        vars.insert("blank".to_string(), json!("   "));
        vars.insert("filled".to_string(), json!(" hi "));
        // is_empty(trim(blank)) should be true (trimmed to empty string)
        assert!(default_evaluate_condition("is_empty(trim(blank))", &vars));
        // is_empty(trim(filled)) should be false
        assert!(!default_evaluate_condition("is_empty(trim(filled))", &vars));
    }

    #[test]
    fn test_nested_upper_trim() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!(" world "));
        // upper(trim(name)) == "WORLD"
        assert!(default_evaluate_condition(r#"upper(trim(name)) == "WORLD""#, &vars));
    }

    #[test]
    fn test_nested_lower_trim() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!(" HeLLo "));
        // lower(trim(name)) == "hello"
        assert!(default_evaluate_condition(r#"lower(trim(name)) == "hello""#, &vars));
    }

    #[test]
    fn test_nested_contains_lower() {
        let mut vars = HashMap::new();
        vars.insert("msg".to_string(), json!("Hello World"));
        // contains(lower(msg), "hello") should be true
        assert!(default_evaluate_condition(r#"contains(lower(msg), "hello")"#, &vars));
    }

    #[test]
    fn test_nested_starts_with_trim() {
        let mut vars = HashMap::new();
        vars.insert("path".to_string(), json!("  /api/v1/users"));
        // starts_with(trim(path), "/api") should be true
        assert!(default_evaluate_condition(r#"starts_with(trim(path), "/api")"#, &vars));
    }

    #[test]
    fn test_nested_three_levels() {
        let mut vars = HashMap::new();
        vars.insert("val".to_string(), json!(" Hello "));
        // len(lower(trim(val))) == 5 (trim->"Hello", lower->"hello", len->5)
        assert!(default_evaluate_condition("len(lower(trim(val))) == 5", &vars));
    }

    #[test]
    fn test_nested_not_empty_trim() {
        let mut vars = HashMap::new();
        vars.insert("input".to_string(), json!("  data  "));
        vars.insert("empty".to_string(), json!("     "));
        // not_empty(trim(input)) -> true
        assert!(default_evaluate_condition("not_empty(trim(input))", &vars));
        // not_empty(trim(empty)) -> false
        assert!(!default_evaluate_condition("not_empty(trim(empty))", &vars));
    }

    #[test]
    fn test_nested_type_of_trim() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!("  42  "));
        // type_of(trim(x)) == "string" (trim returns a string)
        assert!(default_evaluate_condition(r#"type_of(trim(x)) == "string""#, &vars));
    }

    #[test]
    fn test_nested_capitalize_lower() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("JOHN"));
        // capitalize(lower(name)) == "John"
        assert!(default_evaluate_condition(r#"capitalize(lower(name)) == "John""#, &vars));
    }

    #[test]
    fn test_nested_len_trim_gt_zero() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("  hello  "));
        // Common validation pattern: len(trim(name)) > 0
        assert!(default_evaluate_condition("len(trim(name)) > 0", &vars));
    }

    // ── one_of / any_of tests ─────────────────────────────────────────────

    #[test]
    fn test_one_of_matches() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("active"));
        assert!(default_evaluate_condition(
            r#"one_of(status, "pending", "active", "review")"#,
            &vars
        ));
    }

    #[test]
    fn test_one_of_no_match() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("deleted"));
        assert!(!default_evaluate_condition(
            r#"one_of(status, "pending", "active", "review")"#,
            &vars
        ));
    }

    #[test]
    fn test_one_of_with_variable_candidates() {
        let mut vars = HashMap::new();
        vars.insert("color".to_string(), json!("blue"));
        vars.insert("primary".to_string(), json!("red"));
        vars.insert("secondary".to_string(), json!("blue"));
        // color matches secondary variable
        assert!(default_evaluate_condition(
            "one_of(color, primary, secondary)",
            &vars
        ));
    }

    #[test]
    fn test_none_of_passes() {
        let mut vars = HashMap::new();
        vars.insert("role".to_string(), json!("admin"));
        assert!(default_evaluate_condition(
            r#"none_of(role, "guest", "banned", "suspended")"#,
            &vars
        ));
    }

    #[test]
    fn test_none_of_fails() {
        let mut vars = HashMap::new();
        vars.insert("role".to_string(), json!("banned"));
        assert!(!default_evaluate_condition(
            r#"none_of(role, "guest", "banned", "suspended")"#,
            &vars
        ));
    }

    // ── if_else / ternary tests ───────────────────────────────────────────

    #[test]
    fn test_if_else_true_branch() {
        let mut vars = HashMap::new();
        vars.insert("is_admin".to_string(), json!(true));
        // if_else returns "full" when condition is truthy
        let result = resolve_function_call(r#"if_else(is_admin, "full", "limited")"#, &vars);
        assert_eq!(result, Some(json!("full")));
    }

    #[test]
    fn test_if_else_false_branch() {
        let mut vars = HashMap::new();
        vars.insert("is_admin".to_string(), json!(false));
        let result = resolve_function_call(r#"if_else(is_admin, "full", "limited")"#, &vars);
        assert_eq!(result, Some(json!("limited")));
    }

    #[test]
    fn test_if_else_with_nested_condition() {
        let mut vars = HashMap::new();
        vars.insert("count".to_string(), json!(5));
        // Uses a nested function as condition
        let result = resolve_function_call(r#"if_else(count, "has items", "empty")"#, &vars);
        assert_eq!(result, Some(json!("has items")));
    }

    // ── coalesce tests ────────────────────────────────────────────────────

    #[test]
    fn test_coalesce_first_truthy() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("Alice"));
        vars.insert("fallback".to_string(), json!("Unknown"));
        let result = resolve_function_call("coalesce(name, fallback)", &vars);
        assert_eq!(result, Some(json!("Alice")));
    }

    #[test]
    fn test_coalesce_skips_empty() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!(""));
        vars.insert("nick".to_string(), json!("Bob"));
        let result = resolve_function_call("coalesce(name, nick)", &vars);
        assert_eq!(result, Some(json!("Bob")));
    }

    #[test]
    fn test_coalesce_all_empty() {
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!(null));
        vars.insert("b".to_string(), json!(""));
        let result = resolve_function_call("coalesce(a, b)", &vars);
        assert_eq!(result, Some(Value::Null));
    }

    // ── min / max / abs / clamp tests ─────────────────────────────────────

    #[test]
    fn test_min_basic() {
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!(10));
        vars.insert("b".to_string(), json!(3));
        vars.insert("c".to_string(), json!(7));
        let result = resolve_function_call("min(a, b, c)", &vars);
        assert_eq!(result, Some(json!(3)));
    }

    #[test]
    fn test_max_basic() {
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!(10));
        vars.insert("b".to_string(), json!(3));
        vars.insert("c".to_string(), json!(7));
        let result = resolve_function_call("max(a, b, c)", &vars);
        assert_eq!(result, Some(json!(10)));
    }

    #[test]
    fn test_abs_negative() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!(-42));
        let result = resolve_function_call("abs(x)", &vars);
        assert_eq!(result, Some(json!(42)));
    }

    #[test]
    fn test_abs_positive() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!(7));
        let result = resolve_function_call("abs(x)", &vars);
        assert_eq!(result, Some(json!(7)));
    }

    #[test]
    fn test_clamp_within_range() {
        let mut vars = HashMap::new();
        vars.insert("val".to_string(), json!(5));
        let result = resolve_function_call("clamp(val, 0, 10)", &vars);
        assert_eq!(result, Some(json!(5)));
    }

    #[test]
    fn test_clamp_below_min() {
        let mut vars = HashMap::new();
        vars.insert("val".to_string(), json!(-3));
        let result = resolve_function_call("clamp(val, 0, 10)", &vars);
        assert_eq!(result, Some(json!(0)));
    }

    #[test]
    fn test_clamp_above_max() {
        let mut vars = HashMap::new();
        vars.insert("val".to_string(), json!(15));
        let result = resolve_function_call("clamp(val, 0, 10)", &vars);
        assert_eq!(result, Some(json!(10)));
    }

    // ── Combined expression tests (real-world patterns) ───────────────────

    #[test]
    fn test_one_of_combined_with_logic() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("active"));
        vars.insert("role".to_string(), json!("admin"));
        // Complex guard: status is active AND role is admin or moderator
        assert!(default_evaluate_condition(
            r#"one_of(status, "active") && one_of(role, "admin", "moderator")"#,
            &vars
        ));
    }

    #[test]
    fn test_one_of_in_when_guard_pattern() {
        let mut vars = HashMap::new();
        vars.insert("event_type".to_string(), json!("push"));
        // Typical when-guard: only fire on certain event types
        assert!(default_evaluate_condition(
            r#"one_of(event_type, "push", "pull_request", "release")"#,
            &vars
        ));
        vars.insert("event_type".to_string(), json!("comment"));
        assert!(!default_evaluate_condition(
            r#"one_of(event_type, "push", "pull_request", "release")"#,
            &vars
        ));
    }

    // === Match Expression Tests ===

    #[test]
    fn test_match_expr_string_match() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("active"));
        let result = try_match_expr(
            r#"match status { "active" => "green", "inactive" => "red", _ => "grey" }"#,
            &vars,
        );
        assert_eq!(result, Some("green".to_string()));
    }

    #[test]
    fn test_match_expr_second_arm() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("inactive"));
        let result = try_match_expr(
            r#"match status { "active" => "green", "inactive" => "red", _ => "grey" }"#,
            &vars,
        );
        assert_eq!(result, Some("red".to_string()));
    }

    #[test]
    fn test_match_expr_default_arm() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("unknown"));
        let result = try_match_expr(
            r#"match status { "active" => "green", "inactive" => "red", _ => "grey" }"#,
            &vars,
        );
        assert_eq!(result, Some("grey".to_string()));
    }

    #[test]
    fn test_match_expr_no_default_no_match() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("unknown"));
        let result = try_match_expr(
            r#"match status { "active" => "green", "inactive" => "red" }"#,
            &vars,
        );
        assert_eq!(result, None);
    }

    #[test]
    fn test_match_expr_numeric_patterns() {
        let mut vars = HashMap::new();
        vars.insert("level".to_string(), json!("3"));
        let result = try_match_expr(
            r#"match level { "1" => "low", "2" => "medium", "3" => "high", _ => "unknown" }"#,
            &vars,
        );
        assert_eq!(result, Some("high".to_string()));
    }

    #[test]
    fn test_match_expr_variable_result() {
        let mut vars = HashMap::new();
        vars.insert("mode".to_string(), json!("prod"));
        vars.insert("prod_url".to_string(), json!("https://api.prod.com"));
        let result = try_match_expr(
            r#"match mode { "prod" => prod_url, "dev" => "http://localhost", _ => "" }"#,
            &vars,
        );
        assert_eq!(result, Some("https://api.prod.com".to_string()));
    }

    #[test]
    fn test_match_expr_in_interpolation() {
        let mut vars = HashMap::new();
        vars.insert("env".to_string(), json!("staging"));
        let result = resolve_interpolation_expr(
            r#"match env { "prod" => "production", "staging" => "stage", _ => "dev" }"#,
            &vars,
        );
        assert_eq!(result, "stage");
    }

    #[test]
    fn test_match_expr_single_quotes() {
        let mut vars = HashMap::new();
        vars.insert("color".to_string(), json!("blue"));
        let result = try_match_expr(
            "match color { 'red' => 'stop', 'blue' => 'go', _ => 'wait' }",
            &vars,
        );
        assert_eq!(result, Some("go".to_string()));
    }

    #[test]
    fn test_match_expr_not_a_match() {
        let vars = HashMap::new();
        // Should return None for non-match expressions
        let result = try_match_expr("status == \"active\"", &vars);
        assert_eq!(result, None);
    }

    #[test]
    fn test_match_expr_integer_subject() {
        let mut vars = HashMap::new();
        vars.insert("code".to_string(), json!(200));
        let result = try_match_expr(
            r#"match code { "200" => "ok", "404" => "not_found", "500" => "error", _ => "unknown" }"#,
            &vars,
        );
        assert_eq!(result, Some("ok".to_string()));
    }

    // ── match_expr in condition evaluation tests ────────────────────────────

    #[test]
    fn test_condition_match_expr_truthy() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("active"));
        // match resolves to "true" → truthy
        assert!(default_evaluate_condition(
            r#"match status { "active" => true, _ => false }"#,
            &vars,
        ));
    }

    #[test]
    fn test_condition_match_expr_falsy() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("inactive"));
        // match resolves to "false" → falsy
        assert!(!default_evaluate_condition(
            r#"match status { "active" => true, _ => false }"#,
            &vars,
        ));
    }

    #[test]
    fn test_condition_match_expr_non_empty_is_truthy() {
        let mut vars = HashMap::new();
        vars.insert("level".to_string(), json!("error"));
        // match resolves to "red" — non-empty, truthy
        assert!(default_evaluate_condition(
            r#"match level { "error" => "red", _ => "" }"#,
            &vars,
        ));
    }

    #[test]
    fn test_condition_match_expr_empty_is_falsy() {
        let mut vars = HashMap::new();
        vars.insert("level".to_string(), json!("info"));
        // match resolves to "" — empty, falsy
        assert!(!default_evaluate_condition(
            r#"match level { "error" => "red", _ => "" }"#,
            &vars,
        ));
    }

    #[test]
    fn test_condition_match_expr_no_match_is_falsy() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!("unknown"));
        // No arm matches and no default → try_match_expr returns None → false
        assert!(!default_evaluate_condition(
            r#"match x { "a" => true, "b" => true }"#,
            &vars,
        ));
    }

    #[test]
    fn test_condition_match_expr_in_comparison() {
        let mut vars = HashMap::new();
        vars.insert("tier".to_string(), json!("premium"));
        // match tier { "premium" => "gold", _ => "silver" } == "gold"
        assert!(default_evaluate_condition(
            r#"match tier { "premium" => "gold", _ => "silver" } == "gold""#,
            &vars,
        ));
        assert!(!default_evaluate_condition(
            r#"match tier { "premium" => "gold", _ => "silver" } == "silver""#,
            &vars,
        ));
    }

    #[test]
    fn test_condition_match_expr_with_logical_ops() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("active"));
        vars.insert("role".to_string(), json!("admin"));
        // match resolves to true AND role == admin
        assert!(default_evaluate_condition(
            r#"match status { "active" => true, _ => false } && role == admin"#,
            &vars,
        ));
        // match resolves to false AND role == admin → false
        vars.insert("status".to_string(), json!("inactive"));
        assert!(!default_evaluate_condition(
            r#"match status { "active" => true, _ => false } && role == admin"#,
            &vars,
        ));
    }

    #[test]
    fn test_condition_match_expr_numeric_truthy() {
        let mut vars = HashMap::new();
        vars.insert("code".to_string(), json!(200));
        // "1" is truthy, "0" is falsy
        assert!(default_evaluate_condition(
            r#"match code { "200" => 1, _ => 0 }"#,
            &vars,
        ));
        vars.insert("code".to_string(), json!(500));
        assert!(!default_evaluate_condition(
            r#"match code { "200" => 1, _ => 0 }"#,
            &vars,
        ));
    }

    // ── multi-pattern (| separator) tests ───────────────────────────────────

    #[test]
    fn test_match_expr_multi_pattern_first_alt() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("active"));
        let result = try_match_expr(
            r#"match status { "active" | "enabled" => "on", "inactive" | "disabled" => "off", _ => "unknown" }"#,
            &vars,
        );
        assert_eq!(result, Some("on".to_string()));
    }

    #[test]
    fn test_match_expr_multi_pattern_second_alt() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("enabled"));
        let result = try_match_expr(
            r#"match status { "active" | "enabled" => "on", _ => "off" }"#,
            &vars,
        );
        assert_eq!(result, Some("on".to_string()));
    }

    #[test]
    fn test_match_expr_multi_pattern_no_match_falls_to_default() {
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("pending"));
        let result = try_match_expr(
            r#"match status { "active" | "enabled" => "on", "inactive" | "disabled" => "off", _ => "unknown" }"#,
            &vars,
        );
        assert_eq!(result, Some("unknown".to_string()));
    }

    #[test]
    fn test_match_expr_multi_pattern_three_alternatives() {
        let mut vars = HashMap::new();
        vars.insert("code".to_string(), json!("503"));
        let result = try_match_expr(
            r#"match code { "500" | "502" | "503" => "server_error", "400" | "404" => "client_error", _ => "ok" }"#,
            &vars,
        );
        assert_eq!(result, Some("server_error".to_string()));
    }

    #[test]
    fn test_match_expr_multi_pattern_with_variables() {
        let mut vars = HashMap::new();
        vars.insert("level".to_string(), json!("warn"));
        vars.insert("alt_level".to_string(), json!("warn"));
        // Pattern uses a variable reference
        let result = try_match_expr(
            r#"match level { "error" | "warn" | "fatal" => "alert", _ => "normal" }"#,
            &vars,
        );
        assert_eq!(result, Some("alert".to_string()));
    }

    #[test]
    fn test_match_expr_multi_pattern_numeric() {
        let mut vars = HashMap::new();
        vars.insert("exit_code".to_string(), json!(1));
        let result = try_match_expr(
            r#"match exit_code { 0 => "success", 1 | 2 | 3 => "failure", _ => "unknown" }"#,
            &vars,
        );
        assert_eq!(result, Some("failure".to_string()));
    }

    #[test]
    fn test_match_expr_multi_pattern_in_condition() {
        let mut vars = HashMap::new();
        vars.insert("env".to_string(), json!("staging"));
        assert!(default_evaluate_condition(
            r#"match env { "prod" | "staging" => true, _ => false }"#,
            &vars,
        ));
        vars.insert("env".to_string(), json!("dev"));
        assert!(!default_evaluate_condition(
            r#"match env { "prod" | "staging" => true, _ => false }"#,
            &vars,
        ));
    }

    #[test]
    fn test_match_expr_range_exclusive() {
        let mut vars = HashMap::new();
        vars.insert("age".to_string(), json!(3));
        let result = try_match_expr(r#"match age { 1..5 => "child", 5..18 => "teen", _ => "adult" }"#, &vars);
        assert_eq!(result, Some("child".to_string()));
    }

    #[test]
    fn test_match_expr_range_exclusive_boundary() {
        let mut vars = HashMap::new();
        // 5 should NOT match 1..5 (exclusive end), should match 5..18
        vars.insert("age".to_string(), json!(5));
        let result = try_match_expr(r#"match age { 1..5 => "child", 5..18 => "teen", _ => "adult" }"#, &vars);
        assert_eq!(result, Some("teen".to_string()));
    }

    #[test]
    fn test_match_expr_range_inclusive() {
        let mut vars = HashMap::new();
        vars.insert("score".to_string(), json!(100));
        let result = try_match_expr(r#"match score { 0..=59 => "fail", 60..=100 => "pass", _ => "invalid" }"#, &vars);
        assert_eq!(result, Some("pass".to_string()));
    }

    #[test]
    fn test_match_expr_range_inclusive_boundary() {
        let mut vars = HashMap::new();
        vars.insert("score".to_string(), json!(59));
        let result = try_match_expr(r#"match score { 0..=59 => "fail", 60..=100 => "pass", _ => "invalid" }"#, &vars);
        assert_eq!(result, Some("fail".to_string()));
    }

    #[test]
    fn test_match_expr_range_no_match_falls_to_default() {
        let mut vars = HashMap::new();
        vars.insert("val".to_string(), json!(200));
        let result = try_match_expr(r#"match val { 0..100 => "low", _ => "high" }"#, &vars);
        assert_eq!(result, Some("high".to_string()));
    }

    #[test]
    fn test_match_expr_range_with_multi_pattern() {
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!(7));
        // Range combined with literal via multi-pattern
        let result = try_match_expr(r#"match x { 1..5 | "7" => "match", _ => "no" }"#, &vars);
        assert_eq!(result, Some("match".to_string()));
    }

    #[test]
    fn test_match_expr_range_float() {
        let mut vars = HashMap::new();
        vars.insert("temp".to_string(), json!(36.6));
        let result = try_match_expr(r#"match temp { 35.0..=37.5 => "normal", _ => "abnormal" }"#, &vars);
        assert_eq!(result, Some("normal".to_string()));
    }

    #[test]
    fn test_step_match_with_subject() {
        let handler = MockHandler::new();
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("active"));

        let step = json!({
            "kind": "match",
            "subject": "status",
            "arms": [
                { "pattern": "\"active\"", "result": "running" },
                { "pattern": "\"paused\"", "result": "stopped" },
                { "pattern": "_", "result": "unknown" }
            ]
        });

        let result = execute_match(&step, 0, &mut vars, &handler).unwrap();
        assert_eq!(result.output, Some(json!("running")));
        assert!(!result.skipped);
    }

    #[test]
    fn test_step_match_with_subject_default() {
        let handler = MockHandler::new();
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("deleted"));

        let step = json!({
            "kind": "match",
            "subject": "status",
            "arms": [
                { "pattern": "\"active\"", "result": "running" },
                { "pattern": "_", "result": "unknown" }
            ]
        });

        let result = execute_match(&step, 0, &mut vars, &handler).unwrap();
        assert_eq!(result.output, Some(json!("unknown")));
    }

    #[test]
    fn test_step_match_with_subject_range() {
        let handler = MockHandler::new();
        let mut vars = HashMap::new();
        vars.insert("code".to_string(), json!(404));

        let step = json!({
            "kind": "match",
            "subject": "code",
            "arms": [
                { "pattern": "200..=299", "result": "success" },
                { "pattern": "400..=499", "result": "client_error" },
                { "pattern": "500..=599", "result": "server_error" },
                { "pattern": "_", "result": "unknown" }
            ]
        });

        let result = execute_match(&step, 0, &mut vars, &handler).unwrap();
        assert_eq!(result.output, Some(json!("client_error")));
    }

    #[test]
    fn test_step_match_with_subject_no_match() {
        let handler = MockHandler::new();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!("foo"));

        let step = json!({
            "kind": "match",
            "subject": "x",
            "arms": [
                { "pattern": "\"bar\"", "result": "matched" }
            ]
        });

        let result = execute_match(&step, 0, &mut vars, &handler).unwrap();
        assert!(result.skipped);
        assert_eq!(result.output, None);
    }

    // ── Guard Patterns in Match Arms ─────────────────────────────────────────────

    #[test]
    fn test_match_guard_passes() {
        // match x { "active" if score > 50 => "high", _ => "low" }
        let handler = MockHandler::new();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!("active"));
        vars.insert("score".to_string(), json!(80));

        let step = json!({
            "kind": "match",
            "subject": "x",
            "arms": [
                { "pattern": "\"active\" if score > 50", "result": "high" },
                { "pattern": "_", "result": "low" }
            ]
        });

        let result = execute_match(&step, 0, &mut vars, &handler).unwrap();
        assert!(!result.skipped);
        assert_eq!(result.output, Some(json!("high")));
    }

    #[test]
    fn test_match_guard_fails_falls_through() {
        // match x { "active" if score > 50 => "high", "active" => "normal", _ => "low" }
        let handler = MockHandler::new();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!("active"));
        vars.insert("score".to_string(), json!(30));

        let step = json!({
            "kind": "match",
            "subject": "x",
            "arms": [
                { "pattern": "\"active\" if score > 50", "result": "high" },
                { "pattern": "\"active\"", "result": "normal" },
                { "pattern": "_", "result": "low" }
            ]
        });

        let result = execute_match(&step, 0, &mut vars, &handler).unwrap();
        assert!(!result.skipped);
        assert_eq!(result.output, Some(json!("normal")));
    }

    #[test]
    fn test_match_guard_on_default_arm() {
        // match x { _ if enabled == "true" => "on", _ => "off" }
        let handler = MockHandler::new();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!("anything"));
        vars.insert("enabled".to_string(), json!("false"));

        let step = json!({
            "kind": "match",
            "subject": "x",
            "arms": [
                { "pattern": "_ if enabled == \"true\"", "result": "on" },
                { "pattern": "_", "result": "off" }
            ]
        });

        let result = execute_match(&step, 0, &mut vars, &handler).unwrap();
        assert!(!result.skipped);
        assert_eq!(result.output, Some(json!("off")));
    }

    #[test]
    fn test_match_guard_with_range_pattern() {
        // match score { 1..=100 if premium == "true" => "vip", 1..=100 => "standard", _ => "invalid" }
        let handler = MockHandler::new();
        let mut vars = HashMap::new();
        vars.insert("score".to_string(), json!("75"));
        vars.insert("premium".to_string(), json!("true"));

        let step = json!({
            "kind": "match",
            "subject": "score",
            "arms": [
                { "pattern": "1..=100 if premium == \"true\"", "result": "vip" },
                { "pattern": "1..=100", "result": "standard" },
                { "pattern": "_", "result": "invalid" }
            ]
        });

        let result = execute_match(&step, 0, &mut vars, &handler).unwrap();
        assert!(!result.skipped);
        assert_eq!(result.output, Some(json!("vip")));
    }

    #[test]
    fn test_match_guard_with_multi_pattern() {
        // match status { "active" | "pending" if role == "admin" => "allowed", _ => "denied" }
        let handler = MockHandler::new();
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("pending"));
        vars.insert("role".to_string(), json!("admin"));

        let step = json!({
            "kind": "match",
            "subject": "status",
            "arms": [
                { "pattern": "\"active\" | \"pending\" if role == \"admin\"", "result": "allowed" },
                { "pattern": "_", "result": "denied" }
            ]
        });

        let result = execute_match(&step, 0, &mut vars, &handler).unwrap();
        assert!(!result.skipped);
        assert_eq!(result.output, Some(json!("allowed")));
    }

    #[test]
    fn test_match_guard_multi_pattern_guard_fails() {
        // Pattern matches but guard fails → falls through
        let handler = MockHandler::new();
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("active"));
        vars.insert("role".to_string(), json!("viewer"));

        let step = json!({
            "kind": "match",
            "subject": "status",
            "arms": [
                { "pattern": "\"active\" | \"pending\" if role == \"admin\"", "result": "allowed" },
                { "pattern": "_", "result": "denied" }
            ]
        });

        let result = execute_match(&step, 0, &mut vars, &handler).unwrap();
        assert!(!result.skipped);
        assert_eq!(result.output, Some(json!("denied")));
    }

    #[test]
    fn test_match_guard_no_false_positive_on_if_in_string() {
        // Pattern with "if" inside a quoted string should NOT be treated as a guard
        let handler = MockHandler::new();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!("check if ready"));

        let step = json!({
            "kind": "match",
            "subject": "x",
            "arms": [
                { "pattern": "\"check if ready\"", "result": "found" },
                { "pattern": "_", "result": "not found" }
            ]
        });

        let result = execute_match(&step, 0, &mut vars, &handler).unwrap();
        assert!(!result.skipped);
        assert_eq!(result.output, Some(json!("found")));
    }

    #[test]
    fn test_extract_guard_helper() {
        // Direct unit tests for extract_guard
        assert_eq!(extract_guard("\"active\""), ("\"active\"", None));
        assert_eq!(
            extract_guard("\"active\" if score > 50"),
            ("\"active\"", Some("score > 50"))
        );
        assert_eq!(
            extract_guard("_ if enabled == \"true\""),
            ("_", Some("enabled == \"true\""))
        );
        // "if" inside quotes should NOT be detected as guard
        assert_eq!(extract_guard("\"check if ready\""), ("\"check if ready\"", None));
    }

    // ── Tuple Destructuring Tests ───────────────────────────────────────────────

    #[test]
    fn test_tuple_pattern_basic_string_match() {
        let vars = HashMap::new();
        let subject = Value::Array(vec![Value::String("error".into()), Value::Number(500.into())]);
        assert_eq!(try_tuple_pattern(r#"("error", 500)"#, &subject, &vars).map(|(m, _)| m), Some(true));
    }

    #[test]
    fn test_tuple_pattern_no_match() {
        let vars = HashMap::new();
        let subject = Value::Array(vec![Value::String("ok".into()), Value::Number(200.into())]);
        assert_eq!(try_tuple_pattern(r#"("error", 500)"#, &subject, &vars).map(|(m, _)| m), Some(false));
    }

    #[test]
    fn test_tuple_pattern_wildcard() {
        let vars = HashMap::new();
        let subject = Value::Array(vec![Value::String("error".into()), Value::Number(500.into())]);
        assert_eq!(try_tuple_pattern(r#"("error", _)"#, &subject, &vars).map(|(m, _)| m), Some(true));
    }

    #[test]
    fn test_tuple_pattern_all_wildcards() {
        let vars = HashMap::new();
        let subject = Value::Array(vec![Value::String("anything".into()), Value::Bool(true)]);
        assert_eq!(try_tuple_pattern("(_, _)", &subject, &vars).map(|(m, _)| m), Some(true));
    }

    #[test]
    fn test_tuple_pattern_length_mismatch() {
        let vars = HashMap::new();
        let subject = Value::Array(vec![Value::String("a".into())]);
        assert_eq!(try_tuple_pattern(r#"("a", "b")"#, &subject, &vars).map(|(m, _)| m), Some(false));
    }

    #[test]
    fn test_tuple_pattern_not_a_tuple() {
        let vars = HashMap::new();
        let subject = Value::String("hello".into());
        // Pattern starts with ( but subject is not an array
        assert_eq!(try_tuple_pattern("(\"hello\")", &subject, &vars).map(|(m, _)| m), Some(false));
    }

    #[test]
    fn test_tuple_pattern_non_tuple_pattern() {
        let vars = HashMap::new();
        let subject = Value::Array(vec![Value::Number(1.into())]);
        // Pattern doesn't start with ( — not a tuple pattern
        assert_eq!(try_tuple_pattern("\"hello\"", &subject, &vars), None);
    }

    #[test]
    fn test_tuple_pattern_with_variable() {
        let mut vars = HashMap::new();
        vars.insert("code".into(), Value::Number(404.into()));
        let subject = Value::Array(vec![Value::String("error".into()), Value::Number(404.into())]);
        assert_eq!(try_tuple_pattern(r#"("error", $code)"#, &subject, &vars).map(|(m, _)| m), Some(true));
    }

    #[test]
    fn test_tuple_pattern_with_boolean() {
        let vars = HashMap::new();
        let subject = Value::Array(vec![Value::String("flag".into()), Value::Bool(true)]);
        assert_eq!(try_tuple_pattern(r#"("flag", true)"#, &subject, &vars).map(|(m, _)| m), Some(true));
        assert_eq!(try_tuple_pattern(r#"("flag", false)"#, &subject, &vars).map(|(m, _)| m), Some(false));
    }

    #[test]
    fn test_tuple_pattern_with_null() {
        let vars = HashMap::new();
        let subject = Value::Array(vec![Value::String("x".into()), Value::Null]);
        assert_eq!(try_tuple_pattern(r#"("x", null)"#, &subject, &vars).map(|(m, _)| m), Some(true));
    }

    #[test]
    fn test_tuple_pattern_in_match_expr() {
        let mut vars = HashMap::new();
        vars.insert("pair".into(), Value::Array(vec![Value::String("error".into()), Value::Number(500.into())]));
        let expr = r#"match $pair { ("error", 500) => "server_error", ("error", 404) => "not_found", _ => "unknown" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("server_error".to_string()));
    }

    #[test]
    fn test_tuple_pattern_in_match_expr_wildcard_arm() {
        let mut vars = HashMap::new();
        vars.insert("pair".into(), Value::Array(vec![Value::String("ok".into()), Value::Number(200.into())]));
        let expr = r#"match $pair { ("error", _) => "failed", ("ok", _) => "success", _ => "unknown" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("success".to_string()));
    }

    #[test]
    fn test_tuple_pattern_three_elements() {
        let mut vars = HashMap::new();
        vars.insert("triple".into(), Value::Array(vec![
            Value::String("deploy".into()),
            Value::String("prod".into()),
            Value::Number(3.into()),
        ]));
        let expr = r#"match $triple { ("deploy", "prod", 3) => "full_prod", ("deploy", "staging", _) => "staging", _ => "other" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("full_prod".to_string()));
    }

    // ── Struct Destructuring Tests ───────────────────────────────────────────────

    #[test]
    fn test_struct_pattern_basic_match() {
        let subject = json!({"kind": "error", "code": 500});
        let vars = HashMap::new();
        assert_eq!(try_struct_pattern(r#"{kind: "error", code: 500}"#, &subject, &vars).map(|(m, _)| m), Some(true));
    }

    #[test]
    fn test_struct_pattern_no_match() {
        let subject = json!({"kind": "error", "code": 404});
        let vars = HashMap::new();
        assert_eq!(try_struct_pattern(r#"{kind: "error", code: 500}"#, &subject, &vars).map(|(m, _)| m), Some(false));
    }

    #[test]
    fn test_struct_pattern_wildcard() {
        let subject = json!({"kind": "error", "code": 500, "msg": "internal"});
        let vars = HashMap::new();
        assert_eq!(try_struct_pattern(r#"{kind: "error", code: _}"#, &subject, &vars).map(|(m, _)| m), Some(true));
    }

    #[test]
    fn test_struct_pattern_missing_field() {
        let subject = json!({"kind": "error"});
        let vars = HashMap::new();
        assert_eq!(try_struct_pattern(r#"{kind: "error", code: 500}"#, &subject, &vars).map(|(m, _)| m), Some(false));
    }

    #[test]
    fn test_struct_pattern_partial_match() {
        // Pattern doesn't need to cover all fields
        let subject = json!({"kind": "error", "code": 500, "msg": "internal", "retryable": true});
        let vars = HashMap::new();
        assert_eq!(try_struct_pattern(r#"{kind: "error"}"#, &subject, &vars).map(|(m, _)| m), Some(true));
    }

    #[test]
    fn test_struct_pattern_empty() {
        // Empty struct pattern matches any object
        let subject = json!({"anything": 42});
        let vars = HashMap::new();
        assert_eq!(try_struct_pattern("{}", &subject, &vars).map(|(m, _)| m), Some(true));
    }

    #[test]
    fn test_struct_pattern_not_an_object() {
        let subject = json!([1, 2, 3]);
        let vars = HashMap::new();
        assert_eq!(try_struct_pattern(r#"{kind: "error"}"#, &subject, &vars).map(|(m, _)| m), Some(false));
    }

    #[test]
    fn test_struct_pattern_not_a_struct_pattern() {
        let subject = json!({"kind": "error"});
        let vars = HashMap::new();
        // A quoted string is not a struct pattern
        assert_eq!(try_struct_pattern(r#""hello""#, &subject, &vars), None);
    }

    #[test]
    fn test_struct_pattern_boolean_and_null() {
        let subject = json!({"active": true, "deleted": null});
        let vars = HashMap::new();
        assert_eq!(try_struct_pattern("{active: true, deleted: null}", &subject, &vars).map(|(m, _)| m), Some(true));
        assert_eq!(try_struct_pattern("{active: false}", &subject, &vars).map(|(m, _)| m), Some(false));
    }

    #[test]
    fn test_struct_pattern_variable() {
        let subject = json!({"status": "active"});
        let mut vars = HashMap::new();
        vars.insert("expected_status".into(), Value::String("active".into()));
        assert_eq!(try_struct_pattern("{status: $expected_status}", &subject, &vars).map(|(m, _)| m), Some(true));
    }

    #[test]
    fn test_struct_pattern_nested_struct() {
        let subject = json!({"outer": {"inner": "deep"}});
        let vars = HashMap::new();
        assert_eq!(try_struct_pattern(r#"{outer: {inner: "deep"}}"#, &subject, &vars).map(|(m, _)| m), Some(true));
        assert_eq!(try_struct_pattern(r#"{outer: {inner: "wrong"}}"#, &subject, &vars).map(|(m, _)| m), Some(false));
    }

    #[test]
    fn test_struct_pattern_nested_tuple() {
        let subject = json!({"coords": [1, 2]});
        let vars = HashMap::new();
        assert_eq!(try_struct_pattern("{coords: (1, 2)}", &subject, &vars).map(|(m, _)| m), Some(true));
        assert_eq!(try_struct_pattern("{coords: (1, 9)}", &subject, &vars).map(|(m, _)| m), Some(false));
    }

    #[test]
    fn test_struct_pattern_in_match_expr() {
        let mut vars = HashMap::new();
        vars.insert("event".into(), json!({"kind": "error", "code": 500}));
        let expr = r#"match $event { {kind: "error", code: 500} => "server_error", {kind: "error", code: 404} => "not_found", _ => "unknown" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("server_error".to_string()));
    }

    #[test]
    fn test_struct_pattern_in_match_expr_second_arm() {
        let mut vars = HashMap::new();
        vars.insert("event".into(), json!({"kind": "error", "code": 404}));
        let expr = r#"match $event { {kind: "error", code: 500} => "server_error", {kind: "error", code: 404} => "not_found", _ => "unknown" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("not_found".to_string()));
    }

    #[test]
    fn test_struct_pattern_in_match_expr_default() {
        let mut vars = HashMap::new();
        vars.insert("event".into(), json!({"kind": "info", "msg": "hello"}));
        let expr = r#"match $event { {kind: "error", code: 500} => "server_error", _ => "other" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("other".to_string()));
    }

    #[test]
    fn test_struct_pattern_with_guard() {
        let mut vars = HashMap::new();
        vars.insert("event".into(), json!({"kind": "error", "code": 500, "retryable": true}));
        vars.insert("should_retry".into(), Value::Bool(true));
        let expr = r#"match $event { {kind: "error"} if $should_retry => "retry", {kind: "error"} => "fail", _ => "ok" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("retry".to_string()));
    }

    // ── Match Arm Bindings Tests ─────────────────────────────────────────────────

    #[test]
    fn test_struct_binding_captures_value() {
        // $k should bind to "error" since it doesn't exist in vars
        let subject = json!({"kind": "error", "code": 500});
        let vars = HashMap::new();
        let result = try_struct_pattern(r#"{kind: $k, code: $c}"#, &subject, &vars);
        let (matched, bindings) = result.unwrap();
        assert!(matched);
        assert_eq!(bindings.get("k"), Some(&Value::String("error".into())));
        assert_eq!(bindings.get("c"), Some(&json!(500)));
    }

    #[test]
    fn test_struct_binding_existing_var_matches() {
        // $expected exists in vars, so it compares rather than captures
        let subject = json!({"kind": "error"});
        let mut vars = HashMap::new();
        vars.insert("expected".into(), Value::String("error".into()));
        let result = try_struct_pattern("{kind: $expected}", &subject, &vars);
        let (matched, bindings) = result.unwrap();
        assert!(matched);
        assert!(bindings.is_empty()); // No capture, just comparison
    }

    #[test]
    fn test_struct_binding_existing_var_fails() {
        // $expected exists but doesn't match
        let subject = json!({"kind": "warning"});
        let mut vars = HashMap::new();
        vars.insert("expected".into(), Value::String("error".into()));
        let result = try_struct_pattern("{kind: $expected}", &subject, &vars);
        let (matched, _) = result.unwrap();
        assert!(!matched);
    }

    #[test]
    fn test_tuple_binding_captures_value() {
        let subject = Value::Array(vec![Value::String("error".into()), Value::Number(404.into())]);
        let vars = HashMap::new();
        let result = try_tuple_pattern(r#"("error", $code)"#, &subject, &vars);
        let (matched, bindings) = result.unwrap();
        assert!(matched);
        assert_eq!(bindings.get("code"), Some(&json!(404)));
    }

    #[test]
    fn test_tuple_binding_multiple_captures() {
        let subject = Value::Array(vec![
            Value::String("deploy".into()),
            Value::String("prod".into()),
            Value::Number(3.into()),
        ]);
        let vars = HashMap::new();
        let result = try_tuple_pattern("($action, $env, $count)", &subject, &vars);
        let (matched, bindings) = result.unwrap();
        assert!(matched);
        assert_eq!(bindings.get("action"), Some(&Value::String("deploy".into())));
        assert_eq!(bindings.get("env"), Some(&Value::String("prod".into())));
        assert_eq!(bindings.get("count"), Some(&json!(3)));
    }

    #[test]
    fn test_match_expr_with_struct_bindings() {
        // Bindings should be available in the result expression
        let mut vars = HashMap::new();
        vars.insert("event".into(), json!({"kind": "error", "code": 500}));
        let expr = r#"match $event { {kind: "error", code: $c} => $c, _ => "unknown" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("500".to_string()));
    }

    #[test]
    fn test_match_expr_with_tuple_bindings() {
        let mut vars = HashMap::new();
        vars.insert("pair".into(), json!(["error", 404]));
        let expr = r#"match $pair { ("error", $code) => $code, _ => "none" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("404".to_string()));
    }

    #[test]
    fn test_match_step_with_struct_bindings() {
        // Test that bindings from match steps are injected into vars
        let handler = MockHandler::new();
        let step = json!({
            "kind": "match",
            "subject": "$event",
            "arms": [
                {"pattern": "{kind: \"error\", code: $c}", "result": "$c"},
                {"pattern": "_", "result": "unknown"}
            ]
        });
        let mut vars = HashMap::new();
        vars.insert("event".into(), json!({"kind": "error", "code": 500}));
        let result = execute_step(&step, 0, &mut vars, &handler).unwrap();
        assert_eq!(result.output, Some(json!(500)));
        // Binding should persist in vars
        assert_eq!(vars.get("c"), Some(&json!(500)));
    }

    #[test]
    fn test_match_step_binding_not_set_on_mismatch() {
        // If the first arm doesn't match, its bindings shouldn't leak
        let handler = MockHandler::new();
        let step = json!({
            "kind": "match",
            "subject": "$event",
            "arms": [
                {"pattern": "{kind: \"warning\", code: $c}", "result": "$c"},
                {"pattern": "_", "result": "fallback"}
            ]
        });
        let mut vars = HashMap::new();
        vars.insert("event".into(), json!({"kind": "error", "code": 500}));
        let result = execute_step(&step, 0, &mut vars, &handler).unwrap();
        assert_eq!(result.output, Some(json!("fallback")));
        // $c should NOT be in vars since that arm didn't match
        assert_eq!(vars.get("c"), None);
    }

    #[test]
    fn test_nested_struct_binding() {
        let subject = json!({"outer": {"inner": "deep_value"}});
        let vars = HashMap::new();
        let result = try_struct_pattern(r#"{outer: {inner: $val}}"#, &subject, &vars);
        let (matched, bindings) = result.unwrap();
        assert!(matched);
        assert_eq!(bindings.get("val"), Some(&Value::String("deep_value".into())));
    }

    #[test]
    fn test_binding_with_guard() {
        // Bindings should be available in guard expressions
        let mut vars = HashMap::new();
        vars.insert("event".into(), json!({"kind": "error", "code": 500}));
        let expr = r#"match $event { {kind: "error", code: $c} if $c > 499 => "server", {kind: "error", code: $c} => "client", _ => "other" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("server".to_string()));
    }

    #[test]
    fn test_match_result_string_interpolation() {
        // Match result expressions should support ${...} interpolation
        let mut vars = HashMap::new();
        vars.insert("code".to_string(), json!(404));
        vars.insert("status".to_string(), json!("error"));
        let expr = r#"match status { "error" => "Error ${code} occurred", "ok" => "All good", _ => "unknown" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("Error 404 occurred".to_string()));
    }

    #[test]
    fn test_match_result_interpolation_with_binding() {
        // Bindings captured in the pattern should be available in interpolated results
        let mut vars = HashMap::new();
        vars.insert("event".to_string(), json!({"kind": "error", "code": 503}));
        let expr = r#"match $event { {kind: "error", code: $c} => "Error code: ${c}", _ => "ok" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("Error code: 503".to_string()));
    }

    #[test]
    fn test_match_result_interpolation_with_arithmetic() {
        // Arithmetic in interpolation within match results
        let mut vars = HashMap::new();
        vars.insert("level".to_string(), json!("critical"));
        vars.insert("count".to_string(), json!(5));
        let expr = r#"match level { "critical" => "Alert! ${count + 1} issues", _ => "fine" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("Alert! 6 issues".to_string()));
    }

    #[test]
    fn test_match_result_no_interpolation_without_dollar() {
        // Plain strings without ${} should still work normally
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), json!("a"));
        let expr = r#"match x { "a" => "just plain text", _ => "other" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("just plain text".to_string()));
    }

    #[test]
    fn test_ternary_result_string_interpolation() {
        // Ternary expression results should also support ${...} interpolation
        let mut vars = HashMap::new();
        vars.insert("active".to_string(), json!(true));
        vars.insert("name".to_string(), json!("Alice"));
        let result = try_ternary_expr(r#"active == true ? "Hello, ${name}!" : "Goodbye"
"#.trim(), &vars);
        assert_eq!(result, Some("Hello, Alice!".to_string()));
    }

    #[test]
    fn test_ternary_false_branch_interpolation() {
        let mut vars = HashMap::new();
        vars.insert("active".to_string(), json!(false));
        vars.insert("reason".to_string(), json!("timeout"));
        let result = try_ternary_expr(r#"active == true ? "ok" : "Failed: ${reason}""#, &vars);
        assert_eq!(result, Some("Failed: timeout".to_string()));
    }

    #[test]
    fn test_match_default_arm_interpolation() {
        // Default arm should also support interpolation
        let mut vars = HashMap::new();
        vars.insert("status".to_string(), json!("unknown_val"));
        vars.insert("status".to_string(), json!("weird"));
        let expr = r#"match status { "ok" => "good", _ => "Unexpected: ${status}" }"#;
        let result = try_match_expr(expr, &vars);
        assert_eq!(result, Some("Unexpected: weird".to_string()));
    }
}
