use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use rusty_gun_api::server::ApiServer;
use rusty_gun_api::handlers::NodeHandler;
use rusty_gun_api::handlers::VectorHandler;
use rusty_gun_api::handlers::SQLHandler;
use rusty_gun_core::node::Node;
use rusty_gun_core::types::{NodeId, Value, Metadata};
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;
use reqwest::Client;
use serde_json::json;

fn create_test_node(id: &str, data: Value) -> Node {
    Node::new(
        NodeId::from(id),
        data,
        Metadata::new(),
        vec!["test".to_string()],
    )
}

fn api_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("api_operations");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    // API server initialization
    group.bench_function("api_server_init", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();
            let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
            server
        })
    });

    // HTTP request handling
    group.bench_function("http_request_handling", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
            let client = Client::new();
            
            // Simulate HTTP request processing
            let response = client
                .get(&format!("http://{}", server.addr()))
                .send()
                .await
                .unwrap();
            
            response.status()
        })
    });

    group.finish();
}

fn api_endpoint_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("api_endpoint_operations");
    group.measurement_time(Duration::from_secs(10));

    // Node endpoint benchmarks
    group.bench_function("node_create_endpoint", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
            let client = Client::new();
            
            let node_data = json!({
                "id": "test-node",
                "data": {"name": "Test Node", "type": "test"},
                "metadata": {},
                "tags": ["test"]
            });
            
            let response = client
                .post(&format!("http://{}/api/nodes", server.addr()))
                .json(&node_data)
                .send()
                .await
                .unwrap();
            
            response.status()
        })
    });

    group.bench_function("node_get_endpoint", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
            let client = Client::new();
            
            // First create a node
            let node_data = json!({
                "id": "test-node",
                "data": {"name": "Test Node", "type": "test"},
                "metadata": {},
                "tags": ["test"]
            });
            
            client
                .post(&format!("http://{}/api/nodes", server.addr()))
                .json(&node_data)
                .send()
                .await
                .unwrap();
            
            // Then get it
            let response = client
                .get(&format!("http://{}/api/nodes/test-node", server.addr()))
                .send()
                .await
                .unwrap();
            
            response.status()
        })
    });

    group.bench_function("vector_search_endpoint", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
            let client = Client::new();
            
            let search_data = json!({
                "query": "test search query",
                "limit": 10,
                "threshold": 0.5
            });
            
            let response = client
                .post(&format!("http://{}/api/vector/search/text", server.addr()))
                .json(&search_data)
                .send()
                .await
                .unwrap();
            
            response.status()
        })
    });

    group.bench_function("sql_query_endpoint", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
            let client = Client::new();
            
            let query_data = json!({
                "query": "SELECT * FROM nodes LIMIT 10",
                "params": []
            });
            
            let response = client
                .post(&format!("http://{}/api/sql/query", server.addr()))
                .json(&query_data)
                .send()
                .await
                .unwrap();
            
            response.status()
        })
    });

    group.finish();
}

fn api_concurrent_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("api_concurrent_operations");
    group.measurement_time(Duration::from_secs(10));

    for num_requests in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*num_requests as u64));
        
        group.bench_with_input(BenchmarkId::new("concurrent_requests", num_requests), num_requests, |b, &num_requests| {
            let temp_dir = TempDir::new().unwrap();
            
            b.to_async(&rt).iter(|| async {
                let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
                let client = Client::new();
                
                let mut handles = vec![];
                
                for i in 0..num_requests {
                    let client_clone = client.clone();
                    let server_addr = server.addr().clone();
                    let handle = tokio::spawn(async move {
                        let node_data = json!({
                            "id": format!("node-{}", i),
                            "data": {"name": format!("Node {}", i), "type": "test"},
                            "metadata": {},
                            "tags": ["test"]
                        });
                        
                        client_clone
                            .post(&format!("http://{}/api/nodes", server_addr))
                            .json(&node_data)
                            .send()
                            .await
                            .unwrap()
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

fn api_throughput_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("api_throughput");
    group.measurement_time(Duration::from_secs(15));

    // Throughput tests for different request types
    group.bench_function("get_requests_throughput", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
            let client = Client::new();
            
            // Create test data first
            for i in 0..100 {
                let node_data = json!({
                    "id": format!("node-{}", i),
                    "data": {"name": format!("Node {}", i), "type": "test"},
                    "metadata": {},
                    "tags": ["test"]
                });
                
                client
                    .post(&format!("http://{}/api/nodes", server.addr()))
                    .json(&node_data)
                    .send()
                    .await
                    .unwrap();
            }
            
            // Now benchmark GET requests
            let mut handles = vec![];
            for i in 0..1000 {
                let client_clone = client.clone();
                let server_addr = server.addr().clone();
                let handle = tokio::spawn(async move {
                    client_clone
                        .get(&format!("http://{}/api/nodes/node-{}", server_addr, i % 100))
                        .send()
                        .await
                        .unwrap()
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.await.unwrap();
            }
        })
    });

    group.bench_function("post_requests_throughput", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
            let client = Client::new();
            
            let mut handles = vec![];
            for i in 0..1000 {
                let client_clone = client.clone();
                let server_addr = server.addr().clone();
                let handle = tokio::spawn(async move {
                    let node_data = json!({
                        "id": format!("throughput-node-{}", i),
                        "data": {"name": format!("Throughput Node {}", i), "type": "test"},
                        "metadata": {},
                        "tags": ["test", "throughput"]
                    });
                    
                    client_clone
                        .post(&format!("http://{}/api/nodes", server_addr))
                        .json(&node_data)
                        .send()
                        .await
                        .unwrap()
                });
                handles.push(handle);
            }
            
            for handle in handles {
                handle.await.unwrap();
            }
        })
    });

    group.finish();
}

fn api_latency_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("api_latency");
    group.measurement_time(Duration::from_secs(10));

    // Latency tests for different operations
    group.bench_function("node_creation_latency", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
            let client = Client::new();
            
            let node_data = json!({
                "id": "latency-test-node",
                "data": {"name": "Latency Test Node", "type": "test"},
                "metadata": {},
                "tags": ["test", "latency"]
            });
            
            let start = std::time::Instant::now();
            let response = client
                .post(&format!("http://{}/api/nodes", server.addr()))
                .json(&node_data)
                .send()
                .await
                .unwrap();
            let duration = start.elapsed();
            
            (response.status(), duration)
        })
    });

    group.bench_function("node_retrieval_latency", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
            let client = Client::new();
            
            // Create node first
            let node_data = json!({
                "id": "latency-test-node",
                "data": {"name": "Latency Test Node", "type": "test"},
                "metadata": {},
                "tags": ["test", "latency"]
            });
            
            client
                .post(&format!("http://{}/api/nodes", server.addr()))
                .json(&node_data)
                .send()
                .await
                .unwrap();
            
            // Now measure retrieval latency
            let start = std::time::Instant::now();
            let response = client
                .get(&format!("http://{}/api/nodes/latency-test-node", server.addr()))
                .send()
                .await
                .unwrap();
            let duration = start.elapsed();
            
            (response.status(), duration)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    api_benchmarks,
    api_endpoint_benchmarks,
    api_concurrent_benchmarks,
    api_throughput_benchmarks,
    api_latency_benchmarks
);
criterion_main!(benches);

