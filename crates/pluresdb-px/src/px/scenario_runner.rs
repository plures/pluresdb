//! Scenario runner — executes compiled `.px` scenarios for testing.
//!
//! A scenario defines: setup → run procedure → check expectations.
//! This module provides a sync runner that leverages the existing
//! procedure executor infrastructure.
//!
//! # Architecture
//!
//! ```text
//! PxScenario → compile_scenario → CompiledRecord (JSON)
//!                                       ↓
//!                              run_scenario()
//!                                       ↓
//!                         1. Execute setup steps
//!                         2. Run named procedure
//!                         3. Check expectations
//!                                       ↓
//!                              ScenarioResult
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::executor::{ActionHandler, ExecutionError, execute_with_vars};

// ── Types ─────────────────────────────────────────────────────────────────────

/// Result of running a single scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    /// Scenario name.
    pub name: String,
    /// Human-readable description (from `given:`).
    pub given: Option<String>,
    /// Whether all expectations passed.
    pub passed: bool,
    /// Individual expectation results.
    pub expectations: Vec<ExpectationResult>,
    /// Error if the scenario failed to execute (setup/run failure).
    pub error: Option<String>,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

/// Result of checking a single expectation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectationResult {
    /// The expectation check name (e.g. "has_entry", "event_emitted").
    pub check: String,
    /// Parameters passed to the check.
    pub params: Value,
    /// Whether this was negated (NOT).
    pub negated: bool,
    /// Whether the expectation passed.
    pub passed: bool,
    /// Reason for failure (if failed).
    pub reason: Option<String>,
}

/// Aggregate result of running multiple scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioSuiteResult {
    /// Source file (if known).
    pub source: Option<String>,
    /// Individual scenario results.
    pub results: Vec<ScenarioResult>,
    /// Total scenarios.
    pub total: usize,
    /// Passed count.
    pub passed: usize,
    /// Failed count.
    pub failed: usize,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

// ── Expectation Checker Trait ─────────────────────────────────────────────────

/// Trait for evaluating scenario expectations against post-execution state.
///
/// Implementors provide domain-specific checks like `has_entry`, `event_emitted`,
/// `constraint_violated`, etc. The scenario runner calls these after executing
/// setup + procedure.
pub trait ExpectationChecker: Send + Sync {
    /// Check whether an expectation is satisfied.
    ///
    /// Returns `Ok(true)` if the condition holds, `Ok(false)` if it doesn't,
    /// or `Err(reason)` if the check itself failed (e.g. unknown check name).
    fn check(&self, name: &str, params: &Value, state: &ExecutionState) -> Result<bool, String>;

    /// List available check names (for error messages / discovery).
    fn available_checks(&self) -> Vec<&str> {
        vec![]
    }
}

/// Post-execution state available to expectation checkers.
#[derive(Debug, Clone, Default)]
pub struct ExecutionState {
    /// Variable bindings from procedure execution.
    pub variables: HashMap<String, Value>,
    /// Events emitted during execution.
    pub emitted_events: Vec<Value>,
    /// Constraint violations triggered during execution.
    pub constraint_violations: Vec<String>,
    /// Entries in the simulated store (key → value).
    pub store: HashMap<String, Value>,
}

// ── Built-in Expectation Checker ──────────────────────────────────────────────

/// Default expectation checker that handles common built-in checks.
pub struct BuiltinChecker;

impl ExpectationChecker for BuiltinChecker {
    fn check(&self, name: &str, params: &Value, state: &ExecutionState) -> Result<bool, String> {
        match name {
            "has_entry" => {
                let key = params
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "has_entry requires 'key' param".to_string())?;
                Ok(state.store.contains_key(key))
            }
            "event_emitted" => {
                let event_name = params
                    .get("event")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "event_emitted requires 'event' param".to_string())?;
                let matched = state.emitted_events.iter().any(|e| {
                    if let Some(ev) = e.get("event").and_then(|v| v.as_str()) {
                        if ev != event_name {
                            return false;
                        }
                    } else {
                        return false;
                    }
                    // If additional params given, all must match
                    if let Some(obj) = params.as_object() {
                        for (k, v) in obj {
                            if k == "event" {
                                continue;
                            }
                            if e.get(k) != Some(v) {
                                return false;
                            }
                        }
                    }
                    true
                });
                Ok(matched)
            }
            "constraint_violated" => {
                let constraint_name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "constraint_violated requires 'name' param".to_string())?;
                Ok(state
                    .constraint_violations
                    .contains(&constraint_name.to_string()))
            }
            "var_equals" => {
                let var = params
                    .get("var")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "var_equals requires 'var' param".to_string())?;
                let expected = params
                    .get("value")
                    .ok_or_else(|| "var_equals requires 'value' param".to_string())?;
                match state.variables.get(var) {
                    Some(actual) => Ok(actual == expected),
                    None => Ok(false),
                }
            }
            "store_value" => {
                let key = params
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "store_value requires 'key' param".to_string())?;
                let expected = params
                    .get("value")
                    .ok_or_else(|| "store_value requires 'value' param".to_string())?;
                match state.store.get(key) {
                    Some(actual) => Ok(actual == expected),
                    None => Ok(false),
                }
            }
            "is_healthy" => Ok(true), // stub for simple health checks
            other => Err(format!("unknown expectation check: '{other}'")),
        }
    }

    fn available_checks(&self) -> Vec<&str> {
        vec![
            "has_entry",
            "event_emitted",
            "constraint_violated",
            "var_equals",
            "store_value",
            "is_healthy",
        ]
    }
}

// ── Scenario Action Handler ───────────────────────────────────────────────────

/// An ActionHandler that captures state for scenario testing.
///
/// Intercepts emits and store operations to build the ExecutionState
/// for expectation checking.
pub struct ScenarioActionHandler {
    state: std::sync::Mutex<ExecutionState>,
}

impl ScenarioActionHandler {
    /// Create a new scenario action handler.
    pub fn new() -> Self {
        Self {
            state: std::sync::Mutex::new(ExecutionState::default()),
        }
    }

    /// Extract the captured execution state.
    pub fn into_state(self) -> ExecutionState {
        self.state.into_inner().unwrap_or_default()
    }
}

impl Default for ScenarioActionHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionHandler for ScenarioActionHandler {
    fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError> {
        match name {
            // Intercept emit calls
            "emit" => {
                let mut state = self.state.lock().unwrap();
                state.emitted_events.push(params.clone());
                Ok(Value::Null)
            }
            // Intercept store operations
            "put_entry" | "put" => {
                let mut state = self.state.lock().unwrap();
                if let Some(key) = params.get("key").and_then(|v| v.as_str()) {
                    let value = params.get("value").cloned().unwrap_or(params.clone());
                    state.store.insert(key.to_string(), value);
                }
                Ok(Value::Null)
            }
            // Intercept delete operations
            "delete_entry" | "delete" => {
                let mut state = self.state.lock().unwrap();
                if let Some(key) = params.get("key").and_then(|v| v.as_str()) {
                    state.store.remove(key);
                }
                Ok(Value::Null)
            }
            // Intercept advance_time (test utility — just acknowledge)
            "advance_time" => Ok(Value::Null),
            // Intercept constraint violation reporting
            "violate_constraint" => {
                let mut state = self.state.lock().unwrap();
                if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                    state.constraint_violations.push(name.to_string());
                }
                Ok(Value::Null)
            }
            "echo" => Ok(params.clone()),
            // Accept any other call silently (permissive for testing)
            _ => Ok(Value::Null),
        }
    }
}

// ── Scenario Runner ───────────────────────────────────────────────────────────

/// Run a single compiled scenario record.
///
/// The `scenario_data` is the `data` field of a `CompiledRecord` with `type: "scenario"`.
/// The `procedures` map contains compiled procedure records by name (for `run:` references).
pub fn run_scenario(
    scenario_data: &Value,
    procedures: &HashMap<String, Value>,
    checker: &dyn ExpectationChecker,
) -> ScenarioResult {
    let start = std::time::Instant::now();

    let name = scenario_data
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let given = scenario_data
        .get("given")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let handler = ScenarioActionHandler::new();
    let mut vars = HashMap::new();

    // 1. Execute setup steps
    if let Some(setup_steps) = scenario_data.get("setup").and_then(|v| v.as_array()) {
        for step in setup_steps {
            if let Err(e) = execute_step(step, &handler, &mut vars) {
                return ScenarioResult {
                    name,
                    given,
                    passed: false,
                    expectations: vec![],
                    error: Some(format!("setup failed: {e}")),
                    duration_ms: start.elapsed().as_millis() as u64,
                };
            }
        }
    }

    // 2. Execute the run procedure (if specified)
    if let Some(run_info) = scenario_data.get("run").filter(|v| !v.is_null()) {
        let (proc_name, run_params) = if let Some(name_str) = run_info.as_str() {
            (name_str.to_string(), HashMap::new())
        } else if let Some(n) = run_info.get("procedure").and_then(|v| v.as_str()) {
            let params = run_info
                .get("params")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<HashMap<String, Value>>()
                })
                .unwrap_or_default();
            (n.to_string(), params)
        } else {
            return ScenarioResult {
                name,
                given,
                passed: false,
                expectations: vec![],
                error: Some("invalid run clause".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        };

        vars.extend(run_params);

        if let Some(proc_data) = procedures.get(&proc_name) {
            match execute_with_vars(proc_data, &handler, vars) {
                Ok(result) => {
                    // The executor's `emit` step stores events in `result.variables["emit"]`
                    // rather than calling `handler.call("emit", ...)`.  Replay them through
                    // the handler so ScenarioActionHandler captures them in emitted_events.
                    if let Some(Value::Array(events)) =
                        result.variables.get("emit").cloned()
                    {
                        for event in &events {
                            let _ = handler.call("emit", event);
                        }
                    }
                    vars = result.variables;
                }
                Err(e) => {
                    return ScenarioResult {
                        name,
                        given,
                        passed: false,
                        expectations: vec![],
                        error: Some(format!("procedure '{proc_name}' failed: {e}")),
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            }
        } else {
            return ScenarioResult {
                name,
                given,
                passed: false,
                expectations: vec![],
                error: Some(format!("procedure '{proc_name}' not found")),
                duration_ms: start.elapsed().as_millis() as u64,
            };
        }
    }

    // 3. Check expectations
    let mut state = handler.into_state();
    state.variables = vars;
    let mut expectations = vec![];
    let mut all_passed = true;

    if let Some(expect_list) = scenario_data.get("expectations").and_then(|v| v.as_array()) {
        for expectation in expect_list {
            let check_name = expectation
                .get("check")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let params = expectation.get("params").cloned().unwrap_or(Value::Null);
            let negated = expectation
                .get("negated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let result = checker.check(check_name, &params, &state);

            let (passed, reason) = match result {
                Ok(satisfied) => {
                    let effective = if negated { !satisfied } else { satisfied };
                    let reason = if !effective {
                        if negated {
                            Some(format!("expected NOT {check_name} but it was satisfied"))
                        } else {
                            Some(format!("expected {check_name} but it was not satisfied"))
                        }
                    } else {
                        None
                    };
                    (effective, reason)
                }
                Err(err) => (false, Some(format!("check error: {err}"))),
            };

            if !passed {
                all_passed = false;
            }

            expectations.push(ExpectationResult {
                check: check_name.to_string(),
                params,
                negated,
                passed,
                reason,
            });
        }
    }

    ScenarioResult {
        name,
        given,
        passed: all_passed,
        expectations,
        error: None,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

/// Run all scenarios from compiled records, returning aggregate results.
pub fn run_scenarios(
    scenarios: &[Value],
    procedures: &HashMap<String, Value>,
    checker: &dyn ExpectationChecker,
) -> ScenarioSuiteResult {
    let start = std::time::Instant::now();
    let mut results = vec![];

    for scenario_data in scenarios {
        let result = run_scenario(scenario_data, procedures, checker);
        results.push(result);
    }

    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;

    ScenarioSuiteResult {
        source: None,
        results,
        total,
        passed,
        failed,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

// ── Internal Helpers ──────────────────────────────────────────────────────────

/// Execute a single compiled step via the handler.
fn execute_step(
    step: &Value,
    handler: &dyn ActionHandler,
    vars: &mut HashMap<String, Value>,
) -> Result<Value, ExecutionError> {
    let kind = step
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("call");

    match kind {
        "call" => {
            let name = step
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let params = step.get("params").cloned().unwrap_or(Value::Null);
            let resolved_params = resolve_vars(&params, vars);
            let output = handler.call(name, &resolved_params)?;
            if let Some(output_var) = step.get("output_var").and_then(|v| v.as_str()) {
                if !output_var.is_empty() {
                    vars.insert(output_var.to_string(), output.clone());
                }
            }
            Ok(output)
        }
        "emit" => {
            let event = step.get("event").cloned().unwrap_or(Value::Null);
            let resolved_event = resolve_vars(&event, vars);
            handler.call("emit", &resolved_event)
        }
        "when" => {
            let condition = step
                .get("condition")
                .and_then(|v| v.as_str())
                .unwrap_or("true");
            if handler.evaluate_condition(condition, vars) {
                if let Some(steps) = step.get("steps").and_then(|v| v.as_array()) {
                    for s in steps {
                        execute_step(s, handler, vars)?;
                    }
                }
            }
            Ok(Value::Null)
        }
        "loop" => {
            if let Some(times) = step.get("times").and_then(|v| v.as_u64()) {
                if let Some(steps) = step.get("steps").and_then(|v| v.as_array()) {
                    for _ in 0..times.min(10_000) {
                        for s in steps {
                            execute_step(s, handler, vars)?;
                        }
                    }
                }
            }
            Ok(Value::Null)
        }
        _ => {
            // Treat unknown kinds as calls
            let name = step
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(kind);
            let params = step.get("params").cloned().unwrap_or(Value::Null);
            let resolved_params = resolve_vars(&params, vars);
            let output = handler.call(name, &resolved_params)?;
            if let Some(output_var) = step.get("output_var").and_then(|v| v.as_str()) {
                if !output_var.is_empty() {
                    vars.insert(output_var.to_string(), output.clone());
                }
            }
            Ok(output)
        }
    }
}

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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    #[derive(Default)]
    struct TestActionHandler {
        calls: Mutex<Vec<(String, Value)>>,
    }

    impl ActionHandler for TestActionHandler {
        fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError> {
            self.calls
                .lock()
                .unwrap()
                .push((name.to_string(), params.clone()));
            match name {
                "seed" => Ok(json!(42)),
                _ => Ok(Value::Null),
            }
        }
    }

    #[test]
    fn scenario_with_no_expectations_passes() {
        let scenario = json!({
            "name": "empty_scenario",
            "given": "Nothing to check",
            "setup": [],
            "expectations": []
        });

        let result = run_scenario(&scenario, &HashMap::new(), &BuiltinChecker);

        assert!(result.passed);
        assert_eq!(result.name, "empty_scenario");
        assert!(result.expectations.is_empty());
    }

    #[test]
    fn scenario_has_entry_passes_when_entry_exists() {
        let scenario = json!({
            "name": "entry_exists",
            "given": "Entry was put in setup",
            "setup": [
                {"kind": "call", "name": "put_entry", "params": {"key": "mykey", "value": "myval"}}
            ],
            "expectations": [
                {"check": "has_entry", "params": {"key": "mykey"}, "negated": false}
            ]
        });

        let result = run_scenario(&scenario, &HashMap::new(), &BuiltinChecker);

        assert!(result.passed, "expected pass, got: {:?}", result);
        assert_eq!(result.expectations.len(), 1);
        assert!(result.expectations[0].passed);
    }

    #[test]
    fn scenario_has_entry_negated_passes_when_entry_missing() {
        let scenario = json!({
            "name": "entry_missing",
            "given": "Entry was never put",
            "setup": [],
            "expectations": [
                {"check": "has_entry", "params": {"key": "nokey"}, "negated": true}
            ]
        });

        let result = run_scenario(&scenario, &HashMap::new(), &BuiltinChecker);
        assert!(result.passed);
    }

    #[test]
    fn scenario_has_entry_negated_fails_when_entry_exists() {
        let scenario = json!({
            "name": "entry_should_not_exist",
            "setup": [
                {"kind": "call", "name": "put_entry", "params": {"key": "badkey", "value": "x"}}
            ],
            "expectations": [
                {"check": "has_entry", "params": {"key": "badkey"}, "negated": true}
            ]
        });

        let result = run_scenario(&scenario, &HashMap::new(), &BuiltinChecker);

        assert!(!result.passed);
        assert!(!result.expectations[0].passed);
        assert!(result.expectations[0]
            .reason
            .as_ref()
            .unwrap()
            .contains("NOT"));
    }

    #[test]
    fn scenario_event_emitted_passes() {
        let scenario = json!({
            "name": "event_check",
            "setup": [
                {"kind": "emit", "event": {"event": "cache.invalidated", "key": "old"}}
            ],
            "expectations": [
                {"check": "event_emitted", "params": {"event": "cache.invalidated", "key": "old"}, "negated": false}
            ]
        });

        let result = run_scenario(&scenario, &HashMap::new(), &BuiltinChecker);
        assert!(result.passed, "expected pass, got: {:?}", result);
    }

    #[test]
    fn scenario_event_not_emitted_passes() {
        let scenario = json!({
            "name": "no_event",
            "setup": [],
            "expectations": [
                {"check": "event_emitted", "params": {"event": "never.fired"}, "negated": true}
            ]
        });

        let result = run_scenario(&scenario, &HashMap::new(), &BuiltinChecker);
        assert!(result.passed);
    }

    #[test]
    fn scenario_runs_procedure_then_checks() {
        let scenario = json!({
            "name": "with_procedure",
            "given": "Procedure puts an entry",
            "setup": [],
            "run": "my_proc",
            "expectations": [
                {"check": "has_entry", "params": {"key": "proc_key"}, "negated": false}
            ]
        });

        let mut procedures = HashMap::new();
        procedures.insert(
            "my_proc".to_string(),
            json!({
                "name": "my_proc",
                "steps": [
                    {"kind": "call", "name": "put_entry", "params": {"key": "proc_key", "value": "proc_val"}}
                ]
            }),
        );

        let result = run_scenario(&scenario, &procedures, &BuiltinChecker);
        assert!(result.passed, "expected pass, got: {:?}", result);
    }

    #[test]
    fn scenario_fails_when_procedure_not_found() {
        let scenario = json!({
            "name": "missing_proc",
            "run": "nonexistent",
            "expectations": []
        });

        let result = run_scenario(&scenario, &HashMap::new(), &BuiltinChecker);

        assert!(!result.passed);
        assert!(result.error.as_ref().unwrap().contains("not found"));
    }

    #[test]
    fn scenario_delete_removes_entry() {
        let scenario = json!({
            "name": "delete_test",
            "setup": [
                {"kind": "call", "name": "put_entry", "params": {"key": "to_delete", "value": "x"}},
                {"kind": "call", "name": "delete_entry", "params": {"key": "to_delete"}}
            ],
            "expectations": [
                {"check": "has_entry", "params": {"key": "to_delete"}, "negated": true}
            ]
        });

        let result = run_scenario(&scenario, &HashMap::new(), &BuiltinChecker);
        assert!(result.passed);
    }

    #[test]
    fn scenario_constraint_violated_check() {
        let state = ExecutionState {
            constraint_violations: vec!["ttl_positive".to_string()],
            ..Default::default()
        };

        let result = BuiltinChecker
            .check("constraint_violated", &json!({"name": "ttl_positive"}), &state)
            .unwrap();
        assert!(result);

        let result = BuiltinChecker
            .check("constraint_violated", &json!({"name": "other"}), &state)
            .unwrap();
        assert!(!result);
    }

    #[test]
    fn run_scenarios_aggregates_results() {
        let scenarios = vec![
            json!({
                "name": "pass1",
                "setup": [],
                "expectations": []
            }),
            json!({
                "name": "pass2",
                "setup": [
                    {"kind": "call", "name": "put_entry", "params": {"key": "k", "value": "v"}}
                ],
                "expectations": [
                    {"check": "has_entry", "params": {"key": "k"}, "negated": false}
                ]
            }),
            json!({
                "name": "fail1",
                "setup": [],
                "expectations": [
                    {"check": "has_entry", "params": {"key": "missing"}, "negated": false}
                ]
            }),
        ];

        let suite = run_scenarios(&scenarios, &HashMap::new(), &BuiltinChecker);

        assert_eq!(suite.total, 3);
        assert_eq!(suite.passed, 2);
        assert_eq!(suite.failed, 1);
    }

    #[test]
    fn full_cache_invalidation_scenario() {
        // Mirrors the design doc example
        let scenario = json!({
            "name": "expired_entries_removed",
            "given": "Cache has entries with expired TTLs",
            "setup": [
                {"kind": "call", "name": "put_entry", "params": {"key": "old", "value": "stale"}},
                {"kind": "call", "name": "put_entry", "params": {"key": "fresh", "value": "good"}},
                {"kind": "call", "name": "advance_time", "params": {"secs": 10}}
            ],
            "run": "invalidate_expired",
            "expectations": [
                {"check": "has_entry", "params": {"key": "old"}, "negated": true},
                {"check": "has_entry", "params": {"key": "fresh"}, "negated": false},
                {"check": "event_emitted", "params": {"event": "cache.invalidated", "key": "old"}, "negated": false}
            ]
        });

        // The procedure deletes "old" and emits an event
        let mut procedures = HashMap::new();
        procedures.insert(
            "invalidate_expired".to_string(),
            json!({
                "name": "invalidate_expired",
                "steps": [
                    {"kind": "call", "name": "delete_entry", "params": {"key": "old"}},
                    {"kind": "emit", "event": {"event": "cache.invalidated", "key": "old"}}
                ]
            }),
        );

        let result = run_scenario(&scenario, &procedures, &BuiltinChecker);

        assert!(result.passed, "expected pass, got: {:?}", result);
        assert_eq!(result.expectations.len(), 3);
        assert!(result.expectations[0].passed); // NOT has_entry "old"
        assert!(result.expectations[1].passed); // has_entry "fresh"
        assert!(result.expectations[2].passed); // event_emitted cache.invalidated
    }

    #[test]
    fn scenario_with_run_object_format() {
        // Test the {"procedure": "name", "params": {...}} format
        let scenario = json!({
            "name": "run_object",
            "setup": [],
            "run": {"procedure": "my_proc", "params": {"x": 1}},
            "expectations": [
                {"check": "has_entry", "params": {"key": "k"}, "negated": false}
            ]
        });

        let mut procedures = HashMap::new();
        procedures.insert(
            "my_proc".to_string(),
            json!({
                "name": "my_proc",
                "steps": [
                    {"kind": "call", "name": "put_entry", "params": {"key": "k", "value": "v"}}
                ]
            }),
        );

        let result = run_scenario(&scenario, &procedures, &BuiltinChecker);
        assert!(result.passed, "expected pass, got: {:?}", result);
    }

    #[test]
    fn execute_step_resolves_vars_binds_output_and_uses_vars_for_when() {
        let handler = TestActionHandler::default();
        let mut vars = HashMap::new();

        let seed_step = json!({
            "kind": "call",
            "name": "seed",
            "output_var": "value"
        });
        let when_step = json!({
            "kind": "when",
            "condition": "value == 42",
            "steps": [{
                "kind": "call",
                "name": "consume",
                "params": {"input": "$value"}
            }]
        });

        execute_step(&seed_step, &handler, &mut vars).unwrap();
        execute_step(&when_step, &handler, &mut vars).unwrap();

        let calls = handler.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[1].0, "consume");
        assert_eq!(calls[1].1, json!({"input": 42}));
        assert_eq!(vars.get("value"), Some(&json!(42)));
    }

    #[test]
    fn scenario_run_params_and_output_vars_feed_expectations() {
        let scenario = json!({
            "name": "run_params_and_vars",
            "setup": [],
            "run": {"procedure": "my_proc", "params": {"input_key": "k1"}},
            "expectations": [
                {"check": "has_entry", "params": {"key": "k1"}, "negated": false},
                {"check": "var_equals", "params": {"var": "saved_key", "value": "k1"}, "negated": false}
            ]
        });

        let mut procedures = HashMap::new();
        procedures.insert(
            "my_proc".to_string(),
            json!({
                "name": "my_proc",
                "steps": [
                    {"kind": "call", "name": "put_entry", "params": {"key": "$input_key", "value": "v"}},
                    {"kind": "call", "name": "echo", "params": "$input_key", "output_var": "saved_key"}
                ]
            }),
        );

        let result = run_scenario(&scenario, &procedures, &BuiltinChecker);
        assert!(result.passed, "expected pass, got: {:?}", result);
    }

    #[test]
    fn scenario_when_uses_live_vars() {
        let scenario = json!({
            "name": "when_with_vars",
            "setup": [],
            "run": "my_proc",
            "expectations": [
                {"check": "has_entry", "params": {"key": "enabled"}, "negated": false}
            ]
        });

        let mut procedures = HashMap::new();
        procedures.insert(
            "my_proc".to_string(),
            json!({
                "name": "my_proc",
                "steps": [
                    {"kind": "call", "name": "echo", "params": true, "output_var": "should_write"},
                    {
                        "kind": "when",
                        "condition": "should_write == true",
                        "steps": [
                            {"kind": "call", "name": "put_entry", "params": {"key": "enabled", "value": 1}}
                        ]
                    }
                ]
            }),
        );
        let result = run_scenario(&scenario, &procedures, &BuiltinChecker);
        assert!(result.passed, "expected pass, got: {:?}", result);
    }

    #[test]
    fn when_return_short_circuits_procedure_in_scenario() {
        // A `return` inside a `when` block must stop procedure execution.
        // The entry must NOT be stored when the guard fires.
        let mut procedures = HashMap::new();
        procedures.insert(
            "guard_proc".to_string(),
            json!({
                "name": "guard_proc",
                "steps": [
                    // Bind result to the return value via echo so var_equals can check it.
                    // The when fires (input == empty), returns "no_input".
                    {
                        "kind": "when",
                        "condition": "input == empty",
                        "steps": [
                            {"kind": "call", "name": "echo", "params": "no_input", "output_var": "result"},
                            {"kind": "return", "value": "no_input"}
                        ]
                    },
                    // These steps must NOT run.
                    {"kind": "call", "name": "put_entry", "params": {"key": "should_not_exist", "value": 1}},
                    {"kind": "call", "name": "echo", "params": "input_present", "output_var": "result"}
                ]
            }),
        );

        let scenario = json!({
            "name": "guard_short_circuit",
            "setup": [
                {"kind": "call", "name": "echo", "params": "empty", "output_var": "input"}
            ],
            "run": "guard_proc",
            "expectations": [
                {"check": "has_entry", "params": {"key": "should_not_exist"}, "negated": true},
                {"check": "var_equals", "params": {"var": "result", "value": "no_input"}, "negated": false}
            ]
        });

        let result = run_scenario(&scenario, &procedures, &BuiltinChecker);
        assert!(result.passed, "expected pass, got: {:?}", result);
    }

    #[test]
    fn when_condition_false_continues_execution() {
        // When the when-condition is false the return is NOT triggered and execution continues.
        let scenario = json!({
            "name": "no_guard_trigger",
            "setup": [
                {"kind": "call", "name": "echo", "params": "hello", "output_var": "input"}
            ],
            "run": "guard_proc",
            "expectations": [
                {"check": "has_entry", "params": {"key": "did_run"}, "negated": false}
            ]
        });

        let mut procedures = HashMap::new();
        procedures.insert(
            "guard_proc".to_string(),
            json!({
                "name": "guard_proc",
                "steps": [
                    {
                        "kind": "when",
                        "condition": "input == empty",
                        "steps": [
                            {"kind": "return", "value": "no_input"}
                        ]
                    },
                    {"kind": "call", "name": "put_entry", "params": {"key": "did_run", "value": 1}}
                ]
            }),
        );

        let result = run_scenario(&scenario, &procedures, &BuiltinChecker);
        assert!(result.passed, "expected pass, got: {:?}", result);
    }
}
