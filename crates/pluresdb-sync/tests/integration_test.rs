//! Integration tests for PluresDB sync transports
//!
//! These tests verify that the transport layer works correctly for
//! local-only mode, and demonstrate the GUN-compatible wire protocol
//! with in-process peer-to-peer replication.

use pluresdb_sync::{
    create_transport, derive_topic, DisabledTransport, MemConnection, Replicator, Transport,
    TransportConfig, TransportMode,
};
use serde_json::json;

#[tokio::test]
async fn test_disabled_transport_integration() {
    // Create a disabled transport
    let mut transport = DisabledTransport::new();

    // Derive a topic from a database ID
    let db_id = "test-database-123";
    let topic = derive_topic(db_id);

    // Verify topic is derived correctly
    assert_eq!(topic.len(), 32);

    // Announce should succeed silently (no-op)
    let result = transport.announce(topic).await;
    assert!(result.is_ok());

    // Lookup should return empty list
    let peers = transport.lookup(topic).await.unwrap();
    assert!(peers.is_empty());

    // Connect should fail with helpful message
    let result = transport.connect(topic).await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("local-only"));

    // Disconnect should succeed
    let result = transport.disconnect().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_transport_factory() {
    // Test creating disabled transport via factory
    let config = TransportConfig {
        mode: TransportMode::Disabled,
        ..Default::default()
    };
    let transport = create_transport(config);
    assert_eq!(transport.name(), "disabled");

    // Test creating hyperswarm transport via factory
    let config = TransportConfig {
        mode: TransportMode::Hyperswarm,
        ..Default::default()
    };
    let transport = create_transport(config);
    assert_eq!(transport.name(), "hyperswarm");

    // Test creating relay transport via factory
    let config = TransportConfig {
        mode: TransportMode::Relay,
        relay_url: Some("wss://test-relay.example.com".to_string()),
        ..Default::default()
    };
    let transport = create_transport(config);
    assert_eq!(transport.name(), "relay");
}

#[tokio::test]
async fn test_topic_derivation_consistency() {
    // Same database ID should always produce same topic
    let db_id = "my-app-database";
    let topic1 = derive_topic(db_id);
    let topic2 = derive_topic(db_id);
    assert_eq!(topic1, topic2);

    // Different database IDs should produce different topics
    let topic3 = derive_topic("different-database");
    assert_ne!(topic1, topic3);

    // Topics should be deterministic
    let known_db_id = "test";
    let topic = derive_topic(known_db_id);
    // Should always produce the same hash for "test"
    let hex = hex::encode(topic);
    assert_eq!(hex.len(), 64); // 32 bytes = 64 hex chars
}

// ---------------------------------------------------------------------------
// GUN wire protocol + in-process peer-to-peer replication
// ---------------------------------------------------------------------------

/// Populate a store with some graph nodes.
fn populate_store_a() -> Vec<(String, serde_json::Value)> {
    vec![
        (
            "user:alice".to_string(),
            json!({"name": "Alice", "role": "admin", "age": 30}),
        ),
        (
            "user:bob".to_string(),
            json!({"name": "Bob", "role": "member", "age": 25}),
        ),
        (
            "post:1".to_string(),
            json!({"title": "Hello World", "author": "user:alice"}),
        ),
    ]
}

fn populate_store_b() -> Vec<(String, serde_json::Value)> {
    vec![
        (
            "user:charlie".to_string(),
            json!({"name": "Charlie", "role": "viewer"}),
        ),
        (
            "post:2".to_string(),
            json!({"title": "GUN Protocol", "author": "user:charlie"}),
        ),
    ]
}

/// Rust peer A pushes its nodes to peer B — peer B receives and stores them.
#[tokio::test]
async fn test_gun_protocol_push_all() {
    let (mut conn_a, mut conn_b) = MemConnection::pair("peer-a", "peer-b");
    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");

    let nodes_a = populate_store_a();

    // Concurrently: A pushes while B listens.
    let (push_result, received) = tokio::join!(
        rep_a.push_all(&mut conn_a, &nodes_a),
        rep_b.receive_all(&mut conn_b),
    );
    push_result.expect("push_all should succeed");
    let received = received.expect("receive_all should succeed");

    // Peer B should have all three nodes from A.
    assert_eq!(received.len(), 3);
    let souls: std::collections::HashSet<&str> =
        received.iter().map(|(s, _)| s.as_str()).collect();
    assert!(souls.contains("user:alice"));
    assert!(souls.contains("user:bob"));
    assert!(souls.contains("post:1"));

    // Verify a field value survived the round-trip.
    let alice = received
        .iter()
        .find(|(s, _)| s == "user:alice")
        .map(|(_, f)| f)
        .unwrap();
    assert_eq!(alice["name"], json!("Alice"));
    assert_eq!(alice["role"], json!("admin"));
}

/// Bidirectional sync: both peers exchange their local nodes in a single
/// `sync()` call using in-process MemConnection channels.
#[tokio::test]
async fn test_gun_protocol_bidirectional_sync() {
    let (mut conn_a, mut conn_b) = MemConnection::pair("peer-a", "peer-b");
    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");

    let nodes_a = populate_store_a();
    let nodes_b = populate_store_b();

    // Both sides call sync() concurrently — each pushes then receives.
    let (from_b, from_a) = tokio::join!(
        rep_a.sync(&mut conn_a, &nodes_a),
        rep_b.sync(&mut conn_b, &nodes_b),
    );
    let from_b = from_b.expect("peer-a sync should succeed");
    let from_a = from_a.expect("peer-b sync should succeed");

    // peer-a received peer-b's nodes
    assert_eq!(from_b.len(), 2, "peer-a should receive 2 nodes from peer-b");
    let souls_from_b: Vec<&str> = from_b.iter().map(|(s, _)| s.as_str()).collect();
    assert!(souls_from_b.contains(&"user:charlie"));
    assert!(souls_from_b.contains(&"post:2"));

    // peer-b received peer-a's nodes
    assert_eq!(from_a.len(), 3, "peer-b should receive 3 nodes from peer-a");
    let souls_from_a: Vec<&str> = from_a.iter().map(|(s, _)| s.as_str()).collect();
    assert!(souls_from_a.contains(&"user:alice"));
    assert!(souls_from_a.contains(&"user:bob"));
    assert!(souls_from_a.contains(&"post:1"));
}

/// Demonstrate integration with `pluresdb_core::CrdtStore`:
/// nodes written to store A are pushed over GUN PUT messages and
/// applied to store B's hashmap (simulating a store merge).
#[tokio::test]
async fn test_gun_protocol_crdt_store_integration() {
    use pluresdb_core::CrdtStore;

    // Initialize two stores.
    let store_a = CrdtStore::default();
    let store_b = CrdtStore::default();

    // Write nodes to store A.
    store_a.put("user:alice", "peer-a", json!({"name": "Alice", "age": 30}));
    store_a.put("task:1", "peer-a", json!({"title": "Write docs", "done": false}));

    // Collect nodes from store A and convert to (soul, data) pairs.
    let nodes_a: Vec<(String, serde_json::Value)> = store_a
        .list()
        .into_iter()
        .map(|rec| (rec.id, rec.data))
        .collect();

    // Create in-process connections.
    let (mut conn_a, mut conn_b) = MemConnection::pair("peer-a", "peer-b");
    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");

    // Push from A, receive on B.
    let (push_result, received) = tokio::join!(
        rep_a.push_all(&mut conn_a, &nodes_a),
        rep_b.receive_all(&mut conn_b),
    );
    push_result.unwrap();
    let received = received.unwrap();

    // Apply received nodes to store B.
    for (soul, fields) in received {
        store_b.put(soul, "peer-a", serde_json::Value::Object(
            fields.into_iter().collect(),
        ));
    }

    // Verify store B has store A's data.
    let alice = store_b.get("user:alice").expect("user:alice should be in store B");
    assert_eq!(alice.data["name"], json!("Alice"));
    assert_eq!(alice.data["age"], json!(30));

    let task = store_b.get("task:1").expect("task:1 should be in store B");
    assert_eq!(task.data["title"], json!("Write docs"));
}

// TODO: Add these tests once hyperswarm-rs is integrated
#[ignore]
#[tokio::test]
async fn test_hyperswarm_peer_discovery() {
    // This test will be implemented once hyperswarm-rs is available
    // It should:
    // 1. Create two HyperswarmTransport instances
    // 2. Have both announce on the same topic
    // 3. Verify they can discover each other
    // 4. Establish a connection
    // 5. Send/receive a test message
    panic!("Not yet implemented - waiting for hyperswarm-rs");
}

// TODO: Add these tests once relay is implemented
#[ignore]
#[tokio::test]
async fn test_relay_transport_integration() {
    // This test will verify relay transport works
    // It should:
    // 1. Connect to a test relay server
    // 2. Announce on a topic
    // 3. Verify peer discovery through relay
    // 4. Send/receive messages through relay
    panic!("Not yet implemented - waiting for relay implementation");
}

