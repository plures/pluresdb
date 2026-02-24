//! Re-export of the [`pred!`] compile-time predicate macro.
//!
//! The macro is implemented in the `pluresdb-procedures-macros` crate (a
//! proc-macro crate) and re-exported here for convenience.
//!
//! # Example
//!
//! ```rust,ignore
//! use pluresdb_procedures::pred;
//!
//! let p = pred!(category == "decision");
//! let p = pred!(data.score > 0.7);
//! let p = pred!(category == "decision" && data.score > 0.7);
//! ```
//!
//! Syntax errors are reported at compile time:
//!
//! ```rust,compile_fail
//! use pluresdb_procedures::pred;
//! let _ = pred!(123 == "oops");  // compile error: LHS must be a field path
//! ```

pub use pluresdb_procedures_macros::pred;
