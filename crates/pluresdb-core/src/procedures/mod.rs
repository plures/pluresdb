//! High-level document storage and enrichment procedures for PluresDB.
//!
//! These procedures extend the core [`CrdtStore`] API with domain-specific
//! operations for managing documents, chunks, and their relationships.
//!
//! # Modules
//!
//! - [`document`] — procedures for storing, linking, and enriching documents
//!   and their constituent chunks.

pub mod document;

pub use document::{
    enrich_document_metadata, link_document_chunks, store_document, store_document_chunk,
};
