//! Error types for Rusty Gun Core

use thiserror::Error;

/// Result type alias for Rusty Gun operations
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for Rusty Gun Core
#[derive(Error, Debug)]
pub enum Error {
    #[error("Node not found: {id}")]
    NodeNotFound { id: String },

    #[error("Node too large: {size} bytes (max: {max})")]
    NodeTooLarge { size: usize, max: usize },

    #[error("Node depth exceeded: {depth} (max: {max})")]
    NodeDepthExceeded { depth: usize, max: usize },

    #[error("Invalid node ID: {id}")]
    InvalidNodeId { id: String },

    #[error("Conflict resolution failed: {reason}")]
    ConflictResolutionFailed { reason: String },

    #[error("Crypto error: {0}")]
    Crypto(#[from] ring::error::Unspecified),

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::Network(_) | Error::Io(_) | Error::RateLimitExceeded(_)
        )
    }

    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Error::NodeNotFound { id } => format!("Node '{}' not found", id),
            Error::NodeTooLarge { size, max } => {
                format!("Node is too large ({} bytes, max: {} bytes)", size, max)
            }
            Error::NodeDepthExceeded { depth, max } => {
                format!("Node depth too deep ({} levels, max: {} levels)", depth, max)
            }
            Error::InvalidNodeId { id } => format!("Invalid node ID: '{}'", id),
            Error::ConflictResolutionFailed { reason } => {
                format!("Failed to resolve conflict: {}", reason)
            }
            Error::Crypto(_) => "Cryptographic operation failed".to_string(),
            Error::Serialization(_) => "Data serialization failed".to_string(),
            Error::Json(_) => "JSON processing failed".to_string(),
            Error::Io(_) => "I/O operation failed".to_string(),
            Error::Network(msg) => format!("Network error: {}", msg),
            Error::Storage(msg) => format!("Storage error: {}", msg),
            Error::Validation(msg) => format!("Validation error: {}", msg),
            Error::PermissionDenied(msg) => format!("Permission denied: {}", msg),
            Error::RateLimitExceeded(msg) => format!("Rate limit exceeded: {}", msg),
            Error::Internal(msg) => format!("Internal error: {}", msg),
        }
    }
}


