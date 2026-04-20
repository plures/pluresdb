//! SEA key pair generation and encoding.

use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use p256::{
    ecdh::EphemeralSecret,
    ecdsa::{SigningKey, VerifyingKey},
    elliptic_curve::sec1::ToEncodedPoint,
    EncodedPoint, PublicKey, SecretKey,
};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};

/// GUN SEA key pair.
///
/// Holds two P-256 key pairs:
/// - **Signing pair** (`pub` / `priv`): ECDSA P-256 for `SEA.sign` / `SEA.verify`.
/// - **Encryption pair** (`epub` / `epriv`): ECDH P-256 for `SEA.encrypt` / `SEA.decrypt`.
///
/// All fields are base64url-encoded (no padding) byte strings matching the
/// GUN SEA wire format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeaKeyPair {
    /// ECDSA P-256 public key – uncompressed SEC1 point (65 bytes, base64url).
    #[serde(rename = "pub")]
    pub pub_key: String,

    /// ECDSA P-256 private key – secret scalar (32 bytes, base64url).
    #[serde(rename = "priv")]
    pub priv_key: String,

    /// ECDH P-256 public key – uncompressed SEC1 point (65 bytes, base64url).
    pub epub: String,

    /// ECDH P-256 private key – secret scalar (32 bytes, base64url).
    pub epriv: String,
}

impl SeaKeyPair {
    /// Generate a fresh [`SeaKeyPair`] using the OS CSPRNG.
    pub fn generate() -> Self {
        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        let enc_secret = SecretKey::random(&mut OsRng);
        let enc_public = enc_secret.public_key();

        let priv_bytes = signing_key.to_bytes();
        let pub_bytes = verifying_key.to_encoded_point(false); // uncompressed

        let epriv_bytes = enc_secret.to_bytes();
        let epub_bytes = enc_public.to_encoded_point(false); // uncompressed

        Self {
            pub_key: URL_SAFE_NO_PAD.encode(pub_bytes.as_bytes()),
            priv_key: URL_SAFE_NO_PAD.encode(priv_bytes),
            epub: URL_SAFE_NO_PAD.encode(epub_bytes.as_bytes()),
            epriv: URL_SAFE_NO_PAD.encode(epriv_bytes),
        }
    }

    /// Decode the ECDSA signing key from `priv_key`.
    pub(crate) fn signing_key(&self) -> Result<SigningKey> {
        let bytes = URL_SAFE_NO_PAD
            .decode(&self.priv_key)
            .context("decode priv_key base64url")?;
        SigningKey::from_bytes(bytes.as_slice().into())
            .context("reconstruct SigningKey from priv_key bytes")
    }

    /// Decode the ECDSA verifying key from `pub_key`.
    ///
    /// Convenience wrapper around [`decode_verifying_key`] for use within the crate.
    #[allow(dead_code)]
    pub(crate) fn verifying_key(&self) -> Result<VerifyingKey> {
        decode_verifying_key(&self.pub_key)
    }

    /// Decode the ECDH public key from `epub`.
    ///
    /// Convenience wrapper around [`decode_epub`] for use within the crate.
    #[allow(dead_code)]
    pub(crate) fn epub_public_key(&self) -> Result<PublicKey> {
        decode_epub(&self.epub)
    }

    /// Decode the ECDH private key from `epriv` and return its scalar.
    pub(crate) fn epriv_secret(&self) -> Result<SecretKey> {
        let bytes = URL_SAFE_NO_PAD
            .decode(&self.epriv)
            .context("decode epriv base64url")?;
        SecretKey::from_bytes(bytes.as_slice().into())
            .context("reconstruct SecretKey from epriv bytes")
    }
}

/// Decode a base64url-encoded verifying key string.
pub(crate) fn decode_verifying_key(pub_key_b64: &str) -> Result<VerifyingKey> {
    let bytes = URL_SAFE_NO_PAD
        .decode(pub_key_b64)
        .context("decode pub_key base64url")?;
    let point = EncodedPoint::from_bytes(&bytes).context("parse EncodedPoint from pub_key")?;
    VerifyingKey::from_encoded_point(&point).context("reconstruct VerifyingKey")
}

/// Decode a base64url-encoded ECDH public key string.
pub(crate) fn decode_epub(epub_b64: &str) -> Result<PublicKey> {
    let bytes = URL_SAFE_NO_PAD
        .decode(epub_b64)
        .context("decode epub base64url")?;
    PublicKey::from_sec1_bytes(&bytes).context("reconstruct PublicKey from epub")
}

/// Create a fresh ephemeral ECDH secret (for one-shot encrypt/decrypt paths
/// where both sides have long-term key pairs and no ephemeral exchange is needed
/// — use `sea_encrypt` directly with the pair's `epriv`).
#[allow(dead_code)]
pub(crate) fn ephemeral_ecdh_secret() -> EphemeralSecret {
    EphemeralSecret::random(&mut OsRng)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_key_pair() {
        let pair = SeaKeyPair::generate();
        assert!(!pair.pub_key.is_empty());
        assert!(!pair.priv_key.is_empty());
        assert!(!pair.epub.is_empty());
        assert!(!pair.epriv.is_empty());
    }

    #[test]
    fn test_key_pair_round_trip_json() {
        let pair = SeaKeyPair::generate();
        let json = serde_json::to_string(&pair).unwrap();
        let decoded: SeaKeyPair = serde_json::from_str(&json).unwrap();
        assert_eq!(pair, decoded);
    }

    #[test]
    fn test_signing_key_recovery() {
        let pair = SeaKeyPair::generate();
        pair.signing_key().unwrap();
    }

    #[test]
    fn test_verifying_key_recovery() {
        let pair = SeaKeyPair::generate();
        pair.verifying_key().unwrap();
    }

    #[test]
    fn test_epriv_secret_recovery() {
        let pair = SeaKeyPair::generate();
        pair.epriv_secret().unwrap();
    }

    #[test]
    fn test_epub_public_key_recovery() {
        let pair = SeaKeyPair::generate();
        pair.epub_public_key().unwrap();
    }

    #[test]
    fn test_two_key_pairs_are_distinct() {
        let a = SeaKeyPair::generate();
        let b = SeaKeyPair::generate();
        assert_ne!(a.pub_key, b.pub_key);
        assert_ne!(a.priv_key, b.priv_key);
    }

    #[test]
    fn test_pub_key_is_65_bytes_uncompressed() {
        let pair = SeaKeyPair::generate();
        let bytes = URL_SAFE_NO_PAD.decode(&pair.pub_key).unwrap();
        // Uncompressed P-256 point: 0x04 || x (32) || y (32) = 65 bytes.
        assert_eq!(bytes.len(), 65);
        assert_eq!(bytes[0], 0x04);
    }

    #[test]
    fn test_priv_key_is_32_bytes() {
        let pair = SeaKeyPair::generate();
        let bytes = URL_SAFE_NO_PAD.decode(&pair.priv_key).unwrap();
        assert_eq!(bytes.len(), 32);
    }
}
