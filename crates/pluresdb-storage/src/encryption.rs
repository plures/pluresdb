//! Encryption at rest for PluresDB storage.
//!
//! This module provides AES-256-GCM encryption for WAL segments and stored data,
//! with support for key rotation and device revocation.

use aes_gcm::{
    aead::{generic_array::typenum, rand_core::RngCore, Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
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
        let salt_string = SaltString::encode_b64(salt_bytes)
            .map_err(|e| anyhow::anyhow!("Failed to encode salt: {}", e))?;

        // Derive key from password using Argon2id
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt_string)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;

        // Extract the derived key (first 32 bytes of the hash output)
        let hash_output = password_hash.hash.context("No hash output from Argon2")?;
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
    ///
    /// This updates the in-memory key and salt only.  Any previously encrypted
    /// blobs **cannot** be decrypted with the new key; use
    /// [`rotate_key_and_reencrypt_blocks`] to atomically rotate and re-encrypt
    /// existing data.
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

    /// Rotates the master key and re-encrypts a set of existing ciphertext blobs.
    ///
    /// Each blob in `blocks` must have been produced by [`encrypt`] with the
    /// **current** key.  The method:
    ///
    /// 1. Decrypts every block with the current (old) key.
    /// 2. Derives a fresh key from `new_password` (new random salt).
    /// 3. Re-encrypts every block with the new key.
    /// 4. Updates `self` to hold the new key and salt.
    ///
    /// On success the returned `Vec` has the same length as `blocks` and each
    /// element is the re-encrypted counterpart of the corresponding input blob.
    /// The operation is all-or-nothing: if any block fails to decrypt or
    /// re-encrypt, `self` is left unchanged and an error is returned.
    ///
    /// # Errors
    /// Returns an error if encryption is disabled, any block cannot be
    /// decrypted with the current key, or key derivation fails.
    pub fn rotate_key_and_reencrypt_blocks(
        &mut self,
        new_password: &str,
        blocks: &[Vec<u8>],
    ) -> Result<Vec<Vec<u8>>> {
        if !self.enabled {
            anyhow::bail!("cannot rotate key: encryption is disabled");
        }

        // --- Phase 1: decrypt all blocks with the current (old) key ----------
        let plaintexts: Result<Vec<Vec<u8>>> = blocks.iter().map(|b| self.decrypt(b)).collect();
        let plaintexts =
            plaintexts.context("failed to decrypt one or more blocks with the current key")?;

        // --- Phase 2: derive the new key (do NOT update self yet) ------------
        let mut new_salt = [0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut new_salt);
        let new_config = Self::from_password_with_salt(new_password, &new_salt)
            .context("failed to derive new key from password")?;

        // --- Phase 3: re-encrypt all plaintexts with the new key -------------
        let new_ciphertexts: Result<Vec<Vec<u8>>> =
            plaintexts.iter().map(|p| new_config.encrypt(p)).collect();
        let new_ciphertexts =
            new_ciphertexts.context("failed to re-encrypt one or more blocks with the new key")?;

        // --- Phase 4: commit the new key only after all blocks succeeded -----
        self.master_key = new_config.master_key;
        self.salt = new_config.salt;

        Ok(new_ciphertexts)
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
        let nonce: Nonce<typenum::U12> = nonce_bytes.into();

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
        let nonce_arr: [u8; NONCE_SIZE] = nonce_bytes
            .try_into()
            .expect("slice is exactly NONCE_SIZE bytes");
        let nonce: Nonce<typenum::U12> = nonce_arr.into();

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
    /// Creates a disabled encryption config.
    ///
    /// WARNING: This config has zero-filled keys and should NEVER be used for actual encryption.
    /// It exists only for compatibility and testing purposes with encryption explicitly disabled.
    fn default() -> Self {
        Self {
            master_key: [0u8; KEY_SIZE],
            salt: [0u8; SALT_SIZE],
            enabled: false, // Explicitly disabled to prevent accidental use
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
        let salt_b64 = BASE64.encode(config.salt);
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

        let metadata: Self =
            serde_json::from_str(&contents).context("Failed to parse metadata JSON")?;

        Ok(metadata)
    }

    /// Saves encryption metadata to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self).context("Failed to serialize metadata")?;

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
        BASE64
            .decode(&self.salt)
            .map_err(|e| anyhow::anyhow!("Failed to decode salt: {}", e))
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
        assert_ne!(
            ciphertext, plaintext,
            "Ciphertext should differ from plaintext"
        );

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
    fn test_rotate_key_and_reencrypt_blocks_round_trip() {
        let mut config = EncryptionConfig::from_password("old-password").unwrap();

        let plaintexts: Vec<&[u8]> = vec![
            b"block one payload",
            b"block two payload",
            b"block three payload",
        ];

        // Encrypt each block with the old key
        let old_ciphertexts: Vec<Vec<u8>> = plaintexts
            .iter()
            .map(|p| config.encrypt(p).unwrap())
            .collect();

        // Rotate and re-encrypt
        let new_ciphertexts = config
            .rotate_key_and_reencrypt_blocks("new-password", &old_ciphertexts)
            .unwrap();

        assert_eq!(new_ciphertexts.len(), plaintexts.len());

        // New ciphertexts must decrypt correctly with the new key
        for (i, ct) in new_ciphertexts.iter().enumerate() {
            let decrypted = config.decrypt(ct).unwrap();
            assert_eq!(decrypted, plaintexts[i]);
        }
    }

    #[test]
    fn test_rotate_key_and_reencrypt_old_ciphertexts_invalid_with_new_key() {
        let mut config = EncryptionConfig::from_password("old-password").unwrap();

        let plaintext = b"sensitive payload";
        let old_ciphertext = config.encrypt(plaintext).unwrap();

        // Rotate — old_ciphertext is now stale
        config
            .rotate_key_and_reencrypt_blocks("new-password", std::slice::from_ref(&old_ciphertext))
            .unwrap();

        // Attempting to decrypt the old (pre-rotation) ciphertext with the
        // rotated key must fail
        let result = config.decrypt(&old_ciphertext);
        assert!(
            result.is_err(),
            "old ciphertext must not be decryptable with the rotated key"
        );
    }

    #[test]
    fn test_rotate_key_and_reencrypt_blocks_disabled_returns_error() {
        let mut config = EncryptionConfig::default(); // disabled
        let result = config.rotate_key_and_reencrypt_blocks("new-password", &[]);
        assert!(
            result.is_err(),
            "rotation on a disabled config must return an error"
        );
    }

    #[test]
    fn test_rotate_key_and_reencrypt_blocks_wrong_key_returns_error() {
        let mut config1 = EncryptionConfig::from_password("password-a").unwrap();
        let config2 = EncryptionConfig::from_password("password-b").unwrap();

        // Encrypt with config2 (different key)
        let foreign_ciphertext = config2.encrypt(b"foreign data").unwrap();

        // Attempt to rotate config1 using a block encrypted with config2
        let result = config1.rotate_key_and_reencrypt_blocks("new-password", &[foreign_ciphertext]);
        assert!(
            result.is_err(),
            "rotation must fail when a block was encrypted with a different key"
        );
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

    #[test]
    fn test_default_config_is_disabled() {
        let config = EncryptionConfig::default();
        assert!(
            !config.is_enabled(),
            "Default config should be disabled to prevent accidental use"
        );
    }

    // ----------------------------------------------------------------------
    // Mutation-hardening tests (Level-0 #6): pin exact behavioral boundaries
    // so cargo-mutants survivors in decrypt() and disable() are killed.
    // ----------------------------------------------------------------------

    /// L80 `from_password_with_salt`: the guard `hash_bytes.len() < KEY_SIZE`
    /// rejects an Argon2 output shorter than the AES-256 key size before the
    /// `copy_from_slice(&hash_bytes[..KEY_SIZE])` that would otherwise panic.
    ///
    /// This test pins the invariant that a password-derived config is fully
    /// usable, which is only possible if the guard let a >= KEY_SIZE hash
    /// through and exactly KEY_SIZE bytes were copied into the key.
    ///
    /// NOTE (equivalent-mutant disclosure): the `< -> >` mutant at L80 is a
    /// GENUINE EQUIVALENT mutant via the public API. `Argon2::default()` here
    /// always emits a PHC hash of exactly `KEY_SIZE` (32) bytes, so the only
    /// reachable comparison is `32 <op> 32`, which is `false` for BOTH `<` and
    /// `>`. The guard therefore never fires on any input the public surface can
    /// construct, and no behavioral test can distinguish `<` from `>`. It is
    /// retained as defense-in-depth against a future params change that sets a
    /// shorter output length. Documented as unkillable-by-design rather than
    /// papered over with a stub.
    #[test]
    fn test_from_password_with_salt_derives_full_length_usable_key() {
        let salt = [7u8; SALT_SIZE];
        let config = EncryptionConfig::from_password_with_salt("boundary-pw", &salt)
            .expect("valid salt length must derive a key");

        // A full KEY_SIZE key must have been copied: round-trip must succeed.
        let plaintext = b"derived-key-length-invariant";
        let ct = config.encrypt(plaintext).unwrap();
        let pt = config.decrypt(&ct).unwrap();
        assert_eq!(pt, plaintext);

        // The salt the guard accepted must be preserved verbatim.
        assert_eq!(config.salt(), &salt);
    }

    /// Companion to the L80 guard: a salt of the WRONG length is rejected up
    /// front (`salt_bytes.len() != SALT_SIZE`). This pins the sibling length
    /// contract so the function's input validation stays behaviorally tested.
    #[test]
    fn test_from_password_with_salt_rejects_wrong_salt_length() {
        let short_salt = [0u8; SALT_SIZE - 1];
        let long_salt = [0u8; SALT_SIZE + 1];
        assert!(
            EncryptionConfig::from_password_with_salt("pw", &short_salt).is_err(),
            "salt shorter than SALT_SIZE must be rejected"
        );
        assert!(
            EncryptionConfig::from_password_with_salt("pw", &long_salt).is_err(),
            "salt longer than SALT_SIZE must be rejected"
        );
    }

    /// L202 `decrypt`: the length guard is `ciphertext_with_nonce.len() < NONCE_SIZE`.
    /// An input of EXACTLY `NONCE_SIZE` (12) bytes must PASS the length guard
    /// (12 < 12 == false) and fall through to the AEAD layer, which then fails
    /// because there is no ciphertext/tag — yielding the *AEAD* error
    /// ("Decryption failed"), NOT the length-guard error ("too short ... nonce").
    ///
    /// Mutants `< -> <=` and `< -> ==` both make `12 <op> 12 == true`, so they
    /// bail at the length guard with the "too short to contain nonce" message.
    /// Asserting the boundary input does NOT produce the length-guard message
    /// (and DOES surface the AEAD failure) kills both mutants.
    #[test]
    fn test_decrypt_exact_nonce_len_passes_length_guard_then_aead_fails() {
        let config = EncryptionConfig::new().unwrap();
        // Exactly NONCE_SIZE bytes: valid nonce, empty ciphertext, no auth tag.
        let boundary_input = [0u8; NONCE_SIZE];
        assert_eq!(boundary_input.len(), NONCE_SIZE);

        let err = config
            .decrypt(&boundary_input)
            .expect_err("empty ciphertext (no auth tag) must fail to decrypt");
        let msg = err.to_string();

        // Original (`<`): falls through to AEAD -> "Decryption failed".
        // Mutants (`<=`/`==`): bail at guard -> "... too short to contain nonce".
        assert!(
            !msg.contains("too short to contain nonce"),
            "input of exactly NONCE_SIZE must NOT be rejected by the length guard; \
             got length-guard error instead of AEAD error: {msg}"
        );
        assert!(
            msg.contains("Decryption failed"),
            "boundary input must reach the AEAD layer and fail there; got: {msg}"
        );
    }

    /// L202 `decrypt`: an input of `NONCE_SIZE - 1` (11) bytes is genuinely too
    /// short to hold a nonce and MUST be rejected by the length guard with the
    /// "too short to contain nonce" message (original `<`: 11 < 12 == true).
    ///
    /// Mutant `< -> ==` makes `11 == 12 == false`, so it would *skip* the guard
    /// and call `split_at(12)` on an 11-byte slice — which panics. Asserting a
    /// clean `Err` carrying the length-guard message (no panic) kills `==`.
    #[test]
    fn test_decrypt_below_nonce_len_is_rejected_by_length_guard() {
        let config = EncryptionConfig::new().unwrap();
        let too_short = [0u8; NONCE_SIZE - 1];
        assert_eq!(too_short.len(), NONCE_SIZE - 1);

        let err = config
            .decrypt(&too_short)
            .expect_err("input shorter than the nonce must be rejected");
        assert!(
            err.to_string().contains("too short to contain nonce"),
            "sub-nonce input must hit the length guard; got: {err}"
        );
    }

    /// L231 `disable`: must have an OBSERVABLE effect. Starting from an ENABLED
    /// config, `disable()` must flip `is_enabled()` to false AND change the
    /// encrypt path into the plaintext-passthrough no-op.
    ///
    /// The mutant replaces the body with `()` (no-op), leaving `enabled == true`,
    /// so both the state assertion and the passthrough assertion fail.
    /// (The existing `test_encryption_disabled` starts from an already-disabled
    /// `default()` config and therefore does NOT kill this mutant.)
    #[test]
    fn test_disable_has_observable_effect_from_enabled_config() {
        let mut config = EncryptionConfig::new().unwrap();
        assert!(
            config.is_enabled(),
            "precondition: a freshly created config is enabled"
        );

        // Sanity: while enabled, encrypt produces real ciphertext (!= plaintext).
        let plaintext = b"observable-disable-probe";
        let enabled_ct = config.encrypt(plaintext).unwrap();
        assert_ne!(
            enabled_ct.as_slice(),
            plaintext.as_slice(),
            "precondition: enabled encrypt must transform the plaintext"
        );

        config.disable();

        // State flip must be observable.
        assert!(
            !config.is_enabled(),
            "disable() must flip is_enabled() to false (mutant no-op leaves it true)"
        );

        // Behavioral flip: disabled encrypt is an identity passthrough.
        let disabled_ct = config.encrypt(plaintext).unwrap();
        assert_eq!(
            disabled_ct.as_slice(),
            plaintext.as_slice(),
            "after disable(), encrypt must pass plaintext through unchanged"
        );
    }
}
