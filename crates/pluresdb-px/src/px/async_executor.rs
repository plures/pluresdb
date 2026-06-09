//! Async procedure executor — runs compiled `.px` procedures with async action handlers.
//!
//! This is the production-ready executor that supports real tool calls (shell commands,
//! HTTP requests, model invocations) which are inherently asynchronous.
//!
//! # Architecture
//!
//! The async executor mirrors the synchronous [`super::executor`] but uses
//! [`AsyncActionHandler`] which returns futures. This allows procedures to:
//!
//! - Invoke shell commands and wait for output
//! - Make HTTP/API calls
//! - Call language models
//! - Execute MCP tool calls
//!
//! The executor supports optional per-step timeouts via the `timeout_ms` field
//! on call steps.
//!
//! # Example
//!
//! ```rust,ignore
//! use pluresdb_px::px::async_executor::{AsyncActionHandler, execute_async};
//!
//! struct MyHandler;
//!
//! #[async_trait::async_trait]
//! impl AsyncActionHandler for MyHandler {
//!     async fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError> {
//!         // invoke real tools here
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::time::timeout;

use super::executor::{default_evaluate_condition, ExecutionError, ExecutionResult, StepResult};

// ── Async Action Handler Trait ────────────────────────────────────────────────

/// Async trait for handling procedure step calls.
///
/// This is the production integration point. Implementors provide async
/// side-effects (tool invocations, API calls, model calls) that procedure
/// steps reference by name.
#[async_trait]
pub trait AsyncActionHandler: Send + Sync {
    /// Execute a named action with the given parameters asynchronously.
    ///
    /// Returns a JSON value representing the result, which may be bound to
    /// an output variable for subsequent steps.
    async fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError>;

    /// Evaluate a condition expression against the current execution context.
    ///
    /// Default implementation uses the synchronous evaluator. Override for
    /// async condition evaluation (e.g., checking external state).
    fn evaluate_condition(&self, expr: &str, vars: &HashMap<String, Value>) -> bool {
        default_evaluate_condition(expr, vars)
    }

    /// Called before each step executes. Useful for logging, tracing, or
    /// implementing step-level hooks.
    async fn on_step_start(&self, _step_index: usize, _kind: &str) {}

    /// Called after each step completes. Receives the result for inspection.
    async fn on_step_complete(&self, _step_index: usize, _result: &StepResult) {}
}

/// Default step timeout (30 seconds). Individual steps can override via `timeout_ms`.
const DEFAULT_STEP_TIMEOUT_MS: u64 = 30_000;

/// Maximum loop iterations to prevent infinite loops in procedures.
const MAX_LOOP_ITERATIONS: usize = 10_000;

// ── Async Executor ────────────────────────────────────────────────────────────

/// Execute a compiled procedure record asynchronously.
///
/// This is the main entry point for running procedures with real (async) tools.
pub async fn execute_async(
    record_data: &Value,
    handler: &dyn AsyncActionHandler,
) -> Result<ExecutionResult, ExecutionError> {
    execute_async_with_vars(record_data, handler, HashMap::new()).await
}

/// Execute a compiled procedure record asynchronously with pre-seeded variables.
pub async fn execute_async_with_vars(
    record_data: &Value,
    handler: &dyn AsyncActionHandler,
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
        let kind = step
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        handler.on_step_start(index, kind).await;
        let result = execute_step_async(step, index, &mut vars, handler).await?;
        handler.on_step_complete(index, &result).await;
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

/// Execute a single step asynchronously.
fn execute_step_async<'a>(
    step: &'a Value,
    index: usize,
    vars: &'a mut HashMap<String, Value>,
    handler: &'a dyn AsyncActionHandler,
) -> Pin<Box<dyn Future<Output = Result<StepResult, ExecutionError>> + Send + 'a>> {
    Box::pin(async move {
        let kind = step
            .get("kind")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ExecutionError::InvalidStructure("step missing 'kind'".into()))?;

        match kind {
            "call" => execute_call_async(step, index, vars, handler).await,
            "match" => execute_match_async(step, index, vars, handler),
            "when" => execute_when_async(step, index, vars, handler).await,
            "loop" => execute_loop_async(step, index, vars, handler).await,
            "emit" => execute_emit_async(step, index, vars),
            "try" => execute_try_async(step, index, vars, handler).await,
            "parallel" => execute_parallel_async(step, index, vars, handler).await,
            "return" => {
                let value = step.get("value").cloned().map(|v| resolve_vars(&v, vars));
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
    })
}

/// Execute a `call` step asynchronously with optional timeout.
async fn execute_call_async(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn AsyncActionHandler,
) -> Result<StepResult, ExecutionError> {
    let name = step
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ExecutionError::InvalidStructure("call step missing 'name'".into()))?;

    let params = step.get("params").cloned().unwrap_or(Value::Null);
    let resolved_params = resolve_vars(&params, vars);

    // Check for step-level timeout
    let timeout_ms = step
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_STEP_TIMEOUT_MS);

    let output = timeout(
        Duration::from_millis(timeout_ms),
        handler.call(name, &resolved_params),
    )
    .await
    .map_err(|_| ExecutionError::ActionFailed {
        action: name.to_string(),
        message: format!("timed out after {timeout_ms}ms"),
    })??;

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

/// Execute a `match` step (synchronous — condition evaluation is sync).
fn execute_match_async(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn AsyncActionHandler,
) -> Result<StepResult, ExecutionError> {
    let arms = step
        .get("arms")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ExecutionError::InvalidStructure("match step missing 'arms'".into()))?;

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

    Ok(StepResult {
        index,
        kind: "match".into(),
        output: None,
        skipped: true,
    })
}

/// Execute a `when` step asynchronously.
async fn execute_when_async(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn AsyncActionHandler,
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

    let nested_steps = step
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ExecutionError::InvalidStructure("when step missing 'steps'".into()))?;

    let mut last_output = None;
    for (i, nested) in nested_steps.iter().enumerate() {
        let result = execute_step_async(nested, i, vars, handler).await?;
        // If a nested step was a `return` or `abort`, propagate it upward
        if result.kind == "return" || result.kind == "abort" {
            return Ok(StepResult {
                index,
                kind: result.kind,
                output: result.output,
                skipped: false,
            });
        }
        last_output = result.output;
    }

    Ok(StepResult {
        index,
        kind: "when".into(),
        output: last_output,
        skipped: false,
    })
}

/// Execute a `loop` step asynchronously.
async fn execute_loop_async(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn AsyncActionHandler,
) -> Result<StepResult, ExecutionError> {
    let nested_steps = step
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ExecutionError::InvalidStructure("loop step missing 'steps'".into()))?;

    let item_var = step.get("as").and_then(|v| v.as_str()).unwrap_or("item");

    let output_var = step.get("output_var").and_then(|v| v.as_str());

    // Determine iteration source
    let iterations: Vec<Value> = if let Some(over_ref) = step.get("over").and_then(|v| v.as_str()) {
        let var_name = over_ref.strip_prefix('$').unwrap_or(over_ref);
        match vars.get(var_name) {
            Some(Value::Array(arr)) => arr.clone(),
            Some(other) => vec![other.clone()],
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
        (0..times).map(|i| Value::Number(i.into())).collect()
    } else {
        return Err(ExecutionError::InvalidStructure(
            "loop step requires 'over' or 'times'".into(),
        ));
    };

    // Guard against infinite loops
    if iterations.len() > MAX_LOOP_ITERATIONS {
        return Err(ExecutionError::ActionFailed {
            action: "loop".into(),
            message: format!(
                "loop iteration count {} exceeds maximum {}",
                iterations.len(),
                MAX_LOOP_ITERATIONS
            ),
        });
    }

    let mut results: Vec<Value> = Vec::new();

    for (iter_index, item) in iterations.into_iter().enumerate() {
        vars.insert(item_var.to_string(), item);
        vars.insert("index".to_string(), Value::Number(iter_index.into()));

        for nested in nested_steps {
            let result = execute_step_async(nested, iter_index, vars, handler).await?;
            if let Some(output) = &result.output {
                results.push(output.clone());
            }
        }
    }

    // Clean up loop variables
    vars.remove(item_var);
    vars.remove("index");

    let output = Value::Array(results);

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

/// Execute an `emit` step (synchronous — just appends to variables).
fn execute_emit_async(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
) -> Result<StepResult, ExecutionError> {
    let event_data = step
        .get("event")
        .cloned()
        .ok_or_else(|| ExecutionError::InvalidStructure("emit step missing 'event'".into()))?;

    let resolved = resolve_vars(&event_data, vars);

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

/// Execute a `try` step asynchronously with error recovery and optional retry.
///
/// Retry fields:
/// - `retry`: max number of additional attempts after the first failure (default 0).
/// - `retry_delay_ms`: milliseconds to sleep between retry attempts (default 0).
/// - `$retry_count` variable is injected so steps/catch can inspect the attempt number.
async fn execute_try_async(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn AsyncActionHandler,
) -> Result<StepResult, ExecutionError> {
    let try_steps = step
        .get("steps")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ExecutionError::InvalidStructure("try step missing 'steps'".into()))?;

    let catch_steps = step.get("catch").and_then(|v| v.as_array());

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
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }

        let mut last_output = None;
        let mut failed = false;

        for (i, nested) in try_steps.iter().enumerate() {
            match execute_step_async(nested, i, vars, handler).await {
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
            vars.remove("error");
            vars.remove("retry_count");
            return Ok(StepResult {
                index,
                kind: "try".into(),
                output: last_output,
                skipped: false,
            });
        }

        // If we have retries left, try again
        if attempt < max_retries {
            continue;
        }

        // All retries exhausted — run catch or return error
        let err = last_err.take().unwrap();
        vars.insert("error".to_string(), Value::String(err.to_string()));

        if let Some(catch) = catch_steps {
            let mut catch_output = None;
            for (j, catch_step) in catch.iter().enumerate() {
                let result = execute_step_async(catch_step, j, vars, handler).await?;
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

/// Execute a `parallel` step with true concurrent execution via `futures::future::join_all`.
///
/// Each branch runs as a concurrent future with its own isolated copy of the
/// variable bindings. The handler is shared immutably (`&dyn AsyncActionHandler`
/// is `Send + Sync`). Results are collected into a map keyed by branch name.
/// All branches must complete (or fail) before the step returns.
///
/// # Timeouts
///
/// - **Step-level** `timeout_ms`: applies as a global deadline for *all* branches.
///   If any branch exceeds this, the entire parallel step fails with a timeout error.
/// - **Per-branch** `timeout_ms`: each branch can specify its own timeout. If a branch
///   exceeds its timeout, that branch returns an error. The `fail_strategy` field
///   controls whether this aborts the entire step (`"fast"`, default) or other branches
///   still report their results (`"complete"`).
///
/// This achieves real concurrency without requiring `'static` bounds — all
/// futures share the parent task's lifetime through the borrowed handler.
async fn execute_parallel_async(
    step: &Value,
    index: usize,
    vars: &mut HashMap<String, Value>,
    handler: &dyn AsyncActionHandler,
) -> Result<StepResult, ExecutionError> {
    use futures::future::join_all;

    let branches = step
        .get("branches")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            ExecutionError::InvalidStructure("parallel step missing 'branches'".into())
        })?;

    let output_var = step.get("output_var").and_then(|v| v.as_str());

    // Step-level timeout applies to all branches as a group.
    let step_timeout_ms = step
        .get("timeout_ms")
        .and_then(|v| v.as_u64());

    // Fail strategy: "fast" (default) aborts on first error, "complete" collects all.
    let fail_strategy = step
        .get("fail_strategy")
        .and_then(|v| v.as_str())
        .unwrap_or("fast");

    // Build concurrent futures for each branch.
    // Each branch gets its own vars clone (isolation) but shares the handler reference.
    let branch_futs: Vec<_> = branches
        .iter()
        .map(|branch| {
            let branch_name = branch
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unnamed")
                .to_string();

            let branch_steps = branch
                .get("steps")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let branch_timeout_ms = branch
                .get("timeout_ms")
                .and_then(|v| v.as_u64());

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
                .unwrap_or("fixed")
                .to_string();
            let branch_retry_max_delay_ms = branch
                .get("retry_max_delay_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(30_000);
            let branch_retry_jitter = branch
                .get("retry_jitter")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let mut branch_vars = vars.clone();

            async move {
                // Validate structure
                if branch.get("name").and_then(|v| v.as_str()).is_none() {
                    return Err(ExecutionError::InvalidStructure(
                        "parallel branch missing 'name'".into(),
                    ));
                }
                if branch.get("steps").and_then(|v| v.as_array()).is_none() {
                    return Err(ExecutionError::InvalidStructure(
                        "parallel branch missing 'steps'".into(),
                    ));
                }

                // Retry loop for this branch
                let execute_with_retry = async {
                    let mut last_err = None;
                    for attempt in 0..=(branch_retry) {
                        // Delay before retry (not before first attempt)
                        if attempt > 0 && branch_retry_delay_ms > 0 {
                            let base_delay = match branch_retry_backoff.as_str() {
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
                            tokio::time::sleep(Duration::from_millis(delay)).await;
                        }

                        branch_vars.insert(
                            "retry_count".to_string(),
                            Value::Number(attempt.into()),
                        );

                        let mut attempt_vars = branch_vars.clone();
                        let mut last_output = Value::Null;
                        let mut success = true;
                        for (i, nested) in branch_steps.iter().enumerate() {
                            match execute_step_async(nested, i, &mut attempt_vars, handler).await {
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
                            return Ok::<(String, Value), ExecutionError>((
                                branch_name.clone(),
                                last_output,
                            ));
                        }
                    }

                    // All retries exhausted
                    Err(last_err.unwrap_or_else(|| ExecutionError::ActionFailed {
                        action: branch_name.clone(),
                        message: "branch failed after retries".into(),
                    }))
                };

                // Apply per-branch timeout if specified
                if let Some(ms) = branch_timeout_ms {
                    match timeout(Duration::from_millis(ms), execute_with_retry).await {
                        Ok(result) => result,
                        Err(_elapsed) => Err(ExecutionError::ActionFailed {
                            action: branch_name.clone(),
                            message: format!(
                                "parallel branch '{}' timed out after {}ms",
                                branch_name, ms
                            ),
                        }),
                    }
                } else {
                    execute_with_retry.await
                }
            }
        })
        .collect();

    // Execute all branches concurrently, optionally with a step-level timeout.
    let branch_results = if let Some(ms) = step_timeout_ms {
        match timeout(Duration::from_millis(ms), join_all(branch_futs)).await {
            Ok(results) => results,
            Err(_elapsed) => {
                return Err(ExecutionError::ActionFailed {
                    action: "parallel".into(),
                    message: format!("parallel step timed out after {ms}ms"),
                });
            }
        }
    } else {
        join_all(branch_futs).await
    };

    // Collect results based on fail strategy.
    let mut results_map = serde_json::Map::new();
    let mut first_error: Option<ExecutionError> = None;

    for result in branch_results {
        match result {
            Ok((name, output)) => {
                results_map.insert(name, output);
            }
            Err(e) => {
                if fail_strategy == "fast" {
                    return Err(e);
                }
                // "complete" strategy: record error as a value, continue collecting
                if first_error.is_none() {
                    first_error = Some(e.clone());
                }
                let error_name = match &e {
                    ExecutionError::ActionFailed { action, .. } => action.clone(),
                    _ => "unknown".to_string(),
                };
                results_map.insert(
                    error_name,
                    Value::String(format!("error: {}", e)),
                );
            }
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
fn resolve_vars(value: &Value, vars: &HashMap<String, Value>) -> Value {
    match value {
        Value::String(s) if s.starts_with('$') => {
            let var_name = &s[1..];
            vars.get(var_name).cloned().unwrap_or_else(|| value.clone())
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

// ── Adapter: Wrap sync handler as async ───────────────────────────────────────

use super::executor::ActionHandler;

/// Wraps a synchronous [`ActionHandler`] into an [`AsyncActionHandler`].
///
/// Useful for testing or when all actions are CPU-bound.
pub struct SyncAdapter<H: ActionHandler>(pub H);

#[async_trait]
impl<H: ActionHandler + 'static> AsyncActionHandler for SyncAdapter<H> {
    async fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError> {
        self.0.call(name, params)
    }

    fn evaluate_condition(&self, expr: &str, vars: &HashMap<String, Value>) -> bool {
        self.0.evaluate_condition(expr, vars)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Mock async handler for testing.
    struct MockAsyncHandler {
        results: HashMap<String, Value>,
    }

    impl MockAsyncHandler {
        fn new() -> Self {
            Self {
                results: HashMap::new(),
            }
        }

        fn with_result(mut self, name: &str, value: Value) -> Self {
            self.results.insert(name.to_string(), value);
            self
        }
    }

    #[async_trait]
    impl AsyncActionHandler for MockAsyncHandler {
        async fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
            self.results
                .get(name)
                .cloned()
                .ok_or_else(|| ExecutionError::UnknownAction(name.to_string()))
        }
    }

    #[tokio::test]
    async fn execute_simple_procedure() {
        let handler = MockAsyncHandler::new().with_result("greet", json!("hello"));

        let procedure = json!({
            "type": "procedure",
            "name": "test_proc",
            "steps": [
                { "kind": "call", "name": "greet", "params": {} , "output_var": "result" }
            ]
        });

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        assert_eq!(result.procedure_name, "test_proc");
        assert_eq!(result.variables.get("result"), Some(&json!("hello")));
    }

    #[tokio::test]
    async fn execute_with_timeout() {
        struct SlowHandler;

        #[async_trait]
        impl AsyncActionHandler for SlowHandler {
            async fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                tokio::time::sleep(Duration::from_secs(5)).await;
                Ok(json!("too late"))
            }
        }

        let procedure = json!({
            "type": "procedure",
            "name": "timeout_test",
            "steps": [
                { "kind": "call", "name": "slow_action", "params": {}, "timeout_ms": 50 }
            ]
        });

        let result = execute_async(&procedure, &SlowHandler).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutionError::ActionFailed { action, message } => {
                assert_eq!(action, "slow_action");
                assert!(message.contains("timed out"));
            }
            other => panic!("expected ActionFailed, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn execute_loop_with_async_calls() {
        let handler = MockAsyncHandler::new()
            .with_result("get_items", json!(["a", "b", "c"]))
            .with_result("process", json!("done"));

        let procedure = json!({
            "type": "procedure",
            "name": "loop_test",
            "steps": [
                { "kind": "call", "name": "get_items", "params": {}, "output_var": "items" },
                {
                    "kind": "loop",
                    "over": "$items",
                    "as": "item",
                    "output_var": "results",
                    "steps": [
                        { "kind": "call", "name": "process", "params": { "val": "$item" } }
                    ]
                }
            ]
        });

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        assert_eq!(
            result.variables.get("results"),
            Some(&json!(["done", "done", "done"]))
        );
    }

    #[tokio::test]
    async fn execute_try_catch_async() {
        let handler = MockAsyncHandler::new().with_result("fallback", json!("recovered"));

        let procedure = json!({
            "type": "procedure",
            "name": "try_test",
            "steps": [
                {
                    "kind": "try",
                    "steps": [
                        { "kind": "call", "name": "nonexistent", "params": {} }
                    ],
                    "catch": [
                        { "kind": "call", "name": "fallback", "params": {} }
                    ]
                }
            ]
        });

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        assert_eq!(result.step_results[0].output, Some(json!("recovered")));
    }

    #[tokio::test]
    async fn execute_when_condition() {
        let handler = MockAsyncHandler::new().with_result("action_a", json!("a_result"));

        let procedure = json!({
            "type": "procedure",
            "name": "when_test",
            "steps": [
                {
                    "kind": "when",
                    "condition": "mode == fast",
                    "steps": [
                        { "kind": "call", "name": "action_a", "params": {} }
                    ]
                }
            ]
        });

        // Without the variable set — should skip
        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.step_results[0].skipped);

        // With the variable set — should execute
        let mut vars = HashMap::new();
        vars.insert("mode".to_string(), json!("fast"));
        let result = execute_async_with_vars(&procedure, &handler, vars)
            .await
            .unwrap();
        assert!(!result.step_results[0].skipped);
        assert_eq!(result.step_results[0].output, Some(json!("a_result")));
    }

    #[tokio::test]
    async fn sync_adapter_works() {
        use super::super::executor::ActionHandler;

        struct SyncHandler;
        impl ActionHandler for SyncHandler {
            fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                match name {
                    "ping" => Ok(json!("pong")),
                    _ => Err(ExecutionError::UnknownAction(name.to_string())),
                }
            }
        }

        let handler = SyncAdapter(SyncHandler);

        let procedure = json!({
            "type": "procedure",
            "name": "adapter_test",
            "steps": [
                { "kind": "call", "name": "ping", "params": {}, "output_var": "reply" }
            ]
        });

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        assert_eq!(result.variables.get("reply"), Some(&json!("pong")));
    }

    #[tokio::test]
    async fn loop_guard_prevents_excessive_iterations() {
        let handler = MockAsyncHandler::new().with_result("noop", json!(null));

        let procedure = json!({
            "type": "procedure",
            "name": "bomb",
            "steps": [
                {
                    "kind": "loop",
                    "times": 100_001,
                    "as": "i",
                    "steps": [
                        { "kind": "call", "name": "noop", "params": {} }
                    ]
                }
            ]
        });

        let result = execute_async(&procedure, &handler).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutionError::ActionFailed { message, .. } => {
                assert!(message.contains("exceeds maximum"));
            }
            other => panic!("expected ActionFailed, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn execute_parallel_branches_async() {
        let handler = MockAsyncHandler::new()
            .with_result("fetch_a", json!("alpha_result"))
            .with_result("fetch_b", json!("beta_result"));

        let procedure = json!({
            "type": "procedure",
            "name": "parallel_async_test",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        {
                            "name": "a",
                            "steps": [
                                { "kind": "call", "name": "fetch_a", "params": {} }
                            ]
                        },
                        {
                            "name": "b",
                            "steps": [
                                { "kind": "call", "name": "fetch_b", "params": {} }
                            ]
                        }
                    ],
                    "output_var": "par_out"
                }
            ]
        });

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        let par_out = result.variables.get("par_out").unwrap();
        assert_eq!(par_out["a"], json!("alpha_result"));
        assert_eq!(par_out["b"], json!("beta_result"));
    }

    #[tokio::test]
    async fn execute_parallel_multi_step_branches() {
        let handler = MockAsyncHandler::new()
            .with_result("step1", json!("s1"))
            .with_result("step2", json!("s2"))
            .with_result("step3", json!("s3"));

        let procedure = json!({
            "type": "procedure",
            "name": "multi_step_parallel",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        {
                            "name": "pipeline_a",
                            "steps": [
                                { "kind": "call", "name": "step1", "params": {}, "output_var": "r1" },
                                { "kind": "call", "name": "step2", "params": { "prev": "$r1" } }
                            ]
                        },
                        {
                            "name": "pipeline_b",
                            "steps": [
                                { "kind": "call", "name": "step3", "params": {} }
                            ]
                        }
                    ],
                    "output_var": "results"
                }
            ]
        });

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        let results = result.variables.get("results").unwrap();
        // pipeline_a: last output is step2's result
        assert_eq!(results["pipeline_a"], json!("s2"));
        assert_eq!(results["pipeline_b"], json!("s3"));
    }

    /// Proves that parallel branches execute concurrently (not sequentially).
    /// If branches ran sequentially, 3 branches each sleeping 100ms would take ~300ms.
    /// With true concurrency via join_all, they complete in ~100ms.
    #[tokio::test]
    async fn parallel_branches_run_concurrently() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Instant;

        struct TimedHandler {
            call_count: AtomicUsize,
        }

        #[async_trait]
        impl AsyncActionHandler for TimedHandler {
            async fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(json!("done"))
            }
        }

        let handler = TimedHandler {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "concurrency_proof",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        { "name": "a", "steps": [{ "kind": "call", "name": "slow", "params": {} }] },
                        { "name": "b", "steps": [{ "kind": "call", "name": "slow", "params": {} }] },
                        { "name": "c", "steps": [{ "kind": "call", "name": "slow", "params": {} }] }
                    ],
                    "output_var": "results"
                }
            ]
        });

        let start = Instant::now();
        let result = execute_async(&procedure, &handler).await.unwrap();
        let elapsed = start.elapsed();

        assert!(result.success);
        assert_eq!(handler.call_count.load(Ordering::SeqCst), 3);

        // If sequential: ~300ms. If concurrent: ~100ms.
        // Allow generous margin (200ms) but reject sequential timing.
        assert!(
            elapsed < Duration::from_millis(200),
            "parallel branches took {:?} — expected < 200ms for concurrent execution",
            elapsed
        );
    }

    /// Verifies that a failing branch in parallel propagates its error.
    #[tokio::test]
    async fn parallel_branch_error_propagates() {
        struct FailOnB;

        #[async_trait]
        impl AsyncActionHandler for FailOnB {
            async fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                if name == "fail" {
                    Err(ExecutionError::ActionFailed {
                        action: "fail".into(),
                        message: "intentional failure".into(),
                    })
                } else {
                    Ok(json!("ok"))
                }
            }
        }

        let procedure = json!({
            "type": "procedure",
            "name": "error_test",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        { "name": "good", "steps": [{ "kind": "call", "name": "ok_action", "params": {} }] },
                        { "name": "bad", "steps": [{ "kind": "call", "name": "fail", "params": {} }] }
                    ],
                    "output_var": "results"
                }
            ]
        });

        let result = execute_async(&procedure, &FailOnB).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutionError::ActionFailed { action, message } => {
                assert_eq!(action, "fail");
                assert!(message.contains("intentional"));
            }
            other => panic!("expected ActionFailed, got: {:?}", other),
        }
    }

    /// Per-branch timeout: a slow branch is killed while fast branches succeed.
    #[tokio::test]
    async fn parallel_branch_timeout() {
        struct SlowOnName;

        #[async_trait]
        impl AsyncActionHandler for SlowOnName {
            async fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                if name == "slow" {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Ok(json!(format!("{}_done", name)))
            }
        }

        let procedure = json!({
            "type": "procedure",
            "name": "branch_timeout_test",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        {
                            "name": "fast_branch",
                            "steps": [{ "kind": "call", "name": "fast", "params": {} }]
                        },
                        {
                            "name": "slow_branch",
                            "timeout_ms": 50,
                            "steps": [{ "kind": "call", "name": "slow", "params": {} }]
                        }
                    ],
                    "output_var": "results"
                }
            ]
        });

        // Default fail_strategy is "fast" — the timeout error propagates
        let result = execute_async(&procedure, &SlowOnName).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            ExecutionError::ActionFailed { action, message } => {
                assert_eq!(action, "slow_branch");
                assert!(message.contains("timed out after 50ms"), "msg: {}", message);
            }
            other => panic!("expected ActionFailed, got: {:?}", other),
        }
    }

    /// Per-branch timeout with fail_strategy="complete" — timed-out branch is recorded
    /// but other branches still return their results.
    #[tokio::test]
    async fn parallel_branch_timeout_complete_strategy() {
        struct SlowOnName;

        #[async_trait]
        impl AsyncActionHandler for SlowOnName {
            async fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                if name == "slow" {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Ok(json!(format!("{}_done", name)))
            }
        }

        let procedure = json!({
            "type": "procedure",
            "name": "branch_timeout_complete",
            "steps": [
                {
                    "kind": "parallel",
                    "fail_strategy": "complete",
                    "branches": [
                        {
                            "name": "fast_branch",
                            "steps": [{ "kind": "call", "name": "fast", "params": {} }]
                        },
                        {
                            "name": "slow_branch",
                            "timeout_ms": 50,
                            "steps": [{ "kind": "call", "name": "slow", "params": {} }]
                        }
                    ],
                    "output_var": "results"
                }
            ]
        });

        // "complete" strategy: step succeeds, timed-out branch recorded as error string
        let result = execute_async(&procedure, &SlowOnName).await.unwrap();
        assert!(result.success);
        let results = result.variables.get("results").unwrap();
        assert_eq!(results["fast_branch"], json!("fast_done"));
        // slow_branch timed out — its error is keyed by the action name from the error
        let slow_val = results["slow_branch"].as_str().unwrap();
        assert!(slow_val.contains("timed out"), "val: {}", slow_val);
    }

    /// Step-level timeout on parallel: all branches are killed if step deadline exceeded.
    #[tokio::test]
    async fn parallel_step_timeout() {
        struct AlwaysSlow;

        #[async_trait]
        impl AsyncActionHandler for AlwaysSlow {
            async fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok(json!("done"))
            }
        }

        let procedure = json!({
            "type": "procedure",
            "name": "step_timeout_test",
            "steps": [
                {
                    "kind": "parallel",
                    "timeout_ms": 50,
                    "branches": [
                        { "name": "a", "steps": [{ "kind": "call", "name": "x", "params": {} }] },
                        { "name": "b", "steps": [{ "kind": "call", "name": "y", "params": {} }] }
                    ]
                }
            ]
        });

        let result = execute_async(&procedure, &AlwaysSlow).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutionError::ActionFailed { action, message } => {
                assert_eq!(action, "parallel");
                assert!(message.contains("timed out after 50ms"), "msg: {}", message);
            }
            other => panic!("expected ActionFailed, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn step_hooks_called() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct HookHandler {
            start_count: AtomicUsize,
            complete_count: AtomicUsize,
        }

        #[async_trait]
        impl AsyncActionHandler for HookHandler {
            async fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                Ok(json!("ok"))
            }

            async fn on_step_start(&self, _index: usize, _kind: &str) {
                self.start_count.fetch_add(1, Ordering::SeqCst);
            }

            async fn on_step_complete(&self, _index: usize, _result: &StepResult) {
                self.complete_count.fetch_add(1, Ordering::SeqCst);
            }
        }

        let handler = HookHandler {
            start_count: AtomicUsize::new(0),
            complete_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "hooks_test",
            "steps": [
                { "kind": "call", "name": "a", "params": {} },
                { "kind": "call", "name": "b", "params": {} },
                { "kind": "call", "name": "c", "params": {} }
            ]
        });

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        assert_eq!(handler.start_count.load(Ordering::SeqCst), 3);
        assert_eq!(handler.complete_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn try_retry_succeeds_on_second_attempt_async() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct FlakeAsyncHandler {
            call_count: AtomicUsize,
        }

        #[async_trait]
        impl AsyncActionHandler for FlakeAsyncHandler {
            async fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
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

        let handler = FlakeAsyncHandler {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "retry_async_test",
            "steps": [
                {
                    "kind": "try",
                    "retry": 2,
                    "retry_delay_ms": 10,
                    "steps": [
                        { "kind": "call", "name": "flaky", "params": {}, "output_var": "result" }
                    ],
                    "catch": [
                        { "kind": "emit", "event": "should_not_reach" }
                    ]
                }
            ]
        });

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        assert_eq!(result.variables.get("result"), Some(&json!("success_on_retry")));
        assert!(!result.variables.contains_key("retry_count"));
        assert!(!result.variables.contains_key("error"));
        assert_eq!(handler.call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn try_retry_exhausted_runs_catch_async() {
        let handler = MockAsyncHandler::new().with_result("fallback", json!("caught_after_retries"));

        let procedure = json!({
            "type": "procedure",
            "name": "retry_exhausted_async",
            "steps": [
                {
                    "kind": "try",
                    "retry": 3,
                    "retry_delay_ms": 5,
                    "steps": [
                        { "kind": "call", "name": "nonexistent", "params": {} }
                    ],
                    "catch": [
                        { "kind": "call", "name": "fallback", "params": {} }
                    ]
                }
            ]
        });

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        assert_eq!(result.step_results[0].output, Some(json!("caught_after_retries")));
        assert!(result.variables.contains_key("error"));
    }

    #[tokio::test]
    async fn try_retry_with_delay_is_observable() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Instant;

        struct AlwaysFailHandler {
            call_count: AtomicUsize,
        }

        #[async_trait]
        impl AsyncActionHandler for AlwaysFailHandler {
            async fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                Err(ExecutionError::ActionFailed {
                    action: "fail".into(),
                    message: "always fails".into(),
                })
            }
        }

        let handler = AlwaysFailHandler {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "delay_test",
            "steps": [
                {
                    "kind": "try",
                    "retry": 2,
                    "retry_delay_ms": 50,
                    "steps": [
                        { "kind": "call", "name": "fail_action", "params": {} }
                    ]
                }
            ]
        });

        let start = Instant::now();
        let result = execute_async(&procedure, &handler).await.unwrap();
        let elapsed = start.elapsed();

        assert!(result.success); // try swallows the error
        // 3 attempts total (1 initial + 2 retries), 2 delays of 50ms each = ~100ms minimum
        assert!(
            elapsed >= Duration::from_millis(80),
            "expected >= 80ms for retry delays, got {:?}",
            elapsed
        );
        assert_eq!(handler.call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn try_retry_exponential_backoff_async() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Instant;

        struct FlakeHandler {
            call_count: AtomicUsize,
        }

        #[async_trait]
        impl AsyncActionHandler for FlakeHandler {
            async fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count < 3 {
                    Err(ExecutionError::ActionFailed {
                        action: "flaky".into(),
                        message: format!("fail #{}", count),
                    })
                } else {
                    Ok(json!("recovered_async"))
                }
            }
        }

        let handler = FlakeHandler {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "exp_backoff_async_test",
            "steps": [
                {
                    "kind": "try",
                    "retry": 4,
                    "retry_delay_ms": 10,
                    "retry_backoff": "exponential",
                    "retry_max_delay_ms": 30,
                    "steps": [
                        { "kind": "call", "name": "flaky", "params": {} }
                    ]
                }
            ]
        });

        let start = Instant::now();
        let result = execute_async(&procedure, &handler).await.unwrap();
        let elapsed = start.elapsed();

        assert!(result.success);
        assert_eq!(result.step_results[0].output, Some(json!("recovered_async")));
        // Exponential with cap: 10 + 20 + 30 = 60ms (3rd attempt would be 40 but capped at 30)
        assert!(
            elapsed >= Duration::from_millis(50),
            "expected >= 50ms for exponential backoff, got {:?}",
            elapsed
        );
        assert_eq!(handler.call_count.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn try_retry_with_jitter_async() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct FlakeHandler {
            call_count: AtomicUsize,
        }

        #[async_trait::async_trait]
        impl AsyncActionHandler for FlakeHandler {
            async fn call(&self, _name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(ExecutionError::ActionFailed {
                        action: "flaky".into(),
                        message: format!("fail #{}", count),
                    })
                } else {
                    Ok(json!("jittered_async"))
                }
            }
        }

        let handler = FlakeHandler {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "jitter_async_test",
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

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        assert_eq!(result.step_results[0].output, Some(json!("jittered_async")));
    }

    #[tokio::test]
    async fn parallel_branch_retry_succeeds_after_failures() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// Handler that fails the first N calls for a given name, then succeeds.
        struct FailThenSucceedHandler {
            fail_count: AtomicUsize,
            fail_until: usize,
        }

        #[async_trait]
        impl AsyncActionHandler for FailThenSucceedHandler {
            async fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                if name == "flaky" {
                    let count = self.fail_count.fetch_add(1, Ordering::SeqCst);
                    if count < self.fail_until {
                        return Err(ExecutionError::ActionFailed {
                            action: "flaky".into(),
                            message: format!("attempt {} failed", count),
                        });
                    }
                }
                Ok(json!(format!("{}_ok", name)))
            }
        }

        let handler = FailThenSucceedHandler {
            fail_count: AtomicUsize::new(0),
            fail_until: 2, // fail first 2, succeed on 3rd
        };

        let procedure = json!({
            "type": "procedure",
            "name": "branch_retry_test",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        {
                            "name": "reliable",
                            "steps": [
                                { "kind": "call", "name": "stable", "params": {} }
                            ]
                        },
                        {
                            "name": "flaky_branch",
                            "retry": 3,
                            "retry_delay_ms": 10,
                            "steps": [
                                { "kind": "call", "name": "flaky", "params": {} }
                            ]
                        }
                    ],
                    "output_var": "results"
                }
            ]
        });

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        let results = result.variables.get("results").unwrap();
        assert_eq!(results["reliable"], json!("stable_ok"));
        assert_eq!(results["flaky_branch"], json!("flaky_ok"));
        // Confirm 3 calls to flaky (2 failures + 1 success)
        assert_eq!(handler.fail_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn parallel_branch_retry_exhausted_with_fail_fast() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// Handler that always fails for "always_fail".
        struct AlwaysFailHandler {
            call_count: AtomicUsize,
        }

        #[async_trait]
        impl AsyncActionHandler for AlwaysFailHandler {
            async fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                if name == "always_fail" {
                    self.call_count.fetch_add(1, Ordering::SeqCst);
                    return Err(ExecutionError::ActionFailed {
                        action: "always_fail".into(),
                        message: "permanent failure".into(),
                    });
                }
                Ok(json!("ok"))
            }
        }

        let handler = AlwaysFailHandler {
            call_count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "branch_retry_exhausted_test",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        {
                            "name": "ok_branch",
                            "steps": [
                                { "kind": "call", "name": "good", "params": {} }
                            ]
                        },
                        {
                            "name": "bad_branch",
                            "retry": 2,
                            "retry_delay_ms": 5,
                            "retry_backoff": "exponential",
                            "steps": [
                                { "kind": "call", "name": "always_fail", "params": {} }
                            ]
                        }
                    ],
                    "output_var": "results",
                    "fail_strategy": "fast"
                }
            ]
        });

        let result = execute_async(&procedure, &handler).await;
        assert!(result.is_err());
        // 3 total calls: initial + 2 retries
        assert_eq!(handler.call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn parallel_branch_retry_with_jitter() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountHandler {
            count: AtomicUsize,
        }

        #[async_trait]
        impl AsyncActionHandler for CountHandler {
            async fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
                let c = self.count.fetch_add(1, Ordering::SeqCst);
                if name == "flaky" && c < 1 {
                    return Err(ExecutionError::ActionFailed {
                        action: "flaky".into(),
                        message: "transient".into(),
                    });
                }
                Ok(json!("recovered"))
            }
        }

        let handler = CountHandler {
            count: AtomicUsize::new(0),
        };

        let procedure = json!({
            "type": "procedure",
            "name": "branch_jitter_test",
            "steps": [
                {
                    "kind": "parallel",
                    "branches": [
                        {
                            "name": "jittery",
                            "retry": 2,
                            "retry_delay_ms": 50,
                            "retry_backoff": "exponential",
                            "retry_max_delay_ms": 200,
                            "retry_jitter": true,
                            "steps": [
                                { "kind": "call", "name": "flaky", "params": {} }
                            ]
                        }
                    ],
                    "output_var": "out"
                }
            ]
        });

        let result = execute_async(&procedure, &handler).await.unwrap();
        assert!(result.success);
        let out = result.variables.get("out").unwrap();
        assert_eq!(out["jittery"], json!("recovered"));
    }
}
