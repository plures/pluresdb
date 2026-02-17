//! Integration tests for PluresDB sync transports
//!
//! These tests verify that the transport layer works correctly for
//! local-only mode. Hyperswarm and relay tests are skipped until
//! those implementations are complete.

use pluresdb_sync::{
    create_transport, derive_topic, DisabledTransport, Transport, TransportConfig,
    TransportMode,
};

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

// TODO: Add these tests once hyperswarm-rs is integrated
#[ignore]
#[tokio::test]
async fn test_crdt_sync_over_hyperswarm() {
    // This test will verify CRDT operations replicate correctly
    // It should:
    // 1. Create two PluresDB instances with hyperswarm sync
    // 2. Write data to instance A
    // 3. Verify data appears on instance B
    // 4. Test conflict resolution with concurrent writes
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
