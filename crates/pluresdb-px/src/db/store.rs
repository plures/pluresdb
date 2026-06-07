//! [`PraxisStore`] — in-process PluresDB store for praxis primitives.
//!
//! This module provides the storage layer for the praxis runtime.  It holds
//! [`Constraint`], [`Adr`], and [`Evidence`] records indexed by their stable
//! IDs.  In production this would delegate to the `pluresdb-core` crate; here
//! we use in-memory `HashMap`s so the crate has zero external runtime deps.

use std::collections::HashMap;

use thiserror::Error;

use crate::db::schema::{Adr, Constraint, Evidence};

// ---------------------------------------------------------------------------
// StoreError
// ---------------------------------------------------------------------------

/// Errors produced by [`PraxisStore`] operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum StoreError {
    /// A record with that ID already exists.
    #[error("duplicate ID: {0}")]
    DuplicateId(String),
    /// No record was found for the given ID.
    #[error("not found: {0}")]
    NotFound(String),
}

// ---------------------------------------------------------------------------
// PraxisStore
// ---------------------------------------------------------------------------

/// In-process PluresDB store holding the three praxis collections.
///
/// All mutations are synchronous and fallible; concurrent access is left to the
/// caller (wrap in `Arc<Mutex<…>>` if needed).
#[derive(Default)]
pub struct PraxisStore {
    constraints: HashMap<String, Constraint>,
    adrs: HashMap<String, Adr>,
    evidence: HashMap<String, Evidence>,
}

impl PraxisStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    // ── Constraints ──────────────────────────────────────────────────────────

    /// Insert a new [`Constraint`].  Returns `Err(DuplicateId)` if the ID
    /// already exists.
    pub fn insert_constraint(&mut self, c: Constraint) -> Result<(), StoreError> {
        if self.constraints.contains_key(&c.id) {
            return Err(StoreError::DuplicateId(c.id));
        }
        self.constraints.insert(c.id.clone(), c);
        Ok(())
    }

    /// Insert or replace an existing [`Constraint`].
    pub fn upsert_constraint(&mut self, c: Constraint) {
        self.constraints.insert(c.id.clone(), c);
    }

    /// Retrieve a [`Constraint`] by ID.
    #[must_use]
    pub fn get_constraint(&self, id: &str) -> Option<&Constraint> {
        self.constraints.get(id)
    }

    /// Iterate over all constraints in arbitrary order.
    pub fn constraints(&self) -> impl Iterator<Item = &Constraint> {
        self.constraints.values()
    }

    /// Number of constraints stored.
    #[must_use]
    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }

    /// Remove a [`Constraint`] by ID.  Returns the removed constraint, or
    /// `Err(NotFound)` if no constraint with the given ID exists.
    pub fn remove_constraint(&mut self, id: &str) -> Result<Constraint, StoreError> {
        self.constraints
            .remove(id)
            .ok_or_else(|| StoreError::NotFound(id.to_string()))
    }

    // ── ADRs ─────────────────────────────────────────────────────────────────

    /// Insert a new [`Adr`].
    pub fn insert_adr(&mut self, adr: Adr) -> Result<(), StoreError> {
        if self.adrs.contains_key(&adr.id) {
            return Err(StoreError::DuplicateId(adr.id));
        }
        self.adrs.insert(adr.id.clone(), adr);
        Ok(())
    }

    /// Insert or replace an [`Adr`].
    pub fn upsert_adr(&mut self, adr: Adr) {
        self.adrs.insert(adr.id.clone(), adr);
    }

    /// Retrieve an [`Adr`] by ID.
    #[must_use]
    pub fn get_adr(&self, id: &str) -> Option<&Adr> {
        self.adrs.get(id)
    }

    /// Iterate over all ADRs.
    pub fn adrs(&self) -> impl Iterator<Item = &Adr> {
        self.adrs.values()
    }

    /// Number of ADRs stored.
    #[must_use]
    pub fn adr_count(&self) -> usize {
        self.adrs.len()
    }

    // ── Evidence ─────────────────────────────────────────────────────────────

    /// Insert a new [`Evidence`] record.
    pub fn insert_evidence(&mut self, e: Evidence) -> Result<(), StoreError> {
        if self.evidence.contains_key(&e.id) {
            return Err(StoreError::DuplicateId(e.id));
        }
        self.evidence.insert(e.id.clone(), e);
        Ok(())
    }

    /// Insert or replace an [`Evidence`] record.
    pub fn upsert_evidence(&mut self, e: Evidence) {
        self.evidence.insert(e.id.clone(), e);
    }

    /// Retrieve an [`Evidence`] record by ID.
    #[must_use]
    pub fn get_evidence(&self, id: &str) -> Option<&Evidence> {
        self.evidence.get(id)
    }

    /// Iterate over all evidence records.
    pub fn evidence_records(&self) -> impl Iterator<Item = &Evidence> {
        self.evidence.values()
    }

    /// Number of evidence records stored.
    #[must_use]
    pub fn evidence_count(&self) -> usize {
        self.evidence.len()
    }

    // ── Graph traversal ───────────────────────────────────────────────────────

    /// Resolve the [`Adr`] records linked from a [`Constraint`]'s `evidence`
    /// edge list.  Missing IDs are silently skipped.
    pub fn constraint_adrs(&self, constraint_id: &str) -> Vec<&Adr> {
        self.constraints
            .get(constraint_id)
            .map(|c| {
                c.evidence
                    .iter()
                    .filter_map(|id| self.adrs.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolve the [`Evidence`] records linked from an [`Adr`]'s `evidence`
    /// edge list.  Missing IDs are silently skipped.
    pub fn adr_evidence(&self, adr_id: &str) -> Vec<&Evidence> {
        self.adrs
            .get(adr_id)
            .map(|a| {
                a.evidence
                    .iter()
                    .filter_map(|id| self.evidence.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::{AdrStatus, Condition, EvidenceResult, Severity};
    use chrono::Utc;
    use std::collections::HashMap;

    fn make_constraint(id: &str) -> Constraint {
        Constraint {
            id: id.into(),
            description: "test constraint".into(),
            when: Condition::Always,
            require: Condition::Always,
            fix: "no action needed".into(),
            evidence: vec![],
            severity: Severity::Error,
        }
    }

    fn make_adr(id: &str) -> Adr {
        Adr {
            id: id.into(),
            title: "Test ADR".into(),
            status: AdrStatus::Accepted,
            evidence: vec![],
        }
    }

    fn make_evidence(id: &str) -> Evidence {
        Evidence {
            id: id.into(),
            tested_at: Utc::now(),
            condition: HashMap::new(),
            result: EvidenceResult::Passed,
            reference: "https://example.com".into(),
        }
    }

    #[test]
    fn store_starts_empty() {
        let store = PraxisStore::new();
        assert_eq!(store.constraint_count(), 0);
        assert_eq!(store.adr_count(), 0);
        assert_eq!(store.evidence_count(), 0);
    }

    #[test]
    fn insert_constraint_duplicate_returns_error() {
        let mut store = PraxisStore::new();
        store.insert_constraint(make_constraint("C-0001")).unwrap();
        let err = store
            .insert_constraint(make_constraint("C-0001"))
            .unwrap_err();
        assert_eq!(err, StoreError::DuplicateId("C-0001".into()));
    }

    #[test]
    fn get_constraint_returns_inserted() {
        let mut store = PraxisStore::new();
        store.insert_constraint(make_constraint("C-0001")).unwrap();
        assert!(store.get_constraint("C-0001").is_some());
        assert!(store.get_constraint("C-9999").is_none());
    }

    #[test]
    fn upsert_constraint_overwrites() {
        let mut store = PraxisStore::new();
        store.insert_constraint(make_constraint("C-0001")).unwrap();
        let mut c = make_constraint("C-0001");
        c.description = "updated".into();
        store.upsert_constraint(c);
        assert_eq!(
            store.get_constraint("C-0001").unwrap().description,
            "updated"
        );
    }

    #[test]
    fn insert_adr_and_evidence() {
        let mut store = PraxisStore::new();
        store.insert_adr(make_adr("ADR-0001")).unwrap();
        store.insert_evidence(make_evidence("EV-0001")).unwrap();
        assert_eq!(store.adr_count(), 1);
        assert_eq!(store.evidence_count(), 1);
    }

    #[test]
    fn constraint_adrs_traversal() {
        let mut store = PraxisStore::new();
        let mut c = make_constraint("C-0001");
        c.evidence = vec!["ADR-0001".into(), "ADR-MISSING".into()];
        store.insert_constraint(c).unwrap();
        store.insert_adr(make_adr("ADR-0001")).unwrap();

        let adrs = store.constraint_adrs("C-0001");
        assert_eq!(adrs.len(), 1);
        assert_eq!(adrs[0].id, "ADR-0001");
    }

    #[test]
    fn adr_evidence_traversal() {
        let mut store = PraxisStore::new();
        let mut adr = make_adr("ADR-0001");
        adr.evidence = vec!["EV-0001".into()];
        store.insert_adr(adr).unwrap();
        store.insert_evidence(make_evidence("EV-0001")).unwrap();

        let ev = store.adr_evidence("ADR-0001");
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].id, "EV-0001");
    }

    #[test]
    fn constraint_adrs_missing_returns_empty() {
        let store = PraxisStore::new();
        assert!(store.constraint_adrs("C-NONE").is_empty());
    }

    #[test]
    fn remove_constraint_existing() {
        let mut store = PraxisStore::new();
        store.insert_constraint(make_constraint("C-0001")).unwrap();
        let removed = store.remove_constraint("C-0001").unwrap();
        assert_eq!(removed.id, "C-0001");
        assert!(store.get_constraint("C-0001").is_none());
        assert_eq!(store.constraint_count(), 0);
    }

    #[test]
    fn remove_constraint_not_found() {
        let mut store = PraxisStore::new();
        let err = store.remove_constraint("C-NOPE").unwrap_err();
        assert_eq!(err, StoreError::NotFound("C-NOPE".into()));
    }
}
