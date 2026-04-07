use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use pluresdb_core::{CrdtOperation, CrdtStore};
use serde_json::{json, Value as JsonValue};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_payload(i: usize) -> JsonValue {
    let roles = ["admin", "editor", "viewer"];
    let role = roles[i % 3];
    json!({
        "id": i,
        "name": format!("User {}", i),
        "email": format!("user{}@example.com", i),
        "age": 20 + (i % 50),
        "bio": format!("Biography for user {}. They enjoy contributing to open-source.", i),
        "metadata": {
            "role": role,
            "score": (i * 13) % 100,
            "active": i % 4 != 0
        }
    })
}

fn make_ops(count: usize, actor: &str) -> Vec<CrdtOperation> {
    (0..count)
        .map(|i| CrdtOperation::Put {
            id: format!("node:{}", i),
            actor: actor.to_string(),
            data: make_payload(i),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Sequential apply — single actor pushes N distinct inserts
// ---------------------------------------------------------------------------

fn benchmark_sequential_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync_sequential_apply");

    for &size in &[100usize, 1_000, 10_000] {
        let ops = make_ops(size, "peer-alice");

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter_batched(
                CrdtStore::default,
                |store| {
                    for op in &ops {
                        store.apply(black_box(op.clone())).unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Conflict merge — two actors write the same keys; second write wins
// ---------------------------------------------------------------------------

fn benchmark_conflict_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync_conflict_merge");

    for &size in &[100usize, 1_000, 10_000] {
        let alice_ops = make_ops(size, "peer-alice");
        let bob_ops: Vec<CrdtOperation> = (0..size)
            .map(|i| CrdtOperation::Put {
                id: format!("node:{}", i), // same ids → conflict
                actor: "peer-bob".to_string(),
                data: json!({
                    "id": i,
                    "name": format!("Bob's User {}", i),
                    "age": 25 + (i % 40),
                    "source": "peer-bob"
                }),
            })
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter_batched(
                || {
                    // Pre-populate from alice so the merge has real work to do
                    let store = CrdtStore::default();
                    for op in &alice_ops {
                        store.apply(op.clone()).unwrap();
                    }
                    store
                },
                |store| {
                    // Apply bob's conflicting ops
                    for op in &bob_ops {
                        store.apply(black_box(op.clone())).unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Delete propagation
// ---------------------------------------------------------------------------

fn benchmark_delete_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync_delete_propagation");

    for &size in &[100usize, 1_000, 10_000] {
        let put_ops = make_ops(size, "peer-alice");
        let delete_ops: Vec<CrdtOperation> = (0..size)
            .map(|i| CrdtOperation::Delete {
                id: format!("node:{}", i),
            })
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter_batched(
                || {
                    let store = CrdtStore::default();
                    for op in &put_ops {
                        store.apply(op.clone()).unwrap();
                    }
                    store
                },
                |store| {
                    for op in &delete_ops {
                        store.apply(black_box(op.clone())).unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Mixed workload: 60 % put-new, 30 % update-existing, 10 % delete
// ---------------------------------------------------------------------------

fn benchmark_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("sync_mixed_workload");

    for &size in &[100usize, 1_000, 10_000] {
        let mixed_ops: Vec<CrdtOperation> = (0..size)
            .map(|i| {
                let r = i % 10;
                if r < 6 {
                    CrdtOperation::Put {
                        id: format!("new:{}", i),
                        actor: "peer-alice".to_string(),
                        data: make_payload(i),
                    }
                } else if r < 9 {
                    CrdtOperation::Put {
                        id: format!("node:{}", i % (size / 2 + 1)),
                        actor: "peer-alice".to_string(),
                        data: json!({ "updated": true, "seq": i }),
                    }
                } else {
                    CrdtOperation::Delete {
                        id: format!("node:{}", i % (size / 2 + 1)),
                    }
                }
            })
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter_batched(
                || {
                    // Pre-seed targets for updates / deletes
                    let store = CrdtStore::default();
                    for i in 0..(size / 2 + 1) {
                        store
                            .apply(CrdtOperation::Put {
                                id: format!("node:{}", i),
                                actor: "seed".to_string(),
                                data: make_payload(i),
                            })
                            .unwrap();
                    }
                    store
                },
                |store| {
                    for op in &mixed_ops {
                        store.apply(black_box(op.clone())).unwrap();
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_sequential_apply,
    benchmark_conflict_merge,
    benchmark_delete_propagation,
    benchmark_mixed_workload,
);
criterion_main!(benches);
