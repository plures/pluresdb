//! Error types for Rusty Gun Storage

use thiserror::Error;

/// Result type alias for storage operations
pub type Result<T> = std::result::Result<T, StorageError>;

/// Storage engine errors
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Query execution failed: {0}")]
    QueryFailed(String),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Migration failed: {0}")]
    MigrationFailed(String),

    #[error("Vector search failed: {0}")]
    VectorSearchFailed(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("RocksDB error: {0}")]
    RocksDB(#[from] rocksdb::Error),

    #[error("Sled error: {0}")]
    Sled(#[from] sled::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl StorageError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            StorageError::ConnectionFailed(_) |
            StorageError::Timeout(_) |
            StorageError::Sqlx(sqlx::Error::Database(_))
        )
    }

    /// Get a user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            StorageError::ConnectionFailed(msg) => format!("Database connection failed: {}", msg),
            StorageError::QueryFailed(msg) => format!("Query failed: {}", msg),
            StorageError::TransactionFailed(msg) => format!("Transaction failed: {}", msg),
            StorageError::MigrationFailed(msg) => format!("Migration failed: {}", msg),
            StorageError::VectorSearchFailed(msg) => format!("Vector search failed: {}", msg),
            StorageError::Serialization(err) => format!("Data serialization failed: {}", err),
            StorageError::Sqlx(err) => format!("Database error: {}", err),
            StorageError::RocksDB(err) => format!("RocksDB error: {}", err),
            StorageError::Sled(err) => format!("Sled error: {}", err),
            StorageError::Io(err) => format!("I/O error: {}", err),
            StorageError::Configuration(msg) => format!("Configuration error: {}", msg),
            StorageError::NotFound(msg) => format!("Not found: {}", msg),
            StorageError::AlreadyExists(msg) => format!("Already exists: {}", msg),
            StorageError::ConstraintViolation(msg) => format!("Constraint violation: {}", msg),
            StorageError::Timeout(msg) => format!("Operation timeout: {}", msg),
            StorageError::PermissionDenied(msg) => format!("Permission denied: {}", msg),
            StorageError::Internal(msg) => format!("Internal error: {}", msg),
        }
    }
}


