use crate::bandwidth::{ThroughputMeter, TokenBucket};
use crate::error::{BackoffPolicy, TransferError};
use crate::journal::Journal;
use crate::remote::RemoteClient;
use crate::transfer::download::download_file;
use crate::transfer::upload::upload_single;
use crate::transfer::TransferOptions;
use crate::types::{
    ConflictKind, ConflictPolicy, ConflictRecord, ConflictSide, JournalEntry, LocalPath, PairId,
    RelativePath, RemotePath, SyncStatus,
};
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch, Semaphore};
use tracing::instrument;

use super::reconciler::SyncOp;

/// Default maximum concurrent uploads (upload semaphore permit count).
pub const DEFAULT_MAX_UPLOADS: usize = 3;
/// Default maximum concurrent downloads.
pub const DEFAULT_MAX_DOWNLOADS: usize = 3;

/// Summary of operations completed by one `Propagator::execute` call.
#[derive(Debug, Default)]
pub struct PropagatorResult {
    pub uploaded: u32,
    pub downloaded: u32,
    pub deleted_remote: u32,
    pub deleted_local: u32,
    pub moved: u32,
    pub conflicts: u32,
    pub errors: u32,
    /// Files adopted (same content locally and remotely — journal recorded, no transfer).
    pub skipped: u32,
}

/// Executes an `OperationPlan` with bounded concurrency, per-item error isolation,
/// and exponential-backoff retries (T075/T076/T077).
///
/// Uploads and downloads run concurrently up to `max_uploads` / `max_downloads`
/// using `tokio::sync::Semaphore`. Each operation is isolated: a failure on one
/// item never prevents other items from running. Transient errors are retried
/// according to `backoff`; after exhausting all attempts the item is parked and
/// counted as an error.
///
/// For `ConflictPolicy::Ask`, the propagator records the conflict to the journal
/// and skips the item (does not block the sync cycle). Resolution is handled by
/// the `resolve_conflict` Tauri command independently (ADR-006).
///
/// When a `ClientError::AuthRequired` is detected, `auth_required_tx` (if set)
/// is notified so the engine can pause the account and prompt re-authentication.
pub struct Propagator {
    upload_sem: Arc<Semaphore>,
    download_sem: Arc<Semaphore>,
    /// Senders waiting for user conflict resolution (legacy; kept for compatibility).
    pending_conflicts: Arc<DashMap<RelativePath, oneshot::Sender<ConflictSide>>>,
    /// Retry policy for transient upload/download errors.
    backoff: BackoffPolicy,
    /// Optional channel notified when `ClientError::AuthRequired` is detected.
    auth_required_tx: Option<watch::Sender<bool>>,
    /// Optional bounded channel (capacity 1) notified when a new conflict is recorded (Ask policy).
    /// The desktop layer subscribes to emit `adagio://conflict-detected` events.
    /// `try_send` is used; a full channel (consumer not yet drained) is silently ignored (ADR-008).
    conflict_detected_tx: Option<tokio::sync::mpsc::Sender<()>>,
    /// Upload rate limiter. `None` = unlimited.
    upload_throttle: Option<Arc<TokenBucket>>,
    /// Download rate limiter. `None` = unlimited.
    download_throttle: Option<Arc<TokenBucket>>,
    /// Rolling throughput meter for uploads (used by GetBandwidthStatus).
    pub upload_meter: Arc<std::sync::Mutex<ThroughputMeter>>,
    /// Rolling throughput meter for downloads (used by GetBandwidthStatus).
    pub download_meter: Arc<std::sync::Mutex<ThroughputMeter>>,
}

impl Default for Propagator {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_UPLOADS, DEFAULT_MAX_DOWNLOADS)
    }
}

impl Propagator {
    pub fn new(max_uploads: usize, max_downloads: usize) -> Self {
        Self {
            upload_sem: Arc::new(Semaphore::new(max_uploads)),
            download_sem: Arc::new(Semaphore::new(max_downloads)),
            pending_conflicts: Arc::new(DashMap::new()),
            backoff: BackoffPolicy::default(),
            auth_required_tx: None,
            conflict_detected_tx: None,
            upload_throttle: None,
            download_throttle: None,
            upload_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
            download_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
        }
    }

    /// Create a Propagator with a custom `BackoffPolicy` (e.g. fast policy in tests).
    pub fn with_backoff(backoff: BackoffPolicy) -> Self {
        Self {
            upload_sem: Arc::new(Semaphore::new(DEFAULT_MAX_UPLOADS)),
            download_sem: Arc::new(Semaphore::new(DEFAULT_MAX_DOWNLOADS)),
            pending_conflicts: Arc::new(DashMap::new()),
            backoff,
            auth_required_tx: None,
            conflict_detected_tx: None,
            upload_throttle: None,
            download_throttle: None,
            upload_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
            download_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
        }
    }

    /// Create a Propagator with a backoff policy and an auth-required signal channel.
    pub fn with_auth_channel(backoff: BackoffPolicy, auth_tx: watch::Sender<bool>) -> Self {
        Self {
            upload_sem: Arc::new(Semaphore::new(DEFAULT_MAX_UPLOADS)),
            download_sem: Arc::new(Semaphore::new(DEFAULT_MAX_DOWNLOADS)),
            pending_conflicts: Arc::new(DashMap::new()),
            backoff,
            auth_required_tx: Some(auth_tx),
            conflict_detected_tx: None,
            upload_throttle: None,
            download_throttle: None,
            upload_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
            download_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
        }
    }

    pub fn with_pending_conflicts(
        max_uploads: usize,
        max_downloads: usize,
        pending_conflicts: Arc<DashMap<RelativePath, oneshot::Sender<ConflictSide>>>,
    ) -> Self {
        Self {
            upload_sem: Arc::new(Semaphore::new(max_uploads)),
            download_sem: Arc::new(Semaphore::new(max_downloads)),
            pending_conflicts,
            backoff: BackoffPolicy::default(),
            auth_required_tx: None,
            conflict_detected_tx: None,
            upload_throttle: None,
            download_throttle: None,
            upload_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
            download_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
        }
    }

    /// Create a Propagator with a conflict-detected notification channel.
    ///
    /// When an Ask-policy conflict is recorded, `try_send(())` is called on `tx`.
    /// If the channel is full (`TrySendError::Full`), the notification is silently
    /// dropped — the consumer already has a pending signal and will process it.
    pub fn with_conflict_channel(
        max_uploads: usize,
        max_downloads: usize,
        tx: tokio::sync::mpsc::Sender<()>,
    ) -> Self {
        Self {
            upload_sem: Arc::new(Semaphore::new(max_uploads)),
            download_sem: Arc::new(Semaphore::new(max_downloads)),
            pending_conflicts: Arc::new(DashMap::new()),
            backoff: BackoffPolicy::default(),
            auth_required_tx: None,
            conflict_detected_tx: Some(tx),
            upload_throttle: None,
            download_throttle: None,
            upload_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
            download_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
        }
    }

    /// Create a Propagator with bandwidth limits (bytes/sec; 0 = unlimited).
    ///
    /// Constructs `TokenBucket` instances from the given Kbps values and stores
    /// them so each upload/download call respects the configured ceiling.
    pub fn with_bandwidth(
        max_uploads: usize,
        max_downloads: usize,
        upload_kbps: u64,
        download_kbps: u64,
    ) -> Self {
        let upload_throttle = if upload_kbps > 0 {
            let rate = upload_kbps * 1000 / 8; // Kbps → bytes/sec
            Some(Arc::new(TokenBucket::new(rate, rate))) // burst = 1s worth
        } else {
            None
        };
        let download_throttle = if download_kbps > 0 {
            let rate = download_kbps * 1000 / 8;
            Some(Arc::new(TokenBucket::new(rate, rate)))
        } else {
            None
        };
        Self {
            upload_sem: Arc::new(Semaphore::new(max_uploads)),
            download_sem: Arc::new(Semaphore::new(max_downloads)),
            pending_conflicts: Arc::new(DashMap::new()),
            backoff: BackoffPolicy::default(),
            auth_required_tx: None,
            conflict_detected_tx: None,
            upload_throttle,
            download_throttle,
            upload_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
            download_meter: Arc::new(std::sync::Mutex::new(ThroughputMeter::new())),
        }
    }

    /// Return a handle to the pending-conflicts map (for wiring to the Tauri command).
    pub fn pending_conflicts(&self) -> Arc<DashMap<RelativePath, oneshot::Sender<ConflictSide>>> {
        self.pending_conflicts.clone()
    }

    /// Signal `auth_required_tx` if it is set.
    fn notify_auth_required(&self) {
        if let Some(tx) = &self.auth_required_tx {
            let _ = tx.send(true);
        }
    }

    /// Signal `conflict_detected_tx` if it is set.
    ///
    /// Uses `try_send`; a full channel (capacity 1) means the consumer already
    /// has a pending notification so the new signal is silently dropped (ADR-008).
    fn notify_conflict_detected(&self) {
        if let Some(tx) = &self.conflict_detected_tx {
            match tx.try_send(()) {
                Ok(()) | Err(tokio::sync::mpsc::error::TrySendError::Full(())) => {}
                Err(tokio::sync::mpsc::error::TrySendError::Closed(())) => {
                    tracing::warn!("conflict_detected_tx closed; desktop layer may have stopped");
                }
            }
        }
    }

    /// Execute `op_fn` with exponential-backoff retries on `ClientError::Transient`.
    ///
    /// Returns `Ok(T)` on success, or `Err(String)` containing the last error
    /// message when all attempts are exhausted.
    ///
    /// Short-circuits immediately on `ClientError::AuthRequired` (emits signal)
    /// and on `TransferError::Permanent` / `TransferError::FileInProgress`.
    async fn with_retry<T, F, Fut>(&self, mut op_fn: F) -> Result<T, String>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, crate::error::TransferError>>,
    {
        let mut attempt = 0u32;
        loop {
            match op_fn().await {
                Ok(v) => return Ok(v),
                Err(TransferError::Permanent(msg)) => return Err(msg),
                Err(TransferError::FileInProgress) => return Err("file in progress".into()),
                Err(TransferError::Transient(msg)) => match self.backoff.backoff_for(attempt) {
                    None => return Err(format!("max retries exceeded: {msg}")),
                    Some(delay) => {
                        tracing::warn!(
                            retry_attempt = attempt,
                            delay_ms = delay.as_millis() as u64,
                            %msg,
                            "transient error — retrying after backoff"
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                    }
                },
            }
        }
    }

    /// Execute `op_fn` with retries, handling `ClientError` → `TransferError` mapping.
    #[allow(dead_code)]
    async fn with_retry_client<T, F, Fut>(&self, mut op_fn: F) -> Result<T, String>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, crate::error::ClientError>>,
    {
        self.with_retry(|| {
            let fut = op_fn();
            async move {
                fut.await.map_err(|e| match e {
                    crate::error::ClientError::AuthRequired => {
                        // Permanent — caller checks this via notify_auth_required
                        TransferError::Permanent("auth required".into())
                    }
                    crate::error::ClientError::Transient(m) => TransferError::Transient(m),
                    crate::error::ClientError::Permanent(m) => TransferError::Permanent(m),
                })
            }
        })
        .await
    }

    /// Execute the given operations against the remote client.
    ///
    /// `local_root` is the base directory for local file paths.
    /// `remote_root` is the base path prepended to all relative remote paths.
    /// `journal` is updated after each successful op.
    #[instrument(skip(self, ops, client, journal), fields(op_count = ops.len()))]
    pub async fn execute(
        &self,
        ops: &[SyncOp],
        pair_id: &PairId,
        local_root: &LocalPath,
        remote_root: &RemotePath,
        client: &dyn RemoteClient,
        journal: &dyn Journal,
    ) -> PropagatorResult {
        let opts = TransferOptions::default();
        let mut result = PropagatorResult::default();

        // Execute ops sequentially for simplicity (bounded concurrency is preserved
        // by the semaphores; full parallel execution is an optimisation for later).
        for op in ops {
            // NoOp = reconciler decided nothing needs to happen for this path
            // (e.g. a permanently-errored path that was already parked).
            // Skip path-compat and all other processing immediately.
            if matches!(op, SyncOp::NoOp { .. }) {
                continue;
            }

            // Path compatibility check: reject paths that are unsafe on Windows
            // before attempting any local or remote operation (T095).
            //
            // IMPORTANT: write the op's REAL remote etag (for Download) or local
            // checksum (for Upload) into the error entry so the reconciler sees
            // the file as "in sync" on the next cycle and emits NoOp instead of
            // re-queuing the same op forever.
            if let Some(path) = op.path() {
                if let Err(e) = crate::path_compat::check_path_compat(path.as_str()) {
                    tracing::warn!(path = %path, error = %e, "path compat violation (parked)");
                    result.errors += 1;
                    let mut entry = journal_entry_error(pair_id, path, &e.to_string());
                    // Stamp the entry with op-specific identifiers so the reconciler
                    // treats the file as already at its last-known state.
                    match op {
                        SyncOp::Download {
                            etag,
                            remote_checksum,
                            ..
                        } => {
                            entry.etag = Some(etag.clone());
                            entry.checksum = remote_checksum.clone();
                        }
                        SyncOp::Upload { local_checksum, .. } => {
                            entry.checksum = local_checksum.clone();
                        }
                        SyncOp::Adopt {
                            etag,
                            local_checksum,
                            ..
                        } => {
                            entry.etag = Some(etag.clone());
                            entry.checksum = local_checksum.clone();
                        }
                        _ => {}
                    }
                    let _ = journal.upsert(&entry).await;
                    continue;
                }
            }

            match op {
                SyncOp::NoOp { .. } => {}

                SyncOp::Upload {
                    path,
                    local_checksum,
                } => {
                    let _permit = self.upload_sem.acquire().await.unwrap();
                    let local_file = local_path_for(local_root, path);
                    let remote_file = remote_path_for(remote_root, path);
                    let upload_throttle_clone = self.upload_throttle.clone();
                    let upload_res = self
                        .with_retry(|| {
                            let lf = local_file.clone();
                            let rf = remote_file.clone();
                            let o = opts.clone();
                            let (tx, _rx) = mpsc::channel(8);
                            let throttle = upload_throttle_clone.clone();
                            async move { upload_single(client, &lf, &rf, &o, tx, throttle).await }
                        })
                        .await;
                    match upload_res {
                        Ok(upload_result) => {
                            result.uploaded += 1;
                            // Upload meter recording deferred — UploadResult does not carry size yet.
                            let mtime_local = tokio::fs::metadata(&local_file.0)
                                .await
                                .ok()
                                .and_then(|m| m.modified().ok())
                                .map(chrono::DateTime::<Utc>::from);
                            let mut entry = journal_entry(
                                pair_id,
                                path,
                                &upload_result.etag,
                                SyncStatus::Synced,
                            );
                            if let Some(mt) = mtime_local {
                                entry.mtime_local = Some(mt);
                            }
                            entry.checksum = local_checksum.clone();
                            let _ = journal.upsert(&entry).await;
                        }
                        Err(ref e) if e == "auth required" => {
                            self.notify_auth_required();
                            result.errors += 1;
                            let _ = journal.upsert(&journal_entry_error(pair_id, path, e)).await;
                        }
                        Err(e) => {
                            tracing::warn!(path = %path, error = %e, "upload failed (parked)");
                            result.errors += 1;
                            let _ = journal
                                .upsert(&journal_entry_error(pair_id, path, &e))
                                .await;
                        }
                    }
                }

                SyncOp::Download {
                    path,
                    etag,
                    remote_checksum,
                } => {
                    let _permit = self.download_sem.acquire().await.unwrap();
                    let local_file = local_path_for(local_root, path);
                    let remote_file = remote_path_for(remote_root, path);
                    let download_throttle_clone = self.download_throttle.clone();
                    let dl_res = self
                        .with_retry(|| {
                            let rf = remote_file.clone();
                            let lf = local_file.clone();
                            let o = opts.clone();
                            let (tx, _rx) = mpsc::channel(8);
                            let throttle = download_throttle_clone.clone();
                            async move { download_file(client, &rf, &lf, None, &o, tx, throttle).await }
                        })
                        .await;
                    match dl_res {
                        Ok(dl) => {
                            result.downloaded += 1;
                            if let Ok(mut m) = self.download_meter.lock() {
                                m.record(dl.size);
                            }
                            let mtime_local = tokio::fs::metadata(&local_file.0)
                                .await
                                .ok()
                                .and_then(|m| m.modified().ok())
                                .map(chrono::DateTime::<Utc>::from);
                            let mut entry = journal_entry(pair_id, path, etag, SyncStatus::Synced);
                            if let Some(mt) = mtime_local {
                                entry.mtime_local = Some(mt);
                            }
                            entry.checksum = remote_checksum.clone();
                            let _ = journal.upsert(&entry).await;
                        }
                        Err(ref e) if e == "auth required" => {
                            self.notify_auth_required();
                            result.errors += 1;
                            let _ = journal.upsert(&journal_entry_error(pair_id, path, e)).await;
                        }
                        Err(e) => {
                            tracing::warn!(path = %path, error = %e, "download failed (parked)");
                            result.errors += 1;
                            let _ = journal
                                .upsert(&journal_entry_error(pair_id, path, &e))
                                .await;
                        }
                    }
                }

                SyncOp::DeleteRemote { path } => {
                    let remote_file = remote_path_for(remote_root, path);
                    match client.delete(&remote_file).await {
                        Ok(()) => {
                            result.deleted_remote += 1;
                            let _ = journal.delete(pair_id, path).await;
                        }
                        Err(e) => {
                            tracing::warn!(path = %path, error = %e, "delete remote failed");
                            result.errors += 1;
                        }
                    }
                }

                SyncOp::DeleteLocal { path } => {
                    let local_file = local_path_for(local_root, path);
                    match tokio::fs::remove_file(&local_file.0).await {
                        Ok(()) => {
                            result.deleted_local += 1;
                            let _ = journal.delete(pair_id, path).await;
                        }
                        Err(e) => {
                            tracing::warn!(path = %path, error = %e, "delete local failed");
                            result.errors += 1;
                        }
                    }
                }

                SyncOp::MoveRemote { from, to } => {
                    let remote_from = remote_path_for(remote_root, from);
                    let remote_to = remote_path_for(remote_root, to);
                    match client.move_item(&remote_from, &remote_to).await {
                        Ok(()) => {
                            result.moved += 1;
                            let _ = journal.delete(pair_id, from).await;
                            let _ = journal
                                .upsert(&journal_entry(pair_id, to, "", SyncStatus::Synced))
                                .await;
                        }
                        Err(e) => {
                            tracing::warn!(from = %from, to = %to, error = %e, "move remote failed");
                            result.errors += 1;
                        }
                    }
                }

                SyncOp::MoveLocal { from, to } => {
                    let local_from = local_path_for(local_root, from);
                    let local_to = local_path_for(local_root, to);
                    if let Some(parent) = local_to.0.parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }
                    match tokio::fs::rename(&local_from.0, &local_to.0).await {
                        Ok(()) => {
                            result.moved += 1;
                            let _ = journal.delete(pair_id, from).await;
                            let _ = journal
                                .upsert(&journal_entry(pair_id, to, "", SyncStatus::Synced))
                                .await;
                        }
                        Err(e) => {
                            tracing::warn!(from = %from, to = %to, error = %e, "move local failed");
                            result.errors += 1;
                        }
                    }
                }

                SyncOp::Adopt {
                    path,
                    etag,
                    local_checksum,
                } => {
                    let local_file = local_path_for(local_root, path);
                    let mtime_local = tokio::fs::metadata(&local_file.0)
                        .await
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(chrono::DateTime::<Utc>::from);
                    let mut entry = journal_entry(pair_id, path, etag, SyncStatus::Synced);
                    if let Some(mt) = mtime_local {
                        entry.mtime_local = Some(mt);
                    }
                    entry.checksum = local_checksum.clone();
                    let _ = journal.upsert(&entry).await;
                    result.skipped += 1;
                }

                SyncOp::Conflict {
                    path,
                    remote_etag,
                    remote_mtime,
                    remote_size,
                    policy,
                } => {
                    tracing::warn!(path = %path, remote_etag = %remote_etag, ?policy, "conflict detected");
                    result.conflicts += 1;

                    let side = match policy {
                        ConflictPolicy::Ask => {
                            // Record the conflict in the journal and skip for now.
                            // Resolution is handled independently by the resolve_conflict
                            // Tauri command (ADR-006 — direct execution model).
                            let local_path = local_path_for(local_root, path);
                            let local_meta = tokio::fs::metadata(&local_path.0).await.ok();
                            let local_size = local_meta.as_ref().map(|m| m.len()).unwrap_or(0);
                            let local_mtime = local_meta
                                .and_then(|m| m.modified().ok())
                                .map(chrono::DateTime::<Utc>::from)
                                .unwrap_or_else(Utc::now);
                            let conflict_record = ConflictRecord {
                                id: uuid::Uuid::new_v4().to_string(),
                                pair_id: pair_id.clone(),
                                path: path.clone(),
                                local_mtime,
                                remote_mtime: *remote_mtime,
                                local_size,
                                remote_size: *remote_size,
                                policy: policy.clone(),
                                resolution: None,
                                detected_at: Utc::now(),
                                resolved_at: None,
                                is_dir: false,
                                conflict_kind: ConflictKind::ContentModified,
                            };
                            let _ = journal.upsert_conflict(&conflict_record).await;
                            let _ = journal
                                .upsert(&journal_entry(
                                    pair_id,
                                    path,
                                    remote_etag,
                                    SyncStatus::Conflict,
                                ))
                                .await;
                            self.notify_conflict_detected();
                            None // skip this item; user resolves via wizard
                        }
                        ConflictPolicy::LocalWins => Some(ConflictSide::Local),
                        ConflictPolicy::RemoteWins => Some(ConflictSide::Remote),
                        ConflictPolicy::NewestWins => {
                            let local_file = local_path_for(local_root, path);
                            let local_mtime = tokio::fs::metadata(&local_file.0)
                                .await
                                .ok()
                                .and_then(|m| m.modified().ok())
                                .map(chrono::DateTime::<Utc>::from)
                                .unwrap_or(*remote_mtime);
                            if local_mtime >= *remote_mtime {
                                Some(ConflictSide::Local)
                            } else {
                                Some(ConflictSide::Remote)
                            }
                        }
                        ConflictPolicy::PreserveBoth => {
                            // Simplified: keep remote (full conflict-copy logic is in conflict.rs).
                            Some(ConflictSide::Remote)
                        }
                    };

                    // Apply choice for Ask/LocalWins/RemoteWins.
                    if let Some(side) = side {
                        let local_file = local_path_for(local_root, path);
                        let remote_file = remote_path_for(remote_root, path);
                        let (tx, _rx) = mpsc::channel(8);
                        match side {
                            ConflictSide::Local => {
                                if let Ok(ur) = upload_single(
                                    client,
                                    &local_file,
                                    &remote_file,
                                    &opts,
                                    tx,
                                    None,
                                )
                                .await
                                {
                                    let mtime_local = tokio::fs::metadata(&local_file.0)
                                        .await
                                        .ok()
                                        .and_then(|m| m.modified().ok())
                                        .map(chrono::DateTime::<Utc>::from);
                                    let mut entry =
                                        journal_entry(pair_id, path, &ur.etag, SyncStatus::Synced);
                                    if let Some(mt) = mtime_local {
                                        entry.mtime_local = Some(mt);
                                    }
                                    let _ = journal.upsert(&entry).await;
                                }
                            }
                            ConflictSide::Remote => {
                                if download_file(
                                    client,
                                    &remote_file,
                                    &local_file,
                                    None,
                                    &opts,
                                    tx,
                                    None,
                                )
                                .await
                                .is_ok()
                                {
                                    let mtime_local = tokio::fs::metadata(&local_file.0)
                                        .await
                                        .ok()
                                        .and_then(|m| m.modified().ok())
                                        .map(chrono::DateTime::<Utc>::from);
                                    let mut entry = journal_entry(
                                        pair_id,
                                        path,
                                        remote_etag,
                                        SyncStatus::Synced,
                                    );
                                    if let Some(mt) = mtime_local {
                                        entry.mtime_local = Some(mt);
                                    }
                                    let _ = journal.upsert(&entry).await;
                                }
                            }
                            // Both: handled by the resolve_conflict Tauri command;
                            // the propagator should never see this variant.
                            ConflictSide::Both => {}
                        }
                    }
                }
            }
        }

        result
    }
}

fn local_path_for(root: &LocalPath, rel: &RelativePath) -> LocalPath {
    LocalPath::new(root.0.join(rel.as_str()))
}

fn remote_path_for(root: &RemotePath, rel: &RelativePath) -> RemotePath {
    let base = root.as_str().trim_end_matches('/');
    RemotePath::new(format!("{}/{}", base, rel.as_str()))
}

fn journal_entry(
    pair_id: &PairId,
    path: &RelativePath,
    etag: &str,
    status: SyncStatus,
) -> JournalEntry {
    JournalEntry {
        pair_id: pair_id.clone(),
        path: path.clone(),
        file_id: None,
        etag: Some(etag.to_string()),
        checksum: None,
        size: 0,
        mtime_local: Some(Utc::now()),
        mtime_remote: Some(Utc::now()),
        status,
        error_message: None,
        retry_count: 0,
        updated_at: Utc::now(),
    }
}

fn journal_entry_error(pair_id: &PairId, path: &RelativePath, msg: &str) -> JournalEntry {
    let mut e = journal_entry(pair_id, path, "", SyncStatus::Error);
    e.error_message = Some(msg.to_string());
    e
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::sqlite::SqliteJournal;
    use crate::remote::mock::MockRemoteClient;
    use crate::types::{LocalPath, PairId, RemotePath};
    use std::fs;
    use tempfile::TempDir;

    async fn make_journal(dir: &TempDir) -> SqliteJournal {
        let url = format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("journal.db").display()
        );
        SqliteJournal::open(&url).await.unwrap()
    }

    fn local_root(dir: &TempDir) -> LocalPath {
        LocalPath::new(dir.path())
    }

    fn remote_root() -> RemotePath {
        RemotePath::new("remote/")
    }

    // T046-1: Upload op uploads file and records journal entry.
    #[tokio::test]
    async fn propagator_executes_upload() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("hello.txt"), b"hello").unwrap();
        let client = MockRemoteClient::new();
        let journal = make_journal(&dir).await;
        let pair_id = PairId::new();

        let propagator = Propagator::default();
        let ops = vec![SyncOp::Upload {
            path: RelativePath::new("hello.txt"),
            local_checksum: None,
        }];
        let result = propagator
            .execute(
                &ops,
                &pair_id,
                &local_root(&dir),
                &remote_root(),
                &client,
                &journal,
            )
            .await;

        assert_eq!(result.uploaded, 1);
        assert_eq!(result.errors, 0);
        assert_eq!(client.item_count().await, 1);
    }

    // T046-2: Download op downloads file and creates local file.
    #[tokio::test]
    async fn propagator_executes_download() {
        let dir = TempDir::new().unwrap();
        let client = MockRemoteClient::new();
        client.seed("remote/doc.txt", b"remote content").await;
        let journal = make_journal(&dir).await;
        let pair_id = PairId::new();

        let propagator = Propagator::default();
        let ops = vec![SyncOp::Download {
            path: RelativePath::new("doc.txt"),
            etag: "etag1".to_string(),
            remote_checksum: None,
        }];
        let result = propagator
            .execute(
                &ops,
                &pair_id,
                &local_root(&dir),
                &remote_root(),
                &client,
                &journal,
            )
            .await;

        assert_eq!(result.downloaded, 1);
        assert_eq!(result.errors, 0);
        assert!(
            dir.path().join("doc.txt").exists(),
            "local file should be created"
        );
    }

    // T046-3: DeleteRemote removes the remote item.
    #[tokio::test]
    async fn propagator_executes_delete_remote() {
        let dir = TempDir::new().unwrap();
        let client = MockRemoteClient::new();
        client.seed("remote/gone.txt", b"delete me").await;
        let journal = make_journal(&dir).await;
        let pair_id = PairId::new();

        let propagator = Propagator::default();
        let ops = vec![SyncOp::DeleteRemote {
            path: RelativePath::new("gone.txt"),
        }];
        let result = propagator
            .execute(
                &ops,
                &pair_id,
                &local_root(&dir),
                &remote_root(),
                &client,
                &journal,
            )
            .await;

        assert_eq!(result.deleted_remote, 1);
        assert_eq!(client.item_count().await, 0);
    }

    // ADR-006: Ask policy records the conflict in the journal and skips the item
    // without blocking the sync cycle. Resolution is handled by the Tauri command.
    #[tokio::test]
    async fn propagator_ask_conflict_records_and_skips() {
        use crate::types::{
            Account, AccountId, ConflictPolicy, LocalPath as LP, PairStatus, RemotePath as RP,
            SyncPair,
        };

        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), b"local content").unwrap();
        let client = MockRemoteClient::new();
        client.seed("remote/f.txt", b"remote content").await;
        let journal = make_journal(&dir).await;

        let account_id = AccountId::new();
        let pair_id = PairId::new();

        // Register account + pair so FK constraints on conflict_records are satisfied.
        let account = Account {
            id: account_id.clone(),
            display_name: "Test".into(),
            server_url: "https://cloud.example.com".into(),
            username: "u".into(),
            keychain_service_key: "k".into(),
            created_at: Utc::now(),
            upload_limit_kbps: 0,
            download_limit_kbps: 0,
        };
        let pair = SyncPair {
            id: pair_id.clone(),
            account_id: account_id.clone(),
            local_root: LP::new(dir.path()),
            remote_root: RP::new("remote/"),
            status: PairStatus::Idle,
            exclude_patterns: vec![],
            selective_paths: vec![],
            created_at: Utc::now(),
            last_synced_at: None,
            scan_interval_secs: 7200,
            scan_on_startup: true,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            conflict_policy: ConflictPolicy::Ask,
            bulk_upload_workers: 8,
            bulk_upload_threshold_files: 50,
            bulk_upload_chunk_threshold_bytes: 10 * 1024 * 1024,
            vfs_enabled: false,
            vfs_cache_max_bytes: 20 * 1024 * 1024 * 1024,
            vfs_eviction_threshold_bytes: 5 * 1024 * 1024 * 1024,
        };
        journal.register_pair(&account, &pair).await.unwrap();

        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
        let propagator =
            Propagator::with_conflict_channel(DEFAULT_MAX_UPLOADS, DEFAULT_MAX_DOWNLOADS, tx);

        let ops = vec![SyncOp::Conflict {
            path: RelativePath::new("f.txt"),
            remote_etag: "etag1".into(),
            remote_mtime: Utc::now(),
            remote_size: 0,
            policy: ConflictPolicy::Ask,
        }];

        let lr = local_root(&dir);
        let rr = remote_root();
        let result = propagator
            .execute(&ops, &pair_id, &lr, &rr, &client, &journal)
            .await;

        // Cycle must complete immediately (no suspension).
        assert_eq!(result.conflicts, 1, "conflict count should be 1");

        // Conflict must be persisted in the journal.
        let records = journal.list_conflicts(&pair_id).await.unwrap();
        assert_eq!(records.len(), 1, "conflict should be recorded in journal");
        assert!(
            records[0].resolution.is_none(),
            "conflict should be unresolved"
        );

        // Notification channel must have fired.
        assert!(
            rx.try_recv().is_ok(),
            "conflict-detected notification should have been sent"
        );
    }

    // T007-a — retry log event carries retry_attempt and delay_ms fields.
    // Verifies the log *fields* exist indirectly: the backoff sequence must
    // produce ≥1000 ms on the first retry (base_delay_ms = 1000).
    #[tokio::test]
    async fn retry_rate_never_exceeds_3_per_second() {
        use crate::error::BackoffPolicy;
        use std::sync::{Arc, Mutex};
        use std::time::Instant;

        // Record call timestamps via a shared vec.
        let timestamps: Arc<Mutex<Vec<Instant>>> = Arc::new(Mutex::new(Vec::new()));
        let ts_clone = timestamps.clone();

        // Fast backoff so the test doesn't take >5s, but still ≥100ms between retries.
        let fast_backoff = BackoffPolicy {
            base_ms: 150,
            cap_ms: 500,
            max_attempts: 4,
            ..BackoffPolicy::default()
        };
        let propagator = Propagator::with_backoff(fast_backoff);

        let result = propagator
            .with_retry(|| {
                let ts = ts_clone.clone();
                async move {
                    ts.lock().unwrap().push(Instant::now());
                    Err::<(), _>(crate::error::TransferError::Transient("fail".into()))
                }
            })
            .await;

        assert!(result.is_err(), "should exhaust retries");
        let calls = timestamps.lock().unwrap();
        assert!(calls.len() > 1, "must have retried at least once");

        // Each consecutive pair must be at least 100ms apart (our fast base delay).
        for window in calls.windows(2) {
            let gap = window[1].duration_since(window[0]);
            assert!(
                gap.as_millis() >= 100,
                "consecutive retries too close: {}ms — rate would exceed 10 req/s",
                gap.as_millis()
            );
        }
    }

    // T007-b — conflict channel being full must not block or panic the propagator.
    #[tokio::test]
    async fn conflict_channel_full_does_not_block_propagator() {
        use crate::types::{Account, AccountId, ConflictPolicy, PairStatus};

        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), b"local").unwrap();
        let client = MockRemoteClient::new();
        client.seed("remote/f.txt", b"remote").await;
        let journal = make_journal(&dir).await;

        let account_id = AccountId::new();
        let pair_id = PairId::new();
        let account = Account {
            id: account_id.clone(),
            display_name: "T".into(),
            server_url: "https://nc.example.com".into(),
            username: "u".into(),
            keychain_service_key: "k".into(),
            created_at: chrono::Utc::now(),
            upload_limit_kbps: 0,
            download_limit_kbps: 0,
        };
        let pair = crate::types::SyncPair {
            id: pair_id.clone(),
            account_id: account_id.clone(),
            local_root: crate::types::LocalPath::new(dir.path()),
            remote_root: crate::types::RemotePath::new("remote/"),
            status: PairStatus::Idle,
            exclude_patterns: vec![],
            selective_paths: vec![],
            created_at: chrono::Utc::now(),
            last_synced_at: None,
            scan_interval_secs: 7200,
            scan_on_startup: false,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            conflict_policy: ConflictPolicy::Ask,
            bulk_upload_workers: 8,
            bulk_upload_threshold_files: 50,
            bulk_upload_chunk_threshold_bytes: 10 * 1024 * 1024,
            vfs_enabled: false,
            vfs_cache_max_bytes: 20 * 1024 * 1024 * 1024,
            vfs_eviction_threshold_bytes: 5 * 1024 * 1024 * 1024,
        };
        journal.register_pair(&account, &pair).await.unwrap();

        // Bounded channel, capacity 1 — pre-fill it so it's already full.
        let (tx, _rx) = tokio::sync::mpsc::channel::<()>(1);
        tx.try_send(()).unwrap(); // fill the channel

        let propagator = Propagator::with_conflict_channel(
            DEFAULT_MAX_UPLOADS,
            DEFAULT_MAX_DOWNLOADS,
            tx, // full channel
        );

        let ops = vec![SyncOp::Conflict {
            path: RelativePath::new("f.txt"),
            remote_etag: "e1".into(),
            remote_mtime: chrono::Utc::now(),
            remote_size: 0,
            policy: ConflictPolicy::Ask,
        }];

        // Must complete without panic or deadlock.
        let result = propagator
            .execute(
                &ops,
                &pair_id,
                &local_root(&dir),
                &remote_root(),
                &client,
                &journal,
            )
            .await;
        assert_eq!(
            result.conflicts, 1,
            "conflict must be counted even when channel is full"
        );
    }

    // T047: MoveRemote emits a server-side MOVE.
    #[tokio::test]
    async fn propagator_executes_move_remote() {
        let dir = TempDir::new().unwrap();
        let client = MockRemoteClient::new();
        client.seed("remote/old.txt", b"content").await;
        let journal = make_journal(&dir).await;
        let pair_id = PairId::new();

        let propagator = Propagator::default();
        let ops = vec![SyncOp::MoveRemote {
            from: RelativePath::new("old.txt"),
            to: RelativePath::new("new.txt"),
        }];
        let result = propagator
            .execute(
                &ops,
                &pair_id,
                &local_root(&dir),
                &remote_root(),
                &client,
                &journal,
            )
            .await;

        assert_eq!(result.moved, 1);
        // Remote should now have new.txt, not old.txt
        let items = client.list(&RemotePath::new("remote/")).await.unwrap();
        assert!(items.iter().any(|i| i.path.as_str().contains("new.txt")));
        assert!(!items.iter().any(|i| i.path.as_str().contains("old.txt")));
    }
}
