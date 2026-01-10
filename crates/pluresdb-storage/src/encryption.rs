//! Encryption at rest for PluresDB storage.
//!
//! This module provides AES-256-GCM encryption for WAL segments and stored data,
//! with support for key rotation and device revocation.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::SaltString;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const NONCE_SIZE: usize = 12; // 96 bits for AES-GCM
const KEY_SIZE: usize = 32; // 256 bits for AES-256
const SALT_SIZE: usize = 16; // 128 bits

/// Encryption configuration and key management.
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// Master encryption key (32 bytes for AES-256)
    master_key: [u8; KEY_SIZE],
    
    /// Salt for key derivation (16 bytes)
    salt: [u8; SALT_SIZE],
    
    /// Whether encryption is enabled
    enabled: bool,
}

impl EncryptionConfig {
    /// Creates a new encryption config with a random master key.
    pub fn new() -> Result<Self> {
        let mut master_key = [0u8; KEY_SIZE];
        let mut salt = [0u8; SALT_SIZE];
        
        OsRng.fill_bytes(&mut master_key);
        OsRng.fill_bytes(&mut salt);
        
        Ok(Self {
            master_key,
            salt,
            enabled: true,
        })
    }
    
    /// Creates an encryption config from a password using Argon2id.
    pub fn from_password(password: &str) -> Result<Self> {
        // Generate random salt
        let mut salt_bytes = [0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut salt_bytes);
        
        Self::from_password_with_salt(password, &salt_bytes)
    }
    
    /// Creates an encryption config from a password with a specific salt.
    pub fn from_password_with_salt(password: &str, salt_bytes: &[u8]) -> Result<Self> {
        if salt_bytes.len() != SALT_SIZE {
            anyhow::bail!("Salt must be {} bytes", SALT_SIZE);
        }
        
        // Create salt string for Argon2
        let salt_string = SaltString::encode_b64(&salt_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to encode salt: {}", e))?;
        
        // Derive key from password using Argon2id
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt_string)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;
        
        // Extract the derived key (first 32 bytes of the hash output)
        let hash_output = password_hash.hash
            .context("No hash output from Argon2")?;
        let hash_bytes = hash_output.as_bytes();
        
        if hash_bytes.len() < KEY_SIZE {
            anyhow::bail!("Derived key too short: {} bytes", hash_bytes.len());
        }
        
        let mut master_key = [0u8; KEY_SIZE];
        master_key.copy_from_slice(&hash_bytes[..KEY_SIZE]);
        
        let mut salt = [0u8; SALT_SIZE];
        salt.copy_from_slice(salt_bytes);
        
        Ok(Self {
            master_key,
            salt,
            enabled: true,
        })
    }
    
    /// Rotates the master key to a new password.
    pub fn rotate_key(&mut self, new_password: &str) -> Result<()> {
        // Generate new salt for the new password
        let mut new_salt = [0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut new_salt);
        
        // Derive new key from new password
        let new_config = Self::from_password_with_salt(new_password, &new_salt)?;
        
        // Update current config
        self.master_key = new_config.master_key;
        self.salt = new_config.salt;
        
        Ok(())
    }
    
    /// Encrypts data using AES-256-GCM.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        if !self.enabled {
            return Ok(plaintext.to_vec());
        }
        
        // Create cipher
        let cipher = Aes256Gcm::new(&self.master_key.into());
        
        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::clone_from_slice(&nonce_bytes);
        
        // Encrypt the plaintext
        let ciphertext = cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
        
        // Prepend nonce to ciphertext (nonce doesn't need to be secret)
        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }
    
    /// Decrypts data using AES-256-GCM.
    pub fn decrypt(&self, ciphertext_with_nonce: &[u8]) -> Result<Vec<u8>> {
        if !self.enabled {
            return Ok(ciphertext_with_nonce.to_vec());
        }
        
        if ciphertext_with_nonce.len() < NONCE_SIZE {
            anyhow::bail!("Ciphertext too short to contain nonce");
        }
        
        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = ciphertext_with_nonce.split_at(NONCE_SIZE);
        let nonce = Nonce::clone_from_slice(nonce_bytes);
        
        // Create cipher
        let cipher = Aes256Gcm::new(&self.master_key.into());
        
        // Decrypt the ciphertext
        let plaintext = cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;
        
        Ok(plaintext)
    }
    
    /// Returns whether encryption is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    /// Disables encryption (for testing).
    pub fn disable(&mut self) {
        self.enabled = false;
    }
    
    /// Returns the salt used for key derivation.
    pub fn salt(&self) -> &[u8; SALT_SIZE] {
        &self.salt
    }
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            master_key: [0u8; KEY_SIZE],
            salt: [0u8; SALT_SIZE],
            enabled: false,
        }
    }
}

/// Metadata for encrypted segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionMetadata {
    /// Version of the encryption scheme
    pub version: u32,
    
    /// Key derivation algorithm used
    pub kdf: String,
    
    /// Encryption algorithm used
    pub cipher: String,
    
    /// Salt for key derivation (base64 encoded)
    pub salt: String,
    
    /// List of revoked device IDs
    pub revoked_devices: Vec<String>,
}

impl Default for EncryptionMetadata {
    fn default() -> Self {
        Self {
            version: 1,
            kdf: "argon2id".to_string(),
            cipher: "aes-256-gcm".to_string(),
            salt: String::new(),
            revoked_devices: Vec::new(),
        }
    }
}

impl EncryptionMetadata {
    /// Creates metadata from an encryption config.
    pub fn from_config(config: &EncryptionConfig) -> Self {
        let salt_b64 = base64::encode(&config.salt);
        Self {
            version: 1,
            kdf: "argon2id".to_string(),
            cipher: "aes-256-gcm".to_string(),
            salt: salt_b64,
            revoked_devices: Vec::new(),
        }
    }
    
    /// Loads encryption metadata from a JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read metadata file: {}", path.display()))?;
        
        let metadata: Self = serde_json::from_str(&contents)
            .context("Failed to parse metadata JSON")?;
        
        Ok(metadata)
    }
    
    /// Saves encryption metadata to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize metadata")?;
        
        fs::write(path, json)
            .with_context(|| format!("Failed to write metadata file: {}", path.display()))?;
        
        Ok(())
    }
    
    /// Adds a device to the revocation list.
    pub fn revoke_device(&mut self, device_id: String) {
        if !self.revoked_devices.contains(&device_id) {
            self.revoked_devices.push(device_id);
        }
    }
    
    /// Checks if a device is revoked.
    pub fn is_device_revoked(&self, device_id: &str) -> bool {
        self.revoked_devices.contains(&device_id.to_string())
    }
    
    /// Returns the salt as bytes.
    pub fn salt_bytes(&self) -> Result<Vec<u8>> {
        base64::decode(&self.salt)
            .map_err(|e| anyhow::anyhow!("Failed to decode salt: {}", e))
    }
}

// Simple base64 encoding/decoding helpers
mod base64 {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    
    pub fn encode(input: &[u8]) -> String {
        let mut result = String::new();
        let chunks = input.chunks(3);
        
        for chunk in chunks {
            let b1 = chunk[0];
            let b2 = chunk.get(1).copied().unwrap_or(0);
            let b3 = chunk.get(2).copied().unwrap_or(0);
            
            result.push(CHARSET[(b1 >> 2) as usize] as char);
            result.push(CHARSET[(((b1 & 0x03) << 4) | (b2 >> 4)) as usize] as char);
            
            if chunk.len() > 1 {
                result.push(CHARSET[(((b2 & 0x0F) << 2) | (b3 >> 6)) as usize] as char);
            } else {
                result.push('=');
            }
            
            if chunk.len() > 2 {
                result.push(CHARSET[(b3 & 0x3F) as usize] as char);
            } else {
                result.push('=');
            }
        }
        
        result
    }
    
    pub fn decode(input: &str) -> Result<Vec<u8>, String> {
        let input = input.trim_end_matches('=');
        let mut result = Vec::new();
        let chars: Vec<u8> = input.bytes().collect();
        
        for chunk in chars.chunks(4) {
            let decode_char = |c: u8| -> Result<u8, String> {
                CHARSET.iter().position(|&x| x == c)
                    .map(|p| p as u8)
                    .ok_or_else(|| format!("Invalid character: {}", c as char))
            };
            
            let c1 = decode_char(chunk[0])?;
            let c2 = chunk.get(1).map(|&c| decode_char(c)).transpose()?.unwrap_or(0);
            let c3 = chunk.get(2).map(|&c| decode_char(c)).transpose()?.unwrap_or(0);
            let c4 = chunk.get(3).map(|&c| decode_char(c)).transpose()?.unwrap_or(0);
            
            result.push((c1 << 2) | (c2 >> 4));
            
            if chunk.len() > 2 {
                result.push((c2 << 4) | (c3 >> 2));
            }
            
            if chunk.len() > 3 {
                result.push((c3 << 6) | c4);
            }
        }
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_encryption_config_creation() {
        let config = EncryptionConfig::new().unwrap();
        assert!(config.is_enabled());
    }
    
    #[test]
    fn test_encryption_round_trip() {
        let config = EncryptionConfig::new().unwrap();
        let plaintext = b"secret data that needs to be protected";
        
        let ciphertext = config.encrypt(plaintext).unwrap();
        assert_ne!(ciphertext, plaintext, "Ciphertext should differ from plaintext");
        
        let decrypted = config.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_password_based_encryption() {
        let password = "my-secure-password-123";
        let config = EncryptionConfig::from_password(password).unwrap();
        
        let plaintext = b"sensitive agent memory data";
        let ciphertext = config.encrypt(plaintext).unwrap();
        let decrypted = config.decrypt(&ciphertext).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_same_password_different_salt_different_keys() {
        let password = "same-password";
        
        let config1 = EncryptionConfig::from_password(password).unwrap();
        let config2 = EncryptionConfig::from_password(password).unwrap();
        
        // Different salts should produce different keys
        assert_ne!(config1.salt, config2.salt);
        assert_ne!(config1.master_key, config2.master_key);
    }
    
    #[test]
    fn test_key_rotation() {
        let mut config = EncryptionConfig::from_password("old-password").unwrap();
        let old_key = config.master_key;
        
        config.rotate_key("new-password").unwrap();
        
        // Key should have changed
        assert_ne!(config.master_key, old_key);
        
        // Should still be able to encrypt/decrypt
        let plaintext = b"data after rotation";
        let ciphertext = config.encrypt(plaintext).unwrap();
        let decrypted = config.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }
    
    #[test]
    fn test_device_revocation() {
        let mut metadata = EncryptionMetadata::default();
        
        assert!(!metadata.is_device_revoked("device-1"));
        
        metadata.revoke_device("device-1".to_string());
        assert!(metadata.is_device_revoked("device-1"));
        
        // Revoking again should be idempotent
        metadata.revoke_device("device-1".to_string());
        assert_eq!(metadata.revoked_devices.len(), 1);
    }
    
    #[test]
    fn test_metadata_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let metadata_path = temp_dir.path().join("encryption.json");
        
        let config = EncryptionConfig::from_password("test-password").unwrap();
        let mut metadata = EncryptionMetadata::from_config(&config);
        metadata.revoke_device("device-xyz".to_string());
        
        // Save metadata
        metadata.save(&metadata_path).unwrap();
        
        // Load metadata
        let loaded = EncryptionMetadata::load(&metadata_path).unwrap();
        
        assert_eq!(loaded.version, metadata.version);
        assert_eq!(loaded.kdf, metadata.kdf);
        assert_eq!(loaded.cipher, metadata.cipher);
        assert_eq!(loaded.salt, metadata.salt);
        assert_eq!(loaded.revoked_devices, metadata.revoked_devices);
    }
    
    #[test]
    fn test_encryption_disabled() {
        let mut config = EncryptionConfig::default();
        config.disable();
        
        let plaintext = b"unencrypted data";
        let result = config.encrypt(plaintext).unwrap();
        
        // When disabled, encrypt should return plaintext
        assert_eq!(result, plaintext);
    }
    
    #[test]
    fn test_decrypt_wrong_key_fails() {
        let config1 = EncryptionConfig::new().unwrap();
        let config2 = EncryptionConfig::new().unwrap();
        
        let plaintext = b"secret message";
        let ciphertext = config1.encrypt(plaintext).unwrap();
        
        // Attempting to decrypt with wrong key should fail
        let result = config2.decrypt(&ciphertext);
        assert!(result.is_err(), "Decryption with wrong key should fail");
    }
    
    #[test]
    fn test_large_data_encryption() {
        let config = EncryptionConfig::new().unwrap();
        
        // Test with 1MB of data
        let plaintext = vec![0x42u8; 1024 * 1024];
        let ciphertext = config.encrypt(&plaintext).unwrap();
        let decrypted = config.decrypt(&ciphertext).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }
}
