//! P2P sync integration tests: multi-peer harness and relay transport.
//!
//! # Test categories
//!
//! ## 1. Multi-peer MemConnection harness (Hyperswarm-topology simulation)
//!
//! Since `hyperswarm-rs` is not yet integrated, these tests exercise the
//! GUN-protocol replication logic over in-process [`MemConnection`] pairs
//! arranged in the same mesh topology that Hyperswarm DHT would create.
//! When `hyperswarm-rs` is integrated these harness tests provide the baseline
//! behaviour the real transport must match.
//!
//! Covered scenarios:
//! - Three-peer gossip convergence (distinct data sets)
//! - Three-peer concurrent writes with CRDT merge convergence
//! - Reconnection: peer drops connection and rejoins with updated state
//!
//! ## 2. Split-brain partition + reconnect tests
//!
//! Deterministic simulations of network partitions where peers are isolated,
//! both sides continue writing, and then the partition heals.  These tests
//! verify the conflict-resolution policy documented in
//! `docs/CONFLICT_RESOLUTION.md`.
//!
//! Covered scenarios:
//! - Two isolated peers with concurrent same-field writes → LWW convergence
//! - Per-field independent convergence with mixed timestamps
//! - Three-peer two-partition and full mesh reconnect
//!
//! ## 3. Relay transport tests (real `GunRelayServer` over WebSocket)
//!
//! These tests start a [`GunRelayServer`] on an ephemeral port and connect
//! real WebSocket clients via `tokio-tungstenite`, exercising the full relay
//! message-routing code path end-to-end.
//!
//! Covered scenarios:
//! - Two-peer message exchange through relay
//! - Three-peer fanout through relay (≥3 peer convergence)
//! - Peer disconnects and reconnects — new messages are received after rejoin

use futures::{SinkExt, StreamExt};
use pluresdb_sync::{Connection, GunMessage, GunNode, GunRelayServer, MemConnection, Replicator};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

// ============================================================================
// Shared helpers
// ============================================================================

/// Merge received `(soul, GunNode)` pairs into a `HashMap<soul, GunNode>`
/// using per-field last-write-wins CRDT semantics.
fn apply_received(store: &mut HashMap<String, GunNode>, received: Vec<(String, GunNode)>) {
    for (soul, incoming) in received {
        store
            .entry(soul)
            .and_modify(|existing| existing.merge(incoming.clone()))
            .or_insert(incoming);
    }
}

/// Encode every `(soul, GunNode)` entry in `store` into GUN wire bytes using
/// [`Replicator::encode_gun_node`], preserving HAM timestamps exactly.
///
/// This is the correct encoding to use in transport-layer CRDT tests because
/// [`Replicator::encode_put`] would overwrite the per-field timestamps with
/// the current wall-clock time, destroying carefully controlled orderings.
fn encode_store(store: &HashMap<String, GunNode>, rep: &Replicator) -> Vec<Vec<u8>> {
    store
        .iter()
        .map(|(soul, node)| {
            rep.encode_gun_node(soul, node.clone())
                .unwrap_or_else(|e| panic!("encode_gun_node failed for soul '{soul}': {e}"))
        })
        .collect()
}

/// Bind a TCP listener on a random OS-assigned port and return it with its address.
async fn bind_ephemeral() -> (tokio::net::TcpListener, std::net::SocketAddr) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr");
    (listener, addr)
}

/// Start a [`GunRelayServer`] in the background on `listener` and return the
/// bound address so tests can connect WebSocket clients.
///
/// The server task runs for the lifetime of the test (dropped when the test
/// function returns and the tokio runtime shuts down).
async fn start_relay(listener: tokio::net::TcpListener, addr: std::net::SocketAddr) -> String {
    let router = GunRelayServer::new().build_router();
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("relay server error");
    });
    // Give the async task a moment to enter the accept loop.
    tokio::time::sleep(Duration::from_millis(20)).await;
    format!("ws://{}/gun", addr)
}

/// Receive the next non-ping GUN wire message from a WebSocket stream,
/// returning raw bytes on success or an error if the stream times out.
async fn recv_gun_bytes<S>(stream: &mut S) -> anyhow::Result<Vec<u8>>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        let msg = timeout(Duration::from_secs(5), stream.next())
            .await
            .map_err(|_| anyhow::anyhow!("timed out waiting for WebSocket message"))?
            .ok_or_else(|| anyhow::anyhow!("WebSocket stream ended unexpectedly"))?
            .map_err(|e| anyhow::anyhow!("WebSocket error: {e}"))?;
        let raw = match msg {
            Message::Text(utf8) => utf8.as_bytes().to_vec(),
            Message::Binary(data) => data.to_vec(),
            // Ignore ping / pong / close control frames.
            _ => continue,
        };
        return Ok(raw);
    }
}

// ============================================================================
// Multi-peer MemConnection harness (simulates Hyperswarm mesh topology)
// ============================================================================

/// Three peers write distinct graph nodes, then gossip through a full-mesh
/// sync cycle (A↔B, B↔C, A↔C).
///
/// After the three pairwise rounds, every peer's local store must contain all
/// three souls — validating ≥3-peer convergence over the GUN replication
/// protocol using the same mesh topology Hyperswarm would create.
#[tokio::test]
async fn test_three_peer_gossip_convergence() {
    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");
    let rep_c = Replicator::new("peer-c");

    // Each peer starts with a single unique node.
    let mut state_a: HashMap<String, serde_json::Value> = HashMap::new();
    let mut state_b: HashMap<String, serde_json::Value> = HashMap::new();
    let mut state_c: HashMap<String, serde_json::Value> = HashMap::new();

    state_a.insert(
        "user:alice".to_string(),
        json!({"name": "Alice", "role": "admin"}),
    );
    state_b.insert(
        "user:bob".to_string(),
        json!({"name": "Bob", "role": "member"}),
    );
    state_c.insert(
        "user:charlie".to_string(),
        json!({"name": "Charlie", "role": "viewer"}),
    );

    let to_vec = |m: &HashMap<String, serde_json::Value>| -> Vec<(String, serde_json::Value)> {
        m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    };

    // Merge received GunNode fields into the state map.
    let merge_into = |state: &mut HashMap<String, serde_json::Value>,
                      received: Vec<(String, GunNode)>| {
        for (soul, gun_node) in received {
            state.insert(
                soul,
                serde_json::Value::Object(gun_node.fields.into_iter().collect()),
            );
        }
    };

    // --- Round 1: A <-> B ---
    let nodes_a = to_vec(&state_a);
    let nodes_b = to_vec(&state_b);
    let (mut conn_ab, mut conn_ba) = MemConnection::pair("peer-a", "peer-b");
    let (recv_ab, recv_ba) = tokio::join!(
        rep_a.sync(&mut conn_ab, &nodes_a),
        rep_b.sync(&mut conn_ba, &nodes_b),
    );
    // A learns B's nodes; B learns A's nodes.
    merge_into(&mut state_a, recv_ab.unwrap());
    merge_into(&mut state_b, recv_ba.unwrap());
    // After round 1: state_a = {alice, bob}, state_b = {alice, bob}

    // --- Round 2: B <-> C (B carries alice + bob into C) ---
    let nodes_b2 = to_vec(&state_b);
    let nodes_c = to_vec(&state_c);
    let (mut conn_bc, mut conn_cb) = MemConnection::pair("peer-b", "peer-c");
    let (recv_bc, recv_cb) = tokio::join!(
        rep_b.sync(&mut conn_bc, &nodes_b2),
        rep_c.sync(&mut conn_cb, &nodes_c),
    );
    merge_into(&mut state_b, recv_bc.unwrap());
    merge_into(&mut state_c, recv_cb.unwrap());
    // After round 2: state_c = {alice, bob, charlie}

    // --- Round 3: A <-> C (closes the mesh; A picks up charlie) ---
    let nodes_a3 = to_vec(&state_a);
    let nodes_c3 = to_vec(&state_c);
    let (mut conn_ac, mut conn_ca) = MemConnection::pair("peer-a", "peer-c");
    let (recv_ac, recv_ca) = tokio::join!(
        rep_a.sync(&mut conn_ac, &nodes_a3),
        rep_c.sync(&mut conn_ca, &nodes_c3),
    );
    merge_into(&mut state_a, recv_ac.unwrap());
    merge_into(&mut state_c, recv_ca.unwrap());

    // --- Verify convergence ---
    let check = |state: &HashMap<String, serde_json::Value>, peer: &str| {
        assert!(
            state.contains_key("user:alice"),
            "{peer}: missing user:alice"
        );
        assert!(state.contains_key("user:bob"), "{peer}: missing user:bob");
        assert!(
            state.contains_key("user:charlie"),
            "{peer}: missing user:charlie"
        );
    };
    check(&state_a, "peer-a");
    check(&state_b, "peer-b");
    check(&state_c, "peer-c");
}

/// Three peers concurrently write to the **same soul and field** with
/// explicitly controlled HAM timestamps.  After full-mesh gossip, every peer
/// must converge to the value written with the highest timestamp.
///
/// This validates the per-field last-write-wins CRDT semantics that GunNode
/// implements (the "Hypothetical Amnesia Machine" merge rules).
#[tokio::test]
async fn test_three_peer_crdt_concurrent_writes_convergence() {
    // Peer C writes at t=3000 (latest → wins)
    // Peer B writes at t=2000
    // Peer A writes at t=1000 (earliest → loses to B and C)
    let mut fields_a = HashMap::new();
    fields_a.insert("theme".to_string(), json!("light"));
    fields_a.insert("source".to_string(), json!("A"));
    let node_a = GunNode::from_data("setting:theme", fields_a, 1_000.0);

    let mut fields_b = HashMap::new();
    fields_b.insert("theme".to_string(), json!("dark"));
    fields_b.insert("source".to_string(), json!("B"));
    let node_b = GunNode::from_data("setting:theme", fields_b, 2_000.0);

    let mut fields_c = HashMap::new();
    fields_c.insert("theme".to_string(), json!("high-contrast"));
    fields_c.insert("source".to_string(), json!("C"));
    let node_c = GunNode::from_data("setting:theme", fields_c, 3_000.0); // wins

    // --- CRDT merge: apply all three nodes in every possible order ---
    // The outcome must be the same regardless of merge order.
    let merge_all = |base: GunNode, others: [GunNode; 2]| -> GunNode {
        let mut result = base;
        for other in others {
            result.merge(other);
        }
        result
    };

    let result_a = merge_all(node_a.clone(), [node_b.clone(), node_c.clone()]);
    let result_b = merge_all(node_b.clone(), [node_c.clone(), node_a.clone()]);
    let result_c = merge_all(node_c.clone(), [node_a.clone(), node_b.clone()]);

    // All must converge to C's values (highest timestamp).
    for (result, label) in [(&result_a, "A"), (&result_b, "B"), (&result_c, "C")] {
        assert_eq!(
            result.fields["theme"],
            json!("high-contrast"),
            "peer {label}: theme did not converge to C's value"
        );
        assert_eq!(
            result.fields["source"],
            json!("C"),
            "peer {label}: source did not converge to C's value"
        );
    }
}

/// Three peers exchange nodes with conflicting values via the GUN transport
/// layer (MemConnection + Replicator).  Each peer encodes its GunNode with an
/// explicit timestamp, sends it over a pairwise connection, and merges what it
/// receives — validating that the transport correctly preserves HAM state and
/// that convergence is reached after a full gossip round.
#[tokio::test]
async fn test_three_peer_crdt_convergence_via_transport() {
    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");
    let rep_c = Replicator::new("peer-c");

    // Each peer builds a GunNode with an explicit timestamp.
    let mut fields_a = HashMap::new();
    fields_a.insert("score".to_string(), json!(10));
    let node_a = GunNode::from_data("game:score", fields_a, 100.0);

    let mut fields_b = HashMap::new();
    fields_b.insert("score".to_string(), json!(20));
    let node_b = GunNode::from_data("game:score", fields_b, 200.0);

    let mut fields_c = HashMap::new();
    fields_c.insert("score".to_string(), json!(30)); // C wins
    let node_c = GunNode::from_data("game:score", fields_c, 300.0);

    // Each peer stores its initial node.
    let mut store_a: HashMap<String, GunNode> = HashMap::new();
    let mut store_b: HashMap<String, GunNode> = HashMap::new();
    let mut store_c: HashMap<String, GunNode> = HashMap::new();
    store_a.insert("game:score".to_string(), node_a.clone());
    store_b.insert("game:score".to_string(), node_b.clone());
    store_c.insert("game:score".to_string(), node_c.clone());

    // For transport-layer CRDT tests we encode each GunNode with its explicit
    // HAM timestamp using encode_gun_node (rather than encode_put, which would
    // overwrite timestamps with the current wall-clock time).
    //
    // Each side concurrently sends its encoded wire bytes, closes the write
    // direction (signalling EOF to the remote receiver), and then reads until
    // it gets EOF from the other side — the same pattern that sync() uses
    // internally, but with pre-built GunNode payloads.
    //
    // Critically, ALL rounds use encode_gun_node on the *current* merged
    // GunNode so that HAM timestamps are preserved end-to-end through the
    // gossip chain.  Using sync() / encode_put would re-stamp with now_ms()
    // and destroy the carefully controlled timestamp ordering.

    // Send and receive concurrently over a single MemConnection pair,
    // preserving HAM timestamps on both sides.
    macro_rules! ham_sync {
        ($rep_l:expr, $conn_l:expr, $msgs_l:expr,
         $rep_r:expr, $conn_r:expr, $msgs_r:expr) => {{
            let push_recv_l = async {
                for msg in $msgs_l {
                    $conn_l.send(&msg).await.unwrap();
                }
                $conn_l.close().await.unwrap();
                $rep_l.receive_all(&mut $conn_l).await.unwrap()
            };
            let push_recv_r = async {
                for msg in $msgs_r {
                    $conn_r.send(&msg).await.unwrap();
                }
                $conn_r.close().await.unwrap();
                $rep_r.receive_all(&mut $conn_r).await.unwrap()
            };
            tokio::join!(push_recv_l, push_recv_r)
        }};
    }

    // --- Round 1: A <-> B ---
    {
        let (mut c_ab, mut c_ba) = MemConnection::pair("a", "b");
        let msgs_a = encode_store(&store_a, &rep_a);
        let msgs_b = encode_store(&store_b, &rep_b);
        let (from_b, from_a) = ham_sync!(rep_a, c_ab, msgs_a, rep_b, c_ba, msgs_b);
        apply_received(&mut store_a, from_b); // A merges B's node_b (ts=200 > ts=100)
        apply_received(&mut store_b, from_a); // B merges A's node_a (ts=100 < ts=200, B keeps 200)
    }
    // After round 1:
    //   store_a: game:score { score=20 @ ts=200 }  (B's value wins)
    //   store_b: game:score { score=20 @ ts=200 }  (B's value, A's ts=100 loses)

    // --- Round 2: B <-> C (B propagates current merged state to C) ---
    {
        let (mut c_bc, mut c_cb) = MemConnection::pair("b", "c");
        let msgs_b = encode_store(&store_b, &rep_b);
        let msgs_c = encode_store(&store_c, &rep_c);
        let (from_c, from_b) = ham_sync!(rep_b, c_bc, msgs_b, rep_c, c_cb, msgs_c);
        apply_received(&mut store_b, from_c); // B merges C's node_c (ts=300 > ts=200, C wins)
        apply_received(&mut store_c, from_b); // C merges B's merged node (ts=200 < ts=300, C keeps 300)
    }
    // After round 2:
    //   store_b: game:score { score=30 @ ts=300 }  (C wins)
    //   store_c: game:score { score=30 @ ts=300 }  (C's own value)

    // --- Round 3: A <-> C (A learns C's value via the mesh) ---
    {
        let (mut c_ac, mut c_ca) = MemConnection::pair("a", "c");
        let msgs_a = encode_store(&store_a, &rep_a);
        let msgs_c = encode_store(&store_c, &rep_c);
        let (from_c, from_a) = ham_sync!(rep_a, c_ac, msgs_a, rep_c, c_ca, msgs_c);
        apply_received(&mut store_a, from_c); // A merges C's node (ts=300 > ts=200, C wins)
        apply_received(&mut store_c, from_a); // C merges A's merged node (ts=200 < ts=300, C keeps)
    }
    // After round 3: all three peers should have score=30 @ ts=300

    // --- Verify all three peers converged to C's score (highest timestamp) ---
    for (store, peer) in [(&store_a, "A"), (&store_b, "B"), (&store_c, "C")] {
        let node = store
            .get("game:score")
            .unwrap_or_else(|| panic!("peer {peer}: missing game:score"));
        assert_eq!(
            node.fields["score"],
            json!(30),
            "peer {peer}: score did not converge to C's value (30)"
        );
    }
}

/// Simulate peer disconnection and reconnection.
///
/// 1. Peer A and peer B sync initial data (A's `user:alice`).
/// 2. The connection is dropped (simulating a network interruption).
/// 3. Peer A writes additional data (`post:1`).
/// 4. A new connection is established and A sends its full updated state.
/// 5. Peer B must have both `user:alice` (from before disconnect) and
///    `post:1` (from the resync after reconnection).
#[tokio::test]
async fn test_reconnect_and_resync() {
    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");

    let mut state_a: HashMap<String, serde_json::Value> = HashMap::new();
    let mut state_b: HashMap<String, serde_json::Value> = HashMap::new();

    state_a.insert("user:alice".to_string(), json!({"name": "Alice"}));

    // --- Initial sync: A sends alice to B ---
    {
        let (mut conn_ab, mut conn_ba) = MemConnection::pair("peer-a", "peer-b");
        let nodes_a = state_a
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        let (from_b, from_a) = tokio::join!(
            rep_a.sync(&mut conn_ab, &nodes_a),
            rep_b.sync(&mut conn_ba, &[]),
        );
        // B receives alice; A receives nothing from B.
        for (soul, gun_node) in from_a.unwrap() {
            state_b.insert(
                soul,
                serde_json::Value::Object(gun_node.fields.into_iter().collect()),
            );
        }
        let _ = from_b; // A sent nothing new
    }
    assert!(
        state_b.contains_key("user:alice"),
        "B should have alice after initial sync"
    );

    // --- Simulate network drop: old connections are discarded automatically
    //     when they go out of scope above.  A continues writing new data. ---
    state_a.insert(
        "post:1".to_string(),
        json!({"title": "Post-disconnect write"}),
    );

    // --- Reconnect: new MemConnection pair ---
    {
        let (mut conn_ab2, mut conn_ba2) = MemConnection::pair("peer-a", "peer-b");
        let nodes_a2 = state_a
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();
        // A pushes full state (alice + post:1); B receives the diff.
        let (from_b2, from_a2) = tokio::join!(
            rep_a.sync(&mut conn_ab2, &nodes_a2),
            rep_b.sync(&mut conn_ba2, &[]),
        );
        for (soul, gun_node) in from_a2.unwrap() {
            state_b.insert(
                soul,
                serde_json::Value::Object(gun_node.fields.into_iter().collect()),
            );
        }
        let _ = from_b2;
    }

    // B now has both the pre-disconnect and post-reconnect data.
    assert!(
        state_b.contains_key("user:alice"),
        "B should retain alice after reconnect"
    );
    assert!(
        state_b.contains_key("post:1"),
        "B should receive post:1 after reconnect resync"
    );
}

// ============================================================================
// Split-brain partition + reconnect tests
// ============================================================================

/// Two peers are **fully isolated** (no network contact) during a write
/// window.  Both write to the **same field** of the same soul.  After the
/// partition heals (a new connection is established) the peers exchange their
/// diverged states.
///
/// Expected outcome per the LWW policy in `docs/CONFLICT_RESOLUTION.md`:
/// - The field value with the **higher HAM timestamp** wins on both peers.
/// - The peer whose write has the lower timestamp silently defers to the other.
/// - After convergence both stores are bit-for-bit identical.
#[tokio::test]
async fn test_split_brain_isolated_partitions_converge() {
    // --- Partition phase: no network between A and B ---
    // Both peers write to the same field "score" on soul "game:state".
    // A writes at ts=1000, B writes at ts=2000 → B's value must win.
    let mut fields_a = HashMap::new();
    fields_a.insert("score".to_string(), json!(50));
    let node_a = GunNode::from_data("game:state", fields_a, 1_000.0);

    let mut fields_b = HashMap::new();
    fields_b.insert("score".to_string(), json!(99));
    let node_b = GunNode::from_data("game:state", fields_b, 2_000.0);

    let mut store_a: HashMap<String, GunNode> = HashMap::new();
    let mut store_b: HashMap<String, GunNode> = HashMap::new();
    store_a.insert("game:state".to_string(), node_a);
    store_b.insert("game:state".to_string(), node_b);

    // --- Partition heals: A and B exchange their full stores ---
    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");

    let (mut c_ab, mut c_ba) = MemConnection::pair("a", "b");
    let msgs_a = encode_store(&store_a, &rep_a);
    let msgs_b = encode_store(&store_b, &rep_b);

    // Both peers send and receive concurrently (mirrors real reconnect behaviour).
    let (from_b, from_a) = {
        let push_recv_a = async {
            for msg in msgs_a {
                c_ab.send(&msg).await.unwrap();
            }
            c_ab.close().await.unwrap();
            rep_a.receive_all(&mut c_ab).await.unwrap()
        };
        let push_recv_b = async {
            for msg in msgs_b {
                c_ba.send(&msg).await.unwrap();
            }
            c_ba.close().await.unwrap();
            rep_b.receive_all(&mut c_ba).await.unwrap()
        };
        tokio::join!(push_recv_a, push_recv_b)
    };

    apply_received(&mut store_a, from_b);
    apply_received(&mut store_b, from_a);

    // --- Verify convergence ---
    // Both peers must hold B's value (score=99 @ ts=2000) because ts=2000 > ts=1000.
    for (store, peer) in [(&store_a, "A"), (&store_b, "B")] {
        let node = store
            .get("game:state")
            .unwrap_or_else(|| panic!("{peer}: missing game:state after partition heal"));
        assert_eq!(
            node.fields["score"],
            json!(99),
            "{peer}: split-brain: expected B's score (99 @ ts=2000) to win over A's score \
             (50 @ ts=1000), but got {}",
            node.fields["score"]
        );
    }

    // Verify the two stores are identical (bit-for-bit convergence).
    let score_a = &store_a["game:state"].fields["score"];
    let score_b = &store_b["game:state"].fields["score"];
    assert_eq!(
        score_a, score_b,
        "split-brain: stores diverged after heal — A has {score_a}, B has {score_b}"
    );
}

/// Two partitioned peers each write to **different fields** of the same soul
/// with **different timestamps**.
///
/// Expected outcome per the per-field LWW policy:
/// - Each field is resolved independently.
/// - A field written only by one peer is preserved regardless of timestamps.
/// - A field written by both peers resolves to the higher-timestamp value.
#[tokio::test]
async fn test_split_brain_per_field_independent_convergence() {
    // Peer A writes "status" at ts=100 and "label" at ts=100.
    // Peer B writes "status" at ts=200 (wins the conflict) and "count" at ts=50
    // (B is the only writer for "count", so it is preserved unconditionally).
    let mut fields_a = HashMap::new();
    fields_a.insert("status".to_string(), json!("online"));
    fields_a.insert("label".to_string(), json!("primary"));
    let node_a = GunNode::from_data("device:001", fields_a, 100.0);

    let mut fields_b = HashMap::new();
    fields_b.insert("status".to_string(), json!("offline")); // conflict — B wins (ts=200)
    fields_b.insert("count".to_string(), json!(7)); // only B writes this field
    let node_b = GunNode::from_data("device:001", fields_b, 200.0);

    // Merge both ways and verify both produce the same result.
    let mut merged_from_a_perspective = node_a.clone();
    merged_from_a_perspective.merge(node_b.clone());

    let mut merged_from_b_perspective = node_b.clone();
    merged_from_b_perspective.merge(node_a.clone());

    for (merged, label) in [
        (&merged_from_a_perspective, "A-perspective"),
        (&merged_from_b_perspective, "B-perspective"),
    ] {
        // "status": conflict field — B's ts=200 beats A's ts=100.
        assert_eq!(
            merged.fields["status"],
            json!("offline"),
            "{label}: status should be B's 'offline' (ts=200 > ts=100)"
        );

        // "label": only A wrote this field; it must survive the merge regardless.
        assert_eq!(
            merged.fields["label"],
            json!("primary"),
            "{label}: label (written only by A) should be preserved after merge"
        );

        // "count": only B wrote this field; it must survive regardless of A's ts.
        assert_eq!(
            merged.fields["count"],
            json!(7),
            "{label}: count (written only by B) should be preserved after merge"
        );
    }

    // The two merge perspectives must be identical (commutativity).
    assert_eq!(
        merged_from_a_perspective.fields, merged_from_b_perspective.fields,
        "split-brain per-field: merge is not commutative — stores diverged"
    );
}

/// Three peers are split into two partitions:
///   - Partition 1: peers A and B (can communicate with each other, not C)
///   - Partition 2: peer C alone
///
/// All three peers write to the same soul with different timestamps.
/// The partition heals in two reconnect events:
///   1. A ↔ C reconnect (C is now reachable from partition 1 via A).
///   2. B ↔ C reconnect (B catches up directly from C).
///
/// After full reconnect every peer must hold the value with the globally
/// highest timestamp, matching the policy in `docs/CONFLICT_RESOLUTION.md`.
#[tokio::test]
async fn test_split_brain_three_peer_partition_and_full_reconnect() {
    // --- Partition phase ---
    // Partition 1: A (ts=100) and B (ts=200) can sync with each other.
    // Partition 2: C (ts=300) is isolated.
    // C has the globally winning value.
    let mut fields_a = HashMap::new();
    fields_a.insert("level".to_string(), json!(1));
    let node_a = GunNode::from_data("player:x", fields_a, 100.0);

    let mut fields_b = HashMap::new();
    fields_b.insert("level".to_string(), json!(2));
    let node_b = GunNode::from_data("player:x", fields_b, 200.0);

    let mut fields_c = HashMap::new();
    fields_c.insert("level".to_string(), json!(3)); // highest timestamp — wins globally
    let node_c = GunNode::from_data("player:x", fields_c, 300.0);

    let mut store_a: HashMap<String, GunNode> = HashMap::new();
    let mut store_b: HashMap<String, GunNode> = HashMap::new();
    let mut store_c: HashMap<String, GunNode> = HashMap::new();
    store_a.insert("player:x".to_string(), node_a);
    store_b.insert("player:x".to_string(), node_b);
    store_c.insert("player:x".to_string(), node_c);

    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");
    let rep_c = Replicator::new("peer-c");

    // Macro reused from test_three_peer_crdt_convergence_via_transport.
    macro_rules! ham_sync {
        ($rep_l:expr, $conn_l:expr, $msgs_l:expr,
         $rep_r:expr, $conn_r:expr, $msgs_r:expr) => {{
            let push_recv_l = async {
                for msg in $msgs_l {
                    $conn_l.send(&msg).await.unwrap();
                }
                $conn_l.close().await.unwrap();
                $rep_l.receive_all(&mut $conn_l).await.unwrap()
            };
            let push_recv_r = async {
                for msg in $msgs_r {
                    $conn_r.send(&msg).await.unwrap();
                }
                $conn_r.close().await.unwrap();
                $rep_r.receive_all(&mut $conn_r).await.unwrap()
            };
            tokio::join!(push_recv_l, push_recv_r)
        }};
    }

    // --- Intra-partition sync: A <-> B (inside partition 1) ---
    {
        let (mut c_ab, mut c_ba) = MemConnection::pair("a", "b");
        let msgs_a = encode_store(&store_a, &rep_a);
        let msgs_b = encode_store(&store_b, &rep_b);
        let (from_b, from_a) = ham_sync!(rep_a, c_ab, msgs_a, rep_b, c_ba, msgs_b);
        apply_received(&mut store_a, from_b); // A learns level=2 @ ts=200 (B wins)
        apply_received(&mut store_b, from_a); // B keeps level=2 @ ts=200
    }
    // Partition 1 state: A and B both have level=2 @ ts=200.
    // Partition 2 state: C still has level=3 @ ts=300 (isolated).

    // --- Partition heals: A <-> C reconnect ---
    {
        let (mut c_ac, mut c_ca) = MemConnection::pair("a", "c");
        let msgs_a = encode_store(&store_a, &rep_a);
        let msgs_c = encode_store(&store_c, &rep_c);
        let (from_c, from_a) = ham_sync!(rep_a, c_ac, msgs_a, rep_c, c_ca, msgs_c);
        apply_received(&mut store_a, from_c); // A learns level=3 @ ts=300 (C wins)
        apply_received(&mut store_c, from_a); // C keeps level=3 @ ts=300
    }
    // A now has level=3 @ ts=300.  B still has level=2 @ ts=200.

    // --- Full reconnect: B <-> C (or B learns via A; we test B <-> C direct) ---
    {
        let (mut c_bc, mut c_cb) = MemConnection::pair("b", "c");
        let msgs_b = encode_store(&store_b, &rep_b);
        let msgs_c = encode_store(&store_c, &rep_c);
        let (from_c, from_b) = ham_sync!(rep_b, c_bc, msgs_b, rep_c, c_cb, msgs_c);
        apply_received(&mut store_b, from_c); // B learns level=3 @ ts=300 (C wins)
        apply_received(&mut store_c, from_b); // C keeps level=3 @ ts=300
    }

    // --- Verify full convergence ---
    // All three peers must converge to C's value (level=3 @ ts=300).
    for (store, peer) in [(&store_a, "A"), (&store_b, "B"), (&store_c, "C")] {
        let node = store
            .get("player:x")
            .unwrap_or_else(|| panic!("{peer}: missing player:x after full reconnect"));
        assert_eq!(
            node.fields["level"],
            json!(3),
            "{peer}: split-brain three-peer: expected C's level (3 @ ts=300) to win, \
             but got {}",
            node.fields["level"]
        );
    }

    // Cross-check: all three stores are identical for the contested field.
    let level_a = &store_a["player:x"].fields["level"];
    let level_b = &store_b["player:x"].fields["level"];
    let level_c = &store_c["player:x"].fields["level"];
    assert_eq!(
        level_a, level_b,
        "split-brain three-peer: A ({level_a}) and B ({level_b}) diverged after full reconnect"
    );
    assert_eq!(
        level_b, level_c,
        "split-brain three-peer: B ({level_b}) and C ({level_c}) diverged after full reconnect"
    );
}

// ============================================================================

/// Two peers exchange a GUN PUT message through a real `GunRelayServer`.
///
/// Peer B connects first (so it is subscribed before A sends), then A sends a
/// PUT for `user:alice`.  The relay must fan the message out to B, but **not**
/// echo it back to A.
#[tokio::test]
async fn test_relay_two_peer_exchange() {
    let (listener, addr) = bind_ephemeral().await;
    let url = start_relay(listener, addr).await;

    // Peer B subscribes first.
    let (ws_b, _) = connect_async(url.clone()).await.expect("peer-b connect");
    let (mut sink_b, mut stream_b) = ws_b.split();

    // Peer A connects and sends a GUN PUT message.
    let (ws_a, _) = connect_async(url.clone()).await.expect("peer-a connect");
    let (mut sink_a, _stream_a) = ws_a.split();

    // Build a GUN PUT message for user:alice.
    let rep_a = Replicator::new("peer-a");
    let put_bytes = rep_a
        .encode_put("user:alice", json!({"name": "Alice", "role": "admin"}))
        .unwrap();

    // A sends via WebSocket (binary frame — relay accepts both text and binary).
    sink_a
        .send(Message::Binary(put_bytes.clone().into()))
        .await
        .expect("peer-a send");

    // B must receive the fan-out.
    let raw = recv_gun_bytes(&mut stream_b)
        .await
        .expect("peer-b should receive message");

    let msg = GunMessage::decode(&raw).expect("valid GUN message");
    if let GunMessage::Put(put) = msg {
        assert!(
            put.put.contains_key("user:alice"),
            "fanout should carry user:alice node"
        );
        let node = &put.put["user:alice"];
        assert_eq!(node.fields["name"], json!("Alice"));
        assert_eq!(node.fields["role"], json!("admin"));
    } else {
        panic!("expected GunMessage::Put, got: {:?}", msg);
    }

    // Clean-up.
    let _ = sink_b.close().await;
    let _ = sink_a.close().await;
}

/// Three peers connect to a relay server. Each peer sends a PUT message and
/// the other two must receive it — validating ≥3-peer fanout convergence
/// through the relay transport.
#[tokio::test]
async fn test_relay_three_peer_convergence() {
    let (listener, addr) = bind_ephemeral().await;
    let url = start_relay(listener, addr).await;

    // Connect all three peers.
    let (ws_a, _) = connect_async(url.clone()).await.expect("peer-a connect");
    let (ws_b, _) = connect_async(url.clone()).await.expect("peer-b connect");
    let (ws_c, _) = connect_async(url.clone()).await.expect("peer-c connect");

    let (mut sink_a, mut stream_a) = ws_a.split();
    let (mut sink_b, mut stream_b) = ws_b.split();
    let (mut sink_c, mut stream_c) = ws_c.split();

    let rep_a = Replicator::new("peer-a");
    let rep_b = Replicator::new("peer-b");
    let rep_c = Replicator::new("peer-c");

    // --- A sends alice; B and C must receive it ---
    let put_a = rep_a
        .encode_put("user:alice", json!({"name": "Alice"}))
        .unwrap();
    sink_a.send(Message::Binary(put_a.into())).await.unwrap();

    let raw_b = recv_gun_bytes(&mut stream_b)
        .await
        .expect("B should receive A's message");
    let raw_c = recv_gun_bytes(&mut stream_c)
        .await
        .expect("C should receive A's message");

    for (raw, peer) in [(&raw_b, "B"), (&raw_c, "C")] {
        let msg = GunMessage::decode(raw).expect("valid GUN message");
        if let GunMessage::Put(put) = msg {
            assert!(
                put.put.contains_key("user:alice"),
                "peer {peer}: fanout missing user:alice"
            );
        } else {
            panic!("peer {peer}: expected Put, got {:?}", msg);
        }
    }

    // --- B sends bob; A and C must receive it ---
    let put_b = rep_b
        .encode_put("user:bob", json!({"name": "Bob"}))
        .unwrap();
    sink_b.send(Message::Binary(put_b.into())).await.unwrap();

    let raw_a = recv_gun_bytes(&mut stream_a)
        .await
        .expect("A should receive B's message");
    let raw_c2 = recv_gun_bytes(&mut stream_c)
        .await
        .expect("C should receive B's message");

    for (raw, peer) in [(&raw_a, "A"), (&raw_c2, "C")] {
        let msg = GunMessage::decode(raw).expect("valid GUN message");
        if let GunMessage::Put(put) = msg {
            assert!(
                put.put.contains_key("user:bob"),
                "peer {peer}: fanout missing user:bob"
            );
        } else {
            panic!("peer {peer}: expected Put, got {:?}", msg);
        }
    }

    // --- C sends charlie; A and B must receive it ---
    let put_c = rep_c
        .encode_put("user:charlie", json!({"name": "Charlie"}))
        .unwrap();
    sink_c.send(Message::Binary(put_c.into())).await.unwrap();

    let raw_a2 = recv_gun_bytes(&mut stream_a)
        .await
        .expect("A should receive C's message");
    let raw_b2 = recv_gun_bytes(&mut stream_b)
        .await
        .expect("B should receive C's message");

    for (raw, peer) in [(&raw_a2, "A"), (&raw_b2, "B")] {
        let msg = GunMessage::decode(raw).expect("valid GUN message");
        if let GunMessage::Put(put) = msg {
            assert!(
                put.put.contains_key("user:charlie"),
                "peer {peer}: fanout missing user:charlie"
            );
        } else {
            panic!("peer {peer}: expected Put, got {:?}", msg);
        }
    }

    // Clean-up.
    let _ = sink_a.close().await;
    let _ = sink_b.close().await;
    let _ = sink_c.close().await;
}

/// Peer B connects to the relay, then disconnects.  During the disconnection
/// window, peer A sends a message (which B misses — the relay is stateless and
/// does not buffer).  After B reconnects, A sends another message and B must
/// receive it, verifying that new messages flow to reconnected peers.
#[tokio::test]
async fn test_relay_peer_reconnection() {
    let (listener, addr) = bind_ephemeral().await;
    let url = start_relay(listener, addr).await;

    let rep_a = Replicator::new("peer-a");

    // --- Phase 1: both A and B connected; A sends, B receives ---
    let (ws_a, _) = connect_async(url.clone()).await.expect("A initial connect");
    let (mut sink_a, _stream_a) = ws_a.split();

    let (ws_b1, _) = connect_async(url.clone()).await.expect("B initial connect");
    let (mut sink_b1, mut stream_b1) = ws_b1.split();

    let put1 = rep_a
        .encode_put("msg:1", json!({"text": "before disconnect"}))
        .unwrap();
    sink_a.send(Message::Binary(put1.into())).await.unwrap();

    let raw1 = recv_gun_bytes(&mut stream_b1)
        .await
        .expect("B should receive msg:1 before disconnect");
    let msg1 = GunMessage::decode(&raw1).unwrap();
    assert!(
        matches!(&msg1, GunMessage::Put(p) if p.put.contains_key("msg:1")),
        "B should receive msg:1 before disconnect"
    );

    // --- Phase 2: B disconnects ---
    sink_b1.close().await.expect("B disconnect");
    drop(stream_b1);
    // Give the server time to process the Close frame and update its peer list.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // A sends a message while B is offline (relay is stateless; B will miss it).
    let put_missed = rep_a
        .encode_put("msg:missed", json!({"text": "while B was gone"}))
        .unwrap();
    sink_a
        .send(Message::Binary(put_missed.into()))
        .await
        .unwrap();

    // Wait for the relay to fully broadcast "msg:missed" BEFORE B2 subscribes.
    // On the single-thread tokio runtime used by #[tokio::test], the relay's
    // recv_task processes the bytes sent above only when the current task
    // yields.  The sleep below forces a yield and gives the relay enough time
    // to broadcast the message to current subscribers (only A, who skips echo).
    // Without this barrier B2 could accidentally receive "msg:missed" because
    // it subscribed to the broadcast channel before the relay had a chance to
    // process and fan out the message.
    tokio::time::sleep(Duration::from_millis(100)).await;

    // --- Phase 3: B reconnects and receives subsequent messages ---
    let (ws_b2, _) = connect_async(url.clone()).await.expect("B reconnect");
    let (mut sink_b2, mut stream_b2) = ws_b2.split();

    // Give the relay time to register B2's subscription on the broadcast
    // channel before A sends the next message.  The relay's handle_socket
    // coroutine must be scheduled, call subscribe(), and spawn its send_task —
    // all in a separate tokio task that runs during this sleep.
    tokio::time::sleep(Duration::from_millis(100)).await;

    let put2 = rep_a
        .encode_put("msg:2", json!({"text": "after reconnect"}))
        .unwrap();
    sink_a.send(Message::Binary(put2.into())).await.unwrap();

    let raw2 = recv_gun_bytes(&mut stream_b2)
        .await
        .unwrap_or_else(|e| panic!("B should receive msg:2 after reconnect: {e}"));
    let msg2 = GunMessage::decode(&raw2).unwrap();
    assert!(
        matches!(&msg2, GunMessage::Put(p) if p.put.contains_key("msg:2")),
        "B should receive msg:2 after reconnect; got: {:?}",
        msg2
    );

    // Clean-up.
    let _ = sink_a.close().await;
    let _ = sink_b2.close().await;
}

/// Verify that the relay does **not** echo messages back to the originating
/// peer (important for preventing feedback loops in sync protocols).
#[tokio::test]
async fn test_relay_no_echo_to_sender() {
    let (listener, addr) = bind_ephemeral().await;
    let url = start_relay(listener, addr).await;

    // Connect peer A and peer B.
    let (ws_a, _) = connect_async(url.clone()).await.expect("A connect");
    let (ws_b, _) = connect_async(url.clone()).await.expect("B connect");
    let (mut sink_a, mut stream_a) = ws_a.split();
    let (mut sink_b, mut stream_b) = ws_b.split();

    let rep_a = Replicator::new("peer-a");
    let put_a = rep_a
        .encode_put("user:alice", json!({"name": "Alice"}))
        .unwrap();

    // A sends a message.
    sink_a.send(Message::Binary(put_a.into())).await.unwrap();

    // B must receive it.
    recv_gun_bytes(&mut stream_b)
        .await
        .expect("B should receive A's message");

    // A must NOT receive an echo back.  We use a short timeout; if no message
    // arrives within 200 ms we consider the no-echo property satisfied.
    let echo_result = timeout(Duration::from_millis(200), recv_gun_bytes(&mut stream_a)).await;
    assert!(
        echo_result.is_err(),
        "relay must not echo the message back to the sender"
    );

    let _ = sink_a.close().await;
    let _ = sink_b.close().await;
}
