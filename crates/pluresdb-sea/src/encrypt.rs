//! SEA encrypt and decrypt — ECDH P-256 + PBKDF2-SHA-256 + AES-256-GCM.
//!
//! ## Algorithm
//!
//! 1. **Shared secret**: `shared = ECDH(our_epriv, their_epub)` → 32-byte X coordinate.
//! 2. **Key derivation**: `key = PBKDF2-HMAC-SHA256(password=shared, salt, 100_000 iters, 32 bytes)`.
//! 3. **Encryption**: AES-256-GCM with a random 12-byte IV.
//!
//! ## Encrypted wire envelope
//!
//! ```json
//! {"ct":"<base64url(ciphertext)>","iv":"<base64url(12-byte-iv)>","s":"<base64url(16-byte-salt)>"}
//! ```
//!
//! Full wire string: `"SEA" + JSON.stringify(envelope)`.

// `GenericArray::from_slice` / `as_slice` deprecation warnings come from
// transitive p256 / aes-gcm generic-array 0.14 cross-version usage.
#![allow(deprecated)]

use crate::key::SeaKeyPair;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use p256::ecdh;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};

/// Number of PBKDF2 iterations used by GUN SEA for key derivation.
const PBKDF2_ITERATIONS: u32 = 100_000;
/// AES-GCM IV length in bytes (96 bits).
const IV_LEN: usize = 12;
/// PBKDF2 salt length in bytes (128 bits).
const SALT_LEN: usize = 16;
/// AES-256 key length in bytes.
const KEY_LEN: usize = 32;

/// Encrypted data envelope — the JSON object inside a `"SEA{...}"` wire string.
///
/// ```json
/// {"ct":"<base64url-ciphertext>","iv":"<base64url-iv>","s":"<base64url-salt>"}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeaEncryptedEnvelope {
    /// Base64url-encoded AES-256-GCM ciphertext (includes 16-byte GCM tag).
    pub ct: String,
    /// Base64url-encoded 12-byte AES-GCM IV / nonce.
    pub iv: String,
    /// Base64url-encoded 16-byte PBKDF2 salt.
    pub s: String,
}

/// Encrypt `plaintext` for `recipient_pair` from `sender_pair`.
///
/// Both `sender_pair.epriv` and `recipient_pair.epub` are used to derive a
/// shared ECDH secret, which is then passed through PBKDF2 to produce the
/// AES-256-GCM key.  The recipient can decrypt using their own `epriv` and
/// the sender's `epub`.
pub fn sea_encrypt(
    plaintext: &str,
    sender_pair: &SeaKeyPair,
    recipient_epub_b64: &str,
) -> Result<SeaEncryptedEnvelope> {
    let their_pub = crate::key::decode_epub(recipient_epub_b64)?;
    let our_secret = sender_pair.epriv_secret()?;

    // ECDH: derive shared secret from our private key and their public key.
    let shared = ecdh::diffie_hellman(our_secret.to_nonzero_scalar(), their_pub.as_affine());
    let shared_bytes = shared.raw_secret_bytes();

    // Random salt and IV.
    let mut salt = [0u8; SALT_LEN];
    let mut iv = [0u8; IV_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut iv);

    // PBKDF2-SHA256 key derivation.
    let mut key_bytes = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(
        shared_bytes.as_slice(),
        &salt,
        PBKDF2_ITERATIONS,
        &mut key_bytes,
    );

    // AES-256-GCM encryption.
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&iv), plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("AES-GCM encrypt failed: {e}"))?;

    Ok(SeaEncryptedEnvelope {
        ct: URL_SAFE_NO_PAD.encode(&ciphertext),
        iv: URL_SAFE_NO_PAD.encode(iv),
        s: URL_SAFE_NO_PAD.encode(salt),
    })
}

/// Decrypt a [`SeaEncryptedEnvelope`] using `recipient_pair`'s ECDH private
/// key and `sender_epub_b64` (the sender's ECDH public key).
///
/// Returns the original plaintext string.
pub fn sea_decrypt(
    envelope: &SeaEncryptedEnvelope,
    recipient_pair: &SeaKeyPair,
    sender_epub_b64: &str,
) -> Result<String> {
    let their_pub = crate::key::decode_epub(sender_epub_b64)?;
    let our_secret = recipient_pair.epriv_secret()?;

    let shared = ecdh::diffie_hellman(our_secret.to_nonzero_scalar(), their_pub.as_affine());
    let shared_bytes = shared.raw_secret_bytes();

    let salt = URL_SAFE_NO_PAD
        .decode(&envelope.s)
        .context("decode salt base64url")?;
    let iv = URL_SAFE_NO_PAD
        .decode(&envelope.iv)
        .context("decode iv base64url")?;
    let ct = URL_SAFE_NO_PAD
        .decode(&envelope.ct)
        .context("decode ciphertext base64url")?;

    let mut key_bytes = [0u8; KEY_LEN];
    pbkdf2_hmac::<Sha256>(
        shared_bytes.as_slice(),
        &salt,
        PBKDF2_ITERATIONS,
        &mut key_bytes,
    );

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key_bytes));
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&iv), ct.as_slice())
        .map_err(|e| anyhow::anyhow!("AES-GCM decrypt failed (wrong key or tampered): {e}"))?;

    String::from_utf8(plaintext).context("plaintext is not valid UTF-8")
}

/// Encrypt `plaintext` and return the full GUN SEA wire string (`"SEA{...}"`).
pub fn sea_encrypt_wire(
    plaintext: &str,
    sender_pair: &SeaKeyPair,
    recipient_epub_b64: &str,
) -> Result<String> {
    let envelope = sea_encrypt(plaintext, sender_pair, recipient_epub_b64)?;
    let inner = serde_json::to_string(&envelope).context("serialize SeaEncryptedEnvelope")?;
    Ok(crate::sea_wrap(&inner))
}

/// Decrypt a full GUN SEA wire string (`"SEA{...}"`) back to plaintext.
pub fn sea_decrypt_wire(
    wire: &str,
    recipient_pair: &SeaKeyPair,
    sender_epub_b64: &str,
) -> Result<String> {
    let inner = crate::sea_unwrap(wire)
        .ok_or_else(|| anyhow::anyhow!("expected 'SEA' prefix, got: {:?}", wire))?;
    let envelope: SeaEncryptedEnvelope =
        serde_json::from_str(inner).context("deserialize SeaEncryptedEnvelope")?;
    sea_decrypt(&envelope, recipient_pair, sender_epub_b64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SeaKeyPair;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let alice = SeaKeyPair::generate();
        let bob = SeaKeyPair::generate();

        let plaintext = "secret message for bob";
        let envelope = sea_encrypt(plaintext, &alice, &bob.epub).unwrap();

        // Bob decrypts using his epriv and Alice's epub.
        let decrypted = sea_decrypt(&envelope, &bob, &alice.epub).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_rejects_tampered_ciphertext() {
        let alice = SeaKeyPair::generate();
        let bob = SeaKeyPair::generate();

        let mut envelope = sea_encrypt("secret", &alice, &bob.epub).unwrap();
        // Corrupt the ciphertext.
        let mut ct_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&envelope.ct)
            .unwrap();
        ct_bytes[0] ^= 0xFF;
        envelope.ct = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&ct_bytes);

        assert!(sea_decrypt(&envelope, &bob, &alice.epub).is_err());
    }

    #[test]
    fn test_decrypt_rejects_wrong_recipient() {
        let alice = SeaKeyPair::generate();
        let bob = SeaKeyPair::generate();
        let charlie = SeaKeyPair::generate();

        let envelope = sea_encrypt("secret", &alice, &bob.epub).unwrap();

        // Charlie tries to decrypt — wrong key, must fail.
        assert!(sea_decrypt(&envelope, &charlie, &alice.epub).is_err());
    }

    #[test]
    fn test_encrypt_produces_distinct_envelopes() {
        let alice = SeaKeyPair::generate();
        let bob = SeaKeyPair::generate();

        let e1 = sea_encrypt("same", &alice, &bob.epub).unwrap();
        let e2 = sea_encrypt("same", &alice, &bob.epub).unwrap();

        // Each call uses a fresh random IV + salt, so envelopes differ.
        assert_ne!(e1.ct, e2.ct);
        assert_ne!(e1.iv, e2.iv);
        assert_ne!(e1.s, e2.s);
    }

    #[test]
    fn test_encrypt_wire_decrypt_wire_roundtrip() {
        let alice = SeaKeyPair::generate();
        let bob = SeaKeyPair::generate();

        let wire = sea_encrypt_wire("wire plaintext", &alice, &bob.epub).unwrap();
        assert!(wire.starts_with("SEA"));

        let decrypted = sea_decrypt_wire(&wire, &bob, &alice.epub).unwrap();
        assert_eq!(decrypted, "wire plaintext");
    }

    #[test]
    fn test_iv_is_12_bytes() {
        let alice = SeaKeyPair::generate();
        let bob = SeaKeyPair::generate();
        let envelope = sea_encrypt("test", &alice, &bob.epub).unwrap();
        let iv_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&envelope.iv)
            .unwrap();
        assert_eq!(iv_bytes.len(), IV_LEN);
    }

    #[test]
    fn test_salt_is_16_bytes() {
        let alice = SeaKeyPair::generate();
        let bob = SeaKeyPair::generate();
        let envelope = sea_encrypt("test", &alice, &bob.epub).unwrap();
        let salt_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&envelope.s)
            .unwrap();
        assert_eq!(salt_bytes.len(), SALT_LEN);
    }
}
