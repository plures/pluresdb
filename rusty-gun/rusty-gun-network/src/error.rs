//! Error types for Rusty Gun Network

use thiserror::Error;

/// Result type alias for network operations
pub type Result<T> = std::result::Result<T, NetworkError>;

/// Network engine errors
#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Peer not found: {peer_id}")]
    PeerNotFound { peer_id: String },

    #[error("Message too large: {size} bytes (max: {max})")]
    MessageTooLarge { size: usize, max: usize },

    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption error: {0}")]
    Decryption(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Peer rejected: {peer_id} - {reason}")]
    PeerRejected { peer_id: String, reason: String },

    #[error("Sync failed: {0}")]
    SyncFailed(String),

    #[error("Discovery failed: {0}")]
    DiscoveryFailed(String),

    #[error("NAT traversal failed: {0}")]
    NatTraversalFailed(String),

    #[error("Timeout: {operation}")]
    Timeout { operation: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Bincode error: {0}")]
    Bincode(#[from] bincode::Error),

    #[error("QUIC error: {0}")]
    Quic(String),

    #[error("WebRTC error: {0}")]
    WebRTC(String),

    #[error("LibP2P error: {0}")]
    LibP2P(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl NetworkError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            NetworkError::ConnectionFailed(_) |
            NetworkError::Timeout { .. } |
            NetworkError::Io(_) |
            NetworkError::NatTraversalFailed(_)
        )
    }

    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            NetworkError::ConnectionFailed(msg) => format!("Connection failed: {}", msg),
            NetworkError::PeerNotFound { peer_id } => format!("Peer '{}' not found", peer_id),
            NetworkError::MessageTooLarge { size, max } => {
                format!("Message too large ({} bytes, max: {} bytes)", size, max)
            }
            NetworkError::InvalidMessage(msg) => format!("Invalid message: {}", msg),
            NetworkError::Encryption(msg) => format!("Encryption error: {}", msg),
            NetworkError::Decryption(msg) => format!("Decryption error: {}", msg),
            NetworkError::AuthenticationFailed(msg) => format!("Authentication failed: {}", msg),
            NetworkError::PeerRejected { peer_id, reason } => {
                format!("Peer '{}' rejected connection: {}", peer_id, reason)
            }
            NetworkError::SyncFailed(msg) => format!("Sync failed: {}", msg),
            NetworkError::DiscoveryFailed(msg) => format!("Discovery failed: {}", msg),
            NetworkError::NatTraversalFailed(msg) => format!("NAT traversal failed: {}", msg),
            NetworkError::Timeout { operation } => format!("Operation '{}' timed out", operation),
            NetworkError::Io(err) => format!("I/O error: {}", err),
            NetworkError::Serialization(err) => format!("Serialization error: {}", err),
            NetworkError::Bincode(err) => format!("Bincode error: {}", err),
            NetworkError::Quic(msg) => format!("QUIC error: {}", msg),
            NetworkError::WebRTC(msg) => format!("WebRTC error: {}", msg),
            NetworkError::LibP2P(msg) => format!("LibP2P error: {}", msg),
            NetworkError::Configuration(msg) => format!("Configuration error: {}", msg),
            NetworkError::Internal(msg) => format!("Internal error: {}", msg),
        }
    }

    /// Get error code for programmatic handling
    pub fn error_code(&self) -> u32 {
        match self {
            NetworkError::ConnectionFailed(_) => 1001,
            NetworkError::PeerNotFound { .. } => 1002,
            NetworkError::MessageTooLarge { .. } => 1003,
            NetworkError::InvalidMessage(_) => 1004,
            NetworkError::Encryption(_) => 1005,
            NetworkError::Decryption(_) => 1006,
            NetworkError::AuthenticationFailed(_) => 1007,
            NetworkError::PeerRejected { .. } => 1008,
            NetworkError::SyncFailed(_) => 1009,
            NetworkError::DiscoveryFailed(_) => 1010,
            NetworkError::NatTraversalFailed(_) => 1011,
            NetworkError::Timeout { .. } => 1012,
            NetworkError::Io(_) => 1013,
            NetworkError::Serialization(_) => 1014,
            NetworkError::Bincode(_) => 1015,
            NetworkError::Quic(_) => 1016,
            NetworkError::WebRTC(_) => 1017,
            NetworkError::LibP2P(_) => 1018,
            NetworkError::Configuration(_) => 1019,
            NetworkError::Internal(_) => 1020,
        }
    }
}


