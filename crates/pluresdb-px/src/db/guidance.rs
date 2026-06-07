//! PluresDB schema and store for Praxis coprocessor guidance.
//!
//! This module defines the three guidance "tables" and a [`GuidanceStore`]
//! that persists them in-process.  In production the store would delegate to
//! the `pluresdb-core` crate; here we use in-memory `HashMap`s so the crate
//! has zero external runtime dependencies.
//!
//! | Table | Type |
//! |-------|------|
//! | guidance_entries | [`GuidanceEntry`] |
//! | source_spans | [`SourceSpan`] |
//! | analysis_events | [`AnalysisEvent`] |

use std::collections::{HashMap, VecDeque};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Schema types
// ---------------------------------------------------------------------------

/// Categories of Praxis coprocessor guidance displayed in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuidanceCategory {
    /// Factual statements derived from memory.
    Facts,
    /// Operative rules the agent should follow.
    Rules,
    /// Hard constraints that must not be violated.
    Constraints,
    /// Recorded decisions and their rationale.
    Decisions,
    /// Identified risks and mitigations.
    Risks,
    /// General advisory guidance and recommendations.
    Guidance,
}

impl GuidanceCategory {
    /// Return a stable kebab-case string identifier for this category.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Facts => "facts",
            Self::Rules => "rules",
            Self::Constraints => "constraints",
            Self::Decisions => "decisions",
            Self::Risks => "risks",
            Self::Guidance => "guidance",
        }
    }
}

/// A single guidance entry — one row in the `guidance_entries` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidanceEntry {
    /// Unique identifier for this guidance entry.
    pub id: String,
    /// Category of guidance (facts, rules, decisions, etc.).
    pub category: GuidanceCategory,
    /// Human-readable guidance content.
    pub content: String,
    /// Confidence score (0.0 to 1.0) for this guidance.
    pub confidence: f32,
    /// Source span IDs this guidance is derived from.
    pub source_spans: Vec<String>,
    /// RFC-3339 timestamp when this guidance was generated.
    pub generated_at: String,
    /// Priority level (1 = highest, 5 = lowest).
    pub priority: u8,
}

/// Source span — one row in the `source_spans` table.
///
/// Provides character-level traceability from a guidance entry back to the
/// memory content it was derived from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Stable identifier for this span (auto-assigned by [`GuidanceStore`]).
    pub id: String,
    /// Memory entry ID containing this span.
    pub memory_id: String,
    /// Start character position in the memory content.
    pub start_pos: usize,
    /// End character position in the memory content.
    pub end_pos: usize,
    /// The actual text content of the span.
    pub text: String,
    /// Relevance score for this span to the guidance.
    pub relevance: f32,
}

/// Analysis event — one row in the `analysis_events` table.
///
/// Captures a snapshot of a single analysis run so that the history of
/// guidance changes is auditable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisEvent {
    /// Event identifier.
    pub id: String,
    /// Event type (e.g. `"memory_analyzed"`, `"correction_applied"`).
    pub event_type: String,
    /// RFC-3339 timestamp when the analysis completed.
    pub timestamp: String,
    /// Number of guidance entries updated by this analysis.
    pub guidance_updated: u32,
    /// Memory IDs that were analyzed.
    pub analyzed_memory_ids: Vec<String>,
}

// ---------------------------------------------------------------------------
// GuidanceStore
// ---------------------------------------------------------------------------

/// Maximum number of analysis events retained in the store.
const MAX_EVENTS: usize = 50;

/// In-process PluresDB store for the three guidance collections.
///
/// All mutations are synchronous and infallible (no uniqueness constraints on
/// spans or events).  Concurrent access is the caller's responsibility —
/// wrap in `Arc<Mutex<…>>` when sharing across threads.
#[derive(Default)]
pub struct GuidanceStore {
    entries: HashMap<String, GuidanceEntry>,
    spans: HashMap<String, SourceSpan>,
    /// Capped at [`MAX_EVENTS`]; oldest events are evicted from the front.
    events: VecDeque<AnalysisEvent>,
}

impl GuidanceStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    // ── Guidance entries ─────────────────────────────────────────────────────

    /// Insert a guidance entry.  If an entry with the same `id` already
    /// exists it is replaced (upsert semantics).
    pub fn upsert_entry(&mut self, entry: GuidanceEntry) {
        self.entries.insert(entry.id.clone(), entry);
    }

    /// Retrieve a guidance entry by ID.
    #[must_use]
    pub fn get_entry(&self, id: &str) -> Option<&GuidanceEntry> {
        self.entries.get(id)
    }

    /// Remove a guidance entry by ID.  Returns `true` if it existed.
    pub fn remove_entry(&mut self, id: &str) -> bool {
        self.entries.remove(id).is_some()
    }

    /// Return all entries that belong to `category`, sorted by priority
    /// ascending (1 = highest) then confidence descending.
    pub fn entries_by_category(&self, category: &GuidanceCategory) -> Vec<&GuidanceEntry> {
        let mut entries: Vec<&GuidanceEntry> = self
            .entries
            .values()
            .filter(|e| &e.category == category)
            .collect();
        entries.sort_by(|a, b| {
            a.priority.cmp(&b.priority).then_with(|| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });
        entries
    }

    /// Return all guidance entries in arbitrary order.
    pub fn all_entries(&self) -> Vec<&GuidanceEntry> {
        self.entries.values().collect()
    }

    /// Number of guidance entries stored.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    // ── Source spans ──────────────────────────────────────────────────────────

    /// Insert a source span.  The span's `id` field is used as the key; any
    /// existing span with the same ID is replaced.
    pub fn upsert_span(&mut self, span: SourceSpan) {
        self.spans.insert(span.id.clone(), span);
    }

    /// Retrieve a source span by ID.
    #[must_use]
    pub fn get_span(&self, id: &str) -> Option<&SourceSpan> {
        self.spans.get(id)
    }

    /// Look up multiple spans by their IDs.  Missing IDs are silently skipped.
    pub fn spans_by_ids(&self, ids: &[String]) -> Vec<&SourceSpan> {
        ids.iter()
            .filter_map(|id| self.spans.get(id.as_str()))
            .collect()
    }

    /// Number of source spans stored.
    #[must_use]
    pub fn span_count(&self) -> usize {
        self.spans.len()
    }

    // ── Analysis events ───────────────────────────────────────────────────────

    /// Append an analysis event.  When the store already holds [`MAX_EVENTS`]
    /// events the oldest one is evicted first (FIFO retention).
    pub fn push_event(&mut self, event: AnalysisEvent) {
        if self.events.len() >= MAX_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    /// Return up to `limit` of the most recent analysis events, newest first.
    pub fn recent_events(&self, limit: usize) -> Vec<&AnalysisEvent> {
        self.events.iter().rev().take(limit).collect()
    }

    /// Number of analysis events currently stored.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Clear all entries, spans, and events (useful for testing / reset).
    pub fn clear(&mut self) {
        self.entries.clear();
        self.spans.clear();
        self.events.clear();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(
        id: &str,
        category: GuidanceCategory,
        priority: u8,
        confidence: f32,
    ) -> GuidanceEntry {
        GuidanceEntry {
            id: id.to_string(),
            category,
            content: format!("content for {id}"),
            confidence,
            source_spans: vec![],
            generated_at: "2026-01-01T00:00:00Z".to_string(),
            priority,
        }
    }

    fn make_span(id: &str, memory_id: &str) -> SourceSpan {
        SourceSpan {
            id: id.to_string(),
            memory_id: memory_id.to_string(),
            start_pos: 0,
            end_pos: 10,
            text: "some text".to_string(),
            relevance: 0.8,
        }
    }

    fn make_event(id: &str, event_type: &str) -> AnalysisEvent {
        AnalysisEvent {
            id: id.to_string(),
            event_type: event_type.to_string(),
            timestamp: "2026-01-01T00:00:00Z".to_string(),
            guidance_updated: 1,
            analyzed_memory_ids: vec!["mem-1".to_string()],
        }
    }

    // ── Entry CRUD ───────────────────────────────────────────────────────────

    #[test]
    fn store_starts_empty() {
        let store = GuidanceStore::new();
        assert_eq!(store.entry_count(), 0);
        assert_eq!(store.span_count(), 0);
        assert_eq!(store.event_count(), 0);
    }

    #[test]
    fn upsert_and_get_entry() {
        let mut store = GuidanceStore::new();
        store.upsert_entry(make_entry("G-001", GuidanceCategory::Facts, 1, 0.9));
        assert!(store.get_entry("G-001").is_some());
        assert!(store.get_entry("G-999").is_none());
        assert_eq!(store.entry_count(), 1);
    }

    #[test]
    fn upsert_entry_replaces_existing() {
        let mut store = GuidanceStore::new();
        store.upsert_entry(make_entry("G-001", GuidanceCategory::Facts, 1, 0.9));
        let mut updated = make_entry("G-001", GuidanceCategory::Facts, 1, 0.9);
        updated.content = "updated content".to_string();
        store.upsert_entry(updated);
        assert_eq!(store.entry_count(), 1);
        assert_eq!(store.get_entry("G-001").unwrap().content, "updated content");
    }

    #[test]
    fn remove_entry_existing_returns_true() {
        let mut store = GuidanceStore::new();
        store.upsert_entry(make_entry("G-001", GuidanceCategory::Facts, 1, 0.9));
        assert!(store.remove_entry("G-001"));
        assert_eq!(store.entry_count(), 0);
        assert!(store.get_entry("G-001").is_none());
    }

    #[test]
    fn remove_entry_nonexistent_returns_false() {
        let mut store = GuidanceStore::new();
        assert!(!store.remove_entry("G-999"));
    }

    #[test]
    fn entries_by_category_filters_correctly() {
        let mut store = GuidanceStore::new();
        store.upsert_entry(make_entry("G-001", GuidanceCategory::Facts, 1, 0.9));
        store.upsert_entry(make_entry("G-002", GuidanceCategory::Rules, 1, 0.8));
        store.upsert_entry(make_entry("G-003", GuidanceCategory::Facts, 2, 0.7));

        let facts = store.entries_by_category(&GuidanceCategory::Facts);
        assert_eq!(facts.len(), 2);
        let rules = store.entries_by_category(&GuidanceCategory::Rules);
        assert_eq!(rules.len(), 1);
        let decisions = store.entries_by_category(&GuidanceCategory::Decisions);
        assert!(decisions.is_empty());
    }

    #[test]
    fn entries_by_category_sorts_by_priority_then_confidence() {
        let mut store = GuidanceStore::new();
        // same category, different priorities
        store.upsert_entry(make_entry("low", GuidanceCategory::Facts, 3, 0.9));
        store.upsert_entry(make_entry("high", GuidanceCategory::Facts, 1, 0.7));
        // same priority, different confidence
        store.upsert_entry(make_entry("med-hi", GuidanceCategory::Facts, 2, 0.9));
        store.upsert_entry(make_entry("med-lo", GuidanceCategory::Facts, 2, 0.5));

        let facts = store.entries_by_category(&GuidanceCategory::Facts);
        assert_eq!(facts[0].id, "high"); // priority 1
        assert_eq!(facts[1].id, "med-hi"); // priority 2, confidence 0.9
        assert_eq!(facts[2].id, "med-lo"); // priority 2, confidence 0.5
        assert_eq!(facts[3].id, "low"); // priority 3
    }

    #[test]
    fn all_entries_returns_everything() {
        let mut store = GuidanceStore::new();
        store.upsert_entry(make_entry("G-001", GuidanceCategory::Facts, 1, 0.9));
        store.upsert_entry(make_entry("G-002", GuidanceCategory::Rules, 2, 0.8));
        assert_eq!(store.all_entries().len(), 2);
    }

    // ── Span CRUD ────────────────────────────────────────────────────────────

    #[test]
    fn upsert_and_get_span() {
        let mut store = GuidanceStore::new();
        store.upsert_span(make_span("S-001", "mem-1"));
        assert!(store.get_span("S-001").is_some());
        assert!(store.get_span("S-999").is_none());
        assert_eq!(store.span_count(), 1);
    }

    #[test]
    fn spans_by_ids_resolves_existing() {
        let mut store = GuidanceStore::new();
        store.upsert_span(make_span("S-001", "mem-1"));
        store.upsert_span(make_span("S-002", "mem-2"));

        let ids = vec![
            "S-001".to_string(),
            "S-002".to_string(),
            "S-MISSING".to_string(),
        ];
        let spans = store.spans_by_ids(&ids);
        assert_eq!(spans.len(), 2);
    }

    // ── Event retention ───────────────────────────────────────────────────────

    #[test]
    fn push_event_appends_events() {
        let mut store = GuidanceStore::new();
        store.push_event(make_event("E-001", "memory_analyzed"));
        store.push_event(make_event("E-002", "correction_applied"));
        assert_eq!(store.event_count(), 2);
    }

    #[test]
    fn recent_events_returns_newest_first() {
        let mut store = GuidanceStore::new();
        store.push_event(make_event("E-001", "first"));
        store.push_event(make_event("E-002", "second"));
        store.push_event(make_event("E-003", "third"));

        let events = store.recent_events(10);
        assert_eq!(events[0].id, "E-003");
        assert_eq!(events[1].id, "E-002");
        assert_eq!(events[2].id, "E-001");
    }

    #[test]
    fn recent_events_respects_limit() {
        let mut store = GuidanceStore::new();
        for i in 0..10 {
            store.push_event(make_event(&format!("E-{i:03}"), "type"));
        }
        assert_eq!(store.recent_events(3).len(), 3);
    }

    #[test]
    fn event_retention_evicts_oldest_at_max() {
        let mut store = GuidanceStore::new();
        // Fill to the cap
        for i in 0..50 {
            store.push_event(make_event(&format!("E-{i:03}"), "type"));
        }
        assert_eq!(store.event_count(), 50);

        // One more push must evict the oldest (E-000)
        store.push_event(make_event("E-NEW", "type"));
        assert_eq!(store.event_count(), 50);

        // E-NEW is now the most recent
        let newest = store.recent_events(1);
        assert_eq!(newest[0].id, "E-NEW");

        // E-000 was evicted
        let all: Vec<&str> = store
            .recent_events(50)
            .iter()
            .map(|e| e.id.as_str())
            .collect();
        assert!(!all.contains(&"E-000"), "E-000 should have been evicted");
        assert!(all.contains(&"E-001"), "E-001 should still be present");
    }

    // ── clear ─────────────────────────────────────────────────────────────────

    #[test]
    fn clear_removes_all_data() {
        let mut store = GuidanceStore::new();
        store.upsert_entry(make_entry("G-001", GuidanceCategory::Facts, 1, 0.9));
        store.upsert_span(make_span("S-001", "mem-1"));
        store.push_event(make_event("E-001", "type"));

        store.clear();
        assert_eq!(store.entry_count(), 0);
        assert_eq!(store.span_count(), 0);
        assert_eq!(store.event_count(), 0);
    }
}
