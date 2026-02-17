//! Disabled transport for local-only mode.
//!
//! This transport provides graceful fallback when P2P sync is disabled.
//! The database operates in local-only mode with no network synchronization.

use crate::transport::{Connection, PeerInfo, TopicHash, Transport};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Disabled transport for local-only operation
pub struct DisabledTransport;

impl DisabledTransport {
    /// Create a new disabled transport
    pub fn new() -> Self {
        Self
    }
}

impl Default for DisabledTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for DisabledTransport {
    async fn connect(&mut self, _topic: TopicHash) -> Result<mpsc::Receiver<Box<dyn Connection>>> {
        Err(anyhow!("Sync is disabled - database is in local-only mode"))
    }

    async fn announce(&mut self, _topic: TopicHash) -> Result<()> {
        // Silently ignore announce in local-only mode
        Ok(())
    }

    async fn lookup(&self, _topic: TopicHash) -> Result<Vec<PeerInfo>> {
        // No peers in local-only mode
        Ok(vec![])
    }

    async fn disconnect(&mut self) -> Result<()> {
        // Nothing to disconnect
        Ok(())
    }

    fn name(&self) -> &str {
        "disabled"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_disabled_transport() {
        let mut transport = DisabledTransport::new();
        
        assert_eq!(transport.name(), "disabled");
        
        // Connect should fail
        let topic = [0u8; 32];
        let result = transport.connect(topic).await;
        assert!(result.is_err());
        
        // Announce should succeed silently
        let result = transport.announce(topic).await;
        assert!(result.is_ok());
        
        // Lookup should return empty list
        let peers = transport.lookup(topic).await.unwrap();
        assert!(peers.is_empty());
        
        // Disconnect should succeed
        let result = transport.disconnect().await;
        assert!(result.is_ok());
    }
}
