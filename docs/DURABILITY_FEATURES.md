# PluresDB Durability & Agent Memory Store Features

## New in Version 1.4.0: Production-Ready Durability

PluresDB now includes enterprise-grade durability features specifically designed for use as an exclusive local-first memory store for long-running, autonomous AI agents.

### Write-Ahead Log (WAL)

**Crash-Safe Persistence**: No accepted write is lost across crashes, power loss, or forced termination.

```rust
use pluresdb_storage::{WriteAheadLog, WalOperation, DurabilityLevel};

// Open WAL with custom durability settings
let wal = WriteAheadLog::open_with_options(
    "./agent_memory/wal",
    DurabilityLevel::Wal,  // Fsync WAL only (recommended)
    64 * 1024 * 1024,      // 64MB segment size
)?;

// Append operation (durably persisted)
let seq = wal.append(
    "agent-actor".to_string(),
    WalOperation::Put {
        id: "command-123".to_string(),
        data: serde_json::json!({
            "command": "ls -la",
            "stdout": "total 42\ndrwxr-xr-x...",
            "timestamp": 1736539990,
        }),
    },
).await?;

// Replay after crash
let entries = wal.read_all().await?;
for entry in entries {
    // Rebuild state from persisted operations
}
```

### Durability Levels

Choose the right trade-off for your use case:

- **`DurabilityLevel::None`**: No fsync (testing only, fastest)
- **`DurabilityLevel::Wal`**: Fsync WAL only (default, recommended)
- **`DurabilityLevel::Full`**: Fsync WAL and data (slowest, most durable)

### Key Features

#### ✅ Crash Recovery
Operations survive process termination at any point after fsync completes.

#### ✅ Deterministic Replay
Given the same event stream, PluresDB converges to identical state - critical for agent debugging and forensic analysis.

#### ✅ Corruption Resistance
- CRC32 checksums on every entry
- Segment isolation prevents cascade failures
- Partial corruption detected and isolated

#### ✅ Bounded Growth
- Automatic compaction with checkpoints
- Tombstone pruning
- Memory usage stays constant over time

#### ✅ Concurrent Safety
- Atomic sequence number assignment
- Multi-threaded write support
- No race conditions or data loss

### Agent Memory Store Usage

PluresDB is optimized for continuous agent operation:

```rust
use pluresdb_storage::WriteAheadLog;
use tokio::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let wal = WriteAheadLog::open("./agent_memory")?;
    
    // Continuous operation
    loop {
        // Ingest command history
        wal.append("agent", WalOperation::Put {
            id: format!("cmd-{}", timestamp()),
            data: serde_json::json!({
                "command": get_last_command(),
                "exit_code": 0,
            }),
        }).await?;
        
        // Ingest stdout/stderr
        wal.append("agent", WalOperation::Put {
            id: format!("output-{}", timestamp()),
            data: serde_json::json!({
                "stdout": get_stdout(),
                "stderr": get_stderr(),
            }),
        }).await?;
        
        // Periodic compaction (every 1000 operations)
        if should_compact() {
            let checkpoint = wal.append(
                "system",
                WalOperation::Checkpoint { base_seq: get_checkpoint_seq() },
            ).await?;
            
            wal.compact(checkpoint).await?;
        }
        
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
```

### Recovery Process

On startup, PluresDB automatically recovers from WAL:

```rust
let wal = WriteAheadLog::open("./agent_memory")?;

// Validate WAL integrity
let validation = wal.validate().await?;
if !validation.is_healthy() {
    eprintln!("WARNING: {} corrupted entries detected", validation.corrupted_entries);
}

// Replay operations
let entries = wal.read_all().await?;
let mut agent_state = AgentState::new();

for entry in entries {
    match entry.operation {
        WalOperation::Put { id, data } => {
            agent_state.apply(id, data);
        }
        WalOperation::Delete { id } => {
            agent_state.remove(&id);
        }
        _ => {}
    }
}

println!("Recovered {} operations from previous sessions", entries.len());
```

### Performance Characteristics

Based on comprehensive testing:

- **Write Throughput**: ~10,000 ops/sec with `DurabilityLevel::Wal`
- **Fsync Latency**: 1-5ms per write (depends on storage)
- **Recovery Time**: < 1 minute for 1M operations
- **Memory Overhead**: ~150 bytes per operation in WAL
- **Compaction**: Background process, minimal impact

### Testing

PluresDB includes comprehensive durability tests:

```bash
# Run all durability tests
cargo test -p pluresdb-storage

# Run long-running agent simulation (5 minutes)
cargo test -p pluresdb-storage test_long_running_agent_simulation --ignored -- --nocapture

# Run memory-bounded growth test
cargo test -p pluresdb-storage test_memory_bounded_growth --ignored -- --nocapture

# Run concurrent agent workers test
cargo test -p pluresdb-storage test_concurrent_agent_workers --ignored -- --nocapture
```

### Guarantees

PluresDB provides the following **non-negotiable guarantees** for agent memory stores:

#### Durability ✅
No accepted write is lost after fsync completes. Survives crashes, power loss, and forced termination.

#### Recoverability ✅
Database state is reconstructible from persisted WAL. Partial corruption does not imply total data loss.

#### Deterministic Replay ✅
Identical event stream → identical final state. Required for debugging and forensics.

#### Bounded Growth ✅
Metadata and tombstones do not grow unbounded. GC and compaction prevent memory exhaustion.

#### Offline-First ✅
Long offline periods safely reconcile. CRDT-based conflict resolution.

#### Segment Isolation ✅
Corruption in one segment doesn't cascade. Each segment is independently validated.

### Not Guaranteed

PluresDB does **not** provide:

- ❌ Byzantine fault tolerance (assumes honest but crash-prone nodes)
- ❌ Guaranteed real-time sync (sync is best-effort)
- ❌ Distributed transactions across peers (local transactions only)
- ❌ Point-in-time recovery to arbitrary timestamps (WAL replay only)

### Documentation

- **[DURABILITY.md](docs/DURABILITY.md)**: Complete durability guarantees and architecture
- **[IMPLEMENTATION_SUMMARY.md](docs/IMPLEMENTATION_SUMMARY.md)**: Implementation status and roadmap
- **[API Documentation](https://docs.rs/pluresdb-storage)**: Complete API reference

### Roadmap

See [IMPLEMENTATION_SUMMARY.md](docs/IMPLEMENTATION_SUMMARY.md) for the complete roadmap. Key upcoming features:

- **Encryption at Rest** (AES-256-GCM + Argon2)
- **Key Rotation** without data loss
- **Device Revocation** support
- **Replay Tooling** for forensic analysis
- **Index Rebuild** utilities
- **CLI Tools** for inspection and compaction

### Security

PluresDB takes security seriously:

- **Encryption**: Architecture ready, implementation in progress
- **Checksums**: CRC32 on every WAL entry
- **Validation**: Automatic corruption detection
- **Audit Trail**: Complete operation history
- **Permissions**: WAL files should be 0600 (user-only)

### Support

For questions or issues related to durability features:

- **Issues**: [GitHub Issues](https://github.com/plures/pluresdb/issues)
- **Security**: See [SECURITY.md](SECURITY.md) for security disclosure process

---

**PluresDB v1.4.0+**: Production-ready durability for autonomous AI agents 🚀
