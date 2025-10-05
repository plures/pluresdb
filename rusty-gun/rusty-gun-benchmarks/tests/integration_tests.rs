use rusty_gun_core::crdt::Crdt;
use rusty_gun_core::node::Node;
use rusty_gun_core::types::{NodeId, Value, Metadata};
use rusty_gun_storage::sqlite::SQLiteStorage;
use rusty_gun_storage::vector::HNSWIndex;
use rusty_gun_api::server::ApiServer;
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

#[tokio::test]
async fn test_crdt_basic_operations() {
    let mut crdt = Crdt::new();
    
    // Test node creation
    let node = create_test_node("test-node", Value::String("test data".to_string()));
    assert!(crdt.create_node(node).is_ok());
    
    // Test node retrieval
    let retrieved = crdt.get_node(&NodeId::from("test-node"));
    assert!(retrieved.is_ok());
    assert_eq!(retrieved.unwrap().id(), &NodeId::from("test-node"));
    
    // Test node update
    let updated_node = create_test_node("test-node", Value::String("updated data".to_string()));
    assert!(crdt.update_node(updated_node).is_ok());
    
    // Test node deletion
    assert!(crdt.delete_node(&NodeId::from("test-node")).is_ok());
}

#[tokio::test]
async fn test_crdt_conflict_resolution() {
    let mut crdt1 = Crdt::new();
    let mut crdt2 = Crdt::new();
    
    // Create conflicting nodes
    let node1 = create_test_node("conflict-node", Value::String("version 1".to_string()));
    let node2 = create_test_node("conflict-node", Value::String("version 2".to_string()));
    
    crdt1.create_node(node1).unwrap();
    crdt2.create_node(node2).unwrap();
    
    // Merge CRDTs
    assert!(crdt1.merge_crdt(&crdt2).is_ok());
    
    // Check that conflict was resolved
    let resolved = crdt1.get_node(&NodeId::from("conflict-node"));
    assert!(resolved.is_ok());
}

#[tokio::test]
async fn test_storage_operations() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let storage = SQLiteStorage::new(db_path).await.unwrap();
    
    // Test node storage
    let node = create_test_node("storage-test", Value::String("storage data".to_string()));
    assert!(storage.store_node(&node).await.is_ok());
    
    // Test node retrieval
    let retrieved = storage.get_node(&NodeId::from("storage-test")).await;
    assert!(retrieved.is_ok());
    assert_eq!(retrieved.unwrap().id(), &NodeId::from("storage-test"));
    
    // Test node update
    let updated_node = create_test_node("storage-test", Value::String("updated storage data".to_string()));
    assert!(storage.store_node(&updated_node).await.is_ok());
    
    // Test node deletion
    assert!(storage.delete_node(&NodeId::from("storage-test")).await.is_ok());
}

#[tokio::test]
async fn test_vector_search() {
    let temp_dir = TempDir::new().unwrap();
    let index_path = temp_dir.path().join("hnsw_index");
    
    let mut index = HNSWIndex::new(128, 16, 200, index_path).await.unwrap();
    
    // Insert test vectors
    let vectors = vec![
        vec![0.1, 0.2, 0.3, 0.4],
        vec![0.5, 0.6, 0.7, 0.8],
        vec![0.9, 1.0, 1.1, 1.2],
    ];
    
    for (i, vector) in vectors.iter().enumerate() {
        assert!(index.insert(&format!("test-{}", i), vector).await.is_ok());
    }
    
    // Test search
    let query_vector = vec![0.15, 0.25, 0.35, 0.45];
    let results = index.search(&query_vector, 2, 0.5).await.unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_api_server() {
    let temp_dir = TempDir::new().unwrap();
    let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
    let client = Client::new();
    
    // Test node creation via API
    let node_data = json!({
        "id": "api-test-node",
        "data": {"name": "API Test Node", "type": "test"},
        "metadata": {},
        "tags": ["test", "api"]
    });
    
    let response = client
        .post(&format!("http://{}/api/nodes", server.addr()))
        .json(&node_data)
        .send()
        .await
        .unwrap();
    
    assert!(response.status().is_success());
    
    // Test node retrieval via API
    let response = client
        .get(&format!("http://{}/api/nodes/api-test-node", server.addr()))
        .send()
        .await
        .unwrap();
    
    assert!(response.status().is_success());
}

#[tokio::test]
async fn test_sql_compatibility() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let storage = SQLiteStorage::new(db_path).await.unwrap();
    
    // Test SQL execution
    let result = storage.execute_sql("SELECT 1 as test_value", vec![]).await.unwrap();
    assert!(!result.rows.is_empty());
    assert_eq!(result.rows[0]["test_value"], 1);
}

#[tokio::test]
async fn test_vector_search_service() {
    let temp_dir = TempDir::new().unwrap();
    let service_path = temp_dir.path().join("vector_service");
    
    let service = rusty_gun_storage::vector_service::VectorSearchService::new(
        service_path,
        "sentence-transformers/all-MiniLM-L6-v2".to_string(),
        128,
        16,
        200,
    ).await.unwrap();
    
    // Test text addition
    assert!(service.add_text("doc1", "This is a test document", None).await.is_ok());
    assert!(service.add_text("doc2", "This is another test document", None).await.is_ok());
    
    // Test text search
    let results = service.search_text("test document", 10, 0.5).await.unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_end_to_end_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
    let client = Client::new();
    
    // 1. Create multiple nodes
    for i in 0..5 {
        let node_data = json!({
            "id": format!("workflow-node-{}", i),
            "data": {
                "name": format!("Workflow Node {}", i),
                "type": "workflow",
                "content": format!("Content for workflow node {} with some test data", i)
            },
            "metadata": {
                "workflow": true,
                "index": i
            },
            "tags": ["workflow", "test"]
        });
        
        let response = client
            .post(&format!("http://{}/api/nodes", server.addr()))
            .json(&node_data)
            .send()
            .await
            .unwrap();
        
        assert!(response.status().is_success());
    }
    
    // 2. Search for nodes
    let search_data = json!({
        "query": "workflow node content",
        "limit": 10,
        "threshold": 0.5
    });
    
    let response = client
        .post(&format!("http://{}/api/vector/search/text", server.addr()))
        .json(&search_data)
        .send()
        .await
        .unwrap();
    
    assert!(response.status().is_success());
    
    // 3. Query with SQL
    let query_data = json!({
        "query": "SELECT * FROM nodes WHERE data->>'type' = 'workflow'",
        "params": []
    });
    
    let response = client
        .post(&format!("http://{}/api/sql/query", server.addr()))
        .json(&query_data)
        .send()
        .await
        .unwrap();
    
    assert!(response.status().is_success());
    
    // 4. Update a node
    let update_data = json!({
        "data": {
            "name": "Updated Workflow Node 0",
            "type": "workflow",
            "content": "Updated content for workflow node 0",
            "updated": true
        },
        "metadata": {
            "workflow": true,
            "index": 0,
            "updated": true
        }
    });
    
    let response = client
        .put(&format!("http://{}/api/nodes/workflow-node-0", server.addr()))
        .json(&update_data)
        .send()
        .await
        .unwrap();
    
    assert!(response.status().is_success());
    
    // 5. Delete a node
    let response = client
        .delete(&format!("http://{}/api/nodes/workflow-node-4", server.addr()))
        .send()
        .await
        .unwrap();
    
    assert!(response.status().is_success());
}

#[tokio::test]
async fn test_concurrent_operations() {
    let temp_dir = TempDir::new().unwrap();
    let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
    
    let mut handles = vec![];
    
    // Spawn concurrent operations
    for i in 0..10 {
        let handle = tokio::spawn(async move {
            let client = Client::new();
            let server_addr = server.addr().clone();
            
            // Create node
            let node_data = json!({
                "id": format!("concurrent-node-{}", i),
                "data": {
                    "name": format!("Concurrent Node {}", i),
                    "type": "concurrent",
                    "content": format!("Content for concurrent node {}", i)
                },
                "metadata": {
                    "concurrent": true,
                    "index": i
                },
                "tags": ["concurrent", "test"]
            });
            
            let response = client
                .post(&format!("http://{}/api/nodes", server_addr))
                .json(&node_data)
                .send()
                .await
                .unwrap();
            
            assert!(response.status().is_success());
        });
        
        handles.push(handle);
    }
    
    // Wait for all operations to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
    let client = Client::new();
    
    // Test getting non-existent node
    let response = client
        .get(&format!("http://{}/api/nodes/non-existent", server.addr()))
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 404);
    
    // Test invalid JSON
    let response = client
        .post(&format!("http://{}/api/nodes", server.addr()))
        .body("invalid json")
        .header("Content-Type", "application/json")
        .send()
        .await
        .unwrap();
    
    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn test_performance_characteristics() {
    let temp_dir = TempDir::new().unwrap();
    let server = ApiServer::new("127.0.0.1:0".parse().unwrap(), temp_dir.path().to_path_buf()).await.unwrap();
    let client = Client::new();
    
    let start = std::time::Instant::now();
    
    // Create 100 nodes
    for i in 0..100 {
        let node_data = json!({
            "id": format!("perf-node-{}", i),
            "data": {
                "name": format!("Performance Node {}", i),
                "type": "performance",
                "content": format!("Content for performance node {} with some test data", i)
            },
            "metadata": {
                "performance": true,
                "index": i
            },
            "tags": ["performance", "test"]
        });
        
        let response = client
            .post(&format!("http://{}/api/nodes", server.addr()))
            .json(&node_data)
            .send()
            .await
            .unwrap();
        
        assert!(response.status().is_success());
    }
    
    let create_duration = start.elapsed();
    println!("Created 100 nodes in {:?}", create_duration);
    
    // Search all nodes
    let search_start = std::time::Instant::now();
    let search_data = json!({
        "query": "performance node content",
        "limit": 50,
        "threshold": 0.3
    });
    
    let response = client
        .post(&format!("http://{}/api/vector/search/text", server.addr()))
        .json(&search_data)
        .send()
        .await
        .unwrap();
    
    let search_duration = search_start.elapsed();
    println!("Searched nodes in {:?}", search_duration);
    
    assert!(response.status().is_success());
    assert!(create_duration.as_secs() < 10); // Should complete within 10 seconds
    assert!(search_duration.as_secs() < 5); // Search should complete within 5 seconds
}

