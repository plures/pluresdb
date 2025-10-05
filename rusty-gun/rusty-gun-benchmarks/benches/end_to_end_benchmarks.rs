use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use rusty_gun_api::server::ApiServer;
use rusty_gun_core::crdt::Crdt;
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

fn end_to_end_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("end_to_end_operations");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(20);

    // Complete workflow: Create -> Store -> Retrieve -> Search
    group.bench_function("complete_workflow", |b| {
        let temp_dir = TempDir::new().unwrap();
        
        b.to_async(&rt).iter(|| async {
            let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
            let client = Client::new();
            
            // 1. Create node
            let node_data = json!({
                "id": "workflow-test-node",
                "data": {
                    "name": "Workflow Test Node",
                    "type": "test",
                    "content": "This is a test document for end-to-end workflow testing"
                },
                "metadata": {
                    "created_by": "benchmark",
                    "version": "1.0"
                },
                "tags": ["test", "workflow", "benchmark"]
            });
            
            let create_response = client
                .post(&format!("http://{}/api/nodes", server.addr()))
                .json(&node_data)
                .send()
                .await
                .unwrap();
            
            // 2. Retrieve node
            let get_response = client
                .get(&format!("http://{}/api/nodes/workflow-test-node", server.addr()))
                .send()
                .await
                .unwrap();
            
            // 3. Update node
            let update_data = json!({
                "data": {
                    "name": "Updated Workflow Test Node",
                    "type": "test",
                    "content": "This is an updated test document for end-to-end workflow testing",
                    "updated": true
                },
                "metadata": {
                    "created_by": "benchmark",
                    "version": "1.1",
                    "updated_at": "2024-01-01T00:00:00Z"
                }
            });
            
            let update_response = client
                .put(&format!("http://{}/api/nodes/workflow-test-node", server.addr()))
                .json(&update_data)
                .send()
                .await
                .unwrap();
            
            // 4. Vector search
            let search_data = json!({
                "query": "test document workflow",
                "limit": 10,
                "threshold": 0.5
            });
            
            let search_response = client
                .post(&format!("http://{}/api/vector/search/text", server.addr()))
                .json(&search_data)
                .send()
                .await
                .unwrap();
            
            // 5. SQL query
            let query_data = json!({
                "query": "SELECT * FROM nodes WHERE data->>'type' = 'test'",
                "params": []
            });
            
            let sql_response = client
                .post(&format!("http://{}/api/sql/query", server.addr()))
                .json(&query_data)
                .send()
                .await
                .unwrap();
            
            (create_response.status(), get_response.status(), update_response.status(), search_response.status(), sql_response.status())
        })
    });

    group.finish();
}

fn crdt_integration_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_integration_operations");
    group.measurement_time(Duration::from_secs(10));

    // CRDT operations with different data sizes
    for data_size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*data_size as u64));
        
        group.bench_with_input(BenchmarkId::new("crdt_bulk_operations", data_size), data_size, |b, &data_size| {
            b.iter(|| {
                let mut crdt = Crdt::new();
                
                // Create nodes
                for i in 0..data_size {
                    let node = create_test_node(
                        &format!("node-{}", i),
                        Value::String(format!("Data for node {} with some additional content to make it larger", i).repeat(10))
                    );
                    crdt.create_node(node).unwrap();
                }
                
                // Update some nodes
                for i in 0..(data_size / 10) {
                    let updated_node = create_test_node(
                        &format!("node-{}", i),
                        Value::String(format!("Updated data for node {} with more content", i).repeat(10))
                    );
                    crdt.update_node(updated_node).unwrap();
                }
                
                // Merge with another CRDT
                let mut other_crdt = Crdt::new();
                for i in data_size..(data_size * 2) {
                    let node = create_test_node(
                        &format!("other-node-{}", i),
                        Value::String(format!("Other data for node {}", i).repeat(10))
                    );
                    other_crdt.create_node(node).unwrap();
                }
                
                crdt.merge_crdt(&other_crdt).unwrap();
                
                crdt
            })
        });
    }

    group.finish();
}

fn storage_integration_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("storage_integration_operations");
    group.measurement_time(Duration::from_secs(15));

    // Storage operations with different workloads
    for workload_size in [100, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*workload_size as u64));
        
        group.bench_with_input(BenchmarkId::new("storage_workload", workload_size), workload_size, |b, &workload_size| {
            let temp_dir = TempDir::new().unwrap();
            
            b.to_async(&rt).iter(|| async {
                let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
                let client = Client::new();
                
                // Create workload
                let mut handles = vec![];
                for i in 0..workload_size {
                    let client_clone = client.clone();
                    let server_addr = server.addr().clone();
                    let handle = tokio::spawn(async move {
                        let node_data = json!({
                            "id": format!("workload-node-{}", i),
                            "data": {
                                "name": format!("Workload Node {}", i),
                                "type": "workload",
                                "content": format!("Content for workload node {} with some additional data", i).repeat(5),
                                "index": i,
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            },
                            "metadata": {
                                "workload": true,
                                "batch": i / 100,
                                "created_at": chrono::Utc::now().to_rfc3339()
                            },
                            "tags": ["workload", "benchmark", &format!("batch-{}", i / 100)]
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
                
                // Wait for all creates to complete
                for handle in handles {
                    handle.await.unwrap();
                }
                
                // Now perform queries
                let query_handles = vec![
                    tokio::spawn({
                        let client = client.clone();
                        let server_addr = server.addr().clone();
                        async move {
                            client
                                .get(&format!("http://{}/api/nodes?limit=100", server_addr))
                                .send()
                                .await
                                .unwrap()
                        }
                    }),
                    tokio::spawn({
                        let client = client.clone();
                        let server_addr = server.addr().clone();
                        async move {
                            let search_data = json!({
                                "query": "workload node content",
                                "limit": 50,
                                "threshold": 0.3
                            });
                            client
                                .post(&format!("http://{}/api/vector/search/text", server_addr))
                                .json(&search_data)
                                .send()
                                .await
                                .unwrap()
                        }
                    }),
                    tokio::spawn({
                        let client = client.clone();
                        let server_addr = server.addr().clone();
                        async move {
                            let query_data = json!({
                                "query": "SELECT COUNT(*) as total FROM nodes WHERE data->>'type' = 'workload'",
                                "params": []
                            });
                            client
                                .post(&format!("http://{}/api/sql/query", server_addr))
                                .json(&query_data)
                                .send()
                                .await
                                .unwrap()
                        }
                    })
                ];
                
                for handle in query_handles {
                    handle.await.unwrap();
                }
            })
        });
    }

    group.finish();
}

fn vector_search_integration_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("vector_search_integration_operations");
    group.measurement_time(Duration::from_secs(15));

    // Vector search with different document sets
    for doc_count in [50, 200, 500].iter() {
        group.throughput(Throughput::Elements(*doc_count as u64));
        
        group.bench_with_input(BenchmarkId::new("vector_search_workload", doc_count), doc_count, |b, &doc_count| {
            let temp_dir = TempDir::new().unwrap();
            
            b.to_async(&rt).iter(|| async {
                let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
                let client = Client::new();
                
                // Create documents with different content types
                let document_types = vec![
                    "technical documentation",
                    "user manual",
                    "API reference",
                    "tutorial guide",
                    "troubleshooting",
                    "best practices",
                    "configuration guide",
                    "deployment instructions"
                ];
                
                let mut handles = vec![];
                for i in 0..doc_count {
                    let client_clone = client.clone();
                    let server_addr = server.addr().clone();
                    let doc_type = document_types[i % document_types.len()];
                    let handle = tokio::spawn(async move {
                        let node_data = json!({
                            "id": format!("doc-{}", i),
                            "data": {
                                "name": format!("{} Document {}", doc_type, i),
                                "type": "document",
                                "category": doc_type,
                                "content": format!("This is a {} document number {} with detailed information about {} and related topics. It contains comprehensive content for testing vector search capabilities.", doc_type, i, doc_type).repeat(3),
                                "keywords": [doc_type, "documentation", "guide", "reference"],
                                "difficulty": if i % 3 == 0 { "beginner" } else if i % 3 == 1 { "intermediate" } else { "advanced" }
                            },
                            "metadata": {
                                "document_type": doc_type,
                                "created_at": chrono::Utc::now().to_rfc3339(),
                                "word_count": 150 + (i * 10) % 200
                            },
                            "tags": ["document", doc_type, "searchable"]
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
                
                // Wait for all documents to be created
                for handle in handles {
                    handle.await.unwrap();
                }
                
                // Perform various search queries
                let search_queries = vec![
                    "technical documentation API",
                    "user manual tutorial",
                    "troubleshooting configuration",
                    "deployment best practices",
                    "advanced reference guide"
                ];
                
                let mut search_handles = vec![];
                for query in search_queries {
                    let client_clone = client.clone();
                    let server_addr = server.addr().clone();
                    let handle = tokio::spawn(async move {
                        let search_data = json!({
                            "query": query,
                            "limit": 20,
                            "threshold": 0.3
                        });
                        
                        client_clone
                            .post(&format!("http://{}/api/vector/search/text", server_addr))
                            .json(&search_data)
                            .send()
                            .await
                            .unwrap()
                    });
                    search_handles.push(handle);
                }
                
                for handle in search_handles {
                    handle.await.unwrap();
                }
            })
        });
    }

    group.finish();
}

fn concurrent_workload_benchmarks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_workload_operations");
    group.measurement_time(Duration::from_secs(20));

    // Mixed concurrent workloads
    for concurrent_users in [5, 10, 20, 50].iter() {
        group.throughput(Throughput::Elements(*concurrent_users as u64));
        
        group.bench_with_input(BenchmarkId::new("concurrent_users", concurrent_users), concurrent_users, |b, &concurrent_users| {
            let temp_dir = TempDir::new().unwrap();
            
            b.to_async(&rt).iter(|| async {
                let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
                
                let mut handles = vec![];
                for user_id in 0..concurrent_users {
                    let handle = tokio::spawn(async move {
                        let client = Client::new();
                        let server_addr = server.addr().clone();
                        
                        // Each user performs a mix of operations
                        for operation in 0..10 {
                            match operation % 4 {
                                0 => {
                                    // Create node
                                    let node_data = json!({
                                        "id": format!("user-{}-node-{}", user_id, operation),
                                        "data": {
                                            "name": format!("User {} Node {}", user_id, operation),
                                            "type": "user_data",
                                            "content": format!("Content from user {} operation {}", user_id, operation)
                                        },
                                        "metadata": {
                                            "user_id": user_id,
                                            "operation": operation
                                        },
                                        "tags": ["user_data", &format!("user-{}", user_id)]
                                    });
                                    
                                    client
                                        .post(&format!("http://{}/api/nodes", server_addr))
                                        .json(&node_data)
                                        .send()
                                        .await
                                        .unwrap();
                                },
                                1 => {
                                    // Search
                                    let search_data = json!({
                                        "query": format!("user {} data", user_id),
                                        "limit": 5,
                                        "threshold": 0.5
                                    });
                                    
                                    client
                                        .post(&format!("http://{}/api/vector/search/text", server_addr))
                                        .json(&search_data)
                                        .send()
                                        .await
                                        .unwrap();
                                },
                                2 => {
                                    // SQL query
                                    let query_data = json!({
                                        "query": "SELECT * FROM nodes WHERE data->>'type' = 'user_data' LIMIT 10",
                                        "params": []
                                    });
                                    
                                    client
                                        .post(&format!("http://{}/api/sql/query", server_addr))
                                        .json(&query_data)
                                        .send()
                                        .await
                                        .unwrap();
                                },
                                3 => {
                                    // Get node
                                    client
                                        .get(&format!("http://{}/api/nodes/user-{}-node-{}", server_addr, user_id, operation))
                                        .send()
                                        .await
                                        .unwrap();
                                },
                                _ => {}
                            }
                        }
                    });
                    handles.push(handle);
                }
                
                // Wait for all users to complete
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
    end_to_end_benchmarks,
    crdt_integration_benchmarks,
    storage_integration_benchmarks,
    vector_search_integration_benchmarks,
    concurrent_workload_benchmarks
);
criterion_main!(benches);

