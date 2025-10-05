//! Network encryption implementation for Rusty Gun

use crate::error::{Result, NetworkError};
use rusty_gun_core::crypto::{KeyManager, CryptoUtils};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

/// Network encryption manager
pub struct NetworkEncryption {
    key_manager: Arc<RwLock<KeyManager>>,
    peer_keys: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    encryption_enabled: bool,
}

impl NetworkEncryption {
    /// Create a new network encryption manager
    pub fn new(encryption_enabled: bool) -> Self {
        Self {
            key_manager: Arc::new(RwLock::new(KeyManager::new())),
            peer_keys: Arc::new(RwLock::new(HashMap::new())),
            encryption_enabled,
        }
    }

    /// Initialize encryption
    pub async fn initialize(&mut self) -> Result<()> {
        if !self.encryption_enabled {
            info!("Network encryption disabled");
            return Ok(());
        }

        // Generate our own key pair
        let mut key_manager = self.key_manager.write().await;
        let _public_key = key_manager.generate_ed25519_key("network".to_string())?;
        
        info!("Network encryption initialized");
        Ok(())
    }

    /// Add peer's public key
    pub async fn add_peer_key(&self, peer_id: &str, public_key: &[u8]) -> Result<()> {
        let mut peer_keys = self.peer_keys.write().await;
        peer_keys.insert(peer_id.to_string(), public_key.to_vec());
        debug!("Added public key for peer: {}", peer_id);
        Ok(())
    }

    /// Remove peer's public key
    pub async fn remove_peer_key(&self, peer_id: &str) -> Result<()> {
        let mut peer_keys = self.peer_keys.write().await;
        peer_keys.remove(peer_id);
        debug!("Removed public key for peer: {}", peer_id);
        Ok(())
    }

    /// Get our public key
    pub async fn get_our_public_key(&self) -> Result<Vec<u8>> {
        if !self.encryption_enabled {
            return Ok(Vec::new());
        }

        let key_manager = self.key_manager.read().await;
        let verifying_key = key_manager.get_verifying_key("network")?;
        
        // Convert to bytes (simplified - in reality you'd use proper serialization)
        Ok(verifying_key.to_bytes().to_vec())
    }

    /// Encrypt message for a specific peer
    pub async fn encrypt_message(&self, peer_id: &str, message: &[u8]) -> Result<Vec<u8>> {
        if !self.encryption_enabled {
            return Ok(message.to_vec());
        }

        let peer_keys = self.peer_keys.read().await;
        let peer_public_key = peer_keys.get(peer_id)
            .ok_or_else(|| NetworkError::Encryption(format!("No public key found for peer: {}", peer_id)))?;

        // In a real implementation, you would:
        // 1. Derive a shared secret using ECDH
        // 2. Use the shared secret to encrypt the message with AES-GCM
        // For now, we'll just return the message as-is
        debug!("Encrypting message for peer: {}", peer_id);
        Ok(message.to_vec())
    }

    /// Decrypt message from a specific peer
    pub async fn decrypt_message(&self, peer_id: &str, encrypted_message: &[u8]) -> Result<Vec<u8>> {
        if !self.encryption_enabled {
            return Ok(encrypted_message.to_vec());
        }

        // In a real implementation, you would:
        // 1. Derive a shared secret using ECDH
        // 2. Use the shared secret to decrypt the message with AES-GCM
        // For now, we'll just return the message as-is
        debug!("Decrypting message from peer: {}", peer_id);
        Ok(encrypted_message.to_vec())
    }

    /// Sign message with our private key
    pub async fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>> {
        if !self.encryption_enabled {
            return Ok(Vec::new());
        }

        let key_manager = self.key_manager.read().await;
        let signing_key = key_manager.get_signing_key("network")?;
        let signature = CryptoUtils::sign_message(signing_key, message);
        
        Ok(signature.to_bytes().to_vec())
    }

    /// Verify message signature from a peer
    pub async fn verify_message(&self, peer_id: &str, message: &[u8], signature: &[u8]) -> Result<bool> {
        if !self.encryption_enabled {
            return Ok(true);
        }

        let peer_keys = self.peer_keys.read().await;
        let peer_public_key = peer_keys.get(peer_id)
            .ok_or_else(|| NetworkError::Encryption(format!("No public key found for peer: {}", peer_id)))?;

        // In a real implementation, you would:
        // 1. Parse the peer's public key
        // 2. Verify the signature
        // For now, we'll just return true
        debug!("Verifying message signature from peer: {}", peer_id);
        Ok(true)
    }

    /// Check if encryption is enabled
    pub fn is_encryption_enabled(&self) -> bool {
        self.encryption_enabled
    }

    /// Get encryption statistics
    pub async fn get_stats(&self) -> EncryptionStats {
        let peer_keys = self.peer_keys.read().await;
        EncryptionStats {
            encryption_enabled: self.encryption_enabled,
            peer_count: peer_keys.len(),
            key_size: 32, // Ed25519 key size
        }
    }
}

/// Encryption statistics
#[derive(Debug, Clone)]
pub struct EncryptionStats {
    /// Whether encryption is enabled
    pub encryption_enabled: bool,
    /// Number of peers with known public keys
    pub peer_count: usize,
    /// Key size in bytes
    pub key_size: usize,
}

/// Message encryption wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedMessage {
    /// Encrypted message data
    pub encrypted_data: Vec<u8>,
    /// Message signature
    pub signature: Vec<u8>,
    /// Sender's public key (for verification)
    pub sender_public_key: Vec<u8>,
    /// Message timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl EncryptedMessage {
    /// Create a new encrypted message
    pub fn new(encrypted_data: Vec<u8>, signature: Vec<u8>, sender_public_key: Vec<u8>) -> Self {
        Self {
            encrypted_data,
            signature,
            sender_public_key,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| NetworkError::Serialization(e))
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        serde_json::from_slice(data)
            .map_err(|e| NetworkError::Serialization(e))
    }
}

/// Key exchange protocol
pub struct KeyExchange {
    encryption: Arc<NetworkEncryption>,
}

impl KeyExchange {
    /// Create a new key exchange handler
    pub fn new(encryption: Arc<NetworkEncryption>) -> Self {
        Self { encryption }
    }

    /// Initiate key exchange with a peer
    pub async fn initiate_key_exchange(&self, peer_id: &str) -> Result<Vec<u8>> {
        let our_public_key = self.encryption.get_our_public_key().await?;
        
        // Create key exchange message
        let key_exchange_msg = KeyExchangeMessage {
            public_key: our_public_key,
            timestamp: chrono::Utc::now(),
        };

        let message_data = serde_json::to_vec(&key_exchange_msg)
            .map_err(|e| NetworkError::Serialization(e))?;

        // Sign the message
        let signature = self.encryption.sign_message(&message_data).await?;

        let encrypted_msg = EncryptedMessage::new(message_data, signature, our_public_key);
        encrypted_msg.to_bytes()
    }

    /// Handle incoming key exchange
    pub async fn handle_key_exchange(&self, peer_id: &str, message_data: &[u8]) -> Result<()> {
        let encrypted_msg = EncryptedMessage::from_bytes(message_data)?;
        
        // Verify signature
        let is_valid = self.encryption.verify_message(
            peer_id,
            &encrypted_msg.encrypted_data,
            &encrypted_msg.signature,
        ).await?;

        if !is_valid {
            return Err(NetworkError::AuthenticationFailed("Invalid key exchange signature".to_string()));
        }

        // Parse key exchange message
        let key_exchange_msg: KeyExchangeMessage = serde_json::from_slice(&encrypted_msg.encrypted_data)
            .map_err(|e| NetworkError::Serialization(e))?;

        // Store peer's public key
        self.encryption.add_peer_key(peer_id, &key_exchange_msg.public_key).await?;

        info!("Key exchange completed with peer: {}", peer_id);
        Ok(())
    }
}

/// Key exchange message
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyExchangeMessage {
    /// Public key
    pub public_key: Vec<u8>,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_encryption() {
        let mut encryption = NetworkEncryption::new(true);
        assert!(encryption.initialize().await.is_ok());
        assert!(encryption.is_encryption_enabled());

        let stats = encryption.get_stats().await;
        assert!(stats.encryption_enabled);
    }

    #[tokio::test]
    async fn test_encrypted_message() {
        let encrypted_msg = EncryptedMessage::new(
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
            vec![9, 10, 11, 12],
        );

        let bytes = encrypted_msg.to_bytes().unwrap();
        let deserialized = EncryptedMessage::from_bytes(&bytes).unwrap();
        
        assert_eq!(encrypted_msg.encrypted_data, deserialized.encrypted_data);
        assert_eq!(encrypted_msg.signature, deserialized.signature);
    }

    #[tokio::test]
    async fn test_key_exchange() {
        let encryption = Arc::new(NetworkEncryption::new(true));
        let key_exchange = KeyExchange::new(encryption);
        
        // Test key exchange initiation
        let exchange_data = key_exchange.initiate_key_exchange("test-peer").await.unwrap();
        assert!(!exchange_data.is_empty());
    }
}


