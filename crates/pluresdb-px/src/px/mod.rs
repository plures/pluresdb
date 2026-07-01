//! Praxis Intent Language (.px) parser + runtime (pluresdb-px).
//!
//! # Language engine: praxis-lang (single source of truth)
//!
//! As of M6 (praxis-lang epic) this crate NO LONGER carries its own `.px`
//! grammar/parser/AST. The `.px` language \u2014 grammar, parser, and the typed AST
//! \u2014 lives in **praxis-lang** (crates `px-ast`, `px-compiler`, `px-eval`,
//! pinned by git rev). The duplicate in-tree engine (`grammar.pest`,
//! `builder.rs`, the local `PxParser` + flat `Px*` AST, and the local `parse`
//! fn) was deleted here to end the grammar drift (ADR-0021).
//!
//! What STAYS in this crate is the pluresdb-specific RUNTIME that operates over
//! the compiled JSON `CompiledRecord` seam produced by [`compiler`]:
//! [`executor`], [`async_executor`], [`scenario_runner`], [`compose`],
//! [`watcher`], plus [`dataflow`], [`lint`], and [`resolver`] which were
//! rewritten in M6.2 to consume px-ast's typed shape.
//!
//! The crate's parse entry point is re-exported below (`px::parse`,
//! `px::PxDocument`, \u2026) from praxis-lang, so existing call sites
//! (`pluresdb_px::px::parse`) keep working unchanged.

pub mod compiler;
pub mod dataflow;
pub mod executor;
pub mod lint;
pub mod resolver;
pub mod scenario_runner;

#[cfg(feature = "async")]
pub mod async_executor;
#[cfg(feature = "async")]
pub mod compose;
#[cfg(feature = "watcher")]
pub mod watcher;

// ── praxis-lang SSOT engine re-export hub ────────────────────────────────────
// M6.3: the local parser/AST is deleted; these praxis-lang re-exports ARE the
// crate's `.px` engine now. Promoted to top-level so `px::parse` / `px::PxDocument`
// / `px::Statement` resolve exactly as the old local definitions did. The AST
// SHAPES are px-ast's richer typed shape (Ident/TypeExpr/ProcedureBody enum),
// adapted to the executor's JSON records inside compiler/dataflow/lint.
pub use px_ast::{self, PxDocument, Statement};
pub use px_compiler::{parse, parse_statement, CompileError};
pub use px_eval;

// M6.4 compat: pares-radix's `praxis` crate re-exports these dataflow AST types
// under their historical `Px*` names (`crates/praxis/src/lib.rs`). The M6 AST
// migration renamed them to px-ast's canonical `Dataflow*Decl`/`Dataflow*`
// shapes; alias them back so downstream consumers that import
// `pluresdb_px::px::PxDataflow{Procedure,Param,Return}` keep resolving without
// source churn (the "transparent to consumers" contract of the SSOT re-export).
pub use px_ast::{
    DataflowParam as PxDataflowParam, DataflowProcedureDecl as PxDataflowProcedure,
    DataflowReturn as PxDataflowReturn,
};

// Public expr renderer (Expr -> canonical executor source form). Re-exported so
// external consumers (pluresdb-node's .px loader) reuse the ONE renderer rather
// than duplicating it (ADR-0010).
pub use compiler::expr_to_string;

// Back-compat namespaced alias: some call sites reference `px::pxlang::parse`
// (the M6.1 differential-testing name). Keep it as a thin alias of the
// now-top-level re-exports so those sites need no churn.
pub mod pxlang {
    //! Alias of the crate's praxis-lang `.px` engine re-exports (see parent).
    pub use px_ast::{self, PxDocument, Statement};
    pub use px_ast::{
        DataflowParam as PxDataflowParam, DataflowProcedureDecl as PxDataflowProcedure,
        DataflowReturn as PxDataflowReturn,
    };
    pub use px_compiler::{parse, parse_statement, CompileError};
    pub use px_eval;
}
