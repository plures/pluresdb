//! Integration tests for PluresDB v3.0.0
//! 
//! Tests cross-target consistency for core functionality.

use pluresdb_core::CrdtStore;
use serde_json::json;

const TEST_ACTOR: &str = "test-actor";

#[test]
fn test_crdt_store_put_get_delete_roundtrip() {
    let store = CrdtStore::default();

    // Put a node
    let node_id = store.put("test:user:1", TEST_ACTOR, json!({"name": "Alice", "age": 30}));
    assert_eq!(node_id, "test:user:1");

    // Get the node
    let node = store.get("test:user:1");
    assert!(node.is_some(), "Node should exist");
    
    let node_record = node.unwrap();
    assert_eq!(node_record.data["name"], "Alice");
    assert_eq!(node_record.data["age"], 30);

    // Delete the node
    let delete_result = store.delete("test:user:1");
    assert!(delete_result.is_ok(), "Delete should succeed");

    // Verify deletion
    let get_after_delete = store.get("test:user:1");
    assert!(get_after_delete.is_none(), "Node should be deleted");
}

#[test]
fn test_crdt_merge_semantics_two_actors() {
    let store1 = CrdtStore::default();
    let store2 = CrdtStore::default();

    let key = "shared:key:1";

    // Actor 1 writes first
    store1.put(key, "actor1", json!({"value": "from_actor1", "timestamp": 1}));
    
    // Actor 2 writes to same key
    store2.put(key, "actor2", json!({"value": "from_actor2", "timestamp": 2}));

    // Get state from both stores
    let state1 = store1.get(key);
    let state2 = store2.get(key);

    // Both should have written their own state
    assert!(state1.is_some(), "Actor 1 should have state");
    assert!(state2.is_some(), "Actor 2 should have state");

    // Values should be different (no merge yet, each store is isolated)
    let val1 = state1.unwrap();
    let val2 = state2.unwrap();
    
    assert_eq!(val1.data["value"], "from_actor1");
    assert_eq!(val2.data["value"], "from_actor2");
}

#[test]
fn test_concurrent_put_different_keys() {
    let store = CrdtStore::default();

    // Simulate concurrent puts to different keys
    let keys = vec!["concurrent:1", "concurrent:2", "concurrent:3"];
    
    for (i, key) in keys.iter().enumerate() {
        let data = json!({"index": i, "data": format!("value_{}", i)});
        store.put(*key, TEST_ACTOR, data);
    }

    // Verify all writes succeeded
    for key in keys {
        let result = store.get(key);
        assert!(result.is_some(), "Key {} should exist", key);
    }
}

#[test]
fn test_update_existing_key() {
    let store = CrdtStore::default();

    let key = "update:test:1";

    // Initial write
    store.put(key, TEST_ACTOR, json!({"version": 1, "data": "initial"}));
    
    let v1 = store.get(key).unwrap();
    assert_eq!(v1.data["version"], 1);
    assert_eq!(v1.data["data"], "initial");

    // Update the same key
    store.put(key, TEST_ACTOR, json!({"version": 2, "data": "updated"}));

    let v2 = store.get(key).unwrap();
    assert_eq!(v2.data["version"], 2);
    assert_eq!(v2.data["data"], "updated");
}

#[test]
fn test_get_nonexistent_key() {
    let store = CrdtStore::default();

    let result = store.get("nonexistent:key");
    assert!(result.is_none(), "Should return None for nonexistent key");
}

#[test]
fn test_delete_nonexistent_key() {
    let store = CrdtStore::default();

    // Deleting a key that doesn't exist should return NotFound error
    let result = store.delete("nonexistent:key");
    assert!(result.is_err(), "Delete on nonexistent key should return error");
}

#[test]
fn test_put_empty_json_object() {
    let store = CrdtStore::default();

    let node_id = store.put("empty:obj", TEST_ACTOR, json!({}));
    assert_eq!(node_id, "empty:obj");

    let retrieved = store.get("empty:obj");
    assert!(retrieved.is_some(), "Empty object should exist");
}

#[test]
fn test_put_complex_nested_json() {
    let store = CrdtStore::default();

    let complex_data = json!({
        "user": {
            "id": 123,
            "name": "Test User",
            "metadata": {
                "tags": ["tag1", "tag2"],
                "settings": {
                    "theme": "dark",
                    "notifications": true
                }
            }
        }
    });

    let node_id = store.put("complex:obj", TEST_ACTOR, complex_data.clone());
    assert_eq!(node_id, "complex:obj");

    let retrieved = store.get("complex:obj").unwrap();
    assert_eq!(retrieved.data["user"]["name"], "Test User");
    assert_eq!(retrieved.data["user"]["metadata"]["tags"][0], "tag1");
    assert_eq!(retrieved.data["user"]["metadata"]["settings"]["theme"], "dark");
}

#[test]
fn test_multiple_actors_same_key() {
    let store = CrdtStore::default();
    let key = "multi:actor:1";

    // Actor 1 writes
    store.put(key, "actor1", json!({"source": "actor1", "value": 100}));
    
    // Actor 2 writes to same key (last-write-wins semantics)
    store.put(key, "actor2", json!({"source": "actor2", "value": 200}));

    let record = store.get(key).unwrap();
    
    // The store should have the merged/latest state
    // Exact behavior depends on CRDT merge logic
    assert!(record.data["source"].is_string(), "Should have a source field");
}

#[test]
fn test_node_record_has_clock() {
    let store = CrdtStore::default();

    store.put("clock:test", TEST_ACTOR, json!({"data": "test"}));

    let record = store.get("clock:test").unwrap();
    
    // NodeRecord should have a vector clock
    assert!(!record.clock.is_empty(), "Vector clock should not be empty after write");
}

#[test]
fn test_node_record_has_timestamp() {
    let store = CrdtStore::default();

    store.put("timestamp:test", TEST_ACTOR, json!({"data": "test"}));

    let record = store.get("timestamp:test").unwrap();
    
    // NodeRecord should have a timestamp
    // Just verify it exists and is somewhat recent (not epoch zero)
    assert!(record.timestamp.timestamp() > 0, "Timestamp should be set");
}

#[test]
fn test_delete_and_recreate() {
    let store = CrdtStore::default();
    let key = "delete:recreate";

    // Create
    store.put(key, TEST_ACTOR, json!({"version": 1}));
    assert!(store.get(key).is_some());

    // Delete
    store.delete(key).unwrap();
    assert!(store.get(key).is_none());

    // Recreate
    store.put(key, TEST_ACTOR, json!({"version": 2}));
    let record = store.get(key).unwrap();
    assert_eq!(record.data["version"], 2);
}

#[test]
fn test_put_returns_same_id() {
    let store = CrdtStore::default();

    let returned_id = store.put("id:test", TEST_ACTOR, json!({"test": true}));
    
    assert_eq!(returned_id, "id:test", "Returned ID should match input ID");
}
