use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use rusty_gun_storage::vector::HNSWIndex;
use rusty_gun_storage::embeddings::EmbeddingGenerator;
use rusty_gun_storage::vector_service::VectorSearchService;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;
use rand::Rng;

fn generate_test_vectors(dimensions: usize, count: usize) -> Vec<Vec<f32>> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| {
            (0..dimensions)
                .map(|_| rng.gen_range(-1.0..1.0))
                .collect()
        })
        .collect()
}

fn generate_test_texts(count: usize) -> Vec<String> {
    (0..count)
        .map(|i| format!("This is test document number {} with some content for vector search testing", i))
        .collect()
}

fn vector_search_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("vector_search_operations");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    // HNSW index creation
    group.bench_function("hnsw_create_index", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let index_path = temp_dir.path().join("hnsw_index");
            HNSWIndex::new(128, 16, 200, index_path).await.unwrap()
        })
    });

    // Vector insertion
    group.bench_function("hnsw_insert_vector", |b| {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("hnsw_index");
        
        b.to_async(&rt).iter(|| async {
            let mut index = HNSWIndex::new(128, 16, 200, index_path.clone()).await.unwrap();
            let vector = generate_test_vectors(128, 1)[0].clone();
            index.insert("test-id", &vector).await.unwrap();
        })
    });

    // Vector search
    group.bench_function("hnsw_search", |b| {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("hnsw_index");
        
        b.to_async(&rt).iter(|| async {
            let mut index = HNSWIndex::new(128, 16, 200, index_path.clone()).await.unwrap();
            
            // Insert test vectors
            let vectors = generate_test_vectors(128, 100);
            for (i, vector) in vectors.iter().enumerate() {
                index.insert(&format!("id-{}", i), vector).await.unwrap();
            }
            
            // Search
            let query_vector = generate_test_vectors(128, 1)[0].clone();
            index.search(&query_vector, 10, 0.5).await.unwrap()
        })
    });

    group.finish();
}

fn vector_bulk_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("vector_bulk_operations");
    group.measurement_time(Duration::from_secs(15));

    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        
        group.bench_with_input(BenchmarkId::new("hnsw_bulk_insert", size), size, |b, &size| {
            let temp_dir = TempDir::new().unwrap();
            let index_path = temp_dir.path().join("hnsw_index");
            
            b.to_async(&rt).iter(|| async {
                let mut index = HNSWIndex::new(128, 16, 200, index_path.clone()).await.unwrap();
                let vectors = generate_test_vectors(128, size);
                
                for (i, vector) in vectors.iter().enumerate() {
                    index.insert(&format!("id-{}", i), vector).await.unwrap();
                }
            })
        });

        group.bench_with_input(BenchmarkId::new("hnsw_bulk_search", size), size, |b, &size| {
            let temp_dir = TempDir::new().unwrap();
            let index_path = temp_dir.path().join("hnsw_index");
            
            b.to_async(&rt).iter(|| async {
                let mut index = HNSWIndex::new(128, 16, 200, index_path.clone()).await.unwrap();
                let vectors = generate_test_vectors(128, size);
                
                // Insert vectors
                for (i, vector) in vectors.iter().enumerate() {
                    index.insert(&format!("id-{}", i), vector).await.unwrap();
                }
                
                // Search multiple queries
                let query_vectors = generate_test_vectors(128, 10);
                for query_vector in query_vectors {
                    index.search(&query_vector, 10, 0.5).await.unwrap();
                }
            })
        });
    }

    group.finish();
}

fn embedding_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("embedding_operations");
    group.measurement_time(Duration::from_secs(10));

    // Text embedding generation
    group.bench_function("generate_embeddings", |b| {
        b.to_async(&rt).iter(|| async {
            let generator = EmbeddingGenerator::new("sentence-transformers/all-MiniLM-L6-v2".to_string()).await.unwrap();
            let texts = generate_test_texts(10);
            generator.generate_embeddings(&texts).await.unwrap()
        })
    });

    // Single text embedding
    group.bench_function("generate_single_embedding", |b| {
        b.to_async(&rt).iter(|| async {
            let generator = EmbeddingGenerator::new("sentence-transformers/all-MiniLM-L6-v2".to_string()).await.unwrap();
            generator.generate_embedding("This is a test document for embedding generation").await.unwrap()
        })
    });

    // Batch embedding generation
    for batch_size in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::new("batch_embeddings", batch_size), batch_size, |b, &batch_size| {
            b.to_async(&rt).iter(|| async {
                let generator = EmbeddingGenerator::new("sentence-transformers/all-MiniLM-L6-v2".to_string()).await.unwrap();
                let texts = generate_test_texts(batch_size);
                generator.generate_embeddings(&texts).await.unwrap()
            })
        });
    }

    group.finish();
}

fn vector_service_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("vector_service_operations");
    group.measurement_time(Duration::from_secs(10));

    // Vector service initialization
    group.bench_function("vector_service_init", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let service = VectorSearchService::new(
                temp_dir.path().join("vector_service"),
                "sentence-transformers/all-MiniLM-L6-v2".to_string(),
                128,
                16,
                200,
            ).await.unwrap();
            service
        })
    });

    // Text search
    group.bench_function("vector_service_text_search", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let service = VectorSearchService::new(
                temp_dir.path().join("vector_service"),
                "sentence-transformers/all-MiniLM-L6-v2".to_string(),
                128,
                16,
                200,
            ).await.unwrap();
            
            // Add some test documents
            for i in 0..100 {
                service.add_text(&format!("doc-{}", i), &format!("Document {} content", i), None).await.unwrap();
            }
            
            // Search
            service.search_text("test query", 10, 0.5).await.unwrap()
        })
    });

    // Vector search
    group.bench_function("vector_service_vector_search", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let service = VectorSearchService::new(
                temp_dir.path().join("vector_service"),
                "sentence-transformers/all-MiniLM-L6-v2".to_string(),
                128,
                16,
                200,
            ).await.unwrap();
            
            // Add some test documents
            for i in 0..100 {
                service.add_text(&format!("doc-{}", i), &format!("Document {} content", i), None).await.unwrap();
            }
            
            // Search with vector
            let query_vector = generate_test_vectors(128, 1)[0].clone();
            service.search_vector(&query_vector, 10, 0.5).await.unwrap()
        })
    });

    group.finish();
}

fn vector_concurrent_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("vector_concurrent_operations");
    group.measurement_time(Duration::from_secs(10));

    for num_threads in [2, 4, 8].iter() {
        group.bench_with_input(BenchmarkId::new("concurrent_vector_ops", num_threads), num_threads, |b, &num_threads| {
            let temp_dir = TempDir::new().unwrap();
            let index_path = temp_dir.path().join("hnsw_index");
            
            b.to_async(&rt).iter(|| async {
                let index = std::sync::Arc::new(HNSWIndex::new(128, 16, 200, index_path).await.unwrap());
                let mut handles = vec![];
                
                for thread_id in 0..num_threads {
                    let index_clone = std::sync::Arc::clone(&index);
                    let handle = tokio::spawn(async move {
                        let vectors = generate_test_vectors(128, 50);
                        for (i, vector) in vectors.iter().enumerate() {
                            index_clone.insert(&format!("thread-{}-id-{}", thread_id, i), vector).await.unwrap();
                        }
                    });
                    handles.push(handle);
                }
                
                for handle in handles {
                    handle.await.unwrap();
                }
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    vector_search_benchmarks,
    vector_bulk_benchmarks,
    embedding_benchmarks,
    vector_service_benchmarks,
    vector_concurrent_benchmarks
);
criterion_main!(benches);

