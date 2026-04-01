//! SEA work function — PBKDF2-SHA-256 key derivation.
//!
//! `SEA.work(data, pair)` is GUN's proof-of-work / key-derivation helper.
//! In the most common form it runs PBKDF2-HMAC-SHA256 over `data` with a
//! deterministic or random salt and returns the derived key as a base64url
//! string.
//!
//! ## Modes
//!
//! | Mode                  | Salt source                     | GUN equivalent           |
//! |-----------------------|---------------------------------|--------------------------|
//! | [`sea_work`]          | caller-supplied salt            | `SEA.work(data, salt)`   |
//! | [`sea_work_random`]   | 16-byte random salt (returns both)| —                      |

use anyhow::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;

/// Default number of PBKDF2 iterations, matching GUN SEA's `SEA.work`.
pub const SEA_WORK_ITERATIONS: u32 = 100_000;
/// Output key length in bytes (256 bits).
pub const SEA_WORK_KEY_LEN: usize = 32;

/// Derive a 32-byte key from `data` and an explicit `salt` using
/// PBKDF2-HMAC-SHA256 at 100 000 iterations.
///
/// Returns the derived key as a base64url string (no padding), matching
/// GUN SEA `work` output encoding.
pub fn sea_work(data: &str, salt: &[u8]) -> String {
    let mut key = [0u8; SEA_WORK_KEY_LEN];
    pbkdf2_hmac::<Sha256>(data.as_bytes(), salt, SEA_WORK_ITERATIONS, &mut key);
    URL_SAFE_NO_PAD.encode(key)
}

/// Convenience variant: derive a key with a random 16-byte salt.
///
/// Returns `(derived_key_b64, salt_b64)` so callers can store the salt
/// alongside the derived value for later verification.
pub fn sea_work_random(data: &str) -> Result<(String, String)> {
    use rand::RngCore;
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    let key = sea_work(data, &salt);
    Ok((key, URL_SAFE_NO_PAD.encode(salt)))
}

/// Verify that `data` + `salt_b64` reproduce `expected_key_b64`.
///
/// Decodes `salt_b64` from base64url, runs [`sea_work`], and compares the
/// result in constant time.
pub fn sea_work_verify(data: &str, salt_b64: &str, expected_key_b64: &str) -> Result<bool> {
    let salt = URL_SAFE_NO_PAD
        .decode(salt_b64)
        .map_err(|e| anyhow::anyhow!("decode salt base64url: {e}"))?;
    let derived = sea_work(data, &salt);
    // Constant-time comparison to avoid timing attacks.
    Ok(constant_time_eq(
        derived.as_bytes(),
        expected_key_b64.as_bytes(),
    ))
}

/// Constant-time byte-slice equality (avoid timing side-channels).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sea_work_is_deterministic() {
        let salt = b"fixed-salt";
        let k1 = sea_work("password", salt);
        let k2 = sea_work("password", salt);
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_sea_work_differs_on_different_input() {
        let salt = b"same-salt";
        let k1 = sea_work("password1", salt);
        let k2 = sea_work("password2", salt);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_sea_work_differs_on_different_salt() {
        let k1 = sea_work("password", b"salt1");
        let k2 = sea_work("password", b"salt2");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_sea_work_output_is_32_bytes_base64() {
        let key = sea_work("test", b"salt");
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(&key)
            .unwrap();
        assert_eq!(bytes.len(), SEA_WORK_KEY_LEN);
    }

    #[test]
    fn test_sea_work_random_returns_different_salts() {
        let (k1, s1) = sea_work_random("password").unwrap();
        let (k2, s2) = sea_work_random("password").unwrap();
        // Two random invocations should (almost certainly) use different salts.
        assert_ne!(s1, s2);
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_sea_work_verify_accepts_correct() {
        let (key, salt) = sea_work_random("my-password").unwrap();
        assert!(sea_work_verify("my-password", &salt, &key).unwrap());
    }

    #[test]
    fn test_sea_work_verify_rejects_wrong_password() {
        let (key, salt) = sea_work_random("correct").unwrap();
        assert!(!sea_work_verify("wrong", &salt, &key).unwrap());
    }
}
