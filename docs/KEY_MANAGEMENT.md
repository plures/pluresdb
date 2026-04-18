# Key Management for PluresDB

This document describes how PluresDB handles encryption keys, how to perform
key rotation, and what to do if a key is lost.

---

## Table of Contents

1. [Overview](#overview)
2. [Key Derivation](#key-derivation)
3. [Encryption Metadata](#encryption-metadata)
4. [Key Rotation](#key-rotation)
5. [Recovery from Key Loss](#recovery-from-key-loss)
6. [Security Recommendations](#security-recommendations)

---

## Overview

PluresDB uses **AES-256-GCM** for all data at rest.  Every encrypted blob is
prefixed with a 96-bit random nonce so that repeated writes of the same value
produce distinct ciphertexts.  Keys are never stored on disk; only the
**Argon2id salt** required to re-derive the key from a passphrase is persisted
(in `encryption.json` alongside the database directory).

```
Passphrase ──Argon2id──► 256-bit master key ──AES-256-GCM──► ciphertext
                │
                └─► salt  ──► encryption.json   (safe to store on disk)
```

---

## Key Derivation

### From a passphrase (recommended)

```rust
use pluresdb_storage::{EncryptionConfig, EncryptionMetadata};
use std::path::Path;

// Create — generates a new random salt
let enc  = EncryptionConfig::from_password("correct-horse-battery-staple")?;
let meta = EncryptionMetadata::from_config(&enc);
meta.save(Path::new("./db/encryption.json"))?;
```

### Re-opening an existing database

```rust
let meta = EncryptionMetadata::load(Path::new("./db/encryption.json"))?;
let salt = meta.salt_bytes()?;

// Deterministic key derivation — same passphrase + same salt = same key
let enc  = EncryptionConfig::from_password_with_salt("correct-horse-battery-staple", &salt)?;
```

### Random key (no passphrase)

```rust
// Use this when the key is stored in a secure secrets manager instead of
// being derived from a user passphrase.
let enc = EncryptionConfig::new()?;
```

> **Important:** With a random key there is no way to re-derive it from the
> metadata alone.  Store the raw key bytes in a hardware security module (HSM)
> or a secrets manager such as HashiCorp Vault.

---

## Encryption Metadata

`EncryptionMetadata` is persisted as a JSON file next to the database
directory.  It contains only the information needed to re-derive the key and
is safe to back up alongside the database because it does **not** store the
key itself.

```json
{
  "version": 1,
  "kdf": "argon2id",
  "cipher": "aes-256-gcm",
  "salt": "<base64-encoded 16-byte Argon2id salt>",
  "revoked_devices": []
}
```

### Device revocation

If a device's credentials are compromised you can add its ID to the revocation
list so that future key-distribution logic can reject it:

```rust
let mut meta = EncryptionMetadata::load(&meta_path)?;
meta.revoke_device("device-id-abc123".to_string());
meta.save(&meta_path)?;
```

---

## Key Rotation

Key rotation replaces the master key derived from `old_password` with a key
derived from `new_password` and a freshly generated salt.  All existing
encrypted blocks must be re-encrypted so they can be decrypted with the new
key.

### Two-phase rotation (recommended)

`rotate_key_and_reencrypt_blocks` performs the rotation atomically:

1. Decrypts every supplied block with the **current** (old) key.
2. Derives a fresh key from `new_password` (new random salt).
3. Re-encrypts every block with the **new** key.
4. Commits the new key to `self` **only if** all blocks succeeded.

```rust
use pluresdb_storage::{EncryptionConfig, EncryptionMetadata};
use std::path::Path;

fn rotate_database_key(
    db_path:      &Path,
    old_password: &str,
    new_password: &str,
    blocks:       Vec<Vec<u8>>,   // existing ciphertexts to re-encrypt
) -> anyhow::Result<Vec<Vec<u8>>> {
    // 1. Load the current salt and re-derive the old key
    let meta = EncryptionMetadata::load(&db_path.join("encryption.json"))?;
    let salt = meta.salt_bytes()?;
    let mut enc = EncryptionConfig::from_password_with_salt(old_password, &salt)?;

    // 2. Rotate and re-encrypt all blocks in one atomic step
    let new_blocks = enc.rotate_key_and_reencrypt_blocks(new_password, &blocks)?;

    // 3. Persist the new metadata (new salt) — do this AFTER step 2 succeeds
    let new_meta = EncryptionMetadata::from_config(&enc);
    new_meta.save(&db_path.join("encryption.json"))?;

    Ok(new_blocks)
}
```

### Metadata-only rotation (no stored blocks)

When all data is held in memory (e.g. in-process caches) or when a new empty
database is being created with a fresh passphrase:

```rust
let new_blocks = enc.rotate_key_and_reencrypt_blocks(new_password, &[])?;
// no blocks to re-encrypt — metadata update only
let new_meta = EncryptionMetadata::from_config(&enc);
new_meta.save(&meta_path)?;
```

### Rotation guarantees

| Property | Behaviour |
|---|---|
| Atomicity | `self` is updated only after **all** blocks are successfully re-encrypted |
| Old ciphertexts | Undecryptable with the new key after rotation |
| New salt | A fresh random salt is generated for every rotation |
| Data loss | `rotate_key_and_reencrypt_blocks` returns an error on the first decryption failure — `self` is not modified |

---

## Recovery from Key Loss

> **Warning:** PluresDB does not store master keys on disk.  If you lose the
> passphrase (or the raw key for non-password-derived configs) you **cannot**
> recover your data.  Take the precautions below seriously.

### Precautions

1. **Back up `encryption.json`** alongside your database.  The file contains
   only the salt, not the key.  Combined with the passphrase it is sufficient
   to re-derive the key.
2. **Store the passphrase securely** — use a password manager, HSM, or a
   secrets manager.  Never hard-code it.
3. **Test recovery regularly** — at least once per quarter, attempt a
   key-rotation drill against a non-production copy of the database.

### Recovery steps

| Scenario | Action |
|---|---|
| Passphrase lost, `encryption.json` intact | Data is unrecoverable — restore from a backup taken before the key was lost |
| `encryption.json` deleted, passphrase known | Restore `encryption.json` from backup; re-derive key with `from_password_with_salt(passphrase, &saved_salt)` |
| Both lost | Restore from an **unencrypted** backup or accept data loss |
| Device compromise, key unchanged | Revoke the device in `encryption.json` (`revoke_device`) and rotate the key |

### Recreating the config after `encryption.json` is restored

```rust
// Restore from backup
std::fs::copy("encryption.json.bak", "db/encryption.json")?;

// Re-derive using the restored salt
let meta = EncryptionMetadata::load(Path::new("db/encryption.json"))?;
let salt = meta.salt_bytes()?;
let enc  = EncryptionConfig::from_password_with_salt("your-passphrase", &salt)?;
```

---

## Security Recommendations

- **Argon2id parameters**: The defaults (memory = 19 MiB, iterations = 2,
  parallelism = 1) follow OWASP recommendations for interactive logins.
  Increase them for batch/offline workflows where latency is less critical.
  You can supply custom parameters via `argon2::Params` and pass a custom
  `Argon2` instance — see the `argon2` crate docs for details.  For example:

  ```rust
  use argon2::{Argon2, Params, Algorithm, Version};

  let params = Params::new(
      64 * 1024, // memory in KiB (64 MiB)
      3,         // iterations
      1,         // parallelism
      Some(32),  // output key length
  )?;
  let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
  // Pass `argon2` to your own key-derivation wrapper.
  ```

  The built-in `EncryptionConfig::from_password` uses `Argon2::default()`
  which selects the library defaults.  Override this in production if your
  threat model requires stronger parameters.
- **Nonce uniqueness**: Each `encrypt()` call generates a fresh 96-bit random
  nonce.  With AES-256-GCM the nonce collision probability is negligible
  under 2³² encryptions per key.  Rotate the key well before that threshold.
- **Key storage**: Never log, print, or persist the derived `master_key` bytes.
  Only `salt` (stored in `encryption.json`) should leave process memory.
- **Rotation frequency**: Rotate the key after any suspected credential
  exposure, and on a regular schedule (e.g. every 90 days for high-security
  deployments).
- **Authenticated encryption**: AES-256-GCM provides both confidentiality and
  integrity (AEAD).  Any tampered ciphertext will cause `decrypt()` to return
  an error rather than silently producing corrupted plaintext.
