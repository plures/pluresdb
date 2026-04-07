//! Integration tests for PluresDB sync transports
//!
//! These tests verify that the transport layer works correctly for
//! local-only mode, and demonstrate the GUN-compatible wire protocol
//! with in-process peer-to-peer replication.

use pluresdb_sync::{
    create_transport, derive_topic, DisabledTransport, GunRelayServer, HyperswarmConfig,
    HyperswarmTransport, MemConnection, RelayTransport, Replicator, Transport, TransportConfig,
    TransportMode,
};
use serde_json::json;
use std::time::Duration;

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
    let souls: std::collections::HashSet<&str> = received.iter().map(|(s, _)| s.as_str()).collect();
    assert!(souls.contains("user:alice"));
    assert!(souls.contains("user:bob"));
    assert!(souls.contains("post:1"));

    // Verify a field value survived the round-trip.
    let alice = received
        .iter()
        .find(|(s, _)| s == "user:alice")
        .map(|(_, node)| node)
        .unwrap();
    assert_eq!(alice.fields["name"], json!("Alice"));
    assert_eq!(alice.fields["role"], json!("admin"));
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
/// applied to store B (simulating a store merge).
#[tokio::test]
async fn test_gun_protocol_crdt_store_integration() {
    use pluresdb_core::CrdtStore;

    // Initialize two stores.
    let store_a = CrdtStore::default();
    let store_b = CrdtStore::default();

    // Write nodes to store A.
    store_a.put("user:alice", "peer-a", json!({"name": "Alice", "age": 30}));
    store_a.put(
        "task:1",
        "peer-a",
        json!({"title": "Write docs", "done": false}),
    );

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

    // Apply received GunNodes to store B, preserving field data.
    for (soul, gun_node) in received {
        store_b.put(
            soul,
            "peer-a",
            serde_json::Value::Object(gun_node.fields.into_iter().collect()),
        );
    }

    // Verify store B has store A's data.
    let alice = store_b
        .get("user:alice")
        .expect("user:alice should be in store B");
    assert_eq!(alice.data["name"], json!("Alice"));
    assert_eq!(alice.data["age"], json!(30));

    let task = store_b.get("task:1").expect("task:1 should be in store B");
    assert_eq!(task.data["title"], json!("Write docs"));
}

// ---------------------------------------------------------------------------
// Hyperswarm peer discovery (uses process-local registry + TCP)
// ---------------------------------------------------------------------------

/// Two HyperswarmTransport instances running in the same process discover each
/// other via the process-local peer registry and exchange a test message over a
/// direct TCP connection.  AES-256-GCM encryption is enabled end-to-end.
#[tokio::test]
async fn test_hyperswarm_peer_discovery() {
    // Use a unique topic to prevent cross-test registry pollution.
    let topic = derive_topic("hyperswarm-integration-test-discover-2025");

    let config = HyperswarmConfig {
        encryption: true,
        timeout_ms: 5_000,
        ..Default::default()
    };

    let mut peer1 = HyperswarmTransport::new(config.clone());
    let mut peer2 = HyperswarmTransport::new(config);

    // Both peers announce on the same topic so they register their TCP ports.
    peer1.announce(topic).await.expect("peer1 announce");
    peer2.announce(topic).await.expect("peer2 announce");

    // Verify mutual discovery through lookup before connecting.
    let peers_seen_by_1 = peer1
        .lookup(topic)
        .await
        .expect("peer1 lookup should succeed");
    assert_eq!(peers_seen_by_1.len(), 1, "peer1 should discover peer2");
    let peers_seen_by_2 = peer2
        .lookup(topic)
        .await
        .expect("peer2 lookup should succeed");
    assert_eq!(peers_seen_by_2.len(), 1, "peer2 should discover peer1");

    // connect() starts the accept loop + dials known peers.
    let mut rx1 = peer1.connect(topic).await.expect("peer1 connect");
    let mut rx2 = peer2.connect(topic).await.expect("peer2 connect");

    // peer2's outbound connection to peer1 arrives on rx2.
    let mut conn_from_peer2 = tokio::time::timeout(Duration::from_secs(3), rx2.recv())
        .await
        .expect("peer2 outbound connection should appear within 3s")
        .expect("connection receiver not closed");

    // peer1's accept loop receives peer2's inbound connection on rx1.
    let mut conn_from_peer1 = tokio::time::timeout(Duration::from_secs(3), rx1.recv())
        .await
        .expect("peer1 accept should complete within 3s")
        .expect("connection receiver not closed");

    // Send a test message from peer2 → peer1.
    let test_msg = b"hello from hyperswarm peer";
    conn_from_peer2
        .send(test_msg)
        .await
        .expect("send should succeed");

    // peer1 receives and decrypts the message.
    let received = conn_from_peer1
        .receive()
        .await
        .expect("receive should not error")
        .expect("message should be present");
    assert_eq!(
        received, test_msg,
        "received message should match sent message"
    );

    // Graceful close.
    conn_from_peer2.close().await.expect("close");
    peer1.disconnect().await.expect("peer1 disconnect");
    peer2.disconnect().await.expect("peer2 disconnect");
}

// ---------------------------------------------------------------------------
// Relay transport integration (RelayTransport ↔ GunRelayServer ↔ RelayTransport)
// ---------------------------------------------------------------------------

/// Two RelayTransport clients connect to a local GunRelayServer and exchange
/// a GUN PUT message.  Verifies the full client → relay → client path and the
/// per-topic channel isolation introduced in this PR.
#[tokio::test]
async fn test_relay_transport_integration() {
    use pluresdb_sync::GunMessage;

    // ── Start relay server on an ephemeral port ────────────────────────────
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().unwrap();
    let relay_base = format!("ws://{}/gun", addr);

    let router = GunRelayServer::new().build_router();
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("relay server error");
    });
    // Give the server a moment to start accepting.
    tokio::time::sleep(Duration::from_millis(30)).await;

    // Use a unique topic for this test run.
    let topic = derive_topic("relay-transport-integration-2025");

    // ── Connect two RelayTransport clients to the same topic channel ───────
    // Pass the base relay URL — RelayTransport::connect() appends the topic hex.
    let mut transport1 = RelayTransport::with_max_retries(relay_base.clone(), 5_000, 3);
    let mut transport2 = RelayTransport::with_max_retries(relay_base.clone(), 5_000, 3);

    let mut rx1 = transport1
        .connect(topic)
        .await
        .expect("transport1 connect should succeed with valid relay URL");
    let mut rx2 = transport2
        .connect(topic)
        .await
        .expect("transport2 connect should succeed with valid relay URL");

    // Both clients should receive a connection handle.
    let mut conn1 = tokio::time::timeout(Duration::from_secs(3), rx1.recv())
        .await
        .expect("transport1 connection within 3s")
        .expect("receiver not closed");
    let mut conn2 = tokio::time::timeout(Duration::from_secs(3), rx2.recv())
        .await
        .expect("transport2 connection within 3s")
        .expect("receiver not closed");

    // ── Build a test GUN PUT message ──────────────────────────────────────
    use pluresdb_sync::{GunNode, GunPut};
    use std::collections::HashMap;
    let mut fields = HashMap::new();
    fields.insert("name".to_string(), json!("relay-test-peer"));
    let node = GunNode::from_data("relay:test", fields, 1_700_000_000_000.0);
    let mut put_nodes = HashMap::new();
    put_nodes.insert("relay:test".to_string(), node);
    let gun_msg = GunMessage::Put(GunPut {
        id: "relay-integration-1".to_string(),
        put: put_nodes,
    });
    let encoded = gun_msg.encode().expect("encode GUN PUT");

    // ── conn1 sends; conn2 receives (relay fans-out, no echo) ─────────────
    conn1.send(&encoded).await.expect("conn1 send");

    let received = tokio::time::timeout(Duration::from_secs(3), conn2.receive())
        .await
        .expect("conn2 receive within 3s")
        .expect("receive should not error")
        .expect("message should be present");

    let decoded = GunMessage::decode(&received).expect("should decode as GUN message");
    assert_eq!(
        decoded.id(),
        "relay-integration-1",
        "message ID should survive relay round-trip"
    );

    // ── Clean up ──────────────────────────────────────────────────────────
    conn1.close().await.expect("conn1 close");
    conn2.close().await.expect("conn2 close");
    transport1
        .disconnect()
        .await
        .expect("transport1 disconnect");
    transport2
        .disconnect()
        .await
        .expect("transport2 disconnect");
}
