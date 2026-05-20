//! Procedure composition — call one procedure from another.
//!
//! The [`ProcedureRegistry`] stores compiled procedures by name and provides
//! a [`ComposableHandler`] that wraps an underlying [`AsyncActionHandler`].
//! When a call step references a registered procedure name, the handler
//! executes that procedure inline (passing params as initial variables)
//! and returns its result. Unknown names fall through to the inner handler.
//!
//! # Example
//!
//! ```rust,ignore
//! let mut registry = ProcedureRegistry::new();
//! registry.register("validate_input", validate_proc_data);
//! registry.register("transform", transform_proc_data);
//!
//! let handler = ComposableHandler::new(registry, my_action_handler);
//! let result = execute_async(&pipeline_proc, &handler).await?;
//! ```
//!
//! # Recursion
//!
//! Recursive calls are supported up to [`MAX_CALL_DEPTH`]. Exceeding the
//! limit produces an [`ExecutionError::ActionFailed`] with a clear message.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use serde_json::Value;

use super::async_executor::{execute_async_with_vars, AsyncActionHandler};
use super::executor::{ExecutionError, StepResult};

/// Maximum nesting depth for procedure-to-procedure calls.
pub const MAX_CALL_DEPTH: usize = 16;

// ── Procedure Registry ────────────────────────────────────────────────────────

/// A registry of compiled procedures available for composition.
///
/// Procedures are stored by name and can be called from other procedures
/// via regular `call` steps whose name matches a registered procedure.
#[derive(Debug, Clone, Default)]
pub struct ProcedureRegistry {
    procedures: HashMap<String, Value>,
}

impl ProcedureRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a compiled procedure record.
    ///
    /// The `data` should be the `data` field from a `CompiledRecord` with
    /// `type: "procedure"`. The procedure's `name` field is used as the key.
    pub fn register(&mut self, data: Value) -> Option<String> {
        let name = data.get("name").and_then(|v| v.as_str())?.to_string();
        self.procedures.insert(name.clone(), data);
        Some(name)
    }

    /// Register a procedure under an explicit name.
    pub fn register_as(&mut self, name: impl Into<String>, data: Value) {
        self.procedures.insert(name.into(), data);
    }

    /// Look up a procedure by name.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.procedures.get(name)
    }

    /// Check if a name is a registered procedure.
    pub fn contains(&self, name: &str) -> bool {
        self.procedures.contains_key(name)
    }

    /// Return the number of registered procedures.
    pub fn len(&self) -> usize {
        self.procedures.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.procedures.is_empty()
    }

    /// List all registered procedure names.
    pub fn names(&self) -> Vec<&str> {
        self.procedures.keys().map(|s| s.as_str()).collect()
    }
}

// ── Composable Handler ────────────────────────────────────────────────────────

/// An async action handler that composes procedures from a registry.
///
/// When a call step's name matches a registered procedure, that procedure
/// is executed inline with the call params as initial variables. The
/// procedure's final variable bindings are returned as a JSON object.
///
/// Non-procedure calls are delegated to the inner handler.
pub struct ComposableHandler<H: AsyncActionHandler> {
    registry: ProcedureRegistry,
    inner: H,
    depth: AtomicUsize,
}

impl<H: AsyncActionHandler> ComposableHandler<H> {
    /// Create a new composable handler.
    pub fn new(registry: ProcedureRegistry, inner: H) -> Self {
        Self {
            registry,
            inner,
            depth: AtomicUsize::new(0),
        }
    }

    /// Get a reference to the procedure registry.
    pub fn registry(&self) -> &ProcedureRegistry {
        &self.registry
    }
}

#[async_trait]
impl<H: AsyncActionHandler + 'static> AsyncActionHandler for ComposableHandler<H> {
    async fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError> {
        // Check if this is a registered procedure
        if let Some(proc_data) = self.registry.get(name) {
            // Depth check
            let current_depth = self.depth.fetch_add(1, Ordering::SeqCst);
            if current_depth >= MAX_CALL_DEPTH {
                self.depth.fetch_sub(1, Ordering::SeqCst);
                return Err(ExecutionError::ActionFailed {
                    action: name.to_string(),
                    message: format!(
                        "procedure call depth {} exceeds maximum {}",
                        current_depth + 1,
                        MAX_CALL_DEPTH
                    ),
                });
            }

            // Convert params to initial variables for the sub-procedure
            let initial_vars = params_to_vars(params);

            // Execute the sub-procedure using `self` as the handler
            // (allowing nested procedure calls to also resolve from registry)
            let result = execute_async_with_vars(proc_data, self, initial_vars).await;

            self.depth.fetch_sub(1, Ordering::SeqCst);

            match result {
                Ok(exec_result) => {
                    // Return the procedure's variables as the call result
                    Ok(Value::Object(exec_result.variables.into_iter().collect()))
                }
                Err(e) => Err(ExecutionError::ActionFailed {
                    action: name.to_string(),
                    message: format!("sub-procedure failed: {e}"),
                }),
            }
        } else {
            // Not a procedure — delegate to inner handler
            self.inner.call(name, params).await
        }
    }

    fn evaluate_condition(&self, expr: &str, vars: &HashMap<String, Value>) -> bool {
        self.inner.evaluate_condition(expr, vars)
    }

    async fn on_step_start(&self, step_index: usize, kind: &str) {
        self.inner.on_step_start(step_index, kind).await;
    }

    async fn on_step_complete(&self, step_index: usize, result: &StepResult) {
        self.inner.on_step_complete(step_index, result).await;
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert a JSON params value into a variable map for sub-procedure execution.
///
/// If params is an object, each key-value pair becomes a variable.
/// If params is null or non-object, an empty map is returned.
fn params_to_vars(params: &Value) -> HashMap<String, Value> {
    match params {
        Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => HashMap::new(),
    }
}

// ── Pipe Utility ──────────────────────────────────────────────────────────────

/// Execute a sequence of procedures in a pipeline, passing each result
/// as the `$input` variable to the next procedure.
///
/// Returns the final execution result (variables from the last procedure).
pub async fn pipe(
    procedure_names: &[&str],
    registry: &ProcedureRegistry,
    handler: &dyn AsyncActionHandler,
    initial_input: Value,
) -> Result<Value, ExecutionError> {
    let mut current_input = initial_input;

    for &name in procedure_names {
        let proc_data = registry
            .get(name)
            .ok_or_else(|| ExecutionError::ActionFailed {
                action: name.to_string(),
                message: format!("procedure '{name}' not found in registry"),
            })?;

        let mut vars = HashMap::new();
        vars.insert("input".to_string(), current_input);

        let result = execute_async_with_vars(proc_data, handler, vars)
            .await
            .map_err(|e| ExecutionError::ActionFailed {
                action: name.to_string(),
                message: format!("pipe stage '{name}' failed: {e}"),
            })?;

        // The output is either an explicit $output var, or all variables
        current_input = result
            .variables
            .get("output")
            .cloned()
            .unwrap_or_else(|| Value::Object(result.variables.into_iter().collect()));
    }

    Ok(current_input)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Mock handler that handles leaf actions (non-procedure calls).
    struct LeafHandler {
        results: HashMap<String, Value>,
    }

    impl LeafHandler {
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
    impl AsyncActionHandler for LeafHandler {
        async fn call(&self, name: &str, _params: &Value) -> Result<Value, ExecutionError> {
            self.results
                .get(name)
                .cloned()
                .ok_or_else(|| ExecutionError::UnknownAction(name.to_string()))
        }
    }

    #[tokio::test]
    async fn compose_simple_procedure_call() {
        // Register a "validate" procedure that calls "check_format"
        let validate_proc = json!({
            "type": "procedure",
            "name": "validate",
            "steps": [
                { "kind": "call", "name": "check_format", "params": {}, "output_var": "output" }
            ]
        });

        // Main procedure calls "validate" (which is a registered procedure)
        let main_proc = json!({
            "type": "procedure",
            "name": "main",
            "steps": [
                { "kind": "call", "name": "validate", "params": { "data": "test" }, "output_var": "result" }
            ]
        });

        let mut registry = ProcedureRegistry::new();
        registry.register(validate_proc);

        let leaf = LeafHandler::new().with_result("check_format", json!("valid"));

        let handler = ComposableHandler::new(registry, leaf);

        let result = execute_async_with_vars(&main_proc, &handler, HashMap::new())
            .await
            .unwrap();

        assert!(result.success);
        // The result of calling "validate" is its variables as an object
        let call_result = result.variables.get("result").unwrap();
        assert_eq!(call_result.get("output"), Some(&json!("valid")));
    }

    #[tokio::test]
    async fn compose_chained_procedures() {
        // proc_a: calls leaf "fetch" and stores result
        let proc_a = json!({
            "type": "procedure",
            "name": "fetch_data",
            "steps": [
                { "kind": "call", "name": "fetch", "params": {}, "output_var": "output" }
            ]
        });

        // proc_b: calls leaf "transform" and stores result
        let proc_b = json!({
            "type": "procedure",
            "name": "transform_data",
            "steps": [
                { "kind": "call", "name": "transform", "params": { "val": "$input" }, "output_var": "output" }
            ]
        });

        // main: calls proc_a then proc_b
        let main_proc = json!({
            "type": "procedure",
            "name": "pipeline",
            "steps": [
                { "kind": "call", "name": "fetch_data", "params": {}, "output_var": "raw" },
                { "kind": "call", "name": "transform_data", "params": { "input": "$raw" }, "output_var": "final" }
            ]
        });

        let mut registry = ProcedureRegistry::new();
        registry.register(proc_a);
        registry.register(proc_b);

        let leaf = LeafHandler::new()
            .with_result("fetch", json!({"items": [1, 2, 3]}))
            .with_result("transform", json!("transformed"));

        let handler = ComposableHandler::new(registry, leaf);

        let result = execute_async_with_vars(&main_proc, &handler, HashMap::new())
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.variables.contains_key("raw"));
        assert!(result.variables.contains_key("final"));
    }

    #[tokio::test]
    async fn compose_recursive_depth_limit() {
        // A procedure that calls itself
        let recursive = json!({
            "type": "procedure",
            "name": "infinite",
            "steps": [
                { "kind": "call", "name": "infinite", "params": {}, "output_var": "x" }
            ]
        });

        let mut registry = ProcedureRegistry::new();
        registry.register(recursive.clone());

        let leaf = LeafHandler::new();
        let handler = ComposableHandler::new(registry, leaf);

        let result = execute_async_with_vars(&recursive, &handler, HashMap::new()).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("exceeds maximum"));
    }

    #[tokio::test]
    async fn compose_non_procedure_falls_through() {
        let main_proc = json!({
            "type": "procedure",
            "name": "main",
            "steps": [
                { "kind": "call", "name": "leaf_action", "params": {}, "output_var": "result" }
            ]
        });

        let registry = ProcedureRegistry::new(); // empty — no procedures registered
        let leaf = LeafHandler::new().with_result("leaf_action", json!("leaf_result"));
        let handler = ComposableHandler::new(registry, leaf);

        let result = execute_async_with_vars(&main_proc, &handler, HashMap::new())
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.variables.get("result"), Some(&json!("leaf_result")));
    }

    #[tokio::test]
    async fn compose_params_passed_as_vars() {
        // A procedure that uses $name from its caller's params
        let greet_proc = json!({
            "type": "procedure",
            "name": "greet",
            "steps": [
                { "kind": "call", "name": "say_hello", "params": { "who": "$name" }, "output_var": "output" }
            ]
        });

        let main_proc = json!({
            "type": "procedure",
            "name": "main",
            "steps": [
                { "kind": "call", "name": "greet", "params": { "name": "world" }, "output_var": "greeting" }
            ]
        });

        let mut registry = ProcedureRegistry::new();
        registry.register(greet_proc);

        let leaf = LeafHandler::new().with_result("say_hello", json!("hello world"));
        let handler = ComposableHandler::new(registry, leaf);

        let result = execute_async_with_vars(&main_proc, &handler, HashMap::new())
            .await
            .unwrap();

        assert!(result.success);
        let greeting = result.variables.get("greeting").unwrap();
        assert_eq!(greeting.get("output"), Some(&json!("hello world")));
    }

    #[tokio::test]
    async fn pipe_utility_sequential() {
        // Step 1: fetch
        let fetch_proc = json!({
            "type": "procedure",
            "name": "fetch",
            "steps": [
                { "kind": "call", "name": "do_fetch", "params": {}, "output_var": "output" }
            ]
        });

        // Step 2: transform (uses $input)
        let transform_proc = json!({
            "type": "procedure",
            "name": "transform",
            "steps": [
                { "kind": "call", "name": "do_transform", "params": { "data": "$input" }, "output_var": "output" }
            ]
        });

        let mut registry = ProcedureRegistry::new();
        registry.register(fetch_proc);
        registry.register(transform_proc);

        let leaf = LeafHandler::new()
            .with_result("do_fetch", json!({"raw": "data"}))
            .with_result("do_transform", json!("transformed_data"));

        let result = pipe(&["fetch", "transform"], &registry, &leaf, json!(null))
            .await
            .unwrap();

        assert_eq!(result, json!("transformed_data"));
    }

    #[tokio::test]
    async fn pipe_missing_procedure_errors() {
        let registry = ProcedureRegistry::new();
        let leaf = LeafHandler::new();

        let result = pipe(&["nonexistent"], &registry, &leaf, json!(null)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[tokio::test]
    async fn registry_operations() {
        let mut reg = ProcedureRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);

        let proc_data = json!({
            "type": "procedure",
            "name": "test_proc",
            "steps": []
        });

        let name = reg.register(proc_data.clone());
        assert_eq!(name, Some("test_proc".to_string()));
        assert!(reg.contains("test_proc"));
        assert!(!reg.contains("other"));
        assert_eq!(reg.len(), 1);
        assert!(!reg.is_empty());
        assert_eq!(reg.names(), vec!["test_proc"]);

        reg.register_as("alias", proc_data);
        assert!(reg.contains("alias"));
        assert_eq!(reg.len(), 2);
    }
}
