pub mod journal;
pub mod types;

pub use types::{VfsCacheEntry, VfsError, VfsMountHandle, VfsProvider, VfsState, VfsStats};

use std::sync::Arc;

use chrono::Utc;
use tracing::{debug, info, warn};

use crate::detection::remote::fetch_remote_snapshot;
use crate::journal::sqlite::SqliteJournal;
use crate::journal::Journal as _;
use crate::remote::RemoteClient;
use crate::types::{PairId, RemotePath, SyncPair};

// ── VfsPairRunner ─────────────────────────────────────────────────────────────

/// Runs the metadata-only sync cycle for a VFS-mode pair.
///
/// Unlike the copy-sync `PairRunner`, this runner:
/// - Fetches the remote snapshot and updates `vfs_cache_metadata`
/// - Calls `provider.update_placeholders()` for new cloud-only entries
/// - Never bulk-downloads content (only pinned files are proactively fetched)
/// - Checks disk space and triggers LRU eviction if below threshold
pub struct VfsPairRunner {
    pair: SyncPair,
    client: Arc<dyn RemoteClient>,
    journal: Arc<SqliteJournal>,
    provider: Arc<dyn VfsProvider>,
}

impl VfsPairRunner {
    pub fn new(
        pair: SyncPair,
        client: Arc<dyn RemoteClient>,
        journal: Arc<SqliteJournal>,
        provider: Arc<dyn VfsProvider>,
    ) -> Self {
        Self {
            pair,
            client,
            journal,
            provider,
        }
    }

    /// Run one metadata synchronisation cycle.
    ///
    /// Fetches the full remote snapshot, reconciles with the existing
    /// `vfs_cache_metadata` rows, and updates placeholders for new or
    /// changed files. Deleted remote files have their rows removed.
    pub async fn run_metadata_sync(&self) -> Result<(), crate::error::SyncError> {
        let pair_id = &self.pair.id;
        let remote_root = RemotePath::new(self.pair.remote_root.as_str());

        info!(pair_id = %pair_id, "VFS metadata sync starting");

        // Fetch full remote snapshot.
        let remote_items = fetch_remote_snapshot(&*self.client, &remote_root).await?;

        // Load existing VFS entries to detect deletions.
        let existing = self
            .journal
            .all_vfs_entries(pair_id)
            .await
            .map_err(|e| crate::error::SyncError::Permanent(e.to_string()))?;
        let existing_paths: std::collections::HashSet<String> = existing
            .iter()
            .map(|e| e.path.as_str().to_string())
            .collect();

        // Upsert new / updated entries.
        let mut new_entries: Vec<VfsCacheEntry> = Vec::new();
        for item in &remote_items {
            if item.is_dir {
                continue; // directories handled implicitly
            }
            let entry = VfsCacheEntry::new_cloud_only(
                pair_id.clone(),
                item.path.clone(),
                item.size,
                Some(item.etag.clone()),
                item.mtime,
            );
            // Only insert as cloud_only if not already locally available / pinned.
            if !existing_paths.contains(entry.path.as_str()) {
                new_entries.push(entry.clone());
            }
            self.journal
                .upsert_vfs_entry(&entry)
                .await
                .map_err(|e| crate::error::SyncError::Permanent(e.to_string()))?;
        }

        // Call provider to create OS-level placeholders for truly new files.
        if !new_entries.is_empty() {
            debug!(count = new_entries.len(), "creating VFS placeholders");
            self.provider
                .update_placeholders(&new_entries)
                .await
                .unwrap_or_else(|e| warn!("placeholder update error: {e}"));
        }

        // Remove entries for files deleted from remote.
        let remote_paths: std::collections::HashSet<String> = remote_items
            .iter()
            .filter(|i| !i.is_dir)
            .map(|i| i.path.as_str().to_string())
            .collect();
        for entry in &existing {
            if !remote_paths.contains(entry.path.as_str()) {
                self.journal
                    .delete_vfs_entry(pair_id, entry.path.as_str())
                    .await
                    .unwrap_or_else(|e| warn!("delete vfs entry error: {e}"));
                self.provider
                    .set_cloud_only(&entry.path)
                    .await
                    .unwrap_or_else(|e| warn!("set_cloud_only error: {e}"));
            }
        }

        info!(
            pair_id = %pair_id,
            total = remote_items.iter().filter(|i| !i.is_dir).count(),
            new_placeholders = new_entries.len(),
            "VFS metadata sync completed"
        );

        // Check disk space and auto-evict if needed.
        self.check_and_evict().await;

        Ok(())
    }

    /// Check free disk space and run LRU eviction if below threshold.
    async fn check_and_evict(&self) {
        let threshold = self.pair.vfs_eviction_threshold_bytes;
        if threshold == 0 {
            return;
        }
        let free = free_disk_bytes();
        if free < threshold {
            let to_free = threshold.saturating_sub(free);
            let _ = evict_lru(&self.journal, &self.pair.id, to_free, &*self.provider).await;
        }
    }
}

// ── VFS stats ─────────────────────────────────────────────────────────────────

/// Build a `VfsStats` summary from the journal.
pub async fn get_vfs_stats(journal: &SqliteJournal, pair: &SyncPair) -> VfsStats {
    let (cloud, avail, pinned, bytes) = journal.vfs_stats(&pair.id).await.unwrap_or((0, 0, 0, 0));

    VfsStats {
        pair_id: pair.id.0.clone(),
        cloud_only_count: cloud,
        locally_available_count: avail,
        pinned_count: pinned,
        cached_bytes: bytes,
        cache_max_bytes: pair.vfs_cache_max_bytes,
        last_eviction_at: None, // TODO: persist in separate table
    }
}

// ── LRU eviction ──────────────────────────────────────────────────────────────

/// Evict least-recently-accessed locally-available files until `bytes_to_free`
/// bytes are freed (or no more evictable files remain).
pub async fn evict_lru(
    journal: &SqliteJournal,
    pair_id: &PairId,
    bytes_to_free: u64,
    provider: &dyn VfsProvider,
) -> u64 {
    let candidates = match journal.vfs_lru_candidates(pair_id, 1000).await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "failed to query LRU candidates");
            return 0;
        }
    };

    let mut freed = 0u64;
    for entry in &candidates {
        if freed >= bytes_to_free {
            break;
        }
        // Skip if pinned.
        if let Ok(true) = journal.is_path_pinned(pair_id, entry.path.as_str()).await {
            continue;
        }
        // Remove local content and update state to cloud_only.
        let updated = crate::vfs::types::VfsCacheEntry {
            state: crate::vfs::types::VfsState::CloudOnly,
            cache_bytes: 0,
            ..entry.clone()
        };
        if journal.upsert_vfs_entry(&updated).await.is_ok() {
            freed += entry.cache_bytes;
            provider
                .set_cloud_only(&entry.path)
                .await
                .unwrap_or_else(|e| warn!("set_cloud_only during eviction: {e}"));
        }
    }
    freed
}

// ── Pin path ─────────────────────────────────────────────────────────────────

/// Pin a path (file or directory prefix) and enqueue all matching cloud-only
/// files for immediate background download.
///
/// Returns the number of files enqueued for download.
pub async fn pin_path(
    pair_id: &PairId,
    path: &str,
    journal: Arc<SqliteJournal>,
    client: Arc<dyn RemoteClient>,
    pair: &SyncPair,
    provider: Arc<dyn VfsProvider>,
) -> Result<usize, crate::error::SyncError> {
    // Persist the pin row.
    journal
        .pin_path(pair_id, path, Utc::now())
        .await
        .map_err(|e| crate::error::SyncError::Permanent(e.to_string()))?;

    // Find all cloud-only entries under this path.
    let all = journal
        .all_vfs_entries(pair_id)
        .await
        .map_err(|e| crate::error::SyncError::Permanent(e.to_string()))?;

    let to_pin: Vec<VfsCacheEntry> = all
        .into_iter()
        .filter(|e| {
            e.state == VfsState::CloudOnly
                && (e.path.as_str() == path || e.path.as_str().starts_with(&format!("{path}/")))
        })
        .collect();

    let count = to_pin.len();
    if count == 0 {
        return Ok(0);
    }

    info!(pair_id = %pair_id, count = count, path = path, "pinning paths");

    // Spawn background download task.
    let remote_root = RemotePath::new(pair.remote_root.as_str());
    let local_root = pair.local_root.clone();
    tokio::spawn(async move {
        for entry in to_pin {
            let local_path = crate::types::LocalPath::new(local_root.0.join(entry.path.as_str()));
            let remote_path = RemotePath::new(&format!(
                "{}/{}",
                remote_root.as_str().trim_end_matches('/'),
                entry.path.as_str()
            ));
            let (tx, _rx) = tokio::sync::mpsc::channel(8);
            let download_result = crate::transfer::download::download_file(
                &*client,
                &remote_path,
                &local_path,
                None,
                &Default::default(),
                tx,
                None,
            )
            .await;

            let now = Utc::now();
            let mut updated = entry.clone();
            match download_result {
                Ok(result) => {
                    updated.state = VfsState::Pinned {
                        cached_at: now,
                        last_accessed: now,
                    };
                    updated.cache_bytes = result.size;
                    let _ = journal.upsert_vfs_entry(&updated).await;
                    let _ = provider.set_pinned(&entry.path).await;
                    debug!(path = %entry.path, "pinned file downloaded");
                }
                Err(e) => {
                    warn!(path = %entry.path, error = %e, "pin download failed");
                }
            }
        }
    });

    Ok(count)
}

// ── Evict file ────────────────────────────────────────────────────────────────

/// Evict a single file, removing its local content and setting state to cloud-only.
///
/// Returns `VfsError::PathIsPinned` if the file is pinned.
pub async fn evict_file(
    pair_id: &PairId,
    path: &str,
    journal: &SqliteJournal,
    provider: &dyn VfsProvider,
) -> Result<u64, VfsError> {
    // Guard: pinned paths cannot be auto-evicted.
    if journal.is_path_pinned(pair_id, path).await.unwrap_or(false) {
        return Err(VfsError::PathIsPinned);
    }

    let entry = journal
        .get_vfs_entry(pair_id, path)
        .await
        .map_err(|e| VfsError::Other(e.to_string()))?;

    let freed = match entry {
        Some(e) if e.cache_bytes > 0 => {
            let freed = e.cache_bytes;
            let updated = VfsCacheEntry {
                state: VfsState::CloudOnly,
                cache_bytes: 0,
                ..e
            };
            journal
                .upsert_vfs_entry(&updated)
                .await
                .map_err(|e| VfsError::Other(e.to_string()))?;
            provider
                .set_cloud_only(&updated.path)
                .await
                .unwrap_or_else(|e| warn!("set_cloud_only: {e}"));
            freed
        }
        _ => 0,
    };

    Ok(freed)
}

// ── Convert copy-sync pair to VFS ─────────────────────────────────────────────

/// Convert an existing copy-sync pair to VFS mode.
///
/// Reads all `Synced` journal entries and creates corresponding
/// `locally_available` VFS cache entries — existing local files are not
/// deleted or re-downloaded.
pub async fn convert_to_vfs(
    pair_id: &PairId,
    journal: &SqliteJournal,
    provider: &dyn VfsProvider,
) -> Result<usize, crate::error::SyncError> {
    let existing = journal
        .all_entries(pair_id)
        .await
        .map_err(|e| crate::error::SyncError::Permanent(e.to_string()))?;

    let now = Utc::now();
    let mut converted = 0;

    for entry in existing {
        if entry.status != crate::types::SyncStatus::Synced {
            continue;
        }
        let vfs_entry = VfsCacheEntry {
            pair_id: pair_id.clone(),
            path: entry.path.clone(),
            remote_size: entry.size,
            remote_etag: entry.etag.clone(),
            remote_mtime: entry.mtime_remote.unwrap_or(now),
            state: VfsState::LocallyAvailable {
                cached_at: now,
                last_accessed: now,
            },
            cache_bytes: entry.size,
        };
        journal
            .upsert_vfs_entry(&vfs_entry)
            .await
            .map_err(|e| crate::error::SyncError::Permanent(e.to_string()))?;
        provider
            .set_locally_available(&entry.path)
            .await
            .unwrap_or_else(|e| warn!("set_locally_available during convert: {e}"));
        converted += 1;
    }

    info!(pair_id = %pair_id, converted = converted, "pair converted from copy-sync to VFS");
    Ok(converted)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return free bytes on the filesystem containing the process working directory.
/// Falls back to u64::MAX (unlimited) if the OS call fails.
pub fn free_disk_bytes() -> u64 {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        let path = CString::new("/").unwrap();
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } == 0 {
            return (stat.f_bavail as u64) * (stat.f_frsize as u64);
        }
    }
    u64::MAX
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::sqlite::SqliteJournal;
    use crate::journal::Journal as _;
    use crate::types::{AccountId, PairId, PairStatus};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::path::Path;

    // Minimal mock VfsProvider for unit tests in adagio-core.
    #[derive(Default)]
    struct MockVfsProvider;
    impl MockVfsProvider {
        fn new() -> Self {
            Self
        }
    }
    #[async_trait]
    impl VfsProvider for MockVfsProvider {
        fn is_supported(&self) -> bool {
            true
        }
        async fn mount(
            &self,
            _: &Path,
            pair: &crate::types::SyncPair,
            _: Arc<dyn crate::remote::RemoteClient>,
            _: Arc<dyn crate::journal::Journal>,
        ) -> Result<VfsMountHandle, VfsError> {
            let (tx, _) = tokio::sync::oneshot::channel();
            Ok(VfsMountHandle::new(pair.id.clone(), tx))
        }
        async fn unmount(&self, _: &PairId) -> Result<(), VfsError> {
            Ok(())
        }
        async fn update_placeholders(&self, _: &[VfsCacheEntry]) -> Result<(), VfsError> {
            Ok(())
        }
        async fn set_locally_available(
            &self,
            _: &crate::types::RelativePath,
        ) -> Result<(), VfsError> {
            Ok(())
        }
        async fn set_pinned(&self, _: &crate::types::RelativePath) -> Result<(), VfsError> {
            Ok(())
        }
        async fn set_cloud_only(&self, _: &crate::types::RelativePath) -> Result<(), VfsError> {
            Ok(())
        }
    }

    async fn make_journal() -> Arc<SqliteJournal> {
        use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
        use std::str::FromStr;
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Memory);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        SqliteJournal::run_migrations(&pool).await.unwrap();
        Arc::new(SqliteJournal::new(pool))
    }

    fn default_vfs_pair(pair_id: PairId) -> SyncPair {
        use crate::types::{ConflictPolicy, LocalPath, RemotePath};
        SyncPair {
            id: pair_id,
            account_id: AccountId::new(),
            local_root: LocalPath::new(std::path::PathBuf::from("/tmp/vfs-test")),
            remote_root: RemotePath::new("/"),
            status: PairStatus::Idle,
            exclude_patterns: vec![],
            selective_paths: vec![],
            created_at: Utc::now(),
            last_synced_at: None,
            scan_interval_secs: 7200,
            scan_on_startup: false,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            conflict_policy: ConflictPolicy::Ask,
            bulk_upload_workers: 8,
            bulk_upload_threshold_files: 50,
            bulk_upload_chunk_threshold_bytes: 10 * 1024 * 1024,
            vfs_enabled: true,
            vfs_cache_max_bytes: 20 * 1024 * 1024 * 1024,
            vfs_eviction_threshold_bytes: 0, // disable auto-eviction in tests
        }
    }

    async fn register_pair_in_journal(journal: &SqliteJournal, pair: &SyncPair) {
        use crate::types::Account;
        let acct = Account {
            id: pair.account_id.clone(),
            display_name: "test".to_string(),
            server_url: "https://test.example.com".to_string(),
            username: "test".to_string(),
            keychain_service_key: "test".to_string(),
            created_at: Utc::now(),
            upload_limit_kbps: 0,
            download_limit_kbps: 0,
        };
        journal.register_pair(&acct, pair).await.unwrap();
    }

    // T012: Metadata sync populates cloud_only entries; total cache_bytes = 0.
    #[tokio::test]
    async fn metadata_sync_populates_cloud_only_entries() {
        use crate::remote::mock::MockRemoteClient;

        let journal = make_journal().await;
        let pair_id = PairId::new();
        let pair = default_vfs_pair(pair_id.clone());
        register_pair_in_journal(&journal, &pair).await;

        let client = Arc::new(MockRemoteClient::new());
        // Seed 10 files in the mock remote.
        for i in 0..10 {
            client.seed(&format!("file-{i}.txt"), b"content").await;
        }

        let provider = Arc::new(MockVfsProvider::new());
        let runner = VfsPairRunner::new(pair.clone(), client, journal.clone(), provider.clone());
        runner.run_metadata_sync().await.unwrap();

        let entries = journal.all_vfs_entries(&pair_id).await.unwrap();
        assert_eq!(entries.len(), 10, "all 10 files should be cloud_only");
        assert!(
            entries
                .iter()
                .all(|e| e.state == crate::vfs::types::VfsState::CloudOnly),
            "all should be CloudOnly"
        );
        assert!(
            entries.iter().all(|e| e.cache_bytes == 0),
            "no bytes cached"
        );
    }

    // T013: Metadata sync removes entries for files deleted from remote.
    #[tokio::test]
    async fn metadata_sync_detects_remote_deletions() {
        use crate::remote::mock::MockRemoteClient;

        let journal = make_journal().await;
        let pair_id = PairId::new();
        let pair = default_vfs_pair(pair_id.clone());
        register_pair_in_journal(&journal, &pair).await;

        let client = Arc::new(MockRemoteClient::new());
        for i in 0..5 {
            client.seed(&format!("file-{i}.txt"), b"content").await;
        }

        let provider = Arc::new(MockVfsProvider::new());
        let runner = VfsPairRunner::new(pair.clone(), client.clone(), journal.clone(), provider);

        // First sync: 5 files
        runner.run_metadata_sync().await.unwrap();
        assert_eq!(journal.all_vfs_entries(&pair_id).await.unwrap().len(), 5);

        // Simulate deletion of 2 files from remote (clear client and re-seed 3).
        // MockRemoteClient doesn't have a delete method, so we use a fresh client.
        let client2 = Arc::new(MockRemoteClient::new());
        for i in 0..3 {
            client2.seed(&format!("file-{i}.txt"), b"content").await;
        }

        let provider2 = Arc::new(MockVfsProvider::new());
        let runner2 = VfsPairRunner::new(pair, client2, journal.clone(), provider2);
        runner2.run_metadata_sync().await.unwrap();

        let entries = journal.all_vfs_entries(&pair_id).await.unwrap();
        assert_eq!(entries.len(), 3, "deleted files should be removed");
    }
}
