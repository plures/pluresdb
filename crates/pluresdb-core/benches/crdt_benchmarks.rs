use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use pluresdb_core::{CrdtStore};
use serde_json::json;

fn benchmark_put_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_put");
    
    for &size in [10, 100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let store = CrdtStore::default();
            
            b.iter(|| {
                for i in 0..size {
                    let id = format!("node:{}", i);
                    store.put(
                        id.clone(),
                        "actor-bench",
                        black_box(json!({
                            "value": i,
                            "data": "benchmark data with some content",
                            "timestamp": 1234567890,
                            "metadata": {
                                "type": "test",
                                "tags": ["benchmark", "performance"]
                            }
                        }))
                    );
                }
            });
        });
    }
    
    group.finish();
}

fn benchmark_get_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_get");
    
    for &size in [10, 100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let store = CrdtStore::default();
            
            // Pre-populate store
            let ids: Vec<String> = (0..size)
                .map(|i| {
                    let id = format!("node:{}", i);
                    store.put(
                        id.clone(),
                        "actor-bench",
                        json!({
                            "value": i,
                            "data": "benchmark data"
                        })
                    );
                    id
                })
                .collect();
            
            let mut counter = 0;
            b.iter(|| {
                let id = &ids[counter % ids.len()];
                counter += 1;
                black_box(store.get(id))
            });
        });
    }
    
    group.finish();
}

fn benchmark_list_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_list");
    
    for &size in [10, 100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let store = CrdtStore::default();
            
            // Pre-populate store
            for i in 0..size {
                store.put(
                    format!("node:{}", i),
                    "actor-bench",
                    json!({
                        "value": i,
                        "type": if i % 2 == 0 { "even" } else { "odd" }
                    })
                );
            }
            
            b.iter(|| {
                black_box(store.list())
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_put_operations,
    benchmark_get_operations,
    benchmark_list_operations,
);
criterion_main!(benches);
