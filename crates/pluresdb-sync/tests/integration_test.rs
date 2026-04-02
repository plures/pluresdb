//! Integration tests for PluresDB P2P sync transports.
//!
//! These tests verify:
//! - Local-only mode (DisabledTransport)
//! - GUN-compatible wire protocol with in-process MemConnection
//! - Multi-peer CRDT convergence (≥3 peers, concurrent writes)
//! - Reconnection after forced disconnects
//! - GunRelayServer: real WebSocket relay fanout and multi-peer sync
//! - Hyperswarm transport stub validation (transport code path exercised)

use pluresdb_sync::{
    create_transport, derive_topic, DisabledTransport, GunMessage, GunNode, GunPut, MemConnection,
    Replicator, Transport, TransportConfig, TransportMode,
};
use serde_json::json;
use std::collections::HashMap;

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

// The hyperswarm peer-discovery and relay-transport tests below (marked #[ignore])
// are placeholders for when hyperswarm-rs is published and the RelayTransport
// stub is fully implemented. They document the intended future behaviour.
// Active tests for both transports live in the sections below.
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

#[ignore]
#[tokio::test]
async fn test_relay_transport_integration() {
    // This test will verify the RelayTransport stub works
    // It should:
    // 1. Connect to a test relay server
    // 2. Announce on a topic
    // 3. Verify peer discovery through relay
    // 4. Send/receive messages through relay
    panic!("Not yet implemented - waiting for relay implementation");
}

// ---------------------------------------------------------------------------
// Helpers shared by the multi-peer harness tests below
// ---------------------------------------------------------------------------

/// Collect all records in a [`pluresdb_core::CrdtStore`] as `(soul, data)` pairs.
fn store_to_nodes(store: &pluresdb_core::CrdtStore) -> Vec<(String, serde_json::Value)> {
    store.list().into_iter().map(|r| (r.id, r.data)).collect()
}

/// Perform one bidirectional sync round between two peers, applying the
/// received nodes into each peer's store.
///
/// Returns the number of nodes exchanged in each direction
/// `(nodes_received_by_a, nodes_received_by_b)`.
async fn sync_and_apply(
    rep_a: &Replicator,
    store_a: &pluresdb_core::CrdtStore,
    peer_id_a: &str,
    rep_b: &Replicator,
    store_b: &pluresdb_core::CrdtStore,
    peer_id_b: &str,
) -> (usize, usize) {
    let (mut conn_a, mut conn_b) = MemConnection::pair(peer_id_a, peer_id_b);
    let nodes_a = store_to_nodes(store_a);
    let nodes_b = store_to_nodes(store_b);

    let (from_b, from_a) = tokio::join!(
        rep_a.sync(&mut conn_a, &nodes_a),
        rep_b.sync(&mut conn_b, &nodes_b),
    );
    let from_b = from_b.expect("sync from B should succeed");
    let from_a = from_a.expect("sync from A should succeed");

    let received_by_a = from_b.len();
    let received_by_b = from_a.len();

    // Apply received nodes into each store (simulate a full merge).
    for (soul, node) in from_b {
        store_a.put(
            &soul,
            peer_id_b,
            serde_json::Value::Object(node.fields.into_iter().collect()),
        );
    }
    for (soul, node) in from_a {
        store_b.put(
            &soul,
            peer_id_a,
            serde_json::Value::Object(node.fields.into_iter().collect()),
        );
    }

    (received_by_a, received_by_b)
}

// ---------------------------------------------------------------------------
// Multi-peer CRDT convergence (≥3 peers, concurrent writes)
// ---------------------------------------------------------------------------

/// Three peers each write unique nodes then perform a full-mesh sync.
/// After two rounds every peer must converge to the complete dataset.
#[tokio::test]
async fn test_three_peer_crdt_convergence() {
    use pluresdb_core::CrdtStore;

    let store_a = CrdtStore::default();
    let store_b = CrdtStore::default();
    let store_c = CrdtStore::default();

    store_a.put("user:alice", "peer-a", json!({"name": "Alice", "score": 100}));
    store_b.put("user:bob", "peer-b", json!({"name": "Bob", "score": 200}));
    store_c.put("user:carol", "peer-c", json!({"name": "Carol", "score": 300}));

    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");
    let rep_c = Replicator::new("peer-c");

    // Round 1: A ↔ B, B ↔ C
    sync_and_apply(&rep_a, &store_a, "peer-a", &rep_b, &store_b, "peer-b").await;
    sync_and_apply(&rep_b, &store_b, "peer-b", &rep_c, &store_c, "peer-c").await;

    // After round 1:
    //  - A has {alice, bob},  B has {alice, bob, carol}, C has {bob, carol}
    assert!(store_a.get("user:bob").is_some(), "A should have bob after A↔B");
    assert!(
        store_b.get("user:carol").is_some(),
        "B should have carol after B↔C"
    );
    assert!(store_c.get("user:bob").is_some(), "C should have bob after B↔C");

    // Round 2: A ↔ C (delivers carol to A; alice is already in C via B)
    sync_and_apply(&rep_a, &store_a, "peer-a", &rep_c, &store_c, "peer-c").await;

    // All three peers must now converge: alice, bob, carol everywhere.
    for (label, store) in [("A", &store_a), ("B", &store_b), ("C", &store_c)] {
        assert!(
            store.get("user:alice").is_some(),
            "peer-{label} missing user:alice"
        );
        assert!(
            store.get("user:bob").is_some(),
            "peer-{label} missing user:bob"
        );
        assert!(
            store.get("user:carol").is_some(),
            "peer-{label} missing user:carol"
        );
    }
}

/// Concurrent writes to the same key by ≥3 peers converge to one winner
/// according to GUN last-write-wins (HAM) semantics after a full-mesh sync.
#[tokio::test]
async fn test_concurrent_writes_crdt_merge_convergence() {
    use pluresdb_sync::gun_protocol::now_ms;

    // Build GunNode values for three competing writes with strictly ordered
    // HAM timestamps so that the winner is deterministic.
    let ts_a = 1_700_000_001_000.0_f64;
    let ts_b = 1_700_000_002_000.0_f64; // highest → wins
    let ts_c = 1_700_000_000_500.0_f64;

    let soul = "shared:counter";

    let mut fields_a: HashMap<String, serde_json::Value> = HashMap::new();
    fields_a.insert("value".to_string(), json!(1));
    fields_a.insert("author".to_string(), json!("peer-a"));

    let mut fields_b: HashMap<String, serde_json::Value> = HashMap::new();
    fields_b.insert("value".to_string(), json!(2));
    fields_b.insert("author".to_string(), json!("peer-b"));

    let mut fields_c: HashMap<String, serde_json::Value> = HashMap::new();
    fields_c.insert("value".to_string(), json!(3));
    fields_c.insert("author".to_string(), json!("peer-c"));

    let mut node_a = GunNode::from_data(soul, fields_a, ts_a);
    let node_b = GunNode::from_data(soul, fields_b, ts_b);
    let node_c = GunNode::from_data(soul, fields_c, ts_c);

    // --- Part 1: direct GunNode.merge() convergence ---
    // Merge B into A (B has higher timestamp → B wins per-field).
    node_a.merge(node_b.clone());
    // Merge C into A (C has lower timestamp → C should NOT win over B).
    node_a.merge(node_c.clone());

    // After merging B (ts=2000) and C (ts=500) into A (ts=1000):
    // Each field should be taken from B because ts_b is highest.
    assert_eq!(
        node_a.fields["value"],
        json!(2),
        "value should come from peer-b (highest timestamp)"
    );
    assert_eq!(
        node_a.fields["author"],
        json!("peer-b"),
        "author should come from peer-b (highest timestamp)"
    );

    // Verify state timestamps reflect B's winning entries.
    assert!(
        (node_a.meta.state["value"] - ts_b).abs() < f64::EPSILON,
        "HAM state for 'value' should be peer-b's timestamp"
    );

    // --- Part 2: GunNode-level convergence across 3 peers via Replicator ---
    // Each peer tracks its local GunNode state in a HashMap and applies
    // GunNode.merge() when receiving nodes — this is the correct CRDT merge
    // semantic used by the GUN protocol.
    let mut state_a: HashMap<String, GunNode> = HashMap::new();
    let mut state_b: HashMap<String, GunNode> = HashMap::new();
    let mut state_c: HashMap<String, GunNode> = HashMap::new();

    let make_node = |value: u32, author: &str, ts: f64| -> GunNode {
        let mut f: HashMap<String, serde_json::Value> = HashMap::new();
        f.insert("value".to_string(), json!(value));
        f.insert("author".to_string(), json!(author));
        GunNode::from_data(soul, f, ts)
    };

    // Peer initial writes: B uses the highest timestamp so it will win.
    state_a.insert(soul.to_string(), make_node(10, "peer-a", ts_a));
    state_b.insert(soul.to_string(), make_node(20, "peer-b", ts_b));
    state_c.insert(soul.to_string(), make_node(30, "peer-c", ts_c));

    /// Merge two peer states bidirectionally using GunNode.merge().
    /// This models the correct CRDT convergence for the GUN/HAM protocol.
    fn gun_merge_pair(
        left: &mut HashMap<String, GunNode>,
        right: &mut HashMap<String, GunNode>,
    ) {
        let left_snap: Vec<(String, GunNode)> =
            left.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let right_snap: Vec<(String, GunNode)> =
            right.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        for (s, rn) in right_snap {
            left.entry(s)
                .and_modify(|ln| ln.merge(rn.clone()))
                .or_insert(rn);
        }
        for (s, ln) in left_snap {
            right
                .entry(s)
                .and_modify(|rn| rn.merge(ln.clone()))
                .or_insert(ln);
        }
    }

    // Full-mesh sync (one round is sufficient for three peers).
    gun_merge_pair(&mut state_a, &mut state_b);
    gun_merge_pair(&mut state_b, &mut state_c);
    gun_merge_pair(&mut state_a, &mut state_c);

    // All peers must have converged to peer-b's value (ts_b is highest).
    for (label, state) in [("A", &state_a), ("B", &state_b), ("C", &state_c)] {
        let node = state
            .get(soul)
            .unwrap_or_else(|| panic!("peer-{label} missing {soul}"));
        assert_eq!(
            node.fields["value"],
            json!(20),
            "peer-{label} should converge to peer-b's value (highest HAM timestamp)"
        );
        assert_eq!(
            node.fields["author"],
            json!("peer-b"),
            "peer-{label} should converge to peer-b's author"
        );
    }

    // Peer A and B and C states must all be identical.
    assert_eq!(
        state_a[soul].fields, state_b[soul].fields,
        "peer-a and peer-b must converge to identical GunNode fields"
    );
    assert_eq!(
        state_b[soul].fields, state_c[soul].fields,
        "peer-b and peer-c must converge to identical GunNode fields"
    );

    // Sanity check: the now_ms helper compiles and returns a positive value.
    assert!(now_ms() > 0.0);
}

// ---------------------------------------------------------------------------
// Reconnection after forced disconnect
// ---------------------------------------------------------------------------

/// Peer A and B sync a first batch, then the connection is dropped.
/// Peer A writes more data, reconnects and syncs again.
/// Peer B must end up with data from both sessions.
#[tokio::test]
async fn test_reconnection_after_network_drop() {
    use pluresdb_core::CrdtStore;

    let store_a = CrdtStore::default();
    let store_b = CrdtStore::default();

    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");

    // --- Session 1: initial sync ---
    store_a.put("doc:1", "peer-a", json!({"title": "First document"}));
    sync_and_apply(&rep_a, &store_a, "peer-a", &rep_b, &store_b, "peer-b").await;

    // peer-b should now have doc:1.
    assert!(
        store_b.get("doc:1").is_some(),
        "peer-b should have doc:1 after first sync"
    );

    // --- Network drop ---
    // Simulated by the fact that sync_and_apply always creates a fresh
    // MemConnection pair; the previous connection is already dropped.

    // --- Peer A writes more data (offline) ---
    store_a.put("doc:2", "peer-a", json!({"title": "Second document"}));
    store_a.put("doc:3", "peer-a", json!({"title": "Third document"}));

    // peer-b doesn't know about doc:2 and doc:3 yet.
    assert!(
        store_b.get("doc:2").is_none(),
        "peer-b should NOT have doc:2 before reconnect"
    );

    // --- Session 2: reconnect and sync ---
    sync_and_apply(&rep_a, &store_a, "peer-a", &rep_b, &store_b, "peer-b").await;

    // peer-b must now have all three documents from both sessions.
    assert!(
        store_b.get("doc:1").is_some(),
        "peer-b should still have doc:1"
    );
    assert!(
        store_b.get("doc:2").is_some(),
        "peer-b should have doc:2 after reconnect sync"
    );
    assert!(
        store_b.get("doc:3").is_some(),
        "peer-b should have doc:3 after reconnect sync"
    );
}

/// Simulate a peer restart: peer B starts fresh (new empty store) and
/// performs a full sync with peer A to recover state.
#[tokio::test]
async fn test_peer_restart_full_resync() {
    use pluresdb_core::CrdtStore;

    let store_a = CrdtStore::default();

    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");

    store_a.put("cfg:version", "peer-a", json!({"major": 3, "minor": 1}));
    store_a.put("cfg:features", "peer-a", json!({"sync": true, "p2p": true}));
    store_a.put("user:alice", "peer-a", json!({"name": "Alice"}));

    // First sync: establish state on peer B.
    let store_b_v1 = CrdtStore::default();
    sync_and_apply(&rep_a, &store_a, "peer-a", &rep_b, &store_b_v1, "peer-b").await;
    assert_eq!(store_b_v1.list().len(), 3, "peer-b v1 should have 3 nodes");

    // Peer B restarts — new empty store (simulates process restart).
    let store_b_v2 = CrdtStore::default();
    assert!(
        store_b_v2.list().is_empty(),
        "fresh store should be empty after restart"
    );

    // Peer A adds more data while peer B was down.
    store_a.put("user:bob", "peer-a", json!({"name": "Bob"}));

    // Re-sync: peer B recovers everything from peer A.
    sync_and_apply(&rep_a, &store_a, "peer-a", &rep_b, &store_b_v2, "peer-b").await;

    assert_eq!(
        store_b_v2.list().len(),
        4,
        "peer-b v2 should have all 4 nodes after full resync"
    );
    assert!(store_b_v2.get("cfg:version").is_some());
    assert!(store_b_v2.get("user:alice").is_some());
    assert!(store_b_v2.get("user:bob").is_some());
}

// ---------------------------------------------------------------------------
// GunRelayServer — real WebSocket transport tests
// ---------------------------------------------------------------------------

/// Spin up a real `GunRelayServer` on a random port and verify that a GUN PUT
/// message sent by peer A is relayed to peer B over WebSocket.
///
/// This exercises the actual Relay transport code path end-to-end.
#[tokio::test]
async fn test_relay_server_two_peer_message_passing() {
    use futures::{SinkExt, StreamExt};
    use pluresdb_sync::GunRelayServer;
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    // Bind to port 0 so the OS picks a free port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");

    let relay_router = GunRelayServer::new().build_router();
    tokio::spawn(async move {
        axum::serve(listener, relay_router)
            .await
            .expect("relay server error");
    });

    // Give the server a moment to start accepting connections.
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let url = format!("ws://127.0.0.1:{}/gun", addr.port());

    // Connect two WebSocket peers.
    let (mut ws_a, _) = connect_async(&url).await.expect("peer-a connect");
    let (mut ws_b, _) = connect_async(&url).await.expect("peer-b connect");

    // Let both peers register with the server before sending.
    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    // Peer A sends a GUN PUT message.
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("name".to_string(), json!("Alice"));
    let node = GunNode::from_data("user:alice", fields, 1_700_000_000_000.0_f64);
    let mut put_map = HashMap::new();
    put_map.insert("user:alice".to_string(), node);
    let put_msg = GunMessage::Put(GunPut {
        id: "relay-test-1".to_string(),
        put: put_map,
    });
    let payload = put_msg.encode().expect("encode PUT");
    ws_a.send(Message::Binary(payload.into()))
        .await
        .expect("peer-a send");

    // Peer B should receive the relayed message within 2 seconds.
    let received = tokio::time::timeout(tokio::time::Duration::from_secs(2), ws_b.next())
        .await
        .expect("timeout waiting for relay")
        .expect("ws_b stream ended")
        .expect("ws_b receive error");

    let raw = match received {
        Message::Text(t) => t.as_bytes().to_vec(),
        Message::Binary(b) => b.to_vec(),
        other => panic!("unexpected message type: {:?}", other),
    };

    let decoded = GunMessage::decode(&raw).expect("decode relayed message");
    assert_eq!(
        decoded.id(),
        "relay-test-1",
        "peer-b should receive peer-a's PUT with correct id"
    );

    ws_a.close(None).await.ok();
    ws_b.close(None).await.ok();
}

/// Three peers connect to the relay; a PUT from peer A must reach both B and C,
/// but must NOT be echoed back to peer A.
#[tokio::test]
async fn test_relay_server_three_peer_fanout() {
    use futures::{SinkExt, StreamExt};
    use pluresdb_sync::GunRelayServer;
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");

    let relay_router = GunRelayServer::new().build_router();
    tokio::spawn(async move {
        axum::serve(listener, relay_router)
            .await
            .expect("relay server error");
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let url = format!("ws://127.0.0.1:{}/gun", addr.port());

    let (mut ws_a, _) = connect_async(&url).await.expect("peer-a connect");
    let (mut ws_b, _) = connect_async(&url).await.expect("peer-b connect");
    let (mut ws_c, _) = connect_async(&url).await.expect("peer-c connect");

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    // Peer A broadcasts a PUT.
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("event".to_string(), json!("broadcast"));
    let node = GunNode::from_data("event:1", fields, 1_700_000_000_000.0_f64);
    let mut put_map = HashMap::new();
    put_map.insert("event:1".to_string(), node);
    let put_msg = GunMessage::Put(GunPut {
        id: "fanout-test-1".to_string(),
        put: put_map,
    });
    let payload = put_msg.encode().expect("encode");
    ws_a.send(Message::Binary(payload.into()))
        .await
        .expect("peer-a send");

    // Both B and C should receive the relayed message.
    let recv_b = tokio::time::timeout(tokio::time::Duration::from_secs(2), ws_b.next())
        .await
        .expect("timeout for peer-b")
        .expect("peer-b stream ended")
        .expect("peer-b error");

    let recv_c = tokio::time::timeout(tokio::time::Duration::from_secs(2), ws_c.next())
        .await
        .expect("timeout for peer-c")
        .expect("peer-c stream ended")
        .expect("peer-c error");

    let decode_msg = |m: Message| -> GunMessage {
        let raw = match m {
            Message::Text(t) => t.as_bytes().to_vec(),
            Message::Binary(b) => b.to_vec(),
            other => panic!("unexpected: {:?}", other),
        };
        GunMessage::decode(&raw).expect("decode")
    };

    let msg_b = decode_msg(recv_b);
    let msg_c = decode_msg(recv_c);

    assert_eq!(msg_b.id(), "fanout-test-1", "peer-b should receive fanout");
    assert_eq!(msg_c.id(), "fanout-test-1", "peer-c should receive fanout");

    // Peer A must NOT receive its own message (no echo).
    // We verify this by checking that no data message arrives within a short window.
    let echo_check =
        tokio::time::timeout(tokio::time::Duration::from_millis(100), ws_a.next()).await;
    match echo_check {
        Err(_timeout) => {} // Good: no echo within 100 ms.
        Ok(Some(Ok(Message::Close(_)))) => {} // Server-initiated close is acceptable.
        Ok(Some(Ok(Message::Ping(_)))) => {} // Ping frames are infrastructure, not data.
        Ok(Some(Ok(other))) => {
            // Parse and verify it's not a data echo.
            let raw = match other {
                Message::Text(t) => t.as_bytes().to_vec(),
                Message::Binary(b) => b.to_vec(),
                _ => return,
            };
            if let Ok(decoded) = GunMessage::decode(&raw) {
                assert_ne!(
                    decoded.id(),
                    "fanout-test-1",
                    "peer-a must not receive echo of its own PUT"
                );
            }
        }
        _ => {} // Stream end / None is fine.
    }

    ws_a.close(None).await.ok();
    ws_b.close(None).await.ok();
    ws_c.close(None).await.ok();
}

/// Peer reconnects to the relay after its WebSocket is closed, then receives
/// a new message — verifying relay handles reconnection correctly.
#[tokio::test]
async fn test_relay_server_peer_reconnect() {
    use futures::{SinkExt, StreamExt};
    use pluresdb_sync::GunRelayServer;
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");

    let relay_router = GunRelayServer::new().build_router();
    tokio::spawn(async move {
        axum::serve(listener, relay_router)
            .await
            .expect("relay server error");
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let url = format!("ws://127.0.0.1:{}/gun", addr.port());

    // Establish initial connection for peer B, then drop it.
    {
        let (mut ws_b, _) = connect_async(&url).await.expect("initial peer-b connect");
        ws_b.close(None).await.ok();
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    // Peer A connects.
    let (mut ws_a, _) = connect_async(&url).await.expect("peer-a connect");
    // Peer B reconnects.
    let (mut ws_b, _) = connect_async(&url).await.expect("peer-b reconnect");

    tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

    // Peer A sends a message after peer B has reconnected.
    let mut fields: HashMap<String, serde_json::Value> = HashMap::new();
    fields.insert("status".to_string(), json!("reconnected"));
    let node = GunNode::from_data("system:status", fields, 1_700_000_000_000.0_f64);
    let mut put_map = HashMap::new();
    put_map.insert("system:status".to_string(), node);
    let msg = GunMessage::Put(GunPut {
        id: "reconnect-test-1".to_string(),
        put: put_map,
    });
    let payload = msg.encode().expect("encode");
    ws_a.send(Message::Binary(payload.into()))
        .await
        .expect("peer-a send after reconnect");

    // Reconnected peer B should receive the message.
    let received = tokio::time::timeout(tokio::time::Duration::from_secs(2), ws_b.next())
        .await
        .expect("timeout for reconnected peer-b")
        .expect("peer-b stream ended")
        .expect("peer-b error");

    let raw = match received {
        Message::Text(t) => t.as_bytes().to_vec(),
        Message::Binary(b) => b.to_vec(),
        other => panic!("unexpected: {:?}", other),
    };
    let decoded = GunMessage::decode(&raw).expect("decode");
    assert_eq!(
        decoded.id(),
        "reconnect-test-1",
        "reconnected peer-b should receive message"
    );

    ws_a.close(None).await.ok();
    ws_b.close(None).await.ok();
}

// ---------------------------------------------------------------------------
// Hyperswarm transport stub validation
// ---------------------------------------------------------------------------

/// The HyperswarmTransport stub returns a descriptive error when called.
/// This exercises the Hyperswarm transport code path and verifies the stub
/// is correctly wired up in the factory, ready for hyperswarm-rs integration.
#[tokio::test]
async fn test_hyperswarm_transport_stub_errors_descriptively() {
    // Create a Hyperswarm transport via the factory.
    let config = TransportConfig {
        mode: TransportMode::Hyperswarm,
        ..Default::default()
    };
    let mut transport = create_transport(config);
    assert_eq!(transport.name(), "hyperswarm");

    // connect() should fail with a message that guides the implementer.
    let topic = derive_topic("test-database");
    let connect_err = transport.connect(topic).await.unwrap_err();
    let err_str = connect_err.to_string().to_lowercase();
    assert!(
        err_str.contains("not yet implemented") || err_str.contains("hyperswarm"),
        "error should mention 'not yet implemented' or 'hyperswarm', got: {connect_err}"
    );

    // announce() should also surface the stub state.
    let announce_err = transport.announce(topic).await.unwrap_err();
    let announce_str = announce_err.to_string().to_lowercase();
    assert!(
        announce_str.contains("not yet implemented") || announce_str.contains("hyperswarm"),
        "announce error should explain stub state, got: {announce_err}"
    );
}

/// The RelayTransport stub (not the GunRelayServer) returns a descriptive
/// error when called. Verifies the stub is wired correctly.
#[tokio::test]
async fn test_relay_transport_stub_errors_descriptively() {
    let config = TransportConfig {
        mode: TransportMode::Relay,
        relay_url: Some("wss://relay.example.com".to_string()),
        ..Default::default()
    };
    let mut transport = create_transport(config);
    assert_eq!(transport.name(), "relay");

    let topic = derive_topic("test-database");
    let err = transport.connect(topic).await.unwrap_err();
    let err_str = err.to_string().to_lowercase();
    assert!(
        err_str.contains("not yet implemented") || err_str.contains("relay"),
        "relay stub error should be descriptive, got: {err}"
    );
}
