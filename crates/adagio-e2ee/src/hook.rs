//! Implements `adagio_core::e2ee::E2eePropagatorHook` for `NcE2eeClient`,
//! bridging the propagator's encrypt/decrypt calls into the full E2EE provider.

use adagio_core::e2ee::E2eePropagatorHook;
use adagio_core::types::PairId;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{E2eeFileEntry, E2eeMetadata, E2eeProvider};

type PendingEntries = Vec<(String, E2eeFileEntry)>;
type PendingMap = std::collections::HashMap<String, (E2eeMetadata, PendingEntries)>;

/// Wraps an `E2eeProvider` as a propagator hook.
///
/// Accumulates file entries during a cycle and flushes them in `commit()`.
pub struct E2eePropagatorHookImpl {
    provider: Arc<dyn E2eeProvider>,
    pending: Mutex<PendingMap>,
}

impl E2eePropagatorHookImpl {
    pub fn new(provider: Arc<dyn E2eeProvider>) -> Self {
        Self {
            provider,
            pending: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait]
impl E2eePropagatorHook for E2eePropagatorHookImpl {
    async fn encrypt(
        &self,
        pair_id: &PairId,
        original_filename: &str,
        mime: &str,
        plaintext: &[u8],
    ) -> Result<(Vec<u8>, String, String), String> {
        let (ciphertext, uuid, entry) = self
            .provider
            .encrypt_file(pair_id, original_filename, mime, plaintext)
            .await
            .map_err(|e| e.to_string())?;

        let entry_json = serde_json::to_string(&entry).map_err(|e| e.to_string())?;

        // Accumulate entry for the commit step.
        let mut pending = self.pending.lock().await;
        let bucket = pending
            .entry(pair_id.0.clone())
            .or_insert_with(|| (E2eeMetadata::default(), Vec::new()));
        bucket.1.push((uuid.clone(), entry));

        Ok((ciphertext, uuid, entry_json))
    }

    async fn decrypt(
        &self,
        pair_id: &PairId,
        uuid: &str,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, String> {
        self.provider
            .decrypt_file(pair_id, uuid, ciphertext)
            .await
            .map_err(|e| e.to_string())
    }

    async fn commit(&self, pair_id: &PairId) -> Result<(), String> {
        let mut pending = self.pending.lock().await;
        if let Some((_, entries)) = pending.remove(&pair_id.0) {
            if entries.is_empty() {
                return Ok(());
            }
            // Sync current metadata, apply new file entries, and commit.
            let mut metadata = self
                .provider
                .sync_metadata(pair_id)
                .await
                .map_err(|e| e.to_string())?;
            for (uuid, entry) in entries {
                metadata.files.insert(uuid, entry);
            }
            metadata.counter += 1;
            self.provider
                .commit_metadata(pair_id, &metadata)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
