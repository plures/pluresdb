# pluresdb-sync

Synchronization primitives and transport layer for PluresDB P2P database replication.

## Overview

This crate provides the core synchronization infrastructure for PluresDB, including:

- **Transport Trait**: Pluggable transport abstraction for P2P communication
- **Event Broadcasting**: Lightweight pub/sub for sync events
- **Topic Derivation**: BLAKE2b-256 hashing for DHT topic discovery
- **Multiple Transports**: Hyperswarm (DHT), Relay (WebSocket), and Disabled (local-only)

## Features

### Transport Modes

#### Hyperswarm (Direct P2P)
- DHT-based peer discovery (Kademlia)
- UDP holepunching for NAT traversal
- Noise XX encrypted streams (25519/ChaChaPoly/BLAKE2s)
- Best for home networks and personal use

#### Relay (Corporate-Friendly)
- WebSocket connections on port 443
- Works through corporate firewalls
- Stateless relay server (horizontally scalable)
- End-to-end encryption preserved

#### Disabled (Local-Only)
- No network synchronization
- All data stays local
- Graceful fallback when sync is not needed

## Usage

### Basic Example

```rust
use pluresdb_sync::{create_transport, derive_topic, TransportConfig, TransportMode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a transport for your database
    let config = TransportConfig {
        mode: TransportMode::Hyperswarm,
        encryption: true,
        timeout_ms: 30000,
        ..Default::default()
    };
    let mut transport = create_transport(config);

    // Derive a topic from your database ID
    let db_id = "my-app-database";
    let topic = derive_topic(db_id);

    // Announce this peer on the topic (DHT)
    transport.announce(topic).await?;

    // Lookup other peers on the same topic
    let peers = transport.lookup(topic).await?;
    println!("Found {} peers", peers.len());

    // Connect and listen for incoming connections
    let mut connections = transport.connect(topic).await?;
    while let Some(mut conn) = connections.recv().await {
        println!("New peer connected: {}", conn.peer_id());
        
        // Send a message
        conn.send(b"Hello from PluresDB!").await?;
        
        // Receive messages
        while let Ok(Some(msg)) = conn.receive().await {
            println!("Received: {:?}", msg);
        }
    }

    Ok(())
}
```

### Disabled Transport (Local-Only)

```rust
use pluresdb_sync::{DisabledTransport, Transport};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut transport = DisabledTransport::new();
    
    // This is a no-op in local-only mode
    let topic = [0u8; 32];
    transport.announce(topic).await?;
    
    // Returns empty peer list
    let peers = transport.lookup(topic).await?;
    assert!(peers.is_empty());
    
    Ok(())
}
```

### Custom Transport Implementation

You can implement your own transport by implementing the `Transport` trait:

```rust
use pluresdb_sync::{Transport, Connection, PeerInfo, TopicHash};
use async_trait::async_trait;
use tokio::sync::mpsc;

pub struct MyCustomTransport;

#[async_trait]
impl Transport for MyCustomTransport {
    async fn connect(&mut self, topic: TopicHash) -> anyhow::Result<mpsc::Receiver<Box<dyn Connection>>> {
        // Your implementation
        todo!()
    }

    async fn announce(&mut self, topic: TopicHash) -> anyhow::Result<()> {
        // Your implementation
        todo!()
    }

    async fn lookup(&self, topic: TopicHash) -> anyhow::Result<Vec<PeerInfo>> {
        // Your implementation
        todo!()
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        // Your implementation
        todo!()
    }

    fn name(&self) -> &str {
        "my-custom-transport"
    }
}
```

## Topic Derivation

Topics are 32-byte hashes derived from database IDs using BLAKE2b-256:

```rust
use pluresdb_sync::derive_topic;

let db_id = "my-database";
let topic = derive_topic(db_id);
// topic is now a 32-byte BLAKE2b-256 hash

// Same database ID always produces same topic
assert_eq!(derive_topic(db_id), derive_topic(db_id));
```

This ensures:
- Deterministic topic generation
- Same database ID → same topic → peers find each other
- Cryptographically strong hash prevents topic collisions

## Event Broadcasting

Subscribe to sync events:

```rust
use pluresdb_sync::{SyncBroadcaster, SyncEvent};

#[tokio::main]
async fn main() {
    let hub = SyncBroadcaster::default();
    let mut rx = hub.subscribe();

    // Publish events
    hub.publish(SyncEvent::PeerConnected {
        peer_id: "peer-123".to_string(),
    }).unwrap();

    // Receive events
    while let Ok(event) = rx.recv().await {
        match event {
            SyncEvent::PeerConnected { peer_id } => {
                println!("Peer connected: {}", peer_id);
            }
            SyncEvent::PeerDisconnected { peer_id } => {
                println!("Peer disconnected: {}", peer_id);
            }
            _ => {}
        }
    }
}
```

## Configuration

Configure transport via `TransportConfig`:

```rust
use pluresdb_sync::{TransportConfig, TransportMode};

// Hyperswarm (default)
let config = TransportConfig {
    mode: TransportMode::Hyperswarm,
    relay_url: None,
    timeout_ms: 30000,
    encryption: true,
};

// Relay with custom server
let config = TransportConfig {
    mode: TransportMode::Relay,
    relay_url: Some("wss://my-relay.example.com".to_string()),
    timeout_ms: 30000,
    encryption: true,
};

// Disabled (local-only)
let config = TransportConfig {
    mode: TransportMode::Disabled,
    ..Default::default()
};
```

## Status

### Implemented ✅
- Transport trait definition
- Topic derivation (BLAKE2b-256)
- Event broadcasting system
- DisabledTransport (local-only mode)
- Transport factory (`create_transport`)
- Configuration system

### In Progress 🚧
- **HyperswarmTransport**: Stub implementation ready for hyperswarm-rs integration
- **RelayTransport**: Stub implementation ready for WebSocket client

### Pending Dependencies
- `hyperswarm-rs` crate from [plures/hyperswarm](https://github.com/plures/hyperswarm)
- WebSocket client library for relay transport

## Testing

```bash
# Run all tests
cargo test -p pluresdb-sync

# Run integration tests only
cargo test -p pluresdb-sync --test integration_test

# Run with ignored tests (requires hyperswarm-rs)
cargo test -p pluresdb-sync -- --ignored
```

## Architecture

```
┌─────────────────────────────────────────┐
│         Application Layer               │
│    (Database, CRDT operations)          │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│      SyncBroadcaster (Events)           │
│   • PeerConnected / PeerDisconnected    │
│   • NodeUpsert / NodeDelete             │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│       Transport Trait (Pluggable)       │
├─────────────────────────────────────────┤
│  HyperswarmTransport │ RelayTransport   │
│   • DHT discovery    │ • WebSocket      │
│   • UDP holepunch    │ • Port 443       │
│   • Noise crypto     │ • Corporate-safe │
│                      │                  │
│         DisabledTransport               │
│          • Local-only mode              │
└─────────────────────────────────────────┘
```

## License

AGPL-3.0 - See [LICENSE](../../LICENSE) for details.

## Contributing

See [CONTRIBUTING.md](../../CONTRIBUTING.md) for contribution guidelines.
