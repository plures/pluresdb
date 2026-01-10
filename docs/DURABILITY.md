# PluresDB Durability and Replay Guarantees

## Overview

PluresDB provides strong durability, recoverability, and deterministic replay guarantees suitable for use as an exclusive local-first agent memory store. This document defines the invariants, architecture, and guarantees that PluresDB maintains across crashes, power loss, and partial failures.

## Core Guarantees

### 1. Durability

**Guarantee**: No accepted write is lost across crashes, power loss, or forced termination.

#### Write-Ahead Log (WAL) Architecture

PluresDB uses a multi-layer durability strategy:

1. **Operation Log**: All CRDT operations are appended to a write-ahead log before being applied to the in-memory state
2. **SQLite WAL Mode**: The underlying SQLite database operates in WAL mode for atomic, crash-safe commits
3. **Sled Flush**: For sled-backed storage, all writes are explicitly flushed to disk before acknowledgment

#### Write Acceptance Semantics

A write is considered "accepted" when:

1. The CRDT operation has been serialized to the WAL
2. The WAL entry has been fsynced to persistent storage (or sled flush completes)
3. An acknowledgment is returned to the caller

**Write Acceptance Contract**:
```rust
// A write is NOT considered accepted until this function returns Ok
async fn put(&self, node: StoredNode) -> Result<()> {
    // 1. Serialize operation
    let op = serialize_operation(&node)?;
    
    // 2. Append to WAL
    self.wal.append(op).await?;
    
    // 3. Fsync WAL to disk
    self.wal.fsync().await?;
    
    // 4. Apply to in-memory state
    self.memory_state.apply(node.clone()).await?;
    
    // 5. Write is now accepted - safe to acknowledge
    Ok(())
}
```

#### Crash Boundaries

PluresDB maintains crash safety at the following boundaries:

- **Before WAL fsync**: Operation is NOT durable; will be lost on crash
- **After WAL fsync**: Operation IS durable; will be replayed on recovery
- **During compaction**: Old and new segments are both preserved until new segment is fsynced
- **During index rebuild**: Original data remains intact; rebuilds are idempotent

### 2. Recoverability

**Guarantee**: Database state must be reconstructible from raw persisted data. Partial corruption must not imply total data loss.

#### Segment Isolation

PluresDB stores data in isolated segments to prevent cascade failures:

1. **WAL Segments**: Each day/hour creates a new WAL segment (configurable)
2. **Data Segments**: Sled naturally isolates data in B-tree nodes
3. **Index Segments**: Vector indexes stored separately from base data

**Corruption Containment Strategy**:

- Each segment has a checksum for integrity validation
- Corrupted segments are detected and isolated during recovery
- Uncorrupted segments remain fully accessible
- Partial data loss is preferable to total database unavailability

#### Recovery Process

On startup, PluresDB performs the following recovery sequence:

```rust
async fn recover() -> Result<Database> {
    // 1. Validate all WAL segments
    let wal_segments = validate_wal_segments().await?;
    
    // 2. Replay valid WAL segments in order
    for segment in wal_segments {
        if let Ok(operations) = segment.read_operations() {
            for op in operations {
                apply_operation(op).await?;
            }
        } else {
            log_corrupted_segment(segment);
        }
    }
    
    // 3. Rebuild indexes from base data
    rebuild_indexes().await?;
    
    // 4. Validate final state
    validate_database_state().await?;
    
    Ok(database)
}
```

### 3. Deterministic Replay

**Guarantee**: Given the same input event stream, PluresDB must converge to the same state. Required for agent debugging and forensic analysis.

#### CRDT Determinism

PluresDB uses CRDTs (Conflict-free Replicated Data Types) with vector clocks to ensure deterministic conflict resolution:

- Vector clocks provide total ordering for concurrent operations
- Last-write-wins (LWW) semantics with timestamp tiebreakers
- Actor ID lexicographic ordering for timestamp ties

**Replay Determinism Properties**:

1. **Idempotent Operations**: Replaying the same operation multiple times produces the same result
2. **Commutative Operations**: Operation order doesn't affect final state (for concurrent ops)
3. **Associative Operations**: Grouping of operations doesn't affect final state

#### Replay Tooling

PluresDB provides a `pluresdb-replay` tool for forensic analysis:

```bash
# Replay all operations from WAL
pluresdb-replay --wal-dir ./data/wal --output replayed.db

# Replay with filtering
pluresdb-replay --wal-dir ./data/wal --actor alice --after 2026-01-01

# Validate determinism
pluresdb-replay --wal-dir ./data/wal --validate-checksums
```

### 4. Bounded Growth

**Guarantee**: CRDT metadata, tombstones, and vector indexes must not grow unbounded. GC/compaction must exist and be testable.

#### Tombstone Lifecycle

1. **Deletion**: Node is marked as deleted (tombstone created) with deletion timestamp
2. **Retention Period**: Tombstone retained for configurable period (default: 30 days)
3. **Compaction**: After retention period, tombstone is purged during compaction
4. **Validation**: Compaction validates no pending sync operations reference tombstone

#### CRDT Metadata Pruning

Vector clocks are pruned using the following strategy:

1. **Active Actors**: Actors with recent activity (< 30 days) are always retained
2. **Inactive Actors**: Actors with no activity are candidates for pruning
3. **Sync Horizon**: Global sync state tracks minimum required vector clock
4. **Pruning**: Clocks older than sync horizon are safely pruned

#### Vector Index Compaction

Vector indexes are rebuilt periodically to remove deleted nodes:

1. **Incremental Updates**: New vectors added to existing index
2. **Rebuild Trigger**: Threshold of deleted nodes (default: 20%) triggers rebuild
3. **Background Rebuild**: Index rebuilt in background, swapped atomically
4. **Space Reclamation**: Old index deleted after successful rebuild

#### Compaction Tools

```bash
# Force compaction
pluresdb-compact --data-dir ./data --strategy aggressive

# Inspect storage contributors
pluresdb-inspect --data-dir ./data --show-breakdown

# Validate post-compaction
pluresdb-validate --data-dir ./data --check-integrity
```

### 5. Offline-First + Sync Safety

**Guarantee**: Long offline divergence must safely reconcile. Conflicting writes must resolve predictably.

#### Offline Divergence Handling

PluresDB supports arbitrary offline periods:

1. **Local Operations**: All operations work offline with local vector clock
2. **Divergence Tracking**: Vector clocks track divergence between peers
3. **Merge Protocol**: On reconnection, vector clocks determine merge strategy
4. **Conflict Resolution**: LWW with timestamp + actor ID tiebreaker

#### Sync Safety Protocol

```rust
async fn sync_with_peer(peer: &Peer) -> Result<SyncStats> {
    // 1. Exchange vector clocks
    let local_clock = get_vector_clock();
    let remote_clock = peer.get_vector_clock().await?;
    
    // 2. Compute diff (operations peer needs)
    let missing_ops = compute_diff(&local_clock, &remote_clock)?;
    
    // 3. Send missing operations
    peer.send_operations(missing_ops).await?;
    
    // 4. Receive missing operations
    let peer_ops = peer.get_missing_operations(&remote_clock, &local_clock).await?;
    
    // 5. Apply operations with CRDT merge
    for op in peer_ops {
        apply_operation_with_merge(op).await?;
    }
    
    // 6. Update vector clock
    merge_vector_clocks(&local_clock, &remote_clock)?;
    
    Ok(sync_stats)
}
```

### 6. Security & Key Management

**Guarantee**: Encryption at rest must be verifiable. Device revocation and key rotation must be supported without data loss.

#### Encryption at Rest

PluresDB encrypts data using AES-256-GCM:

1. **Master Key**: Derived from user password using Argon2
2. **Data Encryption**: Each segment encrypted with unique key
3. **Key Wrapping**: Segment keys encrypted with master key
4. **Metadata Protection**: File names and metadata also encrypted

**Encryption Architecture**:

```
User Password
    ↓ (Argon2)
Master Key
    ↓ (HKDF)
├─ Segment Key 1 → Encrypts Data Segment 1
├─ Segment Key 2 → Encrypts Data Segment 2
└─ Segment Key N → Encrypts Data Segment N
```

#### Key Rotation

Key rotation is performed without data loss:

1. **New Master Key**: Generate new master key from new password
2. **Segment Re-keying**: Re-encrypt segment keys with new master key
3. **Atomic Swap**: Update key metadata atomically
4. **No Data Re-encryption**: Data segments remain encrypted with same keys

#### Device Revocation

When a device is revoked:

1. **Revocation List**: Device ID added to revocation list
2. **Key Rotation**: Master key rotated to new value
3. **Re-encryption**: Segment keys re-encrypted with new master
4. **Sync Block**: Revoked device can no longer decrypt new data

## Fsync Semantics

PluresDB provides explicit fsync control:

```rust
pub enum DurabilityLevel {
    /// No fsync - fastest, least durable
    None,
    
    /// Fsync WAL only - balanced (default)
    Wal,
    
    /// Fsync WAL and data - slowest, most durable
    Full,
}
```

Default: `DurabilityLevel::Wal` - provides durability guarantees while maintaining performance.

## Supported vs Unsupported Guarantees

### Supported ✅

- ✅ Crash-safe writes (WAL + fsync)
- ✅ Deterministic replay from WAL
- ✅ Partial corruption containment
- ✅ Bounded metadata growth (with compaction)
- ✅ Offline-first operation
- ✅ CRDT-based conflict resolution
- ✅ Encryption at rest (AES-256-GCM)
- ✅ Key rotation without data loss
- ✅ Device revocation

### Unsupported ❌

- ❌ Byzantine fault tolerance (assumes honest but crash-prone peers)
- ❌ Guaranteed real-time sync (sync is best-effort)
- ❌ Distributed transactions across peers (local transactions only)
- ❌ Schema evolution with automatic migration (manual migration required)
- ❌ Point-in-time recovery to arbitrary timestamps (WAL replay only)

## Testing Strategy

All guarantees are validated by the test suite in `tests/durability/`:

- `test_kill_process_mid_write.rs` - Crash during write operation
- `test_kill_during_compaction.rs` - Crash during compaction
- `test_fsync_boundary.rs` - Power loss simulation
- `test_deterministic_replay.rs` - Replay produces identical state
- `test_offline_divergence.rs` - Extended offline reconciliation
- `test_encryption_at_rest.rs` - Encryption verification
- `test_key_rotation.rs` - Key rotation without data loss
- `test_device_revocation.rs` - Revoked device access
- `test_long_running_agent.rs` - Continuous operation with crashes

## Performance Considerations

While these guarantees provide strong correctness properties, they have performance implications:

- **WAL Overhead**: ~5-10% write throughput reduction
- **Fsync Latency**: ~1-5ms per write (depends on storage)
- **Compaction**: Background task, minimal impact
- **Encryption**: ~5-10% CPU overhead
- **Memory**: Vector clocks require ~100 bytes per actor per node

For this phase, **correctness > speed** is the priority.

## Version Compatibility

These guarantees are introduced in PluresDB v1.4.0 and are maintained in all subsequent releases. Breaking changes to these guarantees will result in a major version bump.
