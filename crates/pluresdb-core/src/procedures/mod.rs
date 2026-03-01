//! High-level document storage and enrichment procedures for PluresDB.
//!
//! These procedures extend the core [`CrdtStore`] API with domain-specific
//! operations for managing documents, chunks, and their relationships.
//!
//! # Modules
//!
//! - [`document`] — procedures for storing, linking, and enriching documents
//!   and their constituent chunks.
//! - [`training`] — event-driven and periodic procedures for training data
//!   processing (enrichment, contradiction detection, context attachment,
//!   pair generation, quality scoring, and JSONL export).

pub mod document;
pub mod training;

pub use document::{
    enrich_document_metadata, link_document_chunks, store_document, store_document_chunk,
};
pub use training::{
    consolidate_training_pairs, export_training_set, on_memory_insert_attach_context,
    on_memory_insert_detect_contradictions, on_memory_insert_enrich, score_quality,
    TrainingPair,
};
