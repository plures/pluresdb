//! `pluresdb-chronos` — Graph-native application state chronicle.
//!
//! Zero-effort observability through PluresDB state diffs. Every data mutation
//! is recorded with causal chains, actor attribution, timestamps, and
//! constraint evaluation results.
//!
//! # Architecture
//!
//! Chronos sits between your application writes and PluresDB, recording a
//! causal version timeline:
//!
//! ```text
//! Application write → ChronosTimeline.record() → PluresDB (chronos: prefix)
//!                                                    ↓
//!                                              Causal chains
//!                                              Actor attribution
//!                                              Level filtering
//!                                              JSONL file sink (optional)
//! ```
//!
//! # Quick Start
//!
//! ```rust
//! use std::sync::Arc;
//! use pluresdb_core::CrdtStore;
//! use pluresdb_chronos::{ChronosTimeline, ChronosAction};
//! use serde_json::json;
//!
//! let store = Arc::new(CrdtStore::default());
//! let timeline = ChronosTimeline::new(store);
//!
//! let entry = timeline.build_entry(
//!     "user:123",
//!     "my-app",
//!     ChronosAction::Create,
//!     &json!({"name": "Alice"}),
//!     vec![],
//!     Some("user registration".into()),
//! );
//! timeline.record(&entry);
//!
//! // Query the timeline
//! let history = timeline.history("user:123", 10);
//! let recent = timeline.recent(50);
//! ```
//!
//! # Features
//!
//! - **Causal chains** — every entry links to its parent via `parent_id`
//! - **Level filtering** — Debug/Info/Warn/Error severity with runtime control
//! - **JSONL file sink** — optional daily log files for cross-machine analysis
//! - **Timeline queries** — by key, actor, time range, or severity
//! - **Replay** — walk entries chronologically between two points

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use pluresdb_core::CrdtStore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

/// The PluresDB actor used for Chronos writes.
const CHRONOS_ACTOR: &str = "chronos";

/// Recording severity level for Chronos events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChronosLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

impl ChronosLevel {
    /// Parse from string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    fn as_u8(self) -> u8 {
        self as u8
    }

    fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Debug,
            1 => Self::Info,
            2 => Self::Warn,
            3 => Self::Error,
            _ => Self::Info,
        }
    }
}

impl std::fmt::Display for ChronosLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A version timeline entry — records every data mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChronosEntry {
    pub id: String,
    pub timestamp: u64,
    pub actor: String,
    pub key: String,
    pub action: ChronosAction,
    pub level: ChronosLevel,
    pub data_hash: String,
    pub parent_id: Option<String>,
    pub rationale: Option<String>,
    pub constraint_results: Vec<String>,
}

/// The kind of mutation recorded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChronosAction {
    Create,
    Update,
    Delete,
    Move,
    /// Agent received a user message.
    MessageReceived,
    /// Agent generated a response.
    ResponseGenerated,
    /// A tool was invoked.
    ToolInvoked,
    /// Context manager adjusted the window.
    ContextManaged,
    /// Model was called (conscious/deep/bitnet).
    ModelCalled,
    /// Outcome recorded (user correction or acceptance).
    OutcomeRecorded,
}

impl std::fmt::Display for ChronosAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Create => write!(f, "Create"),
            Self::Update => write!(f, "Update"),
            Self::Delete => write!(f, "Delete"),
            Self::Move => write!(f, "Move"),
            Self::MessageReceived => write!(f, "MessageReceived"),
            Self::ResponseGenerated => write!(f, "ResponseGenerated"),
            Self::ToolInvoked => write!(f, "ToolInvoked"),
            Self::ContextManaged => write!(f, "ContextManaged"),
            Self::ModelCalled => write!(f, "ModelCalled"),
            Self::OutcomeRecorded => write!(f, "OutcomeRecorded"),
        }
    }
}

/// Causal version timeline backed by PluresDB.
///
/// Stores to PluresDB (primary) and optionally writes JSONL to a file sink
/// for cross-machine debugging and analysis.
pub struct ChronosTimeline {
    store: Arc<CrdtStore>,
    /// Optional JSONL output directory. When set, every record() also
    /// appends the entry as one JSON line to `<dir>/YYYY-MM-DD.jsonl`.
    jsonl_dir: Option<std::path::PathBuf>,
    /// Minimum recording level. Events below this level are silently dropped.
    min_level: AtomicU8,
}

impl ChronosTimeline {
    /// Create a new timeline backed by the given store.
    pub fn new(store: Arc<CrdtStore>) -> Self {
        Self {
            store,
            jsonl_dir: None,
            min_level: AtomicU8::new(ChronosLevel::Info.as_u8()),
        }
    }

    /// Create a timeline with JSONL file output.
    pub fn with_jsonl(store: Arc<CrdtStore>, dir: std::path::PathBuf) -> Self {
        std::fs::create_dir_all(&dir).ok();
        Self {
            store,
            jsonl_dir: Some(dir),
            min_level: AtomicU8::new(ChronosLevel::Info.as_u8()),
        }
    }

    /// Enable JSONL output from an environment variable.
    pub fn with_jsonl_from_env(store: Arc<CrdtStore>) -> Self {
        if let Ok(dir) = std::env::var("PLURESDB_CHRONOS_DIR") {
            let path = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&path).ok();
            tracing::info!(dir = %path.display(), "chronos JSONL output enabled");
            Self {
                store,
                jsonl_dir: Some(path),
                min_level: AtomicU8::new(ChronosLevel::Info.as_u8()),
            }
        } else {
            Self {
                store,
                jsonl_dir: None,
                min_level: AtomicU8::new(ChronosLevel::Info.as_u8()),
            }
        }
    }

    /// Set the minimum recording level. Events below this are dropped.
    pub fn set_level(&self, level: ChronosLevel) {
        self.min_level.store(level.as_u8(), Ordering::Relaxed);
    }

    /// Get the current minimum recording level.
    pub fn get_level(&self) -> ChronosLevel {
        ChronosLevel::from_u8(self.min_level.load(Ordering::Relaxed))
    }

    /// Build a new [`ChronosEntry`] for a write, automatically resolving the
    /// causal parent from the latest entry for this key.
    pub fn build_entry(
        &self,
        key: &str,
        actor: &str,
        action: ChronosAction,
        data: &Value,
        constraint_results: Vec<String>,
        rationale: Option<String>,
    ) -> ChronosEntry {
        self.build_entry_with_level(key, actor, action, ChronosLevel::Info, data, constraint_results, rationale)
    }

    /// Build an entry with an explicit severity level.
    #[allow(clippy::too_many_arguments)]
    pub fn build_entry_with_level(
        &self,
        key: &str,
        actor: &str,
        action: ChronosAction,
        level: ChronosLevel,
        data: &Value,
        constraint_results: Vec<String>,
        rationale: Option<String>,
    ) -> ChronosEntry {
        let parent_id = self.latest(key).map(|e| e.id);
        let data_hash = sha256_json(data);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        ChronosEntry {
            id: Uuid::new_v4().to_string(),
            timestamp,
            actor: actor.to_string(),
            key: key.to_string(),
            action,
            level,
            data_hash,
            parent_id,
            rationale,
            constraint_results,
        }
    }

    /// Record a mutation in the timeline. Returns false if filtered by level.
    pub fn record(&self, entry: &ChronosEntry) -> bool {
        let min = ChronosLevel::from_u8(self.min_level.load(Ordering::Relaxed));
        if entry.level < min {
            return false;
        }
        let entry_key = format!("chronos:entry:{}", entry.id);
        self.store.put(
            entry_key,
            CHRONOS_ACTOR,
            serde_json::to_value(entry).expect("ChronosEntry serializes"),
        );

        // Update the "latest" pointer for this data key.
        let latest_key = format!("chronos:latest:{}", entry.key);
        self.store.put(
            latest_key,
            CHRONOS_ACTOR,
            json!({ "entry_id": entry.id, "timestamp": entry.timestamp }),
        );

        // JSONL file sink — one line per entry, daily files.
        if let Some(ref dir) = self.jsonl_dir {
            if let Ok(json) = serde_json::to_string(entry) {
                let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
                let path = dir.join(format!("{date}.jsonl"));
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                {
                    use std::io::Write;
                    let _ = writeln!(file, "{json}");
                }
            }
        }
        true
    }

    /// Get the version history for a key (newest first), up to `limit`.
    pub fn history(&self, data_key: &str, limit: usize) -> Vec<ChronosEntry> {
        let mut entries: Vec<ChronosEntry> = self
            .store
            .list()
            .into_iter()
            .filter_map(|r| {
                let e: ChronosEntry = serde_json::from_value(r.data).ok()?;
                if e.key == data_key {
                    Some(e)
                } else {
                    None
                }
            })
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.timestamp));
        entries.truncate(limit);
        entries
    }

    /// Get the latest entry for a key.
    pub fn latest(&self, data_key: &str) -> Option<ChronosEntry> {
        let latest_key = format!("chronos:latest:{data_key}");
        let pointer = self.store.get(&latest_key)?;
        let entry_id = pointer.data.get("entry_id")?.as_str()?;
        let entry_key = format!("chronos:entry:{entry_id}");
        let record = self.store.get(&entry_key)?;
        serde_json::from_value(record.data).ok()
    }

    /// Get all entries by an actor (newest first), up to `limit`.
    pub fn by_actor(&self, actor: &str, limit: usize) -> Vec<ChronosEntry> {
        let mut entries: Vec<ChronosEntry> = self
            .store
            .list()
            .into_iter()
            .filter_map(|r| {
                let e: ChronosEntry = serde_json::from_value(r.data).ok()?;
                if e.actor == actor {
                    Some(e)
                } else {
                    None
                }
            })
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.timestamp));
        entries.truncate(limit);
        entries
    }

    /// Get entries in a time range (newest first), up to `limit`.
    pub fn in_range(&self, from: u64, to: u64, limit: usize) -> Vec<ChronosEntry> {
        let mut entries: Vec<ChronosEntry> = self
            .store
            .list()
            .into_iter()
            .filter_map(|r| {
                let e: ChronosEntry = serde_json::from_value(r.data).ok()?;
                if e.timestamp >= from && e.timestamp <= to {
                    Some(e)
                } else {
                    None
                }
            })
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.timestamp));
        entries.truncate(limit);
        entries
    }

    /// Timeline view: recent entries with optional `since` timestamp and `level` filter.
    /// Mirrors OpenClaw's `radix__chronos-timeline` behavior.
    /// `since` is a Unix timestamp in **seconds** — only entries after this time are included.
    /// `level` filters to entries at or above the given severity.
    pub fn timeline(
        &self,
        limit: usize,
        since: Option<u64>,
        level: Option<ChronosLevel>,
    ) -> Vec<ChronosEntry> {
        let mut entries: Vec<ChronosEntry> = self
            .store
            .list()
            .into_iter()
            .filter_map(|r| {
                let e: ChronosEntry = serde_json::from_value(r.data).ok()?;
                if let Some(ts) = since {
                    if e.timestamp < ts {
                        return None;
                    }
                }
                if let Some(ref lvl) = level {
                    if e.level < *lvl {
                        return None;
                    }
                }
                Some(e)
            })
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.timestamp));
        entries.truncate(limit);
        entries
    }

    /// Recent entries across all keys (newest first), up to `limit`.
    pub fn recent(&self, limit: usize) -> Vec<ChronosEntry> {
        let mut entries: Vec<ChronosEntry> = self
            .store
            .list()
            .into_iter()
            .filter_map(|r| serde_json::from_value(r.data).ok())
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.timestamp));
        entries.truncate(limit);
        entries
    }

    /// Replay timeline events between two entry IDs (inclusive).
    ///
    /// If `from_id` is None, starts from the oldest entry.
    /// If `to_id` is None, ends at the newest entry.
    /// Returns entries in chronological order (oldest first) for replay.
    pub fn replay(&self, from_id: Option<&str>, to_id: Option<&str>) -> Vec<ChronosEntry> {
        let all_entries: Vec<ChronosEntry> = self
            .store
            .list()
            .into_iter()
            .filter_map(|r| serde_json::from_value(r.data).ok())
            .collect();

        // Sort chronologically (oldest first) for replay, with ID as tiebreaker
        let mut sorted = all_entries;
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then_with(|| a.id.cmp(&b.id)));

        // Find the from/to boundaries
        let start_idx = match from_id {
            Some(fid) => sorted.iter().position(|e| e.id == fid).unwrap_or(0),
            None => 0,
        };
        let end_idx = match to_id {
            Some(tid) => sorted
                .iter()
                .position(|e| e.id == tid)
                .map(|i| i + 1)
                .unwrap_or(sorted.len()),
            None => sorted.len(),
        };

        if start_idx >= sorted.len() || start_idx >= end_idx {
            return Vec::new();
        }

        sorted[start_idx..end_idx].to_vec()
    }
}

/// SHA-256 hash of a JSON value (deterministic via to_string).
fn sha256_json(data: &Value) -> String {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(data).unwrap_or_default();
    format!("{:x}", Sha256::digest(&bytes))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> Arc<CrdtStore> {
        Arc::new(CrdtStore::default())
    }

    #[test]
    fn record_and_latest() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);

        let entry = timeline.build_entry(
            "test:key1",
            "actor-1",
            ChronosAction::Create,
            &json!({"hello": "world"}),
            vec!["praxis:no-secrets: pass".into()],
            Some("initial creation".into()),
        );
        assert!(entry.parent_id.is_none());
        timeline.record(&entry);

        let latest = timeline.latest("test:key1");
        assert!(latest.is_some());
        let latest = latest.unwrap();
        assert_eq!(latest.id, entry.id);
        assert_eq!(latest.action, ChronosAction::Create);
    }

    #[test]
    fn causal_chain() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);

        let e1 = timeline.build_entry("k", "a", ChronosAction::Create, &json!(1), vec![], None);
        timeline.record(&e1);

        let e2 = timeline.build_entry("k", "a", ChronosAction::Update, &json!(2), vec![], None);
        assert_eq!(e2.parent_id.as_deref(), Some(e1.id.as_str()));
        timeline.record(&e2);

        let hist = timeline.history("k", 10);
        assert_eq!(hist.len(), 2);
        // Both entries should be present; latest pointer should be e2.
        let latest = timeline.latest("k").unwrap();
        assert_eq!(latest.id, e2.id);
    }

    #[test]
    fn by_actor_filter() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);

        let e1 = timeline.build_entry(
            "k1",
            "alice",
            ChronosAction::Create,
            &json!(1),
            vec![],
            None,
        );
        timeline.record(&e1);
        let e2 = timeline.build_entry("k2", "bob", ChronosAction::Create, &json!(2), vec![], None);
        timeline.record(&e2);

        assert_eq!(timeline.by_actor("alice", 10).len(), 1);
        assert_eq!(timeline.by_actor("bob", 10).len(), 1);
        assert_eq!(timeline.by_actor("nobody", 10).len(), 0);
    }

    #[test]
    fn recent_returns_all() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);

        for i in 0..5 {
            let e = timeline.build_entry(
                &format!("k{i}"),
                "a",
                ChronosAction::Create,
                &json!(i),
                vec![],
                None,
            );
            timeline.record(&e);
        }

        assert_eq!(timeline.recent(3).len(), 3);
        assert_eq!(timeline.recent(10).len(), 5);
    }

    #[test]
    fn level_filtering() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);

        // Default level is Info — debug entries should be dropped.
        let debug_entry = timeline.build_entry_with_level(
            "k",
            "a",
            ChronosAction::Create,
            ChronosLevel::Debug,
            &json!("debug-data"),
            vec![],
            None,
        );
        assert!(!timeline.record(&debug_entry));
        assert_eq!(timeline.recent(10).len(), 0);

        // Info entry should be recorded.
        let info_entry = timeline.build_entry(
            "k",
            "a",
            ChronosAction::Create,
            &json!("info-data"),
            vec![],
            None,
        );
        assert!(timeline.record(&info_entry));
        assert_eq!(timeline.recent(10).len(), 1);

        // Lower level to debug — now debug entries should be recorded.
        timeline.set_level(ChronosLevel::Debug);
        let debug_entry2 = timeline.build_entry_with_level(
            "k2",
            "a",
            ChronosAction::Create,
            ChronosLevel::Debug,
            &json!("debug2"),
            vec![],
            None,
        );
        assert!(timeline.record(&debug_entry2));
        assert_eq!(timeline.recent(10).len(), 2);

        // Raise level to error — info entries should be dropped.
        timeline.set_level(ChronosLevel::Error);
        let info_entry2 = timeline.build_entry(
            "k3",
            "a",
            ChronosAction::Create,
            &json!("info2"),
            vec![],
            None,
        );
        assert!(!timeline.record(&info_entry2));
        assert_eq!(timeline.recent(10).len(), 2);

        // Error entry should still be recorded.
        let err_entry = timeline.build_entry_with_level(
            "k4",
            "a",
            ChronosAction::Create,
            ChronosLevel::Error,
            &json!("error"),
            vec![],
            None,
        );
        assert!(timeline.record(&err_entry));
        assert_eq!(timeline.recent(10).len(), 3);
    }

    #[test]
    fn get_set_level() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);
        assert_eq!(timeline.get_level(), ChronosLevel::Info);

        timeline.set_level(ChronosLevel::Warn);
        assert_eq!(timeline.get_level(), ChronosLevel::Warn);

        timeline.set_level(ChronosLevel::Debug);
        assert_eq!(timeline.get_level(), ChronosLevel::Debug);
    }

    #[test]
    fn replay_all_entries() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);
        timeline.set_level(ChronosLevel::Debug);

        let mut ids = Vec::new();
        for i in 0..3 {
            let e = timeline.build_entry(
                &format!("key{i}"),
                "agent",
                ChronosAction::Create,
                &json!({"val": i}),
                vec![],
                None,
            );
            timeline.record(&e);
            ids.push(e.id.clone());
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Replay all
        let replayed = timeline.replay(None, None);
        assert_eq!(replayed.len(), 3);
        // Should be chronological (oldest first)
        assert!(replayed[0].timestamp <= replayed[1].timestamp);
        assert!(replayed[1].timestamp <= replayed[2].timestamp);
    }

    #[test]
    fn replay_with_range() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);
        timeline.set_level(ChronosLevel::Debug);

        let mut entries_created = Vec::new();
        for i in 0..5 {
            let e = timeline.build_entry(
                &format!("key{i}"),
                "agent",
                ChronosAction::Create,
                &json!({"val": i}),
                vec![],
                None,
            );
            timeline.record(&e);
            entries_created.push(e);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Get all replayed entries and verify they're all present
        let all_replayed = timeline.replay(None, None);
        assert_eq!(all_replayed.len(), 5);

        // Pick the first and last entries from the replayed list (sorted by timestamp+id)
        let first_id = &all_replayed[0].id;
        let last_id = &all_replayed[4].id;

        // Replay the full range using first and last IDs
        let replayed = timeline.replay(Some(first_id), Some(last_id));
        assert_eq!(replayed.len(), 5);

        // Replay a subset: entries 1..3 (inclusive)
        let mid_from = &all_replayed[1].id;
        let mid_to = &all_replayed[3].id;
        let subset = timeline.replay(Some(mid_from), Some(mid_to));
        assert_eq!(subset.len(), 3);
        assert_eq!(subset[0].id, *mid_from);
        assert_eq!(subset[2].id, *mid_to);
    }

    #[test]
    fn replay_empty_timeline() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);
        let replayed = timeline.replay(None, None);
        assert!(replayed.is_empty());
    }

    #[test]
    fn timeline_basic() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);

        let entry = timeline.build_entry(
            "tl:key1",
            "actor1",
            ChronosAction::Create,
            &serde_json::json!({"x": 1}),
            vec![],
            None,
        );
        timeline.record(&entry);

        let results = timeline.timeline(50, None, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "tl:key1");
    }

    #[test]
    fn timeline_level_filter() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);
        // Lower min level so debug entries get recorded
        timeline.set_level(ChronosLevel::Debug);

        let debug_entry = timeline.build_entry_with_level(
            "tl:debug",
            "actor1",
            ChronosAction::Create,
            ChronosLevel::Debug,
            &serde_json::json!("d"),
            vec![],
            None,
        );
        timeline.record(&debug_entry);

        let warn_entry = timeline.build_entry_with_level(
            "tl:warn",
            "actor1",
            ChronosAction::Update,
            ChronosLevel::Warn,
            &serde_json::json!("w"),
            vec![],
            None,
        );
        timeline.record(&warn_entry);

        // Filter to warn+ should only return the warn entry
        let results = timeline.timeline(50, None, Some(ChronosLevel::Warn));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "tl:warn");

        // No filter returns both
        let results = timeline.timeline(50, None, None);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn timeline_since_filter() {
        let store = test_store();
        let timeline = ChronosTimeline::new(store);

        let entry = timeline.build_entry(
            "tl:recent",
            "actor1",
            ChronosAction::Create,
            &serde_json::json!("r"),
            vec![],
            None,
        );
        timeline.record(&entry);

        // Since far future: nothing
        let results = timeline.timeline(50, Some(u64::MAX), None);
        assert_eq!(results.len(), 0);

        // Since epoch 0: everything
        let results = timeline.timeline(50, Some(0), None);
        assert_eq!(results.len(), 1);
    }
}
