//! `pluresdb-procedures` — cross-language query DSL, procedure engine, and
//! core operations for PluresDB.
//!
//! ## Quick start
//!
//! ```rust
//! use pluresdb_core::CrdtStore;
//! use pluresdb_procedures::{
//!     builder::QueryBuilder,
//!     engine::ProcedureEngine,
//!     ir::{AggFn, Predicate, SortDir},
//! };
//!
//! // Build an in-memory store and populate it.
//! let store = CrdtStore::default();
//! store.put("a", "actor", serde_json::json!({"category": "decision", "score": 0.9}));
//! store.put("b", "actor", serde_json::json!({"category": "note",     "score": 0.1}));
//!
//! // Execute a pipeline using the fluent builder.
//! let steps = QueryBuilder::new()
//!     .filter(Predicate::eq("category", "decision"))
//!     .sort_desc("score")
//!     .limit(10)
//!     .to_steps();
//!
//! let engine = ProcedureEngine::new(&store, "actor");
//! let result = engine.exec(&steps).unwrap();
//! assert_eq!(result.nodes.len(), 1);
//! ```
//!
//! ## DSL string syntax
//!
//! ```rust
//! use pluresdb_core::CrdtStore;
//! use pluresdb_procedures::engine::ProcedureEngine;
//!
//! let store = CrdtStore::default();
//! store.put("a", "actor", serde_json::json!({"category": "decision"}));
//!
//! let engine = ProcedureEngine::new(&store, "actor");
//! let result = engine.exec_dsl(r#"filter(category == "decision") |> limit(5)"#).unwrap();
//! assert_eq!(result.nodes.len(), 1);
//! ```
//!
//! ## `pred!` macro
//!
//! ```rust,ignore
//! use pluresdb_procedures::pred;
//! let p = pred!(score >= 0.5);
//! ```

pub mod builder;
pub mod engine;
pub mod ir;
pub mod ops;
pub mod parser;
pub mod pred;

// Convenience re-exports
pub use builder::{MutateBuilder, QueryBuilder};
pub use engine::ProcedureEngine;
pub use pred::pred;
