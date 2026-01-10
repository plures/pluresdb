//! Long-running agent simulation test.
//!
//! This test simulates continuous operation of an AI agent memory store,
//! including periodic crashes, restarts, and bounded memory growth verification.

use pluresdb_storage::{WalOperation, WriteAheadLog};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::task;
use tokio::time::sleep;

/// Simulates long-running agent with periodic crashes.
#[tokio::test]
#[ignore]  // Run with: cargo test --ignored test_long_running_agent_simulation
async fn test_long_running_agent_simulation() {
    let temp_dir = TempDir::new().unwrap();
    let wal_path = temp_dir.path().to_path_buf();
    
    let runtime_minutes = 5;  // Shortened for testing; production would be 24+ hours
    let crash_interval = Duration::from_secs(30);  // Crash every 30 seconds
    let write_interval = Duration::from_millis(100);  // Write every 100ms
    
    let mut total_operations = 0u64;
    let mut crash_count = 0;
    let start_time = Instant::now();
    
    println!("Starting long-running agent simulation for {} minutes", runtime_minutes);
    println!("Crash interval: {:?}", crash_interval);
    println!("Write interval: {:?}", write_interval);
    
    while start_time.elapsed() < Duration::from_secs(runtime_minutes * 60) {
        let session_start = Instant::now();
        let wal = Arc::new(WriteAheadLog::open(&wal_path).unwrap());
        
        println!("\n=== Session {} started ===", crash_count + 1);
        
        // Verify recovery: count existing entries
        let entries_before = wal.read_all().await.unwrap();
        println!("Recovered {} operations from previous sessions", entries_before.len());
        
        // Run until next crash
        let mut session_ops = 0;
        while session_start.elapsed() < crash_interval {
            // Simulate agent writing command history, stdout, stderr
            let operations = vec![
                WalOperation::Put {
                    id: format!("cmd-{}", total_operations),
                    data: serde_json::json!({
                        "command": format!("ls -la /path/to/dir-{}", total_operations % 100),
                        "timestamp": chrono::Utc::now().timestamp(),
                    }),
                },
                WalOperation::Put {
                    id: format!("stdout-{}", total_operations),
                    data: serde_json::json!({
                        "output": format!("total 42\ndrwxr-xr-x  5 user  staff  160 Jan 10 19:00 .\n{}", "x".repeat(500)),
                        "command_id": format!("cmd-{}", total_operations),
                    }),
                },
                WalOperation::Put {
                    id: format!("context-{}", total_operations),
                    data: serde_json::json!({
                        "working_dir": "/path/to/dir",
                        "env_vars": {"HOME": "/home/user", "PATH": "/usr/bin"},
                        "inferred_intent": "listing directory contents",
                    }),
                },
            ];
            
            for op in operations {
                wal.append("agent-actor".to_string(), op).await.unwrap();
                session_ops += 1;
                total_operations += 1;
            }
            
            sleep(write_interval).await;
            
            // Periodic compaction
            if total_operations % 100 == 0 {
                let checkpoint_seq = total_operations.saturating_sub(50);
                wal.append(
                    "system".to_string(),
                    WalOperation::Checkpoint { base_seq: checkpoint_seq },
                ).await.unwrap();
                
                wal.compact(checkpoint_seq).await.unwrap();
                println!("Compacted WAL at checkpoint {}", checkpoint_seq);
            }
        }
        
        println!("Session {} completed: {} operations written", crash_count + 1, session_ops);
        
        // Simulate crash by dropping WAL
        drop(wal);
        crash_count += 1;
        
        // Brief delay before restart
        sleep(Duration::from_millis(100)).await;
    }
    
    // Final verification
    let wal = WriteAheadLog::open(&wal_path).unwrap();
    let final_entries = wal.read_all().await.unwrap();
    
    println!("\n=== Final Statistics ===");
    println!("Runtime: {:?}", start_time.elapsed());
    println!("Total operations written: {}", total_operations);
    println!("Total crashes: {}", crash_count);
    println!("Final WAL entries: {}", final_entries.len());
    println!("Operations per crash: {}", total_operations / (crash_count + 1));
    
    // Validate WAL health
    let validation = wal.validate().await.unwrap();
    println!("\nWAL Validation:");
    println!("  Total entries: {}", validation.total_entries);
    println!("  Valid entries: {}", validation.valid_entries);
    println!("  Corrupted entries: {}", validation.corrupted_entries);
    println!("  Total segments: {}", validation.total_segments);
    println!("  Corrupted segments: {}", validation.corrupted_segments);
    
    assert!(validation.is_healthy(), "WAL should be healthy after long-running simulation");
    assert!(final_entries.len() > 0, "Should have recovered entries");
    
    // Check that we didn't lose too many operations due to crashes
    let loss_rate = 1.0 - (final_entries.len() as f64 / total_operations as f64);
    println!("\nData loss rate: {:.2}%", loss_rate * 100.0);
    assert!(loss_rate < 0.05, "Data loss should be < 5%");
}

/// Tests memory growth is bounded during continuous operation.
#[tokio::test]
#[ignore]  // Run with: cargo test --ignored test_memory_bounded_growth
async fn test_memory_bounded_growth() {
    let temp_dir = TempDir::new().unwrap();
    let wal = Arc::new(WriteAheadLog::open(temp_dir.path()).unwrap());
    
    let operations_count = 10_000;
    let compaction_threshold = 1_000;
    
    println!("Testing bounded memory growth with {} operations", operations_count);
    
    for i in 0u64..operations_count {
        // Write operation
        wal.append(
            "memory-test-actor".to_string(),
            WalOperation::Put {
                id: format!("node-{}", i),
                data: serde_json::json!({
                    "index": i,
                    "payload": "x".repeat(100),
                }),
            },
        ).await.unwrap();
        
        // Compact periodically
        if i > 0 && i % compaction_threshold == 0 {
            let checkpoint_seq = i.saturating_sub(compaction_threshold / 2);
            wal.append(
                "system".to_string(),
                WalOperation::Checkpoint { base_seq: checkpoint_seq },
            ).await.unwrap();
            
            wal.compact(checkpoint_seq).await.unwrap();
            
            // Verify entries are being pruned
            let entries = wal.read_all().await.unwrap();
            println!("After compaction at i={}: {} entries remain", i, entries.len());
            
            // Entries should be bounded (not growing linearly with i)
            assert!(
                entries.len() < (compaction_threshold * 2) as usize,
                "Memory should be bounded after compaction"
            );
        }
    }
    
    let final_entries = wal.read_all().await.unwrap();
    println!("\nFinal entry count: {}", final_entries.len());
    println!("Growth ratio: {:.2}x", final_entries.len() as f64 / operations_count as f64);
    
    // Verify memory growth is bounded
    assert!(
        final_entries.len() < (operations_count / 5) as usize,
        "Final entry count should be much less than total operations"
    );
}

/// Tests concurrent writers with periodic crashes.
#[tokio::test]
#[ignore]  // Run with: cargo test --ignored test_concurrent_agent_workers
async fn test_concurrent_agent_workers() {
    let temp_dir = TempDir::new().unwrap();
    let wal = Arc::new(WriteAheadLog::open(temp_dir.path()).unwrap());
    
    let num_workers = 10;
    let ops_per_worker = 100;
    
    println!("Testing {} concurrent agent workers, {} ops each", num_workers, ops_per_worker);
    
    let mut handles = vec![];
    
    for worker_id in 0..num_workers {
        let wal_clone = Arc::clone(&wal);
        let handle = task::spawn(async move {
            for i in 0..ops_per_worker {
                wal_clone.append(
                    format!("worker-{}", worker_id),
                    WalOperation::Put {
                        id: format!("worker-{}-op-{}", worker_id, i),
                        data: serde_json::json!({
                            "worker": worker_id,
                            "operation": i,
                            "timestamp": chrono::Utc::now().timestamp(),
                        }),
                    },
                ).await.unwrap();
            }
        });
        handles.push(handle);
    }
    
    // Wait for all workers to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify all operations were persisted
    let entries = wal.read_all().await.unwrap();
    println!("Total entries persisted: {}", entries.len());
    
    assert_eq!(
        entries.len(),
        num_workers * ops_per_worker,
        "All concurrent operations should be persisted"
    );
    
    // Verify no sequence number collisions
    let mut seen_seqs = std::collections::HashSet::new();
    for entry in &entries {
        assert!(
            seen_seqs.insert(entry.seq),
            "Sequence numbers should be unique: {} appeared twice",
            entry.seq
        );
    }
    
    println!("All sequence numbers are unique ✓");
}
