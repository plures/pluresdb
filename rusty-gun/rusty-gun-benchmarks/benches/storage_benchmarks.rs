use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use rusty_gun_storage::KvStorage;
use serde_json::json;
use tempfile::TempDir;
use tokio::runtime::Runtime;

fn storage_put_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("storage_put");
    
    for size in [1, 10, 100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("bulk_insert", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let db_path = temp_dir.path().join("test.db");
                let mut storage = KvStorage::new(db_path).await.unwrap();
                
                for i in 0..size {
                    let key = format!("key:{}", i);
                    let value = json!({
                        "id": i,
                        "data": format!("value_{}", i),
                        "timestamp": chrono::Utc::now().timestamp_millis()
                    });
                    
                    storage.put(&key, &value).await.unwrap();
                }
                
                storage.close().await.unwrap();
            });
        });
    }
    
    group.finish();
}

fn storage_get_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("storage_get");
    
    for size in [1, 10, 100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("bulk_read", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let db_path = temp_dir.path().join("test.db");
                let mut storage = KvStorage::new(db_path).await.unwrap();
                
                // Pre-populate database
                for i in 0..size {
                    let key = format!("key:{}", i);
                    let value = json!({
                        "id": i,
                        "data": format!("value_{}", i),
                        "timestamp": chrono::Utc::now().timestamp_millis()
                    });
                    
                    storage.put(&key, &value).await.unwrap();
                }
                
                // Benchmark reads
                for i in 0..size {
                    let key = format!("key:{}", i);
                    let _result = storage.get(&key).await.unwrap();
                }
                
                storage.close().await.unwrap();
            });
        });
    }
    
    group.finish();
}

fn storage_update_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("storage_update");
    
    for size in [1, 10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("bulk_update", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let db_path = temp_dir.path().join("test.db");
                let mut storage = KvStorage::new(db_path).await.unwrap();
                
                // Pre-populate database
                for i in 0..size {
                    let key = format!("key:{}", i);
                    let value = json!({
                        "id": i,
                        "data": format!("value_{}", i),
                        "timestamp": chrono::Utc::now().timestamp_millis()
                    });
                    
                    storage.put(&key, &value).await.unwrap();
                }
                
                // Benchmark updates
                for i in 0..size {
                    let key = format!("key:{}", i);
                    let updated_value = json!({
                        "id": i,
                        "data": format!("updated_value_{}", i),
                        "timestamp": chrono::Utc::now().timestamp_millis(),
                        "updated": true
                    });
                    
                    storage.put(&key, &updated_value).await.unwrap();
                }
                
                storage.close().await.unwrap();
            });
        });
    }
    
    group.finish();
}

fn storage_delete_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("storage_delete");
    
    for size in [1, 10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("bulk_delete", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let db_path = temp_dir.path().join("test.db");
                let mut storage = KvStorage::new(db_path).await.unwrap();
                
                // Pre-populate database
                for i in 0..size {
                    let key = format!("key:{}", i);
                    let value = json!({
                        "id": i,
                        "data": format!("value_{}", i),
                        "timestamp": chrono::Utc::now().timestamp_millis()
                    });
                    
                    storage.put(&key, &value).await.unwrap();
                }
                
                // Benchmark deletes
                for i in 0..size {
                    let key = format!("key:{}", i);
                    storage.delete(&key).await.unwrap();
                }
                
                storage.close().await.unwrap();
            });
        });
    }
    
    group.finish();
}

fn storage_iteration_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("storage_iteration");
    
    for size in [1, 10, 100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("iterate_all", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let db_path = temp_dir.path().join("test.db");
                let mut storage = KvStorage::new(db_path).await.unwrap();
                
                // Pre-populate database
                for i in 0..size {
                    let key = format!("key:{}", i);
                    let value = json!({
                        "id": i,
                        "data": format!("value_{}", i),
                        "timestamp": chrono::Utc::now().timestamp_millis()
                    });
                    
                    storage.put(&key, &value).await.unwrap();
                }
                
                // Benchmark iteration
                let mut count = 0;
                let mut iter = storage.iter().await.unwrap();
                while let Some((_key, _value)) = iter.next().await.unwrap() {
                    count += 1;
                }
                
                assert_eq!(count, size);
                storage.close().await.unwrap();
            });
        });
    }
    
    group.finish();
}

fn storage_large_value_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("storage_large_value");
    
    for size in [1024, 10240, 102400, 1024000].iter() { // 1KB, 10KB, 100KB, 1MB
        group.bench_with_input(BenchmarkId::new("large_value", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let db_path = temp_dir.path().join("test.db");
                let mut storage = KvStorage::new(db_path).await.unwrap();
                
                let large_data = "x".repeat(*size);
                let value = json!({
                    "id": 1,
                    "data": large_data,
                    "timestamp": chrono::Utc::now().timestamp_millis()
                });
                
                storage.put("large:key", &value).await.unwrap();
                let retrieved = storage.get("large:key").await.unwrap();
                
                assert!(retrieved.is_some());
                storage.close().await.unwrap();
            });
        });
    }
    
    group.finish();
}

fn storage_concurrent_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("storage_concurrent");
    
    for workers in [1, 2, 4, 8].iter() {
        group.bench_with_input(BenchmarkId::new("concurrent_workers", workers), workers, |b, &workers| {
            b.to_async(&rt).iter(|| async {
                let temp_dir = TempDir::new().unwrap();
                let db_path = temp_dir.path().join("test.db");
                let storage = KvStorage::new(db_path).await.unwrap();
                
                let handles: Vec<_> = (0..*workers)
                    .map(|worker_id| {
                        let storage = storage.clone();
                        tokio::spawn(async move {
                            for i in 0..100 {
                                let key = format!("worker:{}:key:{}", worker_id, i);
                                let value = json!({
                                    "worker": worker_id,
                                    "key": i,
                                    "data": format!("value_{}_{}", worker_id, i),
                                    "timestamp": chrono::Utc::now().timestamp_millis()
                                });
                                
                                storage.put(&key, &value).await.unwrap();
                            }
                        })
                    })
                    .collect();
                
                for handle in handles {
                    handle.await.unwrap();
                }
                
                storage.close().await.unwrap();
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    storage_benches,
    storage_put_benchmark,
    storage_get_benchmark,
    storage_update_benchmark,
    storage_delete_benchmark,
    storage_iteration_benchmark,
    storage_large_value_benchmark,
    storage_concurrent_benchmark
);
criterion_main!(storage_benches);