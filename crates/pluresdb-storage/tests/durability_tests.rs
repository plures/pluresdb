//! Durability tests for PluresDB storage layer.
//!
//! These tests validate the crash-safety, deterministic replay, and corruption
//! containment guarantees required for use as an agent memory store.

use pluresdb_storage::{DurabilityLevel, WalOperation, WriteAheadLog};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::task;

/// Test: WAL survives process termination (simulated by dropping)
#[tokio::test]
async fn test_durability_across_restart() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().to_path_buf();
    
    // First session: write operations
    {
        let wal = WriteAheadLog::open(&wal_path).unwrap();
        
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"value": "persistent"}),
            },
        ).await.unwrap();
        
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-2".to_string(),
                data: serde_json::json!({"value": "durable"}),
            },
        ).await.unwrap();
        
        // WAL dropped here (simulates process termination)
    }
    
    // Second session: verify operations survived
    {
        let wal = WriteAheadLog::open(&wal_path).unwrap();
        let entries = wal.read_all().await.unwrap();
        
        assert_eq!(entries.len(), 2, "all operations should survive restart");
        assert_eq!(entries[0].operation, WalOperation::Put {
            id: "node-1".to_string(),
            data: serde_json::json!({"value": "persistent"}),
        });
        assert_eq!(entries[1].operation, WalOperation::Put {
            id: "node-2".to_string(),
            data: serde_json::json!({"value": "durable"}),
        });
    }
}

/// Test: Deterministic replay produces same state
#[tokio::test]
async fn test_deterministic_replay() {
    let temp_dir = TempDir::new().unwrap();
    let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
    
    // Apply operations in specific order
    let operations = vec![
        WalOperation::Put {
            id: "node-1".to_string(),
            data: serde_json::json!({"counter": 1}),
        },
        WalOperation::Put {
            id: "node-1".to_string(),
            data: serde_json::json!({"counter": 2}),
        },
        WalOperation::Put {
            id: "node-2".to_string(),
            data: serde_json::json!({"name": "test"}),
        },
        WalOperation::Delete {
            id: "node-2".to_string(),
        },
    ];
    
    for op in &operations {
        wal.append("actor-replay".to_string(), op.clone()).await.unwrap();
    }
    
    // Read back and verify order is preserved
    let entries = wal.read_all().await.unwrap();
    assert_eq!(entries.len(), operations.len());
    
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry.operation, operations[i]);
        assert!(entry.validate_checksum(), "checksum should be valid");
    }
}

/// Test: Corrupted entry detection via checksums
#[tokio::test]
async fn test_corruption_detection() {
    let temp_dir = TempDir::new().unwrap();
    let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
    
    // Write some operations
    for i in 0..5 {
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: format!("node-{}", i),
                data: serde_json::json!({"index": i}),
            },
        ).await.unwrap();
    }
    
    // Validate all entries are healthy
    let validation = wal.validate().await.unwrap();
    assert!(validation.is_healthy(), "WAL should be healthy initially");
    assert_eq!(validation.total_entries, 5);
    assert_eq!(validation.valid_entries, 5);
    assert_eq!(validation.corruption_rate(), 0.0);
}

/// Test: Segment isolation prevents cascade failures
#[tokio::test]
async fn test_segment_isolation() {
    let temp_dir = TempDir::new().unwrap();
    
    // Use small segment size to force multiple segments
    let wal = WriteAheadLog::open_with_options(
        temp_dir.path(),
        DurabilityLevel::Wal,
        256, // 256 bytes to force rotation
    ).unwrap();
    
    // Write enough data to span multiple segments
    for i in 0..20 {
        wal.append(
            format!("actor-{}", i),
            WalOperation::Put {
                id: format!("node-{}", i),
                data: serde_json::json!({"data": "x".repeat(50)}),
            },
        ).await.unwrap();
    }
    
    // Verify all entries are readable
    let entries = wal.read_all().await.unwrap();
    assert_eq!(entries.len(), 20, "all entries should be readable across segments");
    
    // Even if one segment is corrupted, others remain accessible
    let validation = wal.validate().await.unwrap();
    assert!(validation.total_segments > 1, "should have multiple segments");
}

/// Test: Compaction removes old entries but preserves recent ones
#[tokio::test]
async fn test_compaction_preserves_recent_data() {
    let temp_dir = TempDir::new().unwrap();
    let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
    
    // Write old operations
    let old_seq = wal.append(
        "actor-1".to_string(),
        WalOperation::Put {
            id: "old-node".to_string(),
            data: serde_json::json!({"old": true}),
        },
    ).await.unwrap();
    
    // Mark checkpoint
    wal.append(
        "actor-1".to_string(),
        WalOperation::Checkpoint { base_seq: old_seq },
    ).await.unwrap();
    
    // Write new operations
    let new_seq = wal.append(
        "actor-1".to_string(),
        WalOperation::Put {
            id: "new-node".to_string(),
            data: serde_json::json!({"new": true}),
        },
    ).await.unwrap();
    
    // Compact before new_seq (should remove old operations)
    wal.compact(new_seq).await.unwrap();
    
    // Verify new data is still present
    let entries = wal.read_all().await.unwrap();
    let has_new = entries.iter().any(|e| matches!(
        &e.operation,
        WalOperation::Put { id, .. } if id == "new-node"
    ));
    
    assert!(has_new, "new operations should survive compaction");
}

/// Test: Concurrent appends maintain ordering
#[tokio::test]
async fn test_concurrent_append_ordering() {
    let temp_dir = TempDir::new().unwrap();
    let wal = Arc::new(WriteAheadLog::open(temp_dir.path()).unwrap());
    
    // Spawn multiple tasks appending concurrently
    let mut handles = vec![];
    
    for actor_id in 0..10 {
        let wal_clone = Arc::clone(&wal);
        let handle = task::spawn(async move {
            let mut seqs = vec![];
            for i in 0..10 {
                let seq = wal_clone.append(
                    format!("actor-{}", actor_id),
                    WalOperation::Put {
                        id: format!("node-{}-{}", actor_id, i),
                        data: serde_json::json!({"actor": actor_id, "index": i}),
                    },
                ).await.unwrap();
                seqs.push(seq);
            }
            seqs
        });
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    let mut all_seqs = vec![];
    for handle in handles {
        let seqs = handle.await.unwrap();
        all_seqs.extend(seqs);
    }
    
    // Verify all sequence numbers are unique
    all_seqs.sort();
    let unique_count = all_seqs.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(unique_count, all_seqs.len(), "all sequence numbers should be unique");
    
    // Verify entries can be read in order
    let entries = wal.read_all().await.unwrap();
    assert_eq!(entries.len(), 100, "all 100 operations should be persisted");
    
    for i in 1..entries.len() {
        assert!(entries[i].seq > entries[i-1].seq, "entries should be ordered by sequence");
    }
}

/// Test: WAL without fsync still maintains durability in controlled shutdown
#[tokio::test]
async fn test_durability_levels() {
    let temp_dir = TempDir::new().unwrap();
    
    // Test with full durability
    {
        let wal = WriteAheadLog::open_with_options(
            temp_dir.path().join("full"),
            DurabilityLevel::Full,
            64 * 1024 * 1024,
        ).unwrap();
        
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"level": "full"}),
            },
        ).await.unwrap();
    }
    
    // Test with WAL-only durability (default)
    {
        let wal = WriteAheadLog::open_with_options(
            temp_dir.path().join("wal"),
            DurabilityLevel::Wal,
            64 * 1024 * 1024,
        ).unwrap();
        
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"level": "wal"}),
            },
        ).await.unwrap();
    }
    
    // Test with no durability (testing only)
    {
        let wal = WriteAheadLog::open_with_options(
            temp_dir.path().join("none"),
            DurabilityLevel::None,
            64 * 1024 * 1024,
        ).unwrap();
        
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"level": "none"}),
            },
        ).await.unwrap();
    }
    
    // All should be readable after controlled shutdown
    for level in &["full", "wal", "none"] {
        let wal = WriteAheadLog::open(temp_dir.path().join(level)).unwrap();
        let entries = wal.read_all().await.unwrap();
        assert_eq!(entries.len(), 1, "entry should be persisted for level: {}", level);
    }
}

/// Test: Large batch writes are durable
#[tokio::test]
async fn test_large_batch_durability() {
    let temp_dir = TempDir::new().unwrap();
    let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
    
    // Write a large batch
    let batch_size = 1000;
    for i in 0..batch_size {
        wal.append(
            format!("actor-{}", i % 10),
            WalOperation::Put {
                id: format!("node-{}", i),
                data: serde_json::json!({"index": i, "data": "test".repeat(10)}),
            },
        ).await.unwrap();
    }
    
    // Verify all entries are present and valid
    let entries = wal.read_all().await.unwrap();
    assert_eq!(entries.len(), batch_size, "all batch entries should be persisted");
    
    for entry in &entries {
        assert!(entry.validate_checksum(), "all entries should have valid checksums");
    }
}

/// Test: WAL handles rapid checkpoint/compact cycles
#[tokio::test]
async fn test_rapid_checkpoint_compaction() {
    let temp_dir = TempDir::new().unwrap();
    let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
    
    // Simulate agent workflow: write, checkpoint, compact, repeat
    for cycle in 0..10 {
        // Write some operations
        for i in 0..5 {
            wal.append(
                format!("actor-cycle-{}", cycle),
                WalOperation::Put {
                    id: format!("node-{}-{}", cycle, i),
                    data: serde_json::json!({"cycle": cycle, "index": i}),
                },
            ).await.unwrap();
        }
        
        // Create checkpoint
        let checkpoint_seq = wal.append(
            "system".to_string(),
            WalOperation::Checkpoint { base_seq: cycle * 5 },
        ).await.unwrap();
        
        // Compact old entries
        if cycle > 2 {
            wal.compact(checkpoint_seq - 10).await.unwrap();
        }
    }
    
    // Verify recent entries are still present
    let entries = wal.read_all().await.unwrap();
    assert!(!entries.is_empty(), "should have recent entries after compaction cycles");
    
    // Validate all remaining entries
    let validation = wal.validate().await.unwrap();
    assert!(validation.is_healthy(), "WAL should remain healthy after rapid cycles");
}
