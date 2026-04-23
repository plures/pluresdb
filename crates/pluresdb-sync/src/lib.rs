//! High-level synchronization primitives for PluresDB.
//!
//! These types provide a foundational event pipeline that higher-level
//! replication components can build on top of. For now we expose a lightweight
//! broadcast hub with typed events that integrates with Tokio tasks.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::instrument;

mod transport;
pub use transport::*;

mod hyperswarm;
pub use hyperswarm::*;

mod relay;
pub use relay::*;

mod disabled;
pub use disabled::*;

pub mod gun_protocol;
pub use gun_protocol::{
    GunAck, GunGet, GunGetRequest, GunMessage, GunMeta, GunNode, GunPut, HamState, Soul,
};

mod gun_relay;
pub use gun_relay::GunRelayServer;

mod replication;
pub use replication::{MemConnection, Replicator};

pub mod git_replication;

/// Stable, documented error codes emitted by `pluresdb-sync`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncErrorCode {
    BroadcastPublishFailed,
}

impl SyncErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BroadcastPublishFailed => "SYNC_BROADCAST_PUBLISH_FAILED",
        }
    }
}

impl std::fmt::Display for SyncErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Events broadcast by [`SyncBroadcaster`] when the local store changes or
/// when P2P peer connections are established / torn down.
///
/// Consumers subscribe via [`SyncBroadcaster::subscribe`] and receive a clone
/// of each event published by any code path that calls
/// [`SyncBroadcaster::publish`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncEvent {
    /// A node was inserted or updated in the local store.
    NodeUpsert {
        /// Identifier of the node that was written.
        id: String,
    },
    /// A node was deleted from the local store.
    NodeDelete {
        /// Identifier of the node that was removed.
        id: String,
    },
    /// A remote peer successfully connected to this node.
    PeerConnected {
        /// Stable identifier of the newly connected peer.
        peer_id: String,
    },
    /// A remote peer disconnected (gracefully or due to error).
    PeerDisconnected {
        /// Stable identifier of the peer that disconnected.
        peer_id: String,
    },
}

/// Tokio broadcast hub for [`SyncEvent`]s.
///
/// Wraps a [`tokio::sync::broadcast`] channel so that multiple independent
/// listeners — e.g. replication workers, WebSocket push handlers, and metrics
/// collectors — can all receive a copy of every sync event without coupling to
/// each other.
///
/// # Example
///
/// ```rust
/// use pluresdb_sync::{SyncBroadcaster, SyncEvent};
///
/// let hub = SyncBroadcaster::default();
/// let mut rx = hub.subscribe();
/// hub.publish(SyncEvent::NodeUpsert { id: "node-1".to_string() }).unwrap();
/// ```
#[derive(Debug)]
pub struct SyncBroadcaster {
    sender: broadcast::Sender<SyncEvent>,
}

impl Default for SyncBroadcaster {
    fn default() -> Self {
        let (sender, _receiver) = broadcast::channel(1024);
        Self { sender }
    }
}

impl SyncBroadcaster {
    /// Create a new broadcaster with the given channel `capacity`.
    ///
    /// `capacity` is the maximum number of events that can be queued in the
    /// underlying broadcast channel before slow receivers start missing events.
    /// For most workloads a value in the range `64`–`1024` is appropriate.
    pub fn new(capacity: usize) -> Self {
        let (sender, _receiver) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribe to the event stream.
    ///
    /// Returns a [`broadcast::Receiver`] that yields every [`SyncEvent`]
    /// published after this call.  Multiple receivers are independent —
    /// each gets its own copy of every event.
    pub fn subscribe(&self) -> broadcast::Receiver<SyncEvent> {
        self.sender.subscribe()
    }

    /// Publish a [`SyncEvent`] to all current subscribers.
    ///
    /// Returns `Ok(())` on success.  Returns an [`anyhow::Error`] wrapping a
    /// [`tokio::sync::broadcast::error::SendError`] only when the underlying
    /// channel has been closed (i.e., all senders have been dropped), which
    /// should not happen during normal operation since the broadcaster itself
    /// holds one sender.
    ///
    /// Slow subscribers that fall behind by more than the channel capacity will
    /// begin to miss events (`RecvError::Lagged`); callers should handle this
    /// in their receive loops.
    #[instrument(skip(self))]
    pub fn publish(&self, event: SyncEvent) -> Result<usize> {
        match self.sender.send(event) {
            Ok(n) => Ok(n),
            Err(_) => {
                // No active receivers — normal for single-node deployments
                // without P2P sync configured. Data is already persisted;
                // sync broadcast is best-effort.
                Ok(0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broadcast_events() {
        let hub = SyncBroadcaster::default();
        let mut rx = hub.subscribe();
        hub.publish(SyncEvent::NodeUpsert {
            id: "node-1".to_string(),
        })
        .unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(
            received,
            SyncEvent::NodeUpsert {
                id: "node-1".to_string()
            }
        );
    }

    #[test]
    fn sync_error_code_is_stable() {
        assert_eq!(
            SyncErrorCode::BroadcastPublishFailed.as_str(),
            "SYNC_BROADCAST_PUBLISH_FAILED"
        );
    }

    #[test]
    fn publish_without_subscribers_succeeds() {
        let hub = SyncBroadcaster::default();
        // No subscribers — should return Ok(0), not an error
        let result = hub.publish(SyncEvent::NodeUpsert {
            id: "orphan".to_string(),
        });
        assert_eq!(result.unwrap(), 0);
    }
}
