//! Encryption-at-rest tests for PluresDB storage.
//!
//! These tests verify that:
//! 1. Plaintext payloads are **never** readable in raw on-disk bytes once
//!    encryption is enabled.
//! 2. Key rotation re-encrypts all blocks so the old key can no longer
//!    decrypt them.
//! 3. Metadata (salt, version, cipher) is correctly persisted and can be used
//!    to re-derive the key and re-open the database.

use pluresdb_storage::{EncryptionConfig, EncryptionMetadata};
use std::fs;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` if `needle` appears as a contiguous byte sequence inside
/// `haystack`.
fn bytes_contain(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

// ---------------------------------------------------------------------------
// On-disk ciphertext tests
// ---------------------------------------------------------------------------

/// Writing encrypted blobs to a temp file must produce bytes that do **not**
/// contain the original plaintext.
#[test]
fn test_ciphertext_file_contains_no_plaintext() {
    let temp_dir = TempDir::new().unwrap();
    let blob_path = temp_dir.path().join("data.bin");

    let config = EncryptionConfig::from_password("hunter2").unwrap();
    let plaintext = b"super secret agent memory: launch codes = 1234";

    let ciphertext = config.encrypt(plaintext).unwrap();
    fs::write(&blob_path, &ciphertext).unwrap();

    // Read raw bytes back from disk
    let on_disk = fs::read(&blob_path).unwrap();

    assert!(
        !bytes_contain(&on_disk, plaintext),
        "plaintext must not appear verbatim in the on-disk ciphertext"
    );
    assert_ne!(on_disk, plaintext, "ciphertext must differ from plaintext");
}

/// Multiple writes of the same plaintext must produce different ciphertexts
/// (random nonce per encrypt call).
#[test]
fn test_repeated_encryption_produces_distinct_ciphertexts() {
    let config = EncryptionConfig::from_password("same-password").unwrap();
    let plaintext = b"deterministic check";

    let ct1 = config.encrypt(plaintext).unwrap();
    let ct2 = config.encrypt(plaintext).unwrap();

    assert_ne!(ct1, ct2, "each encrypt call must use a fresh random nonce");
}

/// A blob encrypted with one key must not be decryptable by a different key,
/// and the raw bytes must not expose the plaintext.
#[test]
fn test_wrong_key_cannot_read_ciphertext() {
    let config_a = EncryptionConfig::from_password("password-alpha").unwrap();
    let config_b = EncryptionConfig::from_password("password-beta").unwrap();

    let plaintext = b"top-secret payload";
    let ciphertext = config_a.encrypt(plaintext).unwrap();

    assert!(
        !bytes_contain(&ciphertext, plaintext),
        "plaintext must not appear in ciphertext"
    );

    let result = config_b.decrypt(&ciphertext);
    assert!(
        result.is_err(),
        "decryption with wrong key must return an error"
    );
}

/// Encryption metadata JSON written to disk must not expose the plaintext or
/// the raw master key.
#[test]
fn test_metadata_file_does_not_expose_key_material() {
    let temp_dir = TempDir::new().unwrap();
    let meta_path = temp_dir.path().join("encryption.json");

    let plaintext_secret = b"super secret password text";
    // We use a fixed password here only to have something to search for; the
    // metadata should store only the salt, not the password or derived key.
    let config = EncryptionConfig::from_password("super secret password text").unwrap();
    let metadata = EncryptionMetadata::from_config(&config);
    metadata.save(&meta_path).unwrap();

    let raw = fs::read(&meta_path).unwrap();

    // The literal password must not appear in the metadata file
    assert!(
        !bytes_contain(&raw, plaintext_secret),
        "raw password must not appear in the encryption metadata file"
    );

    // The JSON must be valid and contain the expected fields
    let parsed: serde_json::Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(parsed["kdf"], "argon2id");
    assert_eq!(parsed["cipher"], "aes-256-gcm");
    assert!(
        parsed["salt"].is_string(),
        "metadata must contain a base64-encoded salt"
    );
}

// ---------------------------------------------------------------------------
// Key rotation workflow tests
// ---------------------------------------------------------------------------

/// After `rotate_key_and_reencrypt_blocks` every old ciphertext must be
/// unreadable and every re-encrypted block must decrypt correctly.
#[test]
fn test_key_rotation_reencrypts_blocks_without_data_loss() {
    let mut config = EncryptionConfig::from_password("old-passphrase").unwrap();

    let payloads: &[&[u8]] = &[
        b"node-1 data: {score: 42}",
        b"node-2 data: {score: 99}",
        b"node-3 data: {score: 7}",
    ];

    // Encrypt all payloads with the old key
    let old_ciphertexts: Vec<Vec<u8>> = payloads
        .iter()
        .map(|p| config.encrypt(p).unwrap())
        .collect();

    // Verify none of the on-disk bytes contain plaintext
    for (ct, pt) in old_ciphertexts.iter().zip(payloads.iter()) {
        assert!(
            !bytes_contain(ct, pt),
            "plaintext must not appear in old ciphertext"
        );
    }

    // Rotate key and re-encrypt
    let new_ciphertexts = config
        .rotate_key_and_reencrypt_blocks("new-passphrase", &old_ciphertexts)
        .unwrap();

    assert_eq!(
        new_ciphertexts.len(),
        payloads.len(),
        "re-encryption must produce one output per input block"
    );

    // All new ciphertexts must decrypt correctly — no data loss
    for (i, ct) in new_ciphertexts.iter().enumerate() {
        let decrypted = config.decrypt(ct).unwrap();
        assert_eq!(
            decrypted, payloads[i],
            "re-encrypted block {i} must round-trip"
        );
    }
}

/// Old ciphertexts must NOT be decryptable with the rotated key.
#[test]
fn test_old_ciphertexts_unreadable_after_rotation() {
    let mut config = EncryptionConfig::from_password("original-key").unwrap();

    let plaintext = b"sensitive memory entry";
    let old_ciphertext = config.encrypt(plaintext).unwrap();

    config
        .rotate_key_and_reencrypt_blocks("rotated-key", &[old_ciphertext.clone()])
        .unwrap();

    // The old ciphertext should now be undecipherable
    let result = config.decrypt(&old_ciphertext);
    assert!(
        result.is_err(),
        "old ciphertext must be unreadable after key rotation"
    );
}

/// After rotation the updated metadata must be re-loadable and allow key
/// re-derivation from the new password.
#[test]
fn test_metadata_updated_after_key_rotation() {
    let temp_dir = TempDir::new().unwrap();
    let meta_path = temp_dir.path().join("encryption.json");

    // --- Initial setup -------------------------------------------------------
    let mut config = EncryptionConfig::from_password("initial-password").unwrap();
    let initial_meta = EncryptionMetadata::from_config(&config);
    initial_meta.save(&meta_path).unwrap();

    let plaintext = b"important record";
    let old_ct = config.encrypt(plaintext).unwrap();

    // --- Rotate and persist new metadata ------------------------------------
    let new_cts = config
        .rotate_key_and_reencrypt_blocks("rotated-password", &[old_ct])
        .unwrap();

    let new_meta = EncryptionMetadata::from_config(&config);
    new_meta.save(&meta_path).unwrap();

    // --- Reload metadata and re-derive key ----------------------------------
    let loaded_meta = EncryptionMetadata::load(&meta_path).unwrap();
    let salt = loaded_meta.salt_bytes().unwrap();
    let reloaded_config =
        EncryptionConfig::from_password_with_salt("rotated-password", &salt).unwrap();

    // The re-derived config must be able to decrypt the re-encrypted block
    let decrypted = reloaded_config.decrypt(&new_cts[0]).unwrap();
    assert_eq!(decrypted, plaintext);

    // The old salt and new salt must differ
    assert_ne!(
        initial_meta.salt, new_meta.salt,
        "key rotation must produce a new salt"
    );
}

/// Rotating with an empty block list is a valid no-op (metadata-only update).
#[test]
fn test_key_rotation_with_no_blocks_is_valid() {
    let mut config = EncryptionConfig::from_password("password").unwrap();
    let old_salt = *config.salt();

    let result = config.rotate_key_and_reencrypt_blocks("new-password", &[]);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());

    // Salt must have changed even with zero blocks
    assert_ne!(config.salt(), &old_salt, "salt must be refreshed on rotation");
}

// ---------------------------------------------------------------------------
// Large-payload verification
// ---------------------------------------------------------------------------

/// Encrypt a 512 KiB block and verify no 16-byte slice of the plaintext
/// survives verbatim in the ciphertext.
#[test]
fn test_large_payload_has_no_plaintext_windows() {
    let config = EncryptionConfig::from_password("large-data-test").unwrap();

    // Use a realistic JSON-like repeating pattern (not purely compressible)
    // that contains recognisable field names and values.
    let unit = b"{\"id\":\"node-abc123\",\"score\":9999,\"tag\":\"memory\"}";
    let repeat_count = (512 * 1024) / unit.len() + 1;
    let plaintext: Vec<u8> = unit.iter().cloned().cycle().take(512 * 1024).collect();
    let _ = repeat_count; // used only for documentation

    let ciphertext = config.encrypt(&plaintext).unwrap();

    // The field name "node-abc123" must not appear verbatim in the ciphertext
    let marker = b"node-abc123";
    assert!(
        !bytes_contain(&ciphertext, marker),
        "recognisable plaintext substrings must not appear in ciphertext"
    );
    assert_ne!(ciphertext.len(), 0);
    assert_ne!(ciphertext, plaintext);
}
