//! E2EE sync runner for adagio-daemon.
//!
//! Replaces the plain-text `PairRunner` for E2EE-enabled sync pairs.
//! Uses `NcE2eeClient` to encrypt uploads / decrypt downloads so the
//! Nextcloud server only ever stores ciphertext.
//!
//! # Cycle (every 30 s or on trigger)
//! 1. Sync metadata from server  → UUID→filename map
//! 2. Upload new local files     → encrypt → write to temp → WebDAV PUT uuid
//! 3. Download missing files     → WebDAV GET uuid → decrypt → write realname
//! 4. Commit updated metadata

use adagio_core::{
    account_manager::AccountManager,
    config::SyncPairManager,
    journal::sqlite::SqliteJournal,
    transfer::{download::download_file, TransferOptions},
    types::{AccountId, LocalPath, PairId, RemotePath, SyncPair},
};
use adagio_e2ee::{
    provider::{AccountCredentialProvider, NcE2eeClient},
    E2eeError, E2eeProvider,
};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

// ── Credential provider ───────────────────────────────────────────────────────

/// Bridges the daemon's account/pair stores into the `AccountCredentialProvider`
/// interface that `NcE2eeClient` requires.
pub struct DaemonCredentialProvider {
    pub accounts: Arc<AccountManager>,
    pub pairs: Arc<RwLock<SyncPairManager>>,
}

#[async_trait]
impl AccountCredentialProvider for DaemonCredentialProvider {
    async fn credentials(
        &self,
        account_id: &AccountId,
    ) -> Result<(String, String, String), E2eeError> {
        let accounts = self
            .accounts
            .list()
            .map_err(|e| E2eeError::Other(e.to_string()))?;
        let acc = accounts
            .into_iter()
            .find(|a| a.id == *account_id)
            .ok_or_else(|| E2eeError::Other(format!("account {} not found", account_id.0)))?;
        let key = acc.keychain_service_key.clone();
        let password =
            tokio::task::spawn_blocking(move || adagio_nextcloud::auth::retrieve_credentials(&key))
                .await
                .map_err(|e| E2eeError::Other(e.to_string()))?
                .map_err(|e| E2eeError::Other(e.to_string()))?
                .ok_or_else(|| {
                    E2eeError::KeychainUnavailable(
                        "E2EE credentials not found — re-init or pair the device".into(),
                    )
                })?;
        Ok((acc.server_url.clone(), acc.username.clone(), password))
    }

    async fn account_id_for_pair(&self, pair_id: &PairId) -> Result<AccountId, E2eeError> {
        let pairs = self.pairs.read().await;
        let pair = pairs
            .get_pair(pair_id)
            .ok_or_else(|| E2eeError::Other(format!("pair {} not found", pair_id.0)))?;
        Ok(pair.account_id.clone())
    }
}

// ── Sync cycle ────────────────────────────────────────────────────────────────

/// Run one full E2EE sync cycle for `pair`.
///
/// - Encrypts local files not yet in the E2EE metadata and uploads them via a
///   locked cycle (lock → PUT with e2e-token → metadata commit → unlock).
/// - Downloads and decrypts files present in the metadata but absent locally.
pub async fn run_e2ee_cycle(
    pair: &SyncPair,
    e2ee: &NcE2eeClient,
    nc_client: &adagio_nextcloud::client::NextcloudClient,
    journal: &SqliteJournal,
) {
    let pair_id = &pair.id;
    let local_root = &pair.local_root.0;
    let remote_root = &pair.remote_root;

    // Step 1 — sync metadata (populates the in-memory cache that upload_locked uses).
    let metadata = match e2ee.sync_metadata(pair_id).await {
        Ok(m) => m,
        Err(e) if e.to_string().contains("not found") || e.to_string().contains("permanent") => {
            info!(pair_id = %pair_id, "E2EE: metadata not found on server — will create on first upload");
            adagio_e2ee::E2eeMetadata::default()
        }
        Err(e) => {
            warn!(pair_id = %pair_id, error = %e, "E2EE: failed to sync metadata");
            return;
        }
    };

    // Build reverse map: real filename → UUID (to skip already-uploaded files).
    let name_to_uuid: std::collections::HashMap<String, String> = metadata
        .files
        .iter()
        .map(|(uuid, entry)| (entry.filename.clone(), uuid.clone()))
        .collect();

    // Step 2 — collect new local files that need uploading.
    let mut files_to_upload: Vec<(String, String, Vec<u8>)> = Vec::new();
    match tokio::fs::read_dir(local_root).await {
        Ok(mut entries) => {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let file_name = entry.file_name().to_string_lossy().to_string();
                let Ok(meta) = entry.metadata().await else {
                    continue;
                };
                if !meta.is_file() || name_to_uuid.contains_key(&file_name) {
                    continue;
                }
                match tokio::fs::read(entry.path()).await {
                    Ok(bytes) => {
                        let mime = mime_for(&file_name);
                        files_to_upload.push((file_name, mime, bytes));
                    }
                    Err(e) => {
                        warn!(pair_id = %pair_id, file = %file_name, error = %e, "E2EE: read failed")
                    }
                }
            }
        }
        Err(e) => {
            warn!(pair_id = %pair_id, error = %e, "E2EE: cannot read local folder");
            return;
        }
    }

    // Step 3 — download files missing locally.
    let opts = TransferOptions::default();
    let mut downloaded = 0u32;
    for (uuid, file_entry) in &metadata.files {
        let local_path = local_root.join(&file_entry.filename);
        if local_path.exists() {
            continue;
        }
        let remote_uuid = RemotePath::new(format!(
            "{}/{}",
            remote_root.as_str().trim_end_matches('/'),
            uuid
        ));
        let tmp = std::env::temp_dir().join(format!("adagio_e2ee_dl_{uuid}.tmp"));
        let local_tmp = LocalPath::new(tmp.clone());
        let (tx, _rx) = mpsc::channel(8);
        let dl_ok = download_file(nc_client, &remote_uuid, &local_tmp, None, &opts, tx, None)
            .await
            .is_ok();
        if !dl_ok {
            warn!(pair_id = %pair_id, uuid = %uuid, "E2EE: download failed");
            let _ = tokio::fs::remove_file(&tmp).await;
            continue;
        }
        let ciphertext = match tokio::fs::read(&tmp).await {
            Ok(b) => b,
            Err(e) => {
                warn!(pair_id = %pair_id, error = %e, "E2EE: read ciphertext failed");
                let _ = tokio::fs::remove_file(&tmp).await;
                continue;
            }
        };
        let _ = tokio::fs::remove_file(&tmp).await;
        match e2ee.decrypt_file(pair_id, uuid, &ciphertext).await {
            Ok(plaintext) => {
                if let Some(parent) = local_path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                match tokio::fs::write(&local_path, &plaintext).await {
                    Ok(()) => {
                        info!(pair_id = %pair_id, file = %file_entry.filename, "E2EE: downloaded and decrypted");
                        downloaded += 1;
                        record_sync(journal, pair_id, &file_entry.filename).await;
                    }
                    Err(e) => {
                        warn!(pair_id = %pair_id, file = %file_entry.filename, error = %e, "E2EE: write failed")
                    }
                }
            }
            Err(e) => warn!(pair_id = %pair_id, uuid = %uuid, error = %e, "E2EE: decrypt failed"),
        }
    }

    // Step 4 — locked upload cycle: lock folder → upload ciphertext with e2e-token
    //           → PUT updated metadata → unlock.  All in one atomic operation.
    let uploaded = if !files_to_upload.is_empty() {
        let dav_folder = remote_root.as_str().trim_start_matches('/');
        let names: Vec<String> = files_to_upload.iter().map(|(n, _, _)| n.clone()).collect();
        match e2ee
            .upload_locked(pair_id, dav_folder, files_to_upload)
            .await
        {
            Ok(uuids) => {
                // Record journal entries for successfully uploaded files.
                // upload_locked returns UUIDs; we write one entry per file in the input list.
                for name in names.iter().take(uuids.len()) {
                    record_sync(journal, pair_id, name).await;
                }
                uuids.len() as u32
            }
            Err(e) => {
                warn!(pair_id = %pair_id, error = %e, "E2EE: locked upload cycle failed");
                0
            }
        }
    } else {
        0
    };

    if uploaded > 0 || downloaded > 0 {
        info!(pair_id = %pair_id, uploaded, downloaded, "E2EE sync cycle complete");
    }
}

// ── Runner (spawned task) ─────────────────────────────────────────────────────

/// Spawn a background E2EE sync task for `pair`.
///
/// Returns a `(cancel_token, trigger_tx)` pair so the daemon can stop or
/// manually trigger a cycle.
pub fn spawn_e2ee_runner(
    pair: SyncPair,
    e2ee: Arc<NcE2eeClient>,
    nc_client: Arc<adagio_nextcloud::client::NextcloudClient>,
    journal: Arc<SqliteJournal>,
) -> (CancellationToken, mpsc::Sender<()>) {
    let cancel = CancellationToken::new();
    let (trigger_tx, mut trigger_rx) = mpsc::channel::<()>(1);

    let cancel_child = cancel.child_token();
    let pair_id_str = pair.id.0.clone();

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await; // consume immediate first tick

        info!(pair_id = %pair_id_str, "E2EE runner started");

        // Run one cycle immediately on startup.
        run_e2ee_cycle(&pair, &e2ee, &nc_client, &journal).await;

        loop {
            tokio::select! {
                biased;
                _ = cancel_child.cancelled() => {
                    info!(pair_id = %pair_id_str, "E2EE runner stopped");
                    break;
                }
                _ = ticker.tick() => {
                    run_e2ee_cycle(&pair, &e2ee, &nc_client, &journal).await;
                }
                Some(()) = trigger_rx.recv() => {
                    while trigger_rx.try_recv().is_ok() {}
                    info!(pair_id = %pair_id_str, "E2EE sync triggered");
                    run_e2ee_cycle(&pair, &e2ee, &nc_client, &journal).await;
                }
            }
        }
    });

    (cancel, trigger_tx)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Write a `synced` journal entry so the file appears in the activity log.
async fn record_sync(journal: &SqliteJournal, pair_id: &PairId, filename: &str) {
    use adagio_core::journal::Journal as _;
    use adagio_core::types::{JournalEntry, RelativePath, SyncStatus};
    let entry = JournalEntry {
        pair_id: pair_id.clone(),
        path: RelativePath::new(filename),
        file_id: None,
        etag: None,
        size: 0,
        checksum: None,
        status: SyncStatus::Synced,
        mtime_local: None,
        mtime_remote: None,
        retry_count: 0,
        error_message: None,
        updated_at: chrono::Utc::now(),
    };
    let _ = journal.upsert(&entry).await;
}

fn mime_for(filename: &str) -> String {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "mp4" | "mov" => "video/mp4",
        "mp3" => "audio/mpeg",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain",
        "json" => "application/json",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
    .to_string()
}
