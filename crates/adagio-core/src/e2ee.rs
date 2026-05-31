//! E2EE propagator hook — minimal trait so `adagio-core`'s propagator can call
//! into `adagio-e2ee` without a circular dependency.
//!
//! `adagio-e2ee::NcE2eeClient` implements `E2eePropagatorHook`; the propagator
//! accepts `Option<Arc<dyn E2eePropagatorHook>>` and calls the methods below
//! when encrypting uploads or decrypting downloads.

use crate::types::PairId;
use async_trait::async_trait;

/// File content transformer for E2EE-enabled sync pairs.
#[async_trait]
pub trait E2eePropagatorHook: Send + Sync + 'static {
    /// Encrypt `plaintext` before upload.
    ///
    /// Returns `(ciphertext, uuid_filename, file_entry_json)`.
    /// `uuid_filename` is the opaque name to use on the server.
    /// `file_entry_json` is the JSON-serialised `E2eeFileEntry` that must be
    /// added to the folder metadata.
    async fn encrypt(
        &self,
        pair_id: &PairId,
        original_filename: &str,
        mime: &str,
        plaintext: &[u8],
    ) -> Result<(Vec<u8>, String, String), String>;

    /// Decrypt `ciphertext` downloaded from the server.
    ///
    /// `uuid` is the server filename; the hook looks it up in cached metadata.
    async fn decrypt(
        &self,
        pair_id: &PairId,
        uuid: &str,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, String>;

    /// Flush accumulated file-entry changes as a single atomic metadata commit.
    ///
    /// Called once after all uploads in a cycle complete.
    async fn commit(&self, pair_id: &PairId) -> Result<(), String>;
}
