//! WAL replay and rebuild utilities.
//!
//! This module provides tools to rebuild database state from WAL operations,
//! including CRDT state reconstruction and index rebuilding.

use crate::wal::{WalOperation, WriteAheadLog};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info};

/// Statistics from a WAL replay operation.
#[derive(Debug, Clone, Default)]
pub struct ReplayStats {
    /// Total entries processed
    pub total_entries: u64,

    /// Put operations applied
    pub puts: u64,

    /// Delete operations applied
    pub deletes: u64,

    /// Checkpoint markers encountered
    pub checkpoints: u64,

    /// Compact operations encountered
    pub compacts: u64,

    /// Entries skipped due to errors
    pub errors: u64,

    /// Final node count
    pub final_node_count: usize,
}

impl ReplayStats {
    /// Returns the success rate of the replay.
    pub fn success_rate(&self) -> f64 {
        if self.total_entries == 0 {
            return 1.0;
        }
        (self.total_entries - self.errors) as f64 / self.total_entries as f64
    }
}

/// Replays WAL operations to reconstruct state.
pub async fn replay_wal(
    wal_path: &Path,
    filter_actor: Option<&str>,
) -> Result<(HashMap<String, serde_json::Value>, ReplayStats)> {
    info!(?wal_path, ?filter_actor, "Starting WAL replay");

    let wal = WriteAheadLog::open(wal_path)
        .with_context(|| format!("Failed to open WAL at {}", wal_path.display()))?;

    let entries = wal.read_all().await.context("Failed to read WAL entries")?;

    let mut state = HashMap::new();
    let mut stats = ReplayStats {
        total_entries: entries.len() as u64,
        ..Default::default()
    };

    for entry in entries {
        // Apply actor filter if specified
        if let Some(actor) = filter_actor {
            if entry.actor != actor {
                continue;
            }
        }

        // Validate checksum
        if !entry.validate_checksum() {
            stats.errors += 1;
            debug!(seq = entry.seq, "Skipping entry with invalid checksum");
            continue;
        }

        // Apply operation
        match &entry.operation {
            WalOperation::Put { id, data } => {
                state.insert(id.clone(), data.clone());
                stats.puts += 1;
            }
            WalOperation::Delete { id } => {
                state.remove(id);
                stats.deletes += 1;
            }
            WalOperation::Checkpoint { .. } => {
                stats.checkpoints += 1;
            }
            WalOperation::Compact { .. } => {
                stats.compacts += 1;
            }
        }
    }

    stats.final_node_count = state.len();

    info!(
        puts = stats.puts,
        deletes = stats.deletes,
        final_count = stats.final_node_count,
        "WAL replay completed"
    );

    Ok((state, stats))
}

/// Rebuilds database state from WAL with validation.
pub async fn rebuild_from_wal(
    wal_path: &Path,
    validate_checksums: bool,
) -> Result<(HashMap<String, serde_json::Value>, ReplayStats)> {
    info!(?wal_path, validate_checksums, "Rebuilding from WAL");

    let wal = WriteAheadLog::open(wal_path)?;

    // Validate WAL if requested
    if validate_checksums {
        let validation = wal.validate().await?;
        if !validation.is_healthy() {
            let guidance = validation.recovery_guidance().unwrap_or_default();
            anyhow::bail!(
                "WAL validation failed: {} corrupted entr{}, {} corrupted segment{}.\n{}",
                validation.corrupted_entries,
                if validation.corrupted_entries == 1 {
                    "y"
                } else {
                    "ies"
                },
                validation.corrupted_segments,
                if validation.corrupted_segments == 1 {
                    ""
                } else {
                    "s"
                },
                guidance,
            );
        }
    }

    // Replay operations
    replay_wal(wal_path, None).await
}

/// CRDT metadata pruning utilities.
pub mod metadata_pruning {
    use chrono::{DateTime, Duration, Utc};
    use std::collections::HashMap;

    /// Configuration for CRDT metadata pruning.
    #[derive(Debug, Clone)]
    pub struct PruningConfig {
        /// Actors inactive for longer than this are candidates for pruning
        pub inactive_threshold: Duration,

        /// Minimum clock value to retain (based on global sync state)
        pub min_clock_value: u64,
    }

    impl Default for PruningConfig {
        fn default() -> Self {
            Self {
                inactive_threshold: Duration::days(30),
                min_clock_value: 0,
            }
        }
    }

    /// Statistics from a pruning operation.
    #[derive(Debug, Clone, Default)]
    pub struct PruningStats {
        pub total_actors: usize,
        pub pruned_actors: usize,
        pub total_clock_entries: usize,
        pub pruned_clock_entries: usize,
    }

    /// Identifies actors that can be safely pruned.
    pub fn identify_prunable_actors(
        actor_last_seen: &HashMap<String, DateTime<Utc>>,
        config: &PruningConfig,
    ) -> Vec<String> {
        let now = Utc::now();
        let threshold = now - config.inactive_threshold;

        actor_last_seen
            .iter()
            .filter(|(_, &last_seen)| last_seen < threshold)
            .map(|(actor, _)| actor.clone())
            .collect()
    }

    /// Prunes vector clock entries below the minimum value.
    pub fn prune_vector_clock(clock: &mut HashMap<String, u64>, min_value: u64) -> usize {
        let initial_size = clock.len();
        clock.retain(|_, &mut value| value >= min_value);
        initial_size - clock.len()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_identify_prunable_actors() {
            let mut last_seen = HashMap::new();
            last_seen.insert("active".to_string(), Utc::now());
            last_seen.insert("old".to_string(), Utc::now() - Duration::days(60));

            let config = PruningConfig::default();
            let prunable = identify_prunable_actors(&last_seen, &config);

            assert_eq!(prunable.len(), 1);
            assert_eq!(prunable[0], "old");
        }

        #[test]
        fn test_prune_vector_clock() {
            let mut clock = HashMap::new();
            clock.insert("actor1".to_string(), 100);
            clock.insert("actor2".to_string(), 50);
            clock.insert("actor3".to_string(), 200);

            let pruned = prune_vector_clock(&mut clock, 75);

            assert_eq!(pruned, 1); // actor2 removed
            assert_eq!(clock.len(), 2);
            assert!(clock.contains_key("actor1"));
            assert!(clock.contains_key("actor3"));
        }

        /// Kills replay.rs:200 `initial_size - clock.len()` -> `initial_size /
        /// clock.len()`.
        ///
        /// The previous test prunes 3 -> 2 (removed 1), but `3 - 2 == 1` and
        /// `3 / 2 == 1` (integer division) are indistinguishable, so it does NOT
        /// catch the `/` mutant. Here we prune 3 -> 1 (removed 2): the correct
        /// subtraction yields 2 while `3 / 1` yields 3.
        #[test]
        fn prune_vector_clock_returns_subtraction_not_division() {
            let mut clock = HashMap::new();
            clock.insert("a".to_string(), 10);
            clock.insert("b".to_string(), 20);
            clock.insert("c".to_string(), 30);

            // min_value 25 keeps only "c" (value 30); "a" and "b" are removed.
            let pruned = prune_vector_clock(&mut clock, 25);

            assert_eq!(
                pruned, 2,
                "must report 2 removed (3 - 1); the `/` mutant would yield 3 (3 / 1)"
            );
            assert_eq!(clock.len(), 1);
            assert!(clock.contains_key("c"));
        }

        /// Documents the boundary intent of replay.rs:191
        /// `last_seen < threshold`. An actor strictly newer than the threshold
        /// is retained; one strictly older is pruned.
        ///
        /// NOTE: the `<` -> `<=` mutant at L191 is only observable when
        /// `last_seen == threshold` EXACTLY. `threshold` is derived from
        /// `Utc::now()` sampled inside the function, which cannot be pinned from
        /// outside, so no external input can force that exact equality. This
        /// test therefore asserts the strict-ordering behavior on both sides of
        /// the boundary but does not claim to kill the `<=` mutant; see the
        /// commit body for the equivalence rationale.
        #[test]
        fn identify_prunable_actors_strict_ordering() {
            let mut last_seen = HashMap::new();
            // Clearly newer than a 30-day threshold => retained.
            last_seen.insert("fresh".to_string(), Utc::now() - Duration::days(1));
            // Clearly older => pruned.
            last_seen.insert("stale".to_string(), Utc::now() - Duration::days(90));

            let config = PruningConfig::default();
            let prunable = identify_prunable_actors(&last_seen, &config);

            assert_eq!(prunable, vec!["stale".to_string()]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wal::{WalEntry, WalOperation, WriteAheadLog};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_replay_wal_basic() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();

        // Write some operations
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"value": 1}),
            },
        )
        .await
        .unwrap();

        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-2".to_string(),
                data: serde_json::json!({"value": 2}),
            },
        )
        .await
        .unwrap();

        // Replay
        let (state, stats) = replay_wal(temp_dir.path(), None).await.unwrap();

        assert_eq!(state.len(), 2);
        assert_eq!(stats.puts, 2);
        assert_eq!(stats.deletes, 0);
        assert_eq!(stats.final_node_count, 2);
    }

    #[tokio::test]
    async fn test_replay_with_deletes() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();

        // Put then delete
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"value": 1}),
            },
        )
        .await
        .unwrap();

        wal.append(
            "actor-1".to_string(),
            WalOperation::Delete {
                id: "node-1".to_string(),
            },
        )
        .await
        .unwrap();

        // Replay
        let (state, stats) = replay_wal(temp_dir.path(), None).await.unwrap();

        assert_eq!(state.len(), 0);
        assert_eq!(stats.puts, 1);
        assert_eq!(stats.deletes, 1);
        assert_eq!(stats.final_node_count, 0);
    }

    #[tokio::test]
    async fn test_replay_with_actor_filter() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();

        // Operations from different actors
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"value": 1}),
            },
        )
        .await
        .unwrap();

        wal.append(
            "actor-2".to_string(),
            WalOperation::Put {
                id: "node-2".to_string(),
                data: serde_json::json!({"value": 2}),
            },
        )
        .await
        .unwrap();

        // Replay only actor-1
        let (state, stats) = replay_wal(temp_dir.path(), Some("actor-1")).await.unwrap();

        assert_eq!(state.len(), 1);
        assert!(state.contains_key("node-1"));
        assert!(!state.contains_key("node-2"));
        assert_eq!(stats.puts, 1);
    }

    #[tokio::test]
    async fn test_rebuild_from_wal_with_validation() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();

        // Write operations
        for i in 0..10 {
            wal.append(
                "actor-1".to_string(),
                WalOperation::Put {
                    id: format!("node-{}", i),
                    data: serde_json::json!({"index": i}),
                },
            )
            .await
            .unwrap();
        }

        // Rebuild with validation
        let (state, stats) = rebuild_from_wal(temp_dir.path(), true).await.unwrap();

        assert_eq!(state.len(), 10);
        assert_eq!(stats.success_rate(), 1.0);
    }

    // ---------------------------------------------------------------------
    // Mutation-hardening tests (Level-0 #6).
    // ---------------------------------------------------------------------

    /// Kills replay.rs:40 `success_rate -> 1.0`, replay.rs:40 `==` -> `!=`, and
    /// replay.rs:43 `-` -> `+`.
    ///
    /// - With known total=4, errors=1 the exact rate is 0.75. The `-> 1.0`
    ///   constant and the `-`->`+` mutant (which gives (4+1)/4 = 1.25) both
    ///   differ from 0.75.
    /// - The empty case (total=0) must short-circuit to 1.0; the `==`->`!=`
    ///   mutant skips that guard and computes 0/0 = NaN instead.
    #[test]
    fn success_rate_is_exact_and_handles_empty() {
        let stats = ReplayStats {
            total_entries: 4,
            errors: 1,
            ..Default::default()
        };
        assert_eq!(
            stats.success_rate(),
            0.75,
            "(4 - 1) / 4 must be 0.75; `-> 1.0` and `-`->`+` mutants differ"
        );

        let empty = ReplayStats::default();
        let rate = empty.success_rate();
        assert_eq!(
            rate, 1.0,
            "empty replay must report 1.0 via the total==0 guard"
        );
        assert!(
            !rate.is_nan(),
            "the total==0 guard must run; `==`->`!=` would yield 0/0 = NaN"
        );

        // A fully-failed replay is 0.0 (further separates from the `-> 1.0`
        // constant).
        let all_failed = ReplayStats {
            total_entries: 3,
            errors: 3,
            ..Default::default()
        };
        assert_eq!(all_failed.success_rate(), 0.0);
    }

    /// Kills replay.rs:61 `delete field total_entries from ReplayStats` in
    /// `replay_wal`.
    ///
    /// After replaying 3 written entries, `stats.total_entries` must equal 3.
    /// Dropping the field initializer defaults it to 0.
    #[tokio::test]
    async fn replay_wal_reports_total_entries() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
        for i in 0..3 {
            wal.append(
                "actor-1".to_string(),
                WalOperation::Put {
                    id: format!("node-{i}"),
                    data: serde_json::json!({ "i": i }),
                },
            )
            .await
            .unwrap();
        }

        let (_state, stats) = replay_wal(temp_dir.path(), None).await.unwrap();
        assert_eq!(
            stats.total_entries, 3,
            "total_entries must reflect the 3 replayed entries, not the default 0"
        );
    }

    /// Kills replay.rs:91 `checkpoints += 1` and replay.rs:94 `compacts += 1`
    /// (`-=` underflow / `*=` stays 0) in `replay_wal`.
    ///
    /// We write two Checkpoint and two Compact operations and assert the exact
    /// counts. `+= 1` from 0 reaches 2; `-= 1` underflow-panics on u64 and
    /// `*= 1` stays 0.
    #[tokio::test]
    async fn replay_wal_counts_checkpoints_and_compacts() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();

        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "n0".to_string(),
                data: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
        wal.append(
            "actor-1".to_string(),
            WalOperation::Checkpoint { base_seq: 0 },
        )
        .await
        .unwrap();
        wal.append(
            "actor-1".to_string(),
            WalOperation::Compact {
                before_timestamp: 1,
            },
        )
        .await
        .unwrap();
        wal.append(
            "actor-1".to_string(),
            WalOperation::Checkpoint { base_seq: 1 },
        )
        .await
        .unwrap();
        wal.append(
            "actor-1".to_string(),
            WalOperation::Compact {
                before_timestamp: 2,
            },
        )
        .await
        .unwrap();

        let (_state, stats) = replay_wal(temp_dir.path(), None).await.unwrap();
        assert_eq!(stats.checkpoints, 2, "two Checkpoint ops must be counted");
        assert_eq!(stats.compacts, 2, "two Compact ops must be counted");
        assert_eq!(stats.puts, 1);
    }

    /// Kills replay.rs:75 `errors += 1` (`-=` underflow / `*=` stays 0) in
    /// `replay_wal`.
    ///
    /// We write two valid entries, then tamper their on-disk checksums to 0 so
    /// both fail `validate_checksum()` during replay. `errors` must reach 2 and
    /// those entries must NOT be applied (state stays empty).
    #[tokio::test]
    async fn replay_wal_counts_checksum_errors() {
        let temp_dir = TempDir::new().unwrap();
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
        for i in 0..2 {
            wal.append(
                "actor-1".to_string(),
                WalOperation::Put {
                    id: format!("node-{i}"),
                    data: serde_json::json!({ "i": i }),
                },
            )
            .await
            .unwrap();
        }
        // Flush active segment to disk.
        wal.read_all().await.unwrap();

        // Rewrite the segment with every entry's checksum forced to 0 (correct
        // length prefixes preserved), so each entry deserializes but fails the
        // checksum check on replay.
        let seg_path = {
            let mut segs: Vec<_> = std::fs::read_dir(temp_dir.path())
                .unwrap()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("wal"))
                .collect();
            segs.sort();
            segs.into_iter().next().unwrap()
        };
        let raw = std::fs::read(&seg_path).unwrap();
        let mut out: Vec<u8> = Vec::with_capacity(raw.len());
        let mut i = 0usize;
        while i + 4 <= raw.len() {
            let len =
                u32::from_le_bytes([raw[i], raw[i + 1], raw[i + 2], raw[i + 3]]) as usize;
            let start = i + 4;
            let end = start + len;
            if end > raw.len() {
                break;
            }
            let mut entry: WalEntry =
                serde_json::from_slice(&raw[start..end]).expect("valid entry");
            entry.checksum = 0; // tamper
            let body = serde_json::to_vec(&entry).unwrap();
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&body);
            i = end;
        }
        std::fs::write(&seg_path, &out).unwrap();

        let (state, stats) = replay_wal(temp_dir.path(), None).await.unwrap();
        assert_eq!(
            stats.errors, 2,
            "both tampered entries must be counted as errors; got {}",
            stats.errors
        );
        assert_eq!(stats.puts, 0, "checksum-failed entries must not be applied");
        assert!(state.is_empty(), "no state should be reconstructed from corrupt entries");
    }
}
