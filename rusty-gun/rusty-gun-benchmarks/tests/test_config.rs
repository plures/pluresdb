use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub crdt: CrdtTestConfig,
    pub storage: StorageTestConfig,
    pub vector_search: VectorSearchTestConfig,
    pub network: NetworkTestConfig,
    pub api: ApiTestConfig,
    pub performance: PerformanceTestConfig,
    pub integration: IntegrationTestConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrdtTestConfig {
    pub max_nodes: usize,
    pub conflict_test_size: usize,
    pub merge_test_size: usize,
    pub concurrent_operations: usize,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageTestConfig {
    pub sqlite: StorageBackendConfig,
    pub rocksdb: StorageBackendConfig,
    pub sled: StorageBackendConfig,
    pub bulk_operation_size: usize,
    pub concurrent_operations: usize,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageBackendConfig {
    pub enabled: bool,
    pub max_connections: u32,
    pub cache_size: usize,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchTestConfig {
    pub dimensions: usize,
    pub max_vectors: usize,
    pub search_limit: usize,
    pub threshold: f32,
    pub concurrent_searches: usize,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkTestConfig {
    pub quic: NetworkProtocolConfig,
    pub webrtc: NetworkProtocolConfig,
    pub libp2p: NetworkProtocolConfig,
    pub max_connections: usize,
    pub message_size: usize,
    pub concurrent_connections: usize,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProtocolConfig {
    pub enabled: bool,
    pub port: u16,
    pub encryption: bool,
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTestConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub request_timeout: Duration,
    pub response_timeout: Duration,
    pub concurrent_requests: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTestConfig {
    pub memory_limit_mb: usize,
    pub cpu_limit_percent: f32,
    pub disk_space_limit_mb: usize,
    pub response_time_limit_ms: u64,
    pub throughput_limit_ops_per_sec: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationTestConfig {
    pub end_to_end_timeout: Duration,
    pub workflow_steps: usize,
    pub concurrent_users: usize,
    pub data_volume: DataVolumeConfig,
    pub cleanup_after_tests: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataVolumeConfig {
    pub small: usize,
    pub medium: usize,
    pub large: usize,
    pub extra_large: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            crdt: CrdtTestConfig::default(),
            storage: StorageTestConfig::default(),
            vector_search: VectorSearchTestConfig::default(),
            network: NetworkTestConfig::default(),
            api: ApiTestConfig::default(),
            performance: PerformanceTestConfig::default(),
            integration: IntegrationTestConfig::default(),
        }
    }
}

impl Default for CrdtTestConfig {
    fn default() -> Self {
        Self {
            max_nodes: 10000,
            conflict_test_size: 1000,
            merge_test_size: 5000,
            concurrent_operations: 10,
            timeout: Duration::from_secs(30),
        }
    }
}

impl Default for StorageTestConfig {
    fn default() -> Self {
        Self {
            sqlite: StorageBackendConfig {
                enabled: true,
                max_connections: 10,
                cache_size: 1000,
                timeout: Duration::from_secs(10),
            },
            rocksdb: StorageBackendConfig {
                enabled: true,
                max_connections: 10,
                cache_size: 1000,
                timeout: Duration::from_secs(10),
            },
            sled: StorageBackendConfig {
                enabled: true,
                max_connections: 10,
                cache_size: 1000,
                timeout: Duration::from_secs(10),
            },
            bulk_operation_size: 1000,
            concurrent_operations: 5,
            timeout: Duration::from_secs(60),
        }
    }
}

impl Default for VectorSearchTestConfig {
    fn default() -> Self {
        Self {
            dimensions: 128,
            max_vectors: 10000,
            search_limit: 10,
            threshold: 0.5,
            concurrent_searches: 5,
            timeout: Duration::from_secs(30),
        }
    }
}

impl Default for NetworkTestConfig {
    fn default() -> Self {
        Self {
            quic: NetworkProtocolConfig {
                enabled: true,
                port: 34569,
                encryption: true,
                timeout: Duration::from_secs(10),
            },
            webrtc: NetworkProtocolConfig {
                enabled: true,
                port: 34570,
                encryption: true,
                timeout: Duration::from_secs(10),
            },
            libp2p: NetworkProtocolConfig {
                enabled: false,
                port: 34571,
                encryption: true,
                timeout: Duration::from_secs(10),
            },
            max_connections: 100,
            message_size: 1024,
            concurrent_connections: 10,
            timeout: Duration::from_secs(30),
        }
    }
}

impl Default for ApiTestConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 0, // Use random port
            max_connections: 1000,
            request_timeout: Duration::from_secs(30),
            response_timeout: Duration::from_secs(30),
            concurrent_requests: 100,
        }
    }
}

impl Default for PerformanceTestConfig {
    fn default() -> Self {
        Self {
            memory_limit_mb: 1024,
            cpu_limit_percent: 80.0,
            disk_space_limit_mb: 1000,
            response_time_limit_ms: 1000,
            throughput_limit_ops_per_sec: 1000,
        }
    }
}

impl Default for IntegrationTestConfig {
    fn default() -> Self {
        Self {
            end_to_end_timeout: Duration::from_secs(300),
            workflow_steps: 10,
            concurrent_users: 10,
            data_volume: DataVolumeConfig::default(),
            cleanup_after_tests: true,
        }
    }
}

impl Default for DataVolumeConfig {
    fn default() -> Self {
        Self {
            small: 100,
            medium: 1000,
            large: 10000,
            extra_large: 100000,
        }
    }
}

impl TestConfig {
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: TestConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub fn save_to_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // Validate CRDT config
        if self.crdt.max_nodes == 0 {
            errors.push("CRDT max_nodes must be greater than 0".to_string());
        }

        if self.crdt.timeout.as_secs() == 0 {
            errors.push("CRDT timeout must be greater than 0".to_string());
        }

        // Validate storage config
        if self.storage.bulk_operation_size == 0 {
            errors.push("Storage bulk_operation_size must be greater than 0".to_string());
        }

        // Validate vector search config
        if self.vector_search.dimensions == 0 {
            errors.push("Vector search dimensions must be greater than 0".to_string());
        }

        if self.vector_search.threshold < 0.0 || self.vector_search.threshold > 1.0 {
            errors.push("Vector search threshold must be between 0.0 and 1.0".to_string());
        }

        // Validate network config
        if self.network.max_connections == 0 {
            errors.push("Network max_connections must be greater than 0".to_string());
        }

        // Validate API config
        if self.api.port == 0 {
            errors.push("API port must be greater than 0".to_string());
        }

        // Validate performance config
        if self.performance.memory_limit_mb == 0 {
            errors.push("Performance memory_limit_mb must be greater than 0".to_string());
        }

        if self.performance.cpu_limit_percent <= 0.0 || self.performance.cpu_limit_percent > 100.0 {
            errors.push("Performance cpu_limit_percent must be between 0.0 and 100.0".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn get_test_data_size(&self, size: &str) -> usize {
        match size {
            "small" => self.integration.data_volume.small,
            "medium" => self.integration.data_volume.medium,
            "large" => self.integration.data_volume.large,
            "extra_large" => self.integration.data_volume.extra_large,
            _ => self.integration.data_volume.medium,
        }
    }

    pub fn is_storage_backend_enabled(&self, backend: &str) -> bool {
        match backend {
            "sqlite" => self.storage.sqlite.enabled,
            "rocksdb" => self.storage.rocksdb.enabled,
            "sled" => self.storage.sled.enabled,
            _ => false,
        }
    }

    pub fn is_network_protocol_enabled(&self, protocol: &str) -> bool {
        match protocol {
            "quic" => self.network.quic.enabled,
            "webrtc" => self.network.webrtc.enabled,
            "libp2p" => self.network.libp2p.enabled,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TestConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = TestConfig::default();
        config.crdt.max_nodes = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("max_nodes"));
    }

    #[test]
    fn test_config_serialization() {
        let config = TestConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: TestConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.crdt.max_nodes, deserialized.crdt.max_nodes);
    }

    #[test]
    fn test_get_test_data_size() {
        let config = TestConfig::default();
        assert_eq!(config.get_test_data_size("small"), config.integration.data_volume.small);
        assert_eq!(config.get_test_data_size("medium"), config.integration.data_volume.medium);
        assert_eq!(config.get_test_data_size("large"), config.integration.data_volume.large);
        assert_eq!(config.get_test_data_size("extra_large"), config.integration.data_volume.extra_large);
        assert_eq!(config.get_test_data_size("unknown"), config.integration.data_volume.medium);
    }

    #[test]
    fn test_storage_backend_enabled() {
        let config = TestConfig::default();
        assert!(config.is_storage_backend_enabled("sqlite"));
        assert!(config.is_storage_backend_enabled("rocksdb"));
        assert!(config.is_storage_backend_enabled("sled"));
        assert!(!config.is_storage_backend_enabled("unknown"));
    }

    #[test]
    fn test_network_protocol_enabled() {
        let config = TestConfig::default();
        assert!(config.is_network_protocol_enabled("quic"));
        assert!(config.is_network_protocol_enabled("webrtc"));
        assert!(!config.is_network_protocol_enabled("libp2p"));
        assert!(!config.is_network_protocol_enabled("unknown"));
    }
}

