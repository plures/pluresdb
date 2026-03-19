//! Core query operations.
//!
//! Each module exposes a single free function that takes a slice of
//! [`NodeRecord`]s and returns a transformed `Vec<NodeRecord>` (or a
//! [`ProcedureResult`] in the case of mutate/aggregate).

pub mod aggregate;
pub mod filter;
pub mod graph;
pub mod mutate;
pub mod project;
pub mod search;
pub mod sort;
pub mod transform;
