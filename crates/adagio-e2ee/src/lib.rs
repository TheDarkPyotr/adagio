//! Nextcloud E2EE v2.x implementation for Adagio.
//!
//! Encrypts file content (AES-128-GCM, per-file key) and filenames (inside an
//! AES-128-GCM metadata blob) before upload.  The RSA-4096 device private key
//! is protected by a BIP-39 mnemonic via PBKDF2-HMAC-SHA256.
//!
//! See `docs/adr/017-e2ee-protocol.md` for protocol decisions.

pub mod cipher;
pub mod hook;
pub mod keys;
pub mod metadata;
pub mod ocs;
pub mod provider;

use adagio_core::types::AccountId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zeroize::ZeroizeOnDrop;

// ── Error ─────────────────────────────────────────────────────────────────────

/// All errors that can arise from E2EE operations.
#[derive(Debug, thiserror::Error)]
pub enum E2eeError {
    #[error("OS keychain unavailable: {0}")]
    KeychainUnavailable(String),

    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),

    #[error("mnemonic does not match the stored key; check spelling")]
    MnemonicMismatch,

    #[error("server error: {0}")]
    ServerError(String),

    #[error("unsupported metadata_ver {0}; upgrade adagio")]
    UnsupportedMetadataVersion(String),

    #[error("legacy metadata v1.x is read-only; uploads blocked")]
    LegacyMetadataReadOnly,

    #[error("folder lock conflict (counter mismatch): {0}")]
    LockConflict(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialisation error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("{0}")]
    Other(String),
}

impl From<adagio_core::error::ClientError> for E2eeError {
    fn from(e: adagio_core::error::ClientError) -> Self {
        match e {
            adagio_core::error::ClientError::Maintenance => {
                E2eeError::ServerError("server in maintenance mode".into())
            }
            other => E2eeError::ServerError(other.to_string()),
        }
    }
}

// ── Inner metadata types ──────────────────────────────────────────────────────

/// Per-file entry stored inside the encrypted metadata blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2eeFileEntry {
    /// Original plaintext filename.
    pub filename: String,
    /// MIME type.
    pub mimetype: String,
    /// 96-bit AES-GCM nonce, base64-encoded.
    pub nonce: String,
    /// AES-GCM authentication tag, base64-encoded.
    #[serde(rename = "authenticationTag")]
    pub auth_tag: String,
    /// 128-bit per-file AES key, base64-encoded.
    pub key: String,
}

/// Decrypted inner metadata for a single E2EE folder.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct E2eeMetadata {
    /// Metadata format version (e.g. `"2.0"` or `"1.0"`).
    #[serde(default = "default_version")]
    pub version: String,
    /// Monotonically increasing counter — server enforces strict succession.
    #[serde(default)]
    pub counter: u64,
    /// SHA-256 checksums of all metadata keys ever used.
    #[serde(rename = "keyChecksums", default)]
    pub key_checksums: Vec<String>,
    /// Deleted flag.
    #[serde(default)]
    pub deleted: bool,
    /// UUID → cleartext subfolder name.
    #[serde(default)]
    pub folders: HashMap<String, String>,
    /// UUID → per-file encryption entry.
    #[serde(default)]
    pub files: HashMap<String, E2eeFileEntry>,
}

fn default_version() -> String {
    "2.0".to_string()
}

// ── E2EE status ───────────────────────────────────────────────────────────────

/// Status summary for a single E2EE-enabled sync pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E2eeStatusDto {
    pub pair_id: String,
    pub enabled: bool,
    pub metadata_version: Option<String>,
    pub counter: u64,
    pub key_fingerprint: Option<String>,
    pub encrypted_file_count: u64,
}

// ── Key material (in-memory only) ─────────────────────────────────────────────

/// In-memory key material for an active E2EE session.
///
/// # Safety
/// All key bytes are zeroed on drop via `ZeroizeOnDrop`.
#[derive(ZeroizeOnDrop)]
pub struct E2eeKeySet {
    /// Raw RSA private key bytes (PKCS#8 DER).
    pub private_key_der: Vec<u8>,
    /// Signed X.509 certificate PEM (public key + server signature).
    pub certificate_pem: String,
    /// 128-bit AES metadata key for the folder.
    #[zeroize(skip)]
    pub metadata_key: [u8; 16],
}

// ── Provider trait ────────────────────────────────────────────────────────────

/// Platform-agnostic E2EE provider — implemented by `NcE2eeClient`.
///
/// All methods are `async` and designed to be called from the daemon's
/// Tokio runtime.  Keys are loaded from the OS keychain on demand.
#[async_trait]
pub trait E2eeProvider: Send + Sync + 'static {
    /// Returns `true` when the account has an initialised E2EE key pair on the server.
    async fn is_initialised(&self, account_id: &AccountId) -> Result<bool, E2eeError>;

    /// Generate RSA-4096 key pair, submit CSR, upload mnemonic-protected private key.
    ///
    /// Returns the 12-word BIP-39 mnemonic — shown **once** to the user,
    /// **never** logged.
    async fn init(&self, pair_id: &adagio_core::types::PairId) -> Result<String, E2eeError>;

    /// Download and decrypt the RSA private key using the supplied mnemonic.
    async fn pair(
        &self,
        pair_id: &adagio_core::types::PairId,
        mnemonic: &str,
    ) -> Result<(), E2eeError>;

    /// Encrypt `plaintext` for `pair_id`.
    ///
    /// Returns `(ciphertext, uuid, entry)` where `uuid` is the server filename
    /// and `entry` must be added to the folder metadata.
    async fn encrypt_file(
        &self,
        pair_id: &adagio_core::types::PairId,
        filename: &str,
        mimetype: &str,
        plaintext: &[u8],
    ) -> Result<(Vec<u8>, String, E2eeFileEntry), E2eeError>;

    /// Decrypt `ciphertext` using the metadata entry for `uuid`.
    async fn decrypt_file(
        &self,
        pair_id: &adagio_core::types::PairId,
        uuid: &str,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, E2eeError>;

    /// Fetch and decrypt the folder metadata from the server; update local counter.
    async fn sync_metadata(
        &self,
        pair_id: &adagio_core::types::PairId,
    ) -> Result<E2eeMetadata, E2eeError>;

    /// Lock folder → update metadata with `new_metadata` → unlock.
    ///
    /// Retries once on `LockConflict` (409).
    async fn commit_metadata(
        &self,
        pair_id: &adagio_core::types::PairId,
        new_metadata: &E2eeMetadata,
    ) -> Result<(), E2eeError>;

    /// Diagnostic status for `adagio e2ee status`.
    async fn status(
        &self,
        pair_id: &adagio_core::types::PairId,
    ) -> Result<E2eeStatusDto, E2eeError>;

    /// Recreate server metadata via POST (used when existing metadata is missing).
    ///
    /// Unlike `commit_metadata` (which requires a lock token and uses PUT),
    /// this method creates a brand-new metadata entry without locking.
    async fn recreate_metadata(
        &self,
        pair_id: &adagio_core::types::PairId,
        metadata: &E2eeMetadata,
    ) -> Result<(), E2eeError>;

    /// Disable E2EE for a pair.
    ///
    /// Deletes the server-side metadata file and removes the local E2EE journal
    /// row so the pair reverts to copy-sync mode on the next cycle.
    /// Existing local plaintext files are kept; the caller is responsible for
    /// persisting `pair.e2ee_enabled = false` in the config.
    async fn disable(&self, pair_id: &adagio_core::types::PairId) -> Result<(), E2eeError>;
}

// ── Tests (T044) ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // T044 — E2eeStatusDto serialises to JSON with all documented fields.
    #[test]
    fn e2ee_status_dto_serialises() {
        let dto = E2eeStatusDto {
            pair_id: "pair-1".to_string(),
            enabled: true,
            metadata_version: Some("2.0".to_string()),
            counter: 42,
            key_fingerprint: Some("sha256:abcd1234".to_string()),
            encrypted_file_count: 7,
        };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["pair_id"], "pair-1");
        assert_eq!(json["enabled"], true);
        assert_eq!(json["metadata_version"], "2.0");
        assert_eq!(json["counter"], 42);
        assert_eq!(json["key_fingerprint"], "sha256:abcd1234");
        assert_eq!(json["encrypted_file_count"], 7);
    }
}
