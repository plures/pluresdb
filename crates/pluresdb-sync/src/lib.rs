//! High-level synchronization primitives for PluresDB.
//!
//! These types provide a foundational event pipeline that higher-level
//! replication components can build on top of. For now we expose a lightweight
//! broadcast hub with typed events that integrates with Tokio tasks.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncEvent {
    NodeUpsert { id: String },
    NodeDelete { id: String },
    PeerConnected { peer_id: String },
    PeerDisconnected { peer_id: String },
}

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
    pub fn new(capacity: usize) -> Self {
        let (sender, _receiver) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SyncEvent> {
        self.sender.subscribe()
    }

    #[instrument(skip(self))]
    pub fn publish(&self, event: SyncEvent) -> Result<()> {
        self.sender.send(event)?;
        Ok(())
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
}
