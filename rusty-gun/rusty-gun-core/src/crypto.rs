//! Cryptographic utilities for Rusty Gun
//! 
//! This module provides cryptographic functions for encryption, decryption,
//! digital signatures, and key management.

use crate::error::{Error, Result};
use ring::{
    aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM},
    digest,
    rand::{SecureRandom, SystemRandom},
    signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey},
};
use std::collections::HashMap;

/// Cryptographic key types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyType {
    /// Ed25519 key pair
    Ed25519,
    /// AES-256-GCM key
    Aes256Gcm,
}

/// A cryptographic key
#[derive(Debug, Clone)]
pub struct CryptoKey {
    pub key_type: KeyType,
    pub public_key: Vec<u8>,
    pub private_key: Option<Vec<u8>>,
    pub key_id: String,
}

/// Key manager for handling cryptographic keys
#[derive(Debug)]
pub struct KeyManager {
    keys: HashMap<String, CryptoKey>,
    rng: SystemRandom,
}

impl KeyManager {
    /// Create a new key manager
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            rng: SystemRandom::new(),
        }
    }

    /// Generate a new Ed25519 key pair
    pub fn generate_ed25519_key(&mut self, key_id: String) -> Result<CryptoKey> {
        let pkcs8_bytes = Ed25519KeyPair::generate_pkcs8(&self.rng)
            .map_err(|_| Error::Crypto(ring::error::Unspecified))?;
        let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())
            .map_err(|_| Error::Crypto(ring::error::Unspecified))?;

        let public_key = key_pair.public_key().as_ref().to_vec();
        let private_key = pkcs8_bytes.as_ref().to_vec();

        let crypto_key = CryptoKey {
            key_type: KeyType::Ed25519,
            public_key,
            private_key: Some(private_key),
            key_id: key_id.clone(),
        };

        self.keys.insert(key_id, crypto_key.clone());
        Ok(crypto_key)
    }

    /// Generate a new AES-256-GCM key
    pub fn generate_aes_key(&mut self, key_id: String) -> Result<CryptoKey> {
        let mut key_bytes = [0u8; 32];
        self.rng.fill(&mut key_bytes)
            .map_err(|_| Error::Crypto(ring::error::Unspecified))?;

        let crypto_key = CryptoKey {
            key_type: KeyType::Aes256Gcm,
            public_key: key_bytes.to_vec(),
            private_key: Some(key_bytes.to_vec()),
            key_id: key_id.clone(),
        };

        self.keys.insert(key_id, crypto_key.clone());
        Ok(crypto_key)
    }

    /// Get a key by ID
    pub fn get_key(&self, key_id: &str) -> Option<&CryptoKey> {
        self.keys.get(key_id)
    }

    /// Add an existing key
    pub fn add_key(&mut self, key: CryptoKey) {
        self.keys.insert(key.key_id.clone(), key);
    }

    /// Remove a key
    pub fn remove_key(&mut self, key_id: &str) -> Option<CryptoKey> {
        self.keys.remove(key_id)
    }

    /// List all key IDs
    pub fn list_key_ids(&self) -> Vec<String> {
        self.keys.keys().cloned().collect()
    }
}

impl Default for KeyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Encryption/decryption utilities
pub struct CryptoUtils;

impl CryptoUtils {
    /// Encrypt data with AES-256-GCM
    pub fn encrypt_aes256gcm(
        data: &[u8],
        key: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>> {
        if key.len() != 32 {
            return Err(Error::Validation("AES-256-GCM key must be 32 bytes".to_string()));
        }

        let unbound_key = UnboundKey::new(&AES_256_GCM, key)
            .map_err(|_| Error::Crypto(ring::error::Unspecified))?;
        let less_safe_key = LessSafeKey::new(unbound_key);

        let mut nonce_bytes = [0u8; 12];
        SystemRandom::new().fill(&mut nonce_bytes)
            .map_err(|_| Error::Crypto(ring::error::Unspecified))?;
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);

        let mut ciphertext = data.to_vec();
        let tag = less_safe_key.seal_in_place_separate_tag(
            nonce,
            Aad::from(aad),
            &mut ciphertext,
        ).map_err(|_| Error::Crypto(ring::error::Unspecified))?;

        // Prepend nonce and append tag
        let mut result = nonce.as_ref().to_vec();
        result.extend_from_slice(&ciphertext);
        result.extend_from_slice(tag.as_ref());

        Ok(result)
    }

    /// Decrypt data with AES-256-GCM
    pub fn decrypt_aes256gcm(
        encrypted_data: &[u8],
        key: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>> {
        if key.len() != 32 {
            return Err(Error::Validation("AES-256-GCM key must be 32 bytes".to_string()));
        }

        if encrypted_data.len() < 28 { // 12 (nonce) + 16 (tag) = 28
            return Err(Error::Validation("Encrypted data too short".to_string()));
        }

        let unbound_key = UnboundKey::new(&AES_256_GCM, key)
            .map_err(|_| Error::Crypto(ring::error::Unspecified))?;
        let less_safe_key = LessSafeKey::new(unbound_key);

        // Extract nonce, ciphertext, and tag
        let nonce = Nonce::try_assume_unique_for_key(&encrypted_data[0..12])
            .map_err(|_| Error::Crypto(ring::error::Unspecified))?;
        let ciphertext_len = encrypted_data.len() - 28;
        let mut ciphertext = encrypted_data[12..12 + ciphertext_len].to_vec();
        let tag_bytes = &encrypted_data[12 + ciphertext_len..];

        // Reconstruct the ciphertext with tag
        ciphertext.extend_from_slice(tag_bytes);

        let plaintext = less_safe_key.open_in_place(
            nonce,
            Aad::from(aad),
            &mut ciphertext,
        ).map_err(|_| Error::Crypto(ring::error::Unspecified))?;

        Ok(plaintext.to_vec())
    }

    /// Sign data with Ed25519
    pub fn sign_ed25519(data: &[u8], private_key: &[u8]) -> Result<Vec<u8>> {
        let key_pair = Ed25519KeyPair::from_pkcs8(private_key)
            .map_err(|_| Error::Crypto(ring::error::Unspecified))?;
        
        let signature = key_pair.sign(data);
        Ok(signature.as_ref().to_vec())
    }

    /// Verify Ed25519 signature
    pub fn verify_ed25519(
        data: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> Result<bool> {
        let public_key = UnparsedPublicKey::new(&ring::signature::ED25519, public_key);
        let result = public_key.verify(data, signature);
        Ok(result.is_ok())
    }

    /// Generate a random nonce
    pub fn generate_nonce() -> Result<[u8; 12]> {
        let mut nonce = [0u8; 12];
        SystemRandom::new().fill(&mut nonce)
            .map_err(|_| Error::Crypto(ring::error::Unspecified))?;
        Ok(nonce)
    }

    /// Generate a random key
    pub fn generate_random_key(size: usize) -> Result<Vec<u8>> {
        let mut key = vec![0u8; size];
        SystemRandom::new().fill(&mut key)
            .map_err(|_| Error::Crypto(ring::error::Unspecified))?;
        Ok(key)
    }

    /// Hash data with SHA-256
    pub fn hash_sha256(data: &[u8]) -> Vec<u8> {
        let digest = digest::digest(&digest::SHA256, data);
        digest.as_ref().to_vec()
    }

    /// Hash data with SHA-512
    pub fn hash_sha512(data: &[u8]) -> Vec<u8> {
        let digest = digest::digest(&digest::SHA512, data);
        digest.as_ref().to_vec()
    }

    /// Derive key from password using PBKDF2
    pub fn derive_key_pbkdf2(
        password: &[u8],
        salt: &[u8],
        iterations: u32,
        key_len: usize,
    ) -> Result<Vec<u8>> {
        use ring::pbkdf2;
        
        let mut key = vec![0u8; key_len];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(iterations).unwrap(),
            salt,
            password,
            &mut key,
        );
        Ok(key)
    }
}

/// Encrypted data structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EncryptedData {
    /// The encrypted data
    pub ciphertext: Vec<u8>,
    /// The encryption algorithm used
    pub algorithm: String,
    /// The key ID used for encryption
    pub key_id: String,
    /// Additional authenticated data
    pub aad: Option<Vec<u8>>,
    /// Timestamp when encrypted
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl EncryptedData {
    /// Create new encrypted data
    pub fn new(
        ciphertext: Vec<u8>,
        algorithm: String,
        key_id: String,
        aad: Option<Vec<u8>>,
    ) -> Self {
        Self {
            ciphertext,
            algorithm,
            key_id,
            aad,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Encrypt data with the given key
    pub fn encrypt(
        data: &[u8],
        key_id: String,
        key: &[u8],
        aad: Option<&[u8]>,
    ) -> Result<Self> {
        let aad_bytes = aad.unwrap_or(&[]);
        let ciphertext = CryptoUtils::encrypt_aes256gcm(data, key, aad_bytes)?;
        
        Ok(Self::new(
            ciphertext,
            "AES-256-GCM".to_string(),
            key_id,
            aad.map(|a| a.to_vec()),
        ))
    }

    /// Decrypt the data with the given key
    pub fn decrypt(&self, key: &[u8]) -> Result<Vec<u8>> {
        if self.algorithm != "AES-256-GCM" {
            return Err(Error::Validation(format!("Unsupported algorithm: {}", self.algorithm)));
        }

        let aad = self.aad.as_deref().unwrap_or(&[]);
        CryptoUtils::decrypt_aes256gcm(&self.ciphertext, key, aad)
    }
}

/// Digital signature structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DigitalSignature {
    /// The signature bytes
    pub signature: Vec<u8>,
    /// The algorithm used
    pub algorithm: String,
    /// The key ID used for signing
    pub key_id: String,
    /// Timestamp when signed
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl DigitalSignature {
    /// Create a new digital signature
    pub fn new(signature: Vec<u8>, algorithm: String, key_id: String) -> Self {
        Self {
            signature,
            algorithm,
            key_id,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Sign data with the given key
    pub fn sign(data: &[u8], key_id: String, private_key: &[u8]) -> Result<Self> {
        let signature = CryptoUtils::sign_ed25519(data, private_key)?;
        
        Ok(Self::new(
            signature,
            "Ed25519".to_string(),
            key_id,
        ))
    }

    /// Verify the signature
    pub fn verify(&self, data: &[u8], public_key: &[u8]) -> Result<bool> {
        if self.algorithm != "Ed25519" {
            return Err(Error::Validation(format!("Unsupported algorithm: {}", self.algorithm)));
        }

        CryptoUtils::verify_ed25519(data, &self.signature, public_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_manager() {
        let mut key_manager = KeyManager::new();
        
        let ed25519_key = key_manager.generate_ed25519_key("test-ed25519".to_string()).unwrap();
        assert_eq!(ed25519_key.key_type, KeyType::Ed25519);
        assert!(ed25519_key.private_key.is_some());

        let aes_key = key_manager.generate_aes_key("test-aes".to_string()).unwrap();
        assert_eq!(aes_key.key_type, KeyType::Aes256Gcm);
        assert!(aes_key.private_key.is_some());
    }

    #[test]
    fn test_encryption_decryption() {
        let data = b"Hello, World!";
        let key = CryptoUtils::generate_random_key(32).unwrap();
        let aad = b"additional data";

        let encrypted = CryptoUtils::encrypt_aes256gcm(data, &key, aad).unwrap();
        let decrypted = CryptoUtils::decrypt_aes256gcm(&encrypted, &key, aad).unwrap();

        assert_eq!(data, decrypted.as_slice());
    }

    #[test]
    fn test_signature_verification() {
        let mut key_manager = KeyManager::new();
        let key = key_manager.generate_ed25519_key("test".to_string()).unwrap();
        
        let data = b"Hello, World!";
        let signature = CryptoUtils::sign_ed25519(data, key.private_key.as_ref().unwrap()).unwrap();
        let is_valid = CryptoUtils::verify_ed25519(data, &signature, &key.public_key).unwrap();

        assert!(is_valid);
    }

    #[test]
    fn test_encrypted_data() {
        let data = b"Secret data";
        let key = CryptoUtils::generate_random_key(32).unwrap();
        let key_id = "test-key".to_string();

        let encrypted = EncryptedData::encrypt(data, key_id.clone(), &key, Some(b"aad")).unwrap();
        let decrypted = encrypted.decrypt(&key).unwrap();

        assert_eq!(data, decrypted.as_slice());
        assert_eq!(encrypted.key_id, key_id);
        assert_eq!(encrypted.algorithm, "AES-256-GCM");
    }

    #[test]
    fn test_digital_signature() {
        let mut key_manager = KeyManager::new();
        let key = key_manager.generate_ed25519_key("test".to_string()).unwrap();
        
        let data = b"Important data";
        let signature = DigitalSignature::sign(data, key.key_id.clone(), key.private_key.as_ref().unwrap()).unwrap();
        let is_valid = signature.verify(data, &key.public_key).unwrap();

        assert!(is_valid);
        assert_eq!(signature.algorithm, "Ed25519");
        assert_eq!(signature.key_id, key.key_id);
    }
}


