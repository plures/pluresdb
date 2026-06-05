//! `pluresdb-px` — The `.px` declarative language runtime for PluresDB.
//!
//! This crate provides everything needed to build reactive applications on
//! PluresDB using the `.px` language:
//!
//! - **Parser** — pest-based grammar for `.px` source files
//! - **Compiler** — `.px` AST → PluresDB-compatible JSON procedure records
//! - **Executor** — walks compiled procedures through a pluggable [`ActionHandler`]
//! - **Async Executor** — parallel branches, retry with backoff, timeouts
//! - **Linter** — static analysis for common `.px` mistakes
//! - **Resolver** — import resolution across `.px` files
//! - **Watcher** — filesystem watcher for hot-reload on `.px` changes
//! - **Compose** — dynamic procedure composition and pipeline building
//! - **Scenario Runner** — test harness for `.px` procedure verification
//!
//! # Architecture
//!
//! ```text
//! .px source files
//!       │
//!       ▼
//!   [Parser] ──► PxDocument (AST)
//!       │
//!       ▼
//!   [Compiler] ──► CompiledRecord (JSON, PluresDB-compatible)
//!       │
//!       ▼
//!   [Executor] ◄── ActionHandler (your IO boundary)
//!       │
//!       ▼
//!   PluresDB (persistence + sync + reactive triggers)
//! ```
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use pluresdb_px::px::{parse, compiler::compile, executor::{execute, ActionHandler}};
//! use serde_json::Value;
//!
//! // 1. Parse .px source
//! let doc = parse(r#"
//!   procedure greet:
//!     trigger: manual
//!     say_hello {name: $sender} -> $greeting
//! "#).unwrap();
//!
//! // 2. Compile to PluresDB records
//! let records = compile(&doc);
//!
//! // 3. Execute with your action handler
//! let handler = MyHandler::new();
//! for record in records.iter().filter(|record| record.data["type"] == "procedure") {
//!     let result = execute(&record.data, &handler).unwrap();
//!     println!("Result: {:?}", result);
//! }
//! ```
//!
//! # The ActionHandler Pattern
//!
//! The [`px::executor::ActionHandler`] trait is the integration point. Your
//! application provides concrete implementations that handle the side effects
//! (API calls, model invocations, shell commands, etc.) that `.px` procedures
//! reference by name.
//!
//! ```rust,ignore
//! use pluresdb_px::px::executor::{ActionHandler, ExecutionError};
//! use serde_json::Value;
//! use std::collections::HashMap;
//!
//! struct MyHandler;
//!
//! impl ActionHandler for MyHandler {
//!     fn call(&self, name: &str, params: &Value) -> Result<Value, ExecutionError> {
//!         match name {
//!             "send_telegram" => { /* your Telegram IO */ Ok(Value::Null) }
//!             "query_model" => { /* your LLM call */ Ok(Value::Null) }
//!             _ => Err(ExecutionError::UnknownAction(name.to_string()))
//!         }
//!     }
//! }
//! ```
//!
//! # Standard Action Handlers (planned)
//!
//! Future releases will include a `pluresdb-px-actions` crate with built-in
//! handlers for common operations:
//! - `exec` — shell command execution
//! - `http` — HTTP requests
//! - `file_read` / `file_write` — filesystem
//! - `emit` — PluresDB event emission
//! - `assert_eq` / `assert_contains` — test assertions

/// The `.px` language runtime: parser, compiler, executors, linter, resolver.
#[allow(missing_docs)] // TODO: remove once the module API is fully documented
pub mod px;

/// Constraint store and evaluation engine (in-memory, zero external deps).
#[allow(missing_docs)] // TODO: remove once the module API is fully documented
pub mod db;
