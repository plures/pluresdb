use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use rusty_gun_core::crdt::{merge_nodes, NodeRecord};
use serde_json::json;
use std::collections::HashMap;

fn create_test_node(id: &str, data: serde_json::Value, timestamp: u64, vector_clock: HashMap<String, u64>) -> NodeRecord {
    NodeRecord {
        id: id.to_string(),
        data,
        vector: Some(vec![0.1, 0.2, 0.3, 0.4, 0.5]),
        type_: Some("TestType".to_string()),
        timestamp,
        vector_clock,
    }
}

fn crdt_merge_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_merge");
    
    for size in [1, 10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("equal_timestamps", size), size, |b, &size| {
            b.iter(|| {
                let mut local_clock = HashMap::new();
                local_clock.insert("peer1".to_string(), size);
                
                let mut incoming_clock = HashMap::new();
                incoming_clock.insert("peer2".to_string(), size);
                
                let local = create_test_node(
                    "test:1",
                    json!({
                        "field1": "value1",
                        "shared": "local_value",
                        "nested": {
                            "x": 1,
                            "y": 1
                        }
                    }),
                    *size as u64,
                    local_clock,
                );
                
                let incoming = create_test_node(
                    "test:1",
                    json!({
                        "field2": "value2",
                        "shared": "incoming_value",
                        "nested": {
                            "y": 2,
                            "z": 3
                        }
                    }),
                    *size as u64,
                    incoming_clock,
                );
                
                merge_nodes(local, incoming)
            });
        });
    }
    
    group.finish();
}

fn crdt_lww_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_lww");
    
    for size in [1, 10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("different_timestamps", size), size, |b, &size| {
            b.iter(|| {
                let mut older_clock = HashMap::new();
                older_clock.insert("peer1".to_string(), size);
                
                let mut newer_clock = HashMap::new();
                newer_clock.insert("peer2".to_string(), size);
                
                let older = create_test_node(
                    "test:2",
                    json!({
                        "a": 1,
                        "b": 1
                    }),
                    *size as u64,
                    older_clock,
                );
                
                let newer = create_test_node(
                    "test:2",
                    json!({
                        "a": 999,
                        "b": 2,
                        "c": 3
                    }),
                    (*size + 1000) as u64,
                    newer_clock,
                );
                
                merge_nodes(older, newer)
            });
        });
    }
    
    group.finish();
}

fn crdt_conflict_resolution_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_conflict_resolution");
    
    for conflicts in [1, 5, 10, 50].iter() {
        group.bench_with_input(BenchmarkId::new("multiple_conflicts", conflicts), conflicts, |b, &conflicts| {
            b.iter(|| {
                let mut current = create_test_node(
                    "test:conflict",
                    json!({
                        "base": "value",
                        "conflicts": 0
                    }),
                    1000,
                    HashMap::new(),
                );
                
                for i in 0..*conflicts {
                    let mut clock = HashMap::new();
                    clock.insert(format!("peer{}", i), i + 1);
                    
                    let conflict = create_test_node(
                        "test:conflict",
                        json!({
                            "base": "value",
                            "conflicts": i,
                            format!("field{}", i): format!("value{}", i)
                        }),
                        1000 + i,
                        clock,
                    );
                    
                    current = merge_nodes(current, conflict);
                }
                
                current
            });
        });
    }
    
    group.finish();
}

fn crdt_vector_clock_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_vector_clock");
    
    for peers in [2, 5, 10, 20].iter() {
        group.bench_with_input(BenchmarkId::new("many_peers", peers), peers, |b, &peers| {
            b.iter(|| {
                let mut clock = HashMap::new();
                for i in 0..*peers {
                    clock.insert(format!("peer{}", i), i + 1);
                }
                
                let node = create_test_node(
                    "test:vector",
                    json!({
                        "data": "test",
                        "peers": peers
                    }),
                    1000,
                    clock,
                );
                
                // Simulate vector clock operations
                let mut updated_clock = node.vector_clock.clone();
                for i in 0..*peers {
                    updated_clock.insert(format!("peer{}", i), i + 2);
                }
                
                updated_clock
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    crdt_benches,
    crdt_merge_benchmark,
    crdt_lww_benchmark,
    crdt_conflict_resolution_benchmark,
    crdt_vector_clock_benchmark
);
criterion_main!(crdt_benches);