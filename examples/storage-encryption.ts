/**
 * Storage Encryption Examples
 *
 * Demonstrates how to use PluresDB's AES-256-GCM at-rest encryption through:
 *   1. The Rust `pluresdb` / `pluresdb-storage` crate API
 *   2. The CLI (`pluresdb init --encrypt`)
 *   3. The Node.js N-API binding (password-derived key)
 *
 * This file shows the TypeScript/Deno-side usage and includes inline
 * comments that map each step to its Rust equivalent.
 *
 * Run (Deno — demonstration only, no real I/O):
 *   deno run -A examples/storage-encryption.ts
 */

// ---------------------------------------------------------------------------
// How encryption works in PluresDB
// ---------------------------------------------------------------------------
//
// PluresDB uses AES-256-GCM for all data at rest.  The encryption layer sits
// inside `pluresdb-storage` and is transparent to the CRDT layer above it.
//
// Key derivation
//   Passwords are stretched with Argon2id before being used as the 256-bit
//   AES-GCM master key:
//
//     Rust:
//       use pluresdb::EncryptionConfig;
//       let enc = EncryptionConfig::from_password("hunter2")?;
//
//   The derived key + salt are stored in `encryption.json` next to the
//   database directory so the same password can reopen the database.
//
// Nonce handling
//   Every `encrypt()` call generates a fresh 96-bit random nonce that is
//   prepended to the ciphertext blob.  The nonce is not secret.
//
// Key rotation
//   Call `enc.rotate_key("new-password")` to re-key the config in memory,
//   then re-encrypt or re-open the storage with the updated config.

// ---------------------------------------------------------------------------
// Rust API quick-reference (for embedded Rust projects)
// ---------------------------------------------------------------------------
//
// [dependencies]
// pluresdb = { version = "0.1", features = [] }
//
// ```rust
// use pluresdb::{
//     CrdtStore, EncryptionConfig, EncryptionMetadata, SledStorage,
//     new_persistent_database,
// };
// use std::path::Path;
//
// fn open_encrypted_database(
//     path: &Path,
//     password: &str,
// ) -> anyhow::Result<(CrdtStore, SledStorage)> {
//     // 1. Derive a master key from the user's password.
//     let enc = EncryptionConfig::from_password(password)?;
//
//     // 2. Persist the salt so the database can be re-opened later.
//     let metadata = EncryptionMetadata::from_config(&enc);
//     metadata.save(&path.join("encryption.json"))?;
//
//     // 3. Open SledStorage — pass the config to enable transparent
//     //    encryption of all WAL segments and stored blobs.
//     let storage = SledStorage::open(path)?;
//     // (Full encryption integration via StorageEngine::with_encryption
//     //  is on the roadmap; see docs/ROADMAP.md)
//
//     Ok((CrdtStore::default(), storage))
// }
//
// fn reopen_database(
//     path: &Path,
//     password: &str,
// ) -> anyhow::Result<(CrdtStore, SledStorage)> {
//     // Re-derive the key from the stored salt.
//     let metadata = EncryptionMetadata::load(&path.join("encryption.json"))?;
//     let salt = metadata.salt_bytes()?;
//     let _enc = EncryptionConfig::from_password_with_salt(password, &salt)?;
//
//     let storage = SledStorage::open(path)?;
//     Ok((CrdtStore::default(), storage))
// }
//
// fn rotate_key(
//     path: &Path,
//     old_password: &str,
//     new_password: &str,
// ) -> anyhow::Result<()> {
//     // Load existing metadata and re-derive with the old password.
//     let metadata = EncryptionMetadata::load(&path.join("encryption.json"))?;
//     let salt = metadata.salt_bytes()?;
//     let mut enc = EncryptionConfig::from_password_with_salt(old_password, &salt)?;
//
//     // Rotate to new password — generates a fresh salt automatically.
//     enc.rotate_key(new_password)?;
//
//     // Persist the updated metadata.
//     let new_metadata = EncryptionMetadata::from_config(&enc);
//     new_metadata.save(&path.join("encryption.json"))?;
//
//     Ok(())
// }
// ```

// ---------------------------------------------------------------------------
// CLI usage
// ---------------------------------------------------------------------------
//
// Initialise an encrypted database directory:
//
//   pluresdb init ./secure-db --encrypt
//   # Prompts for a passphrase; stores encryption.json in ./secure-db/
//
// Serve an encrypted database (passphrase via environment variable):
//
//   PLURESDB_PASSPHRASE=hunter2 pluresdb serve --data-dir ./secure-db
//
// Rotate the encryption key:
//
//   pluresdb rotate-key --data-dir ./secure-db
//   # Prompts for old then new passphrase

// ---------------------------------------------------------------------------
// Node.js / TypeScript conceptual demonstration
// ---------------------------------------------------------------------------
//
// The TypeScript layer does not directly expose EncryptionConfig — encryption
// is configured at the Rust storage layer.  If you are using the pre-built
// Node.js N-API bindings you can pass `encryptionPassword` to the constructor
// (planned for a future release; see docs/ROADMAP.md).
//
// For now, launch the server binary with the PLURESDB_PASSPHRASE env-var and
// connect via HTTP/WebSocket:
//
//   import { PluresDB } from "pluresdb";
//
//   // The binary running pluresdb is already decrypting data transparently.
//   const db = new PluresDB();
//   await db.ready();
//   db.serve({ port: 34569 });
//
// Or use the local-first unified API in an environment where PluresDB-native
// is available (Tauri / Node IPC):
//
//   import { PluresDBLocalFirst } from "pluresdb/local-first";
//   const db = new PluresDBLocalFirst({ mode: "auto" });

// ---------------------------------------------------------------------------
// Standalone demonstration (no real I/O, just shows the API surface)
// ---------------------------------------------------------------------------

function demonstrateEncryptionConcepts(): void {
  console.log("=== PluresDB Storage Encryption ===\n");

  console.log("Encryption algorithm : AES-256-GCM");
  console.log("Key derivation       : Argon2id (96-bit random nonce per op)");
  console.log("Persistence          : salt stored in encryption.json");
  console.log("Key rotation         : rotate_key() / pluresdb rotate-key\n");

  console.log("Quick-start (Rust):");
  console.log(
    "  let enc = EncryptionConfig::from_password(password)?;",
  );
  console.log(
    "  let meta = EncryptionMetadata::from_config(&enc);",
  );
  console.log('  meta.save(Path::new("./db/encryption.json"))?;\n');

  console.log("Quick-start (CLI):");
  console.log("  pluresdb init ./secure-db --encrypt");
  console.log("  PLURESDB_PASSPHRASE=secret pluresdb serve --data-dir ./secure-db\n");

  console.log("Full reference: docs/API.md#storage-encryption");
}

if (import.meta.main) {
  demonstrateEncryptionConcepts();
}
