//! SEA (Security, Encryption, Authorization) primitives for PluresDB.
//!
//! This crate implements the GUN.js SEA wire-protocol-compatible cryptographic
//! primitives needed for P2P identity, data signing, and end-to-end encryption.
//!
//! ## Key primitives
//!
//! | Primitive   | Algorithm                             | GUN wire compat |
//! |-------------|---------------------------------------|-----------------|
//! | Key pair    | ECDSA P-256 (sign) + ECDH P-256 (enc) | Yes             |
//! | Sign        | ECDSA-P256-SHA256, r∥s (IEEE P1363)   | Yes             |
//! | Verify      | Same                                  | Yes             |
//! | Encrypt     | ECDH + PBKDF2-SHA256 + AES-256-GCM    | Yes             |
//! | Decrypt     | Same                                  | Yes             |
//! | Work        | PBKDF2-SHA256, 100 000 iterations      | Yes             |
//!
//! ## GUN SEA envelope formats
//!
//! **Signed envelope** (`SEA.sign`):
//! ```json
//! "SEA{\"m\":\"<data>\",\"s\":\"<base64url-sig>\"}"
//! ```
//!
//! **Encrypted envelope** (`SEA.encrypt`):
//! ```json
//! "SEA{\"ct\":\"<base64url-ct>\",\"iv\":\"<base64url-iv>\",\"s\":\"<base64url-salt>\"}"
//! ```
//!
//! ## Key encoding
//!
//! All key material is base64url-encoded (no padding) for JSON portability:
//!
//! | Field    | Content                                         | Size      |
//! |----------|-------------------------------------------------|-----------|
//! | `pub`    | ECDSA P-256 uncompressed public point (SEC1)    | 65 bytes  |
//! | `priv`   | ECDSA P-256 secret scalar (SEC1)                | 32 bytes  |
//! | `epub`   | ECDH P-256 uncompressed public point (SEC1)     | 65 bytes  |
//! | `epriv`  | ECDH P-256 secret scalar (SEC1)                 | 32 bytes  |

mod encrypt;
mod key;
mod sign;
mod work;

pub use encrypt::{sea_decrypt, sea_decrypt_wire, sea_encrypt, sea_encrypt_wire, SeaEncryptedEnvelope};
pub use key::SeaKeyPair;
pub use sign::{sea_sign, sea_sign_wire, sea_verify, sea_verify_wire, SeaSignedEnvelope};
pub use work::{sea_work, sea_work_random, sea_work_verify};

/// SEA prefix prepended to serialised envelopes on the wire.
pub const SEA_PREFIX: &str = "SEA";

/// Wrap a serialised envelope JSON string with the `"SEA"` prefix.
///
/// Output: `"SEA{...}"` matching the GUN SEA wire format.
pub fn sea_wrap(inner_json: &str) -> String {
    format!("{}{}", SEA_PREFIX, inner_json)
}

/// Parse a `"SEA{...}"` wire string and return the inner JSON string.
///
/// Returns `None` if the string does not start with the `SEA` prefix.
pub fn sea_unwrap(wire: &str) -> Option<&str> {
    wire.strip_prefix(SEA_PREFIX)
}
