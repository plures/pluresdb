//! Main entry point for Rusty Gun API Server

use anyhow::Result;
use rusty_gun_api::{create_api_state, start_server, ServerConfig};
use rusty_gun_storage::{
    StorageConfig, VectorConfig, EmbeddingConfig, EmbeddingModel,
    SqliteStorage, VectorSearchService,
};
use rusty_gun_network::{NetworkConfig, QuicNetworkEngine};
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("🚀 Starting Rusty Gun API Server...");

    // Load configuration
    let server_config = load_server_config();
    let storage_config = load_storage_config();
    let vector_config = load_vector_config();
    let embedding_config = load_embedding_config();
    let network_config = load_network_config();

    info!("📊 Configuration loaded:");
    info!("  Server: {}:{}", server_config.host, server_config.port);
    info!("  Storage: {:?}", storage_config.backend);
    info!("  Vector Search: {} dimensions", vector_config.dimensions);
    info!("  Embedding Model: {}", embedding_config.model.name());
    info!("  Network: QUIC enabled");

    // Initialize storage
    let storage = match storage_config.backend {
        rusty_gun_storage::StorageBackend::Sqlite => {
            Box::new(SqliteStorage::new(storage_config)?)
        }
        _ => {
            return Err(anyhow::anyhow!("Only SQLite backend is currently supported"));
        }
    };

    // Initialize vector search service
    let vector_service = VectorSearchService::new(vector_config, embedding_config);
    
    // Initialize network engine
    let network_engine = Box::new(QuicNetworkEngine::new(
        network_config,
        Box::new(MockPeerManager),
        Box::new(MockSyncEngine),
    ));

    // Create API state
    let api_state = create_api_state(storage, vector_service, network_engine, server_config);

    // Start the server
    if let Err(e) = start_server(api_state).await {
        error!("❌ Server failed to start: {}", e);
        return Err(e.into());
    }

    Ok(())
}

/// Load server configuration from environment or use defaults
fn load_server_config() -> ServerConfig {
    ServerConfig {
        host: std::env::var("RUSTY_GUN_HOST")
            .unwrap_or_else(|_| "0.0.0.0".to_string()),
        port: std::env::var("RUSTY_GUN_PORT")
            .and_then(|s| s.parse().ok())
            .unwrap_or(34569),
        enable_cors: std::env::var("RUSTY_GUN_CORS")
            .map(|s| s == "true")
            .unwrap_or(true),
        enable_tracing: std::env::var("RUSTY_GUN_TRACING")
            .map(|s| s == "true")
            .unwrap_or(true),
        max_request_size: std::env::var("RUSTY_GUN_MAX_REQUEST_SIZE")
            .and_then(|s| s.parse().ok())
            .unwrap_or(10 * 1024 * 1024), // 10MB
        request_timeout: std::env::var("RUSTY_GUN_REQUEST_TIMEOUT")
            .and_then(|s| s.parse().ok())
            .unwrap_or(30),
        enable_metrics: std::env::var("RUSTY_GUN_METRICS")
            .map(|s| s == "true")
            .unwrap_or(true),
        enable_health_check: std::env::var("RUSTY_GUN_HEALTH_CHECK")
            .map(|s| s == "true")
            .unwrap_or(true),
    }
}

/// Load storage configuration
fn load_storage_config() -> StorageConfig {
    StorageConfig {
        backend: std::env::var("RUSTY_GUN_STORAGE_BACKEND")
            .map(|s| match s.as_str() {
                "rocksdb" => rusty_gun_storage::StorageBackend::RocksDB,
                "sled" => rusty_gun_storage::StorageBackend::Sled,
                _ => rusty_gun_storage::StorageBackend::Sqlite,
            })
            .unwrap_or(rusty_gun_storage::StorageBackend::Sqlite),
        path: std::env::var("RUSTY_GUN_DB_PATH")
            .unwrap_or_else(|_| "./data/rusty-gun.db".to_string()),
        max_connections: std::env::var("RUSTY_GUN_MAX_CONNECTIONS")
            .and_then(|s| s.parse().ok())
            .unwrap_or(10),
        enable_wal: std::env::var("RUSTY_GUN_WAL")
            .map(|s| s == "true")
            .unwrap_or(true),
        enable_foreign_keys: std::env::var("RUSTY_GUN_FOREIGN_KEYS")
            .map(|s| s == "true")
            .unwrap_or(true),
        vector_config: load_vector_config(),
    }
}

/// Load vector search configuration
fn load_vector_config() -> VectorConfig {
    VectorConfig {
        dimensions: std::env::var("RUSTY_GUN_VECTOR_DIMENSIONS")
            .and_then(|s| s.parse().ok())
            .unwrap_or(384),
        max_vectors: std::env::var("RUSTY_GUN_MAX_VECTORS")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1_000_000),
        hnsw_m: std::env::var("RUSTY_GUN_HNSW_M")
            .and_then(|s| s.parse().ok())
            .unwrap_or(16),
        hnsw_ef_construction: std::env::var("RUSTY_GUN_HNSW_EF_CONSTRUCTION")
            .and_then(|s| s.parse().ok())
            .unwrap_or(200),
        hnsw_ef: std::env::var("RUSTY_GUN_HNSW_EF")
            .and_then(|s| s.parse().ok())
            .unwrap_or(50),
    }
}

/// Load embedding configuration
fn load_embedding_config() -> EmbeddingConfig {
    let model = std::env::var("RUSTY_GUN_EMBEDDING_MODEL")
        .map(|s| match s.as_str() {
            "openai-ada-002" => EmbeddingModel::OpenAITextEmbeddingAda002,
            "openai-3-small" => EmbeddingModel::OpenAITextEmbedding3Small,
            "openai-3-large" => EmbeddingModel::OpenAITextEmbedding3Large,
            "sentence-transformers-mpnet" => EmbeddingModel::SentenceTransformersMPNet,
            _ => EmbeddingModel::SentenceTransformersMiniLM,
        })
        .unwrap_or(EmbeddingModel::SentenceTransformersMiniLM);

    EmbeddingConfig {
        model,
        api_key: std::env::var("OPENAI_API_KEY").ok(),
        base_url: std::env::var("OPENAI_BASE_URL").ok(),
        max_text_length: std::env::var("RUSTY_GUN_MAX_TEXT_LENGTH")
            .and_then(|s| s.parse().ok())
            .unwrap_or(8192),
        batch_size: std::env::var("RUSTY_GUN_BATCH_SIZE")
            .and_then(|s| s.parse().ok())
            .unwrap_or(100),
        enable_cache: std::env::var("RUSTY_GUN_CACHE")
            .map(|s| s == "true")
            .unwrap_or(true),
        cache_dir: std::env::var("RUSTY_GUN_CACHE_DIR")
            .ok()
            .or_else(|| Some("./cache/embeddings".to_string())),
    }
}

/// Load network configuration
fn load_network_config() -> NetworkConfig {
    NetworkConfig {
        port: std::env::var("RUSTY_GUN_NETWORK_PORT")
            .and_then(|s| s.parse().ok())
            .unwrap_or(34570),
        enable_quic: std::env::var("RUSTY_GUN_QUIC")
            .map(|s| s == "true")
            .unwrap_or(true),
        enable_webrtc: std::env::var("RUSTY_GUN_WEBRTC")
            .map(|s| s == "true")
            .unwrap_or(false),
        enable_libp2p: std::env::var("RUSTY_GUN_LIBP2P")
            .map(|s| s == "true")
            .unwrap_or(false),
        enable_encryption: std::env::var("RUSTY_GUN_ENCRYPTION")
            .map(|s| s == "true")
            .unwrap_or(true),
        max_connections: std::env::var("RUSTY_GUN_MAX_NETWORK_CONNECTIONS")
            .and_then(|s| s.parse().ok())
            .unwrap_or(100),
        bootstrap_peers: std::env::var("RUSTY_GUN_BOOTSTRAP_PEERS")
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default(),
        identity_key_path: std::env::var("RUSTY_GUN_IDENTITY_KEY").ok(),
        certificate_path: std::env::var("RUSTY_GUN_CERTIFICATE").ok(),
        private_key_path: std::env::var("RUSTY_GUN_PRIVATE_KEY").ok(),
    }
}

// Mock implementations for network components
use rusty_gun_network::traits::{PeerManager, SyncEngine};
use rusty_gun_network::error::Result as NetworkResult;
use rusty_gun_network::traits::{PeerId, NetworkMessage};

struct MockPeerManager;

#[async_trait::async_trait]
impl PeerManager for MockPeerManager {
    async fn add_peer(&self, peer_id: PeerId, address: String) -> NetworkResult<()> {
        tracing::info!("Mock: Adding peer {} at {}", peer_id, address);
        Ok(())
    }

    async fn remove_peer(&self, peer_id: &PeerId) -> NetworkResult<()> {
        tracing::info!("Mock: Removing peer {}", peer_id);
        Ok(())
    }

    async fn get_peer_address(&self, peer_id: &PeerId) -> Option<String> {
        tracing::debug!("Mock: Getting address for peer {}", peer_id);
        None
    }

    async fn get_all_peers(&self) -> Vec<(PeerId, String)> {
        tracing::debug!("Mock: Getting all peers");
        Vec::new()
    }

    async fn is_connected(&self, peer_id: &PeerId) -> bool {
        tracing::debug!("Mock: Checking if peer {} is connected", peer_id);
        false
    }
}

struct MockSyncEngine;

#[async_trait::async_trait]
impl SyncEngine for MockSyncEngine {
    async fn start_sync(&self) -> NetworkResult<()> {
        tracing::info!("Mock: Starting sync");
        Ok(())
    }

    async fn sync_with_peer(&self, peer_id: &PeerId) -> NetworkResult<()> {
        tracing::info!("Mock: Syncing with peer {}", peer_id);
        Ok(())
    }

    async fn handle_incoming_data(&self, peer_id: &PeerId, data: NetworkMessage) -> NetworkResult<()> {
        tracing::debug!("Mock: Handling incoming data from peer {} ({} bytes)", peer_id, data.len());
        Ok(())
    }

    async fn get_data_to_sync(&self, peer_id: &PeerId) -> NetworkResult<NetworkMessage> {
        tracing::debug!("Mock: Getting data to sync for peer {}", peer_id);
        Ok(Vec::new())
    }

    async fn apply_synced_data(&self, data: NetworkMessage) -> NetworkResult<()> {
        tracing::debug!("Mock: Applying synced data ({} bytes)", data.len());
        Ok(())
    }
}


