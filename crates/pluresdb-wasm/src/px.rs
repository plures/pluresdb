//! WASM bindings for pluresdb-px (.px language runtime).
//!
//! Exposes the parser, compiler, and synchronous executor to JavaScript.

use serde_json::Value;
use wasm_bindgen::prelude::*;

use pluresdb_px::px::{
    self,
    compiler::{compile, CompiledRecord},
    executor::{self, ActionHandler, ExecutionError, ExecutionResult},
};

use std::cell::RefCell;
use std::collections::HashMap;

use js_sys::Function;
use serde_wasm_bindgen::{from_value, to_value};

/// Parse a .px source string and return the compiled records as JSON.
///
/// This is the full pipeline: parse → compile → JSON output.
#[wasm_bindgen(js_name = pxCompile)]
pub fn px_compile(source: &str) -> Result<JsValue, JsValue> {
    let doc = px::parse(source).map_err(|e| JsValue::from_str(&format!("Parse error: {e}")))?;
    let records = compile(&doc);
    to_value(&records).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Parse a .px source string and return the AST as JSON (for introspection).
#[wasm_bindgen(js_name = pxParse)]
pub fn px_parse(source: &str) -> Result<JsValue, JsValue> {
    let doc = px::parse(source).map_err(|e| JsValue::from_str(&format!("Parse error: {e}")))?;
    to_value(&doc).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Lint a .px source string and return diagnostics as JSON.
#[wasm_bindgen(js_name = pxLint)]
pub fn px_lint(source: &str) -> Result<JsValue, JsValue> {
    let doc = px::parse(source).map_err(|e| JsValue::from_str(&format!("Parse error: {e}")))?;
    let diagnostics = pluresdb_px::px::lint::lint(&doc);
    to_value(&diagnostics).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Execute a compiled procedure record with a JS action handler callback.
///
/// The `handler` function is called with `(actionName: string, params: object)`
/// and should return the result value (or throw on error).
#[wasm_bindgen(js_name = pxExecute)]
pub fn px_execute(compiled_record: JsValue, handler: Function) -> Result<JsValue, JsValue> {
    let record: Value =
        from_value(compiled_record).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let js_handler = JsActionHandler { callback: handler };
    let result =
        executor::execute(&record, &js_handler).map_err(|e| JsValue::from_str(&e.to_string()))?;

    to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// A JS function-backed ActionHandler for the .px executor.
///
/// SAFETY: wasm32 is single-threaded, so Send+Sync is vacuously satisfied.
struct JsActionHandler {
    callback: Function,
}

// SAFETY: WASM is single-threaded; Function cannot be shared across threads
// because there are no threads.
unsafe impl Send for JsActionHandler {}
unsafe impl Sync for JsActionHandler {}

impl ActionHandler for JsActionHandler {
    fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError> {
        let name_js = JsValue::from_str(name);
        let params_js =
            to_value(params).map_err(|e| ExecutionError::ActionFailed {
                action: name.to_string(),
                message: e.to_string(),
            })?;

        let result = self
            .callback
            .call2(&JsValue::NULL, &name_js, &params_js)
            .map_err(|e| ExecutionError::ActionFailed {
                action: name.to_string(),
                message: format!("{:?}", e),
            })?;

        from_value(result).map_err(|e| ExecutionError::ActionFailed {
            action: name.to_string(),
            message: e.to_string(),
        })
    }
}
