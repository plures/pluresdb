//! PluresDB runtime for praxis — schema, store, procedures, and seed data.
//!
//! This module implements the "PluresDB as praxis logic runtime" described in
//! the issue.  All praxis primitives are modelled as PluresDB schema + data +
//! procedures:
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`schema`] | `Constraint`, `Adr`, `Evidence`, `AgentContext` types |
//! | [`store`] | [`PraxisStore`] — in-process collection store with graph traversal |
//! | [`procedures`] | `evaluate`, `on_action`, `compile_nl`, `query_gaps` |
//! | [`seed`] | Built-in constraints (migrated from `.praxis/`) + ADR-0004 records |
//! | [`guidance`] | `GuidanceEntry`, `SourceSpan`, `AnalysisEvent` tables + [`GuidanceStore`] |
//!
//! # Quick start
//!
//! ```rust
//! use pluresdb_px::db::{AgentContext, SessionType};
//! use pluresdb_px::db::procedures::on_action;
//! use pluresdb_px::db::seed::default_store;
//! use serde_json::json;
//!
//! let store = default_store();
//! let ctx = AgentContext::new("read_file", "README.md", SessionType::Main);
//! match on_action(&store, &ctx) {
//!     Ok(warnings) => println!("{} warning(s)", warnings.len()),
//!     Err(blocked) => eprintln!("blocked: {blocked}"),
//! }
//! ```

pub mod guidance;
pub mod procedures;
pub mod schema;
pub mod seed;
pub mod store;

// Re-export the most commonly used types at the `db` level for ergonomic imports.
pub use guidance::{AnalysisEvent, GuidanceCategory, GuidanceEntry, GuidanceStore, SourceSpan};
pub use schema::{
    Adr, AdrStatus, AgentContext, Condition, Constraint, Evidence, EvidenceResult, SessionType,
    Severity,
};
pub use store::{PraxisStore, StoreError};
