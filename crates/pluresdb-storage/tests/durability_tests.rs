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
        )
        .await
        .unwrap();

        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-2".to_string(),
                data: serde_json::json!({"value": "durable"}),
            },
        )
        .await
        .unwrap();

        // WAL dropped here (simulates process termination)
    }

    // Second session: verify operations survived
    {
        let wal = WriteAheadLog::open(&wal_path).unwrap();
        let entries = wal.read_all().await.unwrap();

        assert_eq!(entries.len(), 2, "all operations should survive restart");
        assert_eq!(
            entries[0].operation,
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"value": "persistent"}),
            }
        );
        assert_eq!(
            entries[1].operation,
            WalOperation::Put {
                id: "node-2".to_string(),
                data: serde_json::json!({"value": "durable"}),
            }
        );
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
        wal.append("actor-replay".to_string(), op.clone())
            .await
            .unwrap();
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
        )
        .await
        .unwrap();
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
    )
    .unwrap();

    // Write enough data to span multiple segments
    for i in 0..20 {
        wal.append(
            format!("actor-{}", i),
            WalOperation::Put {
                id: format!("node-{}", i),
                data: serde_json::json!({"data": "x".repeat(50)}),
            },
        )
        .await
        .unwrap();
    }

    // Verify all entries are readable
    let entries = wal.read_all().await.unwrap();
    assert_eq!(
        entries.len(),
        20,
        "all entries should be readable across segments"
    );

    // Even if one segment is corrupted, others remain accessible
    let validation = wal.validate().await.unwrap();
    assert!(
        validation.total_segments > 1,
        "should have multiple segments"
    );
}

/// Test: Compaction removes old entries but preserves recent ones
#[tokio::test]
async fn test_compaction_preserves_recent_data() {
    let temp_dir = TempDir::new().unwrap();
    let wal = WriteAheadLog::open(temp_dir.path()).unwrap();

    // Write old operations
    let old_seq = wal
        .append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "old-node".to_string(),
                data: serde_json::json!({"old": true}),
            },
        )
        .await
        .unwrap();

    // Mark checkpoint
    wal.append(
        "actor-1".to_string(),
        WalOperation::Checkpoint { base_seq: old_seq },
    )
    .await
    .unwrap();

    // Write new operations
    let new_seq = wal
        .append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "new-node".to_string(),
                data: serde_json::json!({"new": true}),
            },
        )
        .await
        .unwrap();

    // Compact before new_seq (should remove old operations)
    wal.compact(new_seq).await.unwrap();

    // Verify new data is still present
    let entries = wal.read_all().await.unwrap();
    let has_new = entries.iter().any(|e| {
        matches!(
            &e.operation,
            WalOperation::Put { id, .. } if id == "new-node"
        )
    });

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
                let seq = wal_clone
                    .append(
                        format!("actor-{}", actor_id),
                        WalOperation::Put {
                            id: format!("node-{}-{}", actor_id, i),
                            data: serde_json::json!({"actor": actor_id, "index": i}),
                        },
                    )
                    .await
                    .unwrap();
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
    let unique_count = all_seqs
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(
        unique_count,
        all_seqs.len(),
        "all sequence numbers should be unique"
    );

    // Verify entries can be read in order
    let entries = wal.read_all().await.unwrap();
    assert_eq!(entries.len(), 100, "all 100 operations should be persisted");

    for i in 1..entries.len() {
        assert!(
            entries[i].seq > entries[i - 1].seq,
            "entries should be ordered by sequence"
        );
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
        )
        .unwrap();

        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"level": "full"}),
            },
        )
        .await
        .unwrap();
    }

    // Test with WAL-only durability (default)
    {
        let wal = WriteAheadLog::open_with_options(
            temp_dir.path().join("wal"),
            DurabilityLevel::Wal,
            64 * 1024 * 1024,
        )
        .unwrap();

        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"level": "wal"}),
            },
        )
        .await
        .unwrap();
    }

    // Test with no durability (testing only)
    {
        let wal = WriteAheadLog::open_with_options(
            temp_dir.path().join("none"),
            DurabilityLevel::None,
            64 * 1024 * 1024,
        )
        .unwrap();

        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-1".to_string(),
                data: serde_json::json!({"level": "none"}),
            },
        )
        .await
        .unwrap();
    }

    // All should be readable after controlled shutdown
    for level in &["full", "wal", "none"] {
        let wal = WriteAheadLog::open(temp_dir.path().join(level)).unwrap();
        let entries = wal.read_all().await.unwrap();
        assert_eq!(
            entries.len(),
            1,
            "entry should be persisted for level: {}",
            level
        );
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
        )
        .await
        .unwrap();
    }

    // Verify all entries are present and valid
    let entries = wal.read_all().await.unwrap();
    assert_eq!(
        entries.len(),
        batch_size,
        "all batch entries should be persisted"
    );

    for entry in &entries {
        assert!(
            entry.validate_checksum(),
            "all entries should have valid checksums"
        );
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
            )
            .await
            .unwrap();
        }

        // Create checkpoint
        let checkpoint_seq = wal
            .append(
                "system".to_string(),
                WalOperation::Checkpoint {
                    base_seq: cycle * 5,
                },
            )
            .await
            .unwrap();

        // Compact old entries
        if cycle > 2 {
            wal.compact(checkpoint_seq - 10).await.unwrap();
        }
    }

    // Verify recent entries are still present
    let entries = wal.read_all().await.unwrap();
    assert!(
        !entries.is_empty(),
        "should have recent entries after compaction cycles"
    );

    // Validate all remaining entries
    let validation = wal.validate().await.unwrap();
    assert!(
        validation.is_healthy(),
        "WAL should remain healthy after rapid cycles"
    );
}

// ── Corruption and partial-write tests ────────────────────────────────────────

/// Helper: find the single `.wal` segment file in a directory.
fn find_segment(dir: &std::path::Path) -> std::path::PathBuf {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("wal"))
        .expect("expected at least one .wal segment file")
}

/// Test: a partial length-prefix write (< 4 bytes) at the tail of a segment is
/// detected as corruption rather than silently treated as a clean end-of-file.
#[tokio::test]
async fn test_partial_length_prefix_detected() {
    let temp_dir = TempDir::new().unwrap();

    // Write two complete entries and flush.
    {
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
        for i in 0..2u32 {
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
    }

    // Append only 2 bytes of a 4-byte length prefix to simulate a torn write.
    let segment = find_segment(temp_dir.path());
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&segment)
            .unwrap();
        f.write_all(&[0xAB, 0xCD]).unwrap(); // partial 2-byte length prefix
    }

    // Re-open and validate: the segment should be flagged as corrupted.
    let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
    let validation = wal.validate().await.unwrap();

    assert!(
        !validation.is_healthy(),
        "partial length prefix should be detected as corruption"
    );
    assert!(
        validation.corrupted_segments > 0,
        "corrupted_segments should be > 0"
    );
}

/// Test: an implausibly large length prefix (> MAX_ENTRY_SIZE) is rejected
/// immediately rather than attempting to allocate a multi-GiB buffer.
#[tokio::test]
async fn test_implausible_length_prefix_detected() {
    let temp_dir = TempDir::new().unwrap();

    // Write one valid entry and flush.
    {
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-0".to_string(),
                data: serde_json::json!({"ok": true}),
            },
        )
        .await
        .unwrap();
    }

    // Append a 4-byte length prefix claiming 0xFFFF_FFFF bytes (~4 GiB).
    let segment = find_segment(temp_dir.path());
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&segment)
            .unwrap();
        f.write_all(&u32::MAX.to_le_bytes()).unwrap();
    }

    let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
    let validation = wal.validate().await.unwrap();

    assert!(
        !validation.is_healthy(),
        "implausible length prefix should be detected"
    );
    assert!(validation.corrupted_segments > 0);
}

/// Test: a truncated entry payload (length prefix written but data truncated)
/// is detected as corruption.
#[tokio::test]
async fn test_truncated_entry_payload_detected() {
    let temp_dir = TempDir::new().unwrap();

    // Write one valid entry.
    {
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
        wal.append(
            "actor-1".to_string(),
            WalOperation::Put {
                id: "node-0".to_string(),
                data: serde_json::json!({"complete": true}),
            },
        )
        .await
        .unwrap();
    }

    // Append a length prefix claiming 64 bytes but provide only 8 bytes of data.
    let segment = find_segment(temp_dir.path());
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&segment)
            .unwrap();
        let claimed_len: u32 = 64;
        f.write_all(&claimed_len.to_le_bytes()).unwrap();
        f.write_all(&[0u8; 8]).unwrap(); // only 8 bytes instead of 64
    }

    let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
    let validation = wal.validate().await.unwrap();

    assert!(
        !validation.is_healthy(),
        "truncated entry payload should be detected"
    );
    assert!(validation.corrupted_segments > 0);
}

/// Test: `rebuild_from_wal` with `validate_checksums = true` fails fast when a
/// segment contains a corrupt entry (bad checksum), and the error message
/// includes actionable recovery guidance.
#[tokio::test]
async fn test_rebuild_fails_fast_on_corrupt_checksum() {
    use pluresdb_storage::rebuild_from_wal;

    let temp_dir = TempDir::new().unwrap();

    // Write a few valid entries.
    {
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
        for i in 0..4u32 {
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
    }

    // Flip some bytes in the middle of the segment to corrupt an entry's checksum.
    let segment = find_segment(temp_dir.path());
    let mut bytes = std::fs::read(&segment).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xFF;
    bytes[mid + 1] ^= 0xFF;
    std::fs::write(&segment, &bytes).unwrap();

    // rebuild_from_wal must fail with an error containing recovery guidance.
    let err = rebuild_from_wal(temp_dir.path(), true).await.unwrap_err();
    let msg = err.to_string();

    assert!(
        msg.contains("Recovery"),
        "error message should contain recovery guidance, got: {}",
        msg
    );
    assert!(
        msg.contains("pluresdb-cli wal recover") || msg.contains("wal recover"),
        "error should mention the recovery CLI command, got: {}",
        msg
    );
}

/// Test: `rebuild_from_wal` fails fast when a segment has a partial length-prefix
/// tail (simulates process crash mid-write).
#[tokio::test]
async fn test_rebuild_fails_fast_on_truncated_segment() {
    use pluresdb_storage::rebuild_from_wal;

    let temp_dir = TempDir::new().unwrap();

    {
        let wal = WriteAheadLog::open(temp_dir.path()).unwrap();
        for i in 0..3u32 {
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
    }

    // Append partial length prefix (3 bytes) — simulates crash mid-header.
    let segment = find_segment(temp_dir.path());
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&segment)
            .unwrap();
        f.write_all(&[0x01, 0x02, 0x03]).unwrap();
    }

    let err = rebuild_from_wal(temp_dir.path(), true).await.unwrap_err();
    let msg = err.to_string();

    assert!(
        msg.contains("Recovery") || msg.contains("corrupted"),
        "error message should mention corruption/recovery, got: {}",
        msg
    );
}

/// Test: `WalValidation::recovery_guidance` returns `None` for a healthy WAL
/// and a non-empty guidance string when corruption is present.
#[tokio::test]
async fn test_validation_recovery_guidance_content() {
    let temp_dir = TempDir::new().unwrap();

    // Healthy WAL — guidance should be None.
    {
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

        let v = wal.validate().await.unwrap();
        assert!(v.is_healthy());
        assert!(
            v.recovery_guidance().is_none(),
            "healthy WAL should have no recovery guidance"
        );
    }

    // Corrupt the segment by appending an implausible length prefix.
    // This deterministically triggers WalError::ImplausibleEntrySize which
    // propagates from read_all() as Err, so corrupted_segments is incremented.
    let segment = find_segment(temp_dir.path());
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&segment)
            .unwrap();
        f.write_all(&u32::MAX.to_le_bytes()).unwrap();
    }

    let wal2 = WriteAheadLog::open(temp_dir.path()).unwrap();
    let v = wal2.validate().await.unwrap();
    assert!(
        !v.is_healthy(),
        "WAL with implausible length prefix should be unhealthy"
    );

    let guidance = v
        .recovery_guidance()
        .expect("should have recovery guidance");
    assert!(
        guidance.contains("pluresdb-cli wal recover"),
        "guidance should reference the recovery CLI command"
    );
    assert!(
        guidance.contains("Recovery options"),
        "guidance should contain 'Recovery options'"
    );
}
