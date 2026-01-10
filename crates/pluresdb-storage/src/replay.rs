//! WAL replay and rebuild utilities.
//!
//! This module provides tools to rebuild database state from WAL operations,
//! including CRDT state reconstruction and index rebuilding.

use crate::wal::{WalEntry, WalOperation, WriteAheadLog};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, debug};

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
    
    let entries = wal.read_all().await
        .context("Failed to read WAL entries")?;
    
    let mut state = HashMap::new();
    let mut stats = ReplayStats::default();
    stats.total_entries = entries.len() as u64;
    
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
            anyhow::bail!(
                "WAL validation failed: {} corrupted entries, {} corrupted segments",
                validation.corrupted_entries,
                validation.corrupted_segments
            );
        }
    }
    
    // Replay operations
    replay_wal(wal_path, None).await
}

/// CRDT metadata pruning utilities.
pub mod metadata_pruning {
    use chrono::{DateTime, Utc, Duration};
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
    pub fn prune_vector_clock(
        clock: &mut HashMap<String, u64>,
        min_value: u64,
    ) -> usize {
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
            assert!(prunable.contains(&"old".to_string()));
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wal::{WalOperation, WriteAheadLog};
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
        ).await.unwrap();
        
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-2".to_string(),
                data: serde_json::json!({"value": 2}),
            },
        ).await.unwrap();
        
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
        ).await.unwrap();
        
        wal.append(
            "actor-1".to_string(),
            WalOperation::Delete {
                id: "node-1".to_string(),
            },
        ).await.unwrap();
        
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
        ).await.unwrap();
        
        wal.append(
            "actor-2".to_string(),
            WalOperation::Put {
                id: "node-2".to_string(),
                data: serde_json::json!({"value": 2}),
            },
        ).await.unwrap();
        
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
            ).await.unwrap();
        }
        
        // Rebuild with validation
        let (state, stats) = rebuild_from_wal(temp_dir.path(), true).await.unwrap();
        
        assert_eq!(state.len(), 10);
        assert_eq!(stats.success_rate(), 1.0);
    }
}
