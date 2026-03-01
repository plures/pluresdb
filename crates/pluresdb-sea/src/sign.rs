//! SEA sign and verify — ECDSA P-256 with SHA-256 (IEEE P1363 r∥s).
//!
//! ## Wire format
//!
//! Signed envelope (inner JSON, before `"SEA"` prefix):
//! ```json
//! {"m":"<original-data>","s":"<base64url-64-byte-signature>"}
//! ```
//!
//! On the wire the full string is `"SEA" + JSON.stringify(envelope)`.
//! Use [`sea_sign_wire`] to produce the full wire string and
//! [`sea_verify_wire`] to verify it.

use crate::key::{decode_verifying_key, SeaKeyPair};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use p256::ecdsa::{
    signature::{Signer, Verifier},
    Signature,
};
use serde::{Deserialize, Serialize};

/// Signed data envelope — the JSON object inside a `"SEA{...}"` wire string.
///
/// ```json
/// {"m":"<data>","s":"<base64url(r∥s)>"}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeaSignedEnvelope {
    /// The original message that was signed.
    pub m: String,
    /// Base64url-encoded 64-byte ECDSA P-256 signature (IEEE P1363 r∥s).
    pub s: String,
}

/// Sign `data` with `key_pair`'s ECDSA P-256 signing key.
///
/// Internally hashes `data` with SHA-256 before signing (matching GUN SEA
/// and WebCrypto `ECDSA { hash: "SHA-256" }` behaviour).
///
/// Returns a [`SeaSignedEnvelope`] containing the original data and
/// the base64url-encoded signature.
pub fn sea_sign(data: &str, key_pair: &SeaKeyPair) -> Result<SeaSignedEnvelope> {
    let signing_key = key_pair.signing_key()?;

    // `sign()` performs SHA-256 internally (DigestSigner blanket impl).
    let sig: Signature = signing_key.sign(data.as_bytes());

    Ok(SeaSignedEnvelope {
        m: data.to_string(),
        s: URL_SAFE_NO_PAD.encode(sig.to_bytes()),
    })
}

/// Verify a [`SeaSignedEnvelope`] against a base64url-encoded ECDSA P-256
/// public key string.
///
/// Returns `Ok(true)` if the signature is valid, `Ok(false)` on failure.
/// Use the `Result` for encoding / decoding errors and the `bool` for
/// cryptographic validity.
pub fn sea_verify(envelope: &SeaSignedEnvelope, pub_key_b64: &str) -> Result<bool> {
    let verifying_key = decode_verifying_key(pub_key_b64)?;

    let sig_bytes = URL_SAFE_NO_PAD
        .decode(&envelope.s)
        .context("decode signature base64url")?;
    let sig = Signature::try_from(sig_bytes.as_slice()).context("parse P-256 signature bytes")?;

    Ok(verifying_key
        .verify(envelope.m.as_bytes(), &sig)
        .is_ok())
}

/// Sign `data` and return the full GUN SEA wire string (`"SEA{...}"`).
pub fn sea_sign_wire(data: &str, key_pair: &SeaKeyPair) -> Result<String> {
    let envelope = sea_sign(data, key_pair)?;
    let inner = serde_json::to_string(&envelope).context("serialize SeaSignedEnvelope")?;
    Ok(crate::sea_wrap(&inner))
}

/// Verify a full GUN SEA wire string (`"SEA{...}"`) against `pub_key_b64`.
///
/// Returns `Ok(true)` on a valid signature.
pub fn sea_verify_wire(wire: &str, pub_key_b64: &str) -> Result<bool> {
    let inner = crate::sea_unwrap(wire)
        .ok_or_else(|| anyhow::anyhow!("expected 'SEA' prefix, got: {:?}", wire))?;
    let envelope: SeaSignedEnvelope =
        serde_json::from_str(inner).context("deserialize SeaSignedEnvelope")?;
    sea_verify(&envelope, pub_key_b64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SeaKeyPair;

    #[test]
    fn test_sign_and_verify_roundtrip() {
        let pair = SeaKeyPair::generate();
        let data = "Hello, GUN SEA!";
        let envelope = sea_sign(data, &pair).unwrap();
        assert_eq!(envelope.m, data);
        assert!(sea_verify(&envelope, &pair.pub_key).unwrap());
    }

    #[test]
    fn test_verify_rejects_tampered_message() {
        let pair = SeaKeyPair::generate();
        let envelope = sea_sign("original", &pair).unwrap();
        let tampered = SeaSignedEnvelope {
            m: "tampered".to_string(),
            s: envelope.s.clone(),
        };
        assert!(!sea_verify(&tampered, &pair.pub_key).unwrap());
    }

    #[test]
    fn test_verify_rejects_wrong_key() {
        let pair_a = SeaKeyPair::generate();
        let pair_b = SeaKeyPair::generate();
        let envelope = sea_sign("data", &pair_a).unwrap();
        assert!(!sea_verify(&envelope, &pair_b.pub_key).unwrap());
    }

    #[test]
    fn test_signature_is_64_bytes() {
        let pair = SeaKeyPair::generate();
        let envelope = sea_sign("test", &pair).unwrap();
        let sig_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&envelope.s)
            .unwrap();
        // IEEE P1363: r (32 bytes) || s (32 bytes) = 64 bytes.
        assert_eq!(sig_bytes.len(), 64);
    }

    #[test]
    fn test_envelope_json_shape() {
        let pair = SeaKeyPair::generate();
        let envelope = sea_sign("hello", &pair).unwrap();
        let json = serde_json::to_value(&envelope).unwrap();
        assert!(json["m"].is_string());
        assert!(json["s"].is_string());
    }

    #[test]
    fn test_sign_wire_and_verify_wire() {
        let pair = SeaKeyPair::generate();
        let wire = sea_sign_wire("wire data", &pair).unwrap();
        assert!(wire.starts_with("SEA"));
        assert!(sea_verify_wire(&wire, &pair.pub_key).unwrap());
    }

    #[test]
    fn test_verify_wire_rejects_missing_prefix() {
        let pair = SeaKeyPair::generate();
        let result = sea_verify_wire("not-a-sea-string", &pair.pub_key);
        assert!(result.is_err());
    }
}
