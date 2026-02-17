use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use pluresdb_core::{Database, DatabaseOptions, SqlValue};

fn benchmark_sql_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_insert");
    
    for batch_size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(batch_size), batch_size, |b, &batch_size| {
            b.iter_batched(
                || {
                    let db = Database::open(DatabaseOptions::in_memory()).unwrap();
                    db.exec("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, email TEXT, age INTEGER)").unwrap();
                    db
                },
                |db| {
                    let stmt = db.prepare("INSERT INTO users (name, email, age) VALUES (?1, ?2, ?3)").unwrap();
                    for i in 0..batch_size {
                        stmt.run(&[
                            SqlValue::Text(format!("User {}", i)),
                            SqlValue::Text(format!("user{}@example.com", i)),
                            SqlValue::Integer(20_i64 + (i as i64 % 50_i64)),
                        ]).unwrap();
                    }
                },
                criterion::BatchSize::SmallInput
            );
        });
    }
    
    group.finish();
}

fn benchmark_sql_select(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_select");
    
    for row_count in [100, 1000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(row_count), row_count, |b, &row_count| {
            // Setup: create and populate database
            let db = Database::open(DatabaseOptions::in_memory()).unwrap();
            db.exec("CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT, price REAL, category TEXT)").unwrap();
            
            let stmt = db.prepare("INSERT INTO products (name, price, category) VALUES (?1, ?2, ?3)").unwrap();
            for i in 0..row_count {
                stmt.run(&[
                    SqlValue::Text(format!("Product {}", i)),
                    SqlValue::Real(9.99 + (i as f64)),
                    SqlValue::Text(["electronics", "books", "clothing", "food"][(i % 4) as usize].to_string()),
                ]).unwrap();
            }
            
            b.iter(|| {
                let stmt = db.prepare("SELECT * FROM products WHERE category = ?1").unwrap();
                black_box(stmt.all(&[SqlValue::Text("electronics".to_string())]).unwrap())
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_sql_insert,
    benchmark_sql_select,
);
criterion_main!(benches);
