use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pluresdb_core::VectorIndex;
use std::hint::black_box;

fn random_embedding(dim: usize, seed: usize) -> Vec<f32> {
    // Deterministic pseudo-random embeddings for reproducibility
    (0..dim)
        .map(|i| {
            let x = ((seed * 6364136223846793005 + i * 1442695040888963407) & 0xFFFFFFFF) as f32;
            (x / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

fn benchmark_vector_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_insert");

    for &count in &[100usize, 1_000, 5_000] {
        let dim = 384;
        let embeddings: Vec<Vec<f32>> = (0..count).map(|i| random_embedding(dim, i)).collect();

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, &count| {
            b.iter_batched(
                || VectorIndex::new(count + 1),
                |index| {
                    for i in 0..count {
                        index.insert(&format!("node:{}", i), black_box(&embeddings[i]));
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn benchmark_vector_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_search");

    for &count in &[100usize, 1_000, 5_000] {
        let dim = 384;
        let embeddings: Vec<Vec<f32>> = (0..count).map(|i| random_embedding(dim, i)).collect();
        let query = random_embedding(dim, 999_999);

        let index = VectorIndex::new(count + 1);
        for i in 0..count {
            index.insert(&format!("node:{}", i), &embeddings[i]);
        }

        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| black_box(index.search(black_box(&query), 10)));
        });
    }

    group.finish();
}

fn benchmark_vector_search_top_k(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_search_top_k");
    let dim = 384;
    let count = 5_000;

    let embeddings: Vec<Vec<f32>> = (0..count).map(|i| random_embedding(dim, i)).collect();
    let index = VectorIndex::new(count + 1);
    for i in 0..count {
        index.insert(&format!("node:{}", i), &embeddings[i]);
    }

    let query = random_embedding(dim, 999_999);

    for &k in &[1usize, 10, 50, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(k), &k, |b, &k| {
            b.iter(|| black_box(index.search(black_box(&query), k)));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_vector_insert,
    benchmark_vector_search,
    benchmark_vector_search_top_k,
);
criterion_main!(benches);
