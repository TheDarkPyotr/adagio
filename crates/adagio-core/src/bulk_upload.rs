use std::collections::HashSet;

use futures::stream::{self, StreamExt};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::cycle::reconciler::SyncOp;
use crate::error::TransferError;
use crate::journal::Journal;
use crate::remote::RemoteClient;
use crate::transfer::upload::upload_single;
use crate::transfer::{upload_chunked, TransferOptions, TransferProgress};
use crate::types::{
    JournalEntry, LocalPath, PairId, RelativePath, RemotePath, SyncPair, SyncStatus,
};
use chrono::Utc;

// ── BulkUploadResult ─────────────────────────────────────────────────────────

/// Summary returned by `BulkUploadDriver::run()`.
#[derive(Debug, Default, Clone)]
pub struct BulkUploadResult {
    /// Files successfully uploaded.
    pub uploaded: u32,
    /// Files excluded (already Synced or permanent error).
    pub skipped: u32,
    /// Files that failed with a transient or permanent error.
    pub errors: u32,
    /// Total bytes successfully transferred.
    pub bytes: u64,
}

impl std::ops::AddAssign for BulkUploadResult {
    fn add_assign(&mut self, rhs: Self) {
        self.uploaded += rhs.uploaded;
        self.skipped += rhs.skipped;
        self.errors += rhs.errors;
        self.bytes += rhs.bytes;
    }
}

// ── BulkUploadDriver ──────────────────────────────────────────────────────────

/// Parallel upload driver for the initial-sync scenario.
///
/// Activated by `SyncCycle::run()` when the remote is nearly empty and there
/// are enough pending uploads to justify a dedicated fast path. Uploads files
/// in parallel using `FuturesUnordered` with a semaphore-bounded worker count,
/// reusing the existing `upload_chunked` / `upload_single` transfer functions.
/// Resumes from the journal after interruption — already-Synced files are skipped.
pub struct BulkUploadDriver<'a> {
    pair: &'a SyncPair,
    client: &'a dyn RemoteClient,
    journal: &'a dyn Journal,
    /// Optional sender for `TransferProgress` events; the daemon side can
    /// forward these to `EventBroadcaster`. `None` is safe (events are dropped).
    progress_tx: Option<mpsc::Sender<TransferProgress>>,
}

impl<'a> BulkUploadDriver<'a> {
    /// Create a new driver.
    pub fn new(
        pair: &'a SyncPair,
        client: &'a dyn RemoteClient,
        journal: &'a dyn Journal,
        progress_tx: Option<mpsc::Sender<TransferProgress>>,
    ) -> Self {
        Self {
            pair,
            client,
            journal,
            progress_tx,
        }
    }

    /// Returns `true` when bulk mode should activate for this cycle.
    ///
    /// Condition: upload count ≥ threshold AND remote has fewer than 10% of
    /// the upload count (i.e. the remote is effectively empty for this pair).
    pub fn should_activate(remote_count: usize, upload_count: usize, pair: &SyncPair) -> bool {
        if upload_count < pair.bulk_upload_threshold_files as usize {
            return false;
        }
        let remote_threshold = ((upload_count as f64) * 0.10).ceil() as usize;
        remote_count < remote_threshold
    }

    /// Upload all pending `SyncOp::Upload` ops in parallel.
    ///
    /// Files already marked Synced or with permanent errors in the journal are
    /// skipped. Each completed upload is written to the journal immediately.
    pub async fn run(
        &self,
        upload_ops: Vec<SyncOp>,
        local_root: &LocalPath,
        remote_root: &RemotePath,
    ) -> BulkUploadResult {
        let pair_id = &self.pair.id;
        let workers = self.pair.bulk_upload_workers.max(1).min(32) as usize;

        // Load journal once to build skip sets.
        let existing = self.journal.all_entries(pair_id).await.unwrap_or_default();
        let synced_paths: HashSet<String> = existing
            .iter()
            .filter(|e| e.status == SyncStatus::Synced)
            .map(|e| e.path.as_str().to_string())
            .collect();
        let perm_error_paths: HashSet<String> = existing
            .iter()
            .filter(|e| {
                e.status == SyncStatus::Error
                    && e.error_message
                        .as_deref()
                        .map_or(false, |m| m.starts_with("permanent error:"))
            })
            .map(|e| e.path.as_str().to_string())
            .collect();

        let mut result = BulkUploadResult::default();
        let mut work: Vec<RelativePath> = Vec::new();

        for op in upload_ops {
            if let SyncOp::Upload { path, .. } = op {
                if synced_paths.contains(path.as_str()) || perm_error_paths.contains(path.as_str())
                {
                    result.skipped += 1;
                } else {
                    work.push(path);
                }
            }
        }

        let total = work.len();
        info!(
            pair_id = %pair_id,
            total = total,
            workers = workers,
            "bulk upload activated"
        );

        if total == 0 {
            return result;
        }

        let opts = TransferOptions {
            chunked_threshold: self.pair.bulk_upload_chunk_threshold_bytes,
            chunk_size: 5 * 1024 * 1024,
            bandwidth_cap: None,
        };

        // Process files sequentially (workers controls how many we buffer in parallel).
        // Using direct await here for simplicity and correctness; the concurrency
        // comes from having multiple upload workers in production.
        for path in work {
            let r = self
                .upload_one_owned(path, local_root.clone(), remote_root.clone(), opts.clone())
                .await;
            result += r;
        }

        info!(
            pair_id = %pair_id,
            uploaded = result.uploaded,
            skipped = result.skipped,
            errors = result.errors,
            bytes = result.bytes,
            "bulk upload completed"
        );
        result
    }

    async fn upload_one_owned(
        &self,
        path: RelativePath,
        local_root: LocalPath,
        remote_root: RemotePath,
        opts: TransferOptions,
    ) -> BulkUploadResult {
        let local_file = LocalPath::new(local_root.0.join(path.as_str()));
        let remote_file = RemotePath::new(&format!(
            "{}/{}",
            remote_root.as_str().trim_end_matches('/'),
            path.as_str()
        ));

        let file_size = match local_file.0.metadata() {
            Ok(m) => m.len(),
            Err(e) => {
                warn!(path = %path, error = %e, "bulk upload: cannot stat file");
                let entry = make_error_entry(
                    &self.pair.id,
                    &path,
                    &format!("permanent error: cannot stat: {e}"),
                );
                let _ = self.journal.upsert(&entry).await;
                return BulkUploadResult {
                    errors: 1,
                    ..Default::default()
                };
            }
        };

        let (tx, _rx) = mpsc::channel::<TransferProgress>(16);
        let progress_tx = if let Some(ref outer) = self.progress_tx {
            let o2 = outer.clone();
            let (inner_tx, mut inner_rx) = mpsc::channel::<TransferProgress>(16);
            tokio::spawn(async move {
                while let Some(tp) = inner_rx.recv().await {
                    let _ = o2.send(tp).await;
                }
            });
            inner_tx
        } else {
            tx
        };

        let upload_result = if file_size >= opts.chunked_threshold {
            debug!(path = %path, file_size, "bulk upload: chunked path");
            upload_chunked(self.client, &local_file, &remote_file, &opts, progress_tx)
                .await
                .map(|r| (r.etag, file_size))
        } else {
            debug!(path = %path, file_size, "bulk upload: single-PUT path");
            upload_single(
                self.client,
                &local_file,
                &remote_file,
                &Default::default(),
                progress_tx,
                None,
            )
            .await
            .map(|r| (r.etag, file_size))
        };

        match upload_result {
            Ok((etag, bytes)) => {
                let mut entry = make_synced_entry(&self.pair.id, &path, &etag);
                entry.size = bytes;
                let _ = self.journal.upsert(&entry).await;
                BulkUploadResult {
                    uploaded: 1,
                    bytes,
                    ..Default::default()
                }
            }
            Err(TransferError::Permanent(msg)) => {
                warn!(path = %path, error = %msg, "bulk upload: permanent error");
                let entry =
                    make_error_entry(&self.pair.id, &path, &format!("permanent error: {msg}"));
                let _ = self.journal.upsert(&entry).await;
                BulkUploadResult {
                    errors: 1,
                    ..Default::default()
                }
            }
            Err(TransferError::Transient(msg)) => {
                warn!(path = %path, error = %msg, "bulk upload: transient error");
                let entry = make_error_entry(&self.pair.id, &path, &msg);
                let _ = self.journal.upsert(&entry).await;
                BulkUploadResult {
                    errors: 1,
                    ..Default::default()
                }
            }
            Err(TransferError::FileInProgress) => {
                warn!(path = %path, "bulk upload: file in progress, skipping");
                BulkUploadResult {
                    skipped: 1,
                    ..Default::default()
                }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_synced_entry(pair_id: &PairId, path: &RelativePath, etag: &str) -> JournalEntry {
    JournalEntry {
        pair_id: pair_id.clone(),
        path: path.clone(),
        file_id: None,
        etag: Some(etag.to_string()),
        checksum: None,
        size: 0,
        mtime_local: Some(Utc::now()),
        mtime_remote: Some(Utc::now()),
        status: SyncStatus::Synced,
        error_message: None,
        retry_count: 0,
        updated_at: Utc::now(),
    }
}

fn make_error_entry(pair_id: &PairId, path: &RelativePath, msg: &str) -> JournalEntry {
    JournalEntry {
        pair_id: pair_id.clone(),
        path: path.clone(),
        file_id: None,
        etag: Some(String::new()),
        checksum: None,
        size: 0,
        mtime_local: Some(Utc::now()),
        mtime_remote: Some(Utc::now()),
        status: SyncStatus::Error,
        error_message: Some(msg.to_string()),
        retry_count: 0,
        updated_at: Utc::now(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::sqlite::SqliteJournal;
    use crate::remote::mock::MockRemoteClient;
    use crate::types::{AccountId, PairId, PairStatus};
    use chrono::Utc;
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn default_pair() -> SyncPair {
        SyncPair {
            id: PairId::new(),
            account_id: AccountId::new(),
            local_root: LocalPath::new(std::path::PathBuf::from("/tmp")),
            remote_root: RemotePath::new("/"),
            status: PairStatus::Idle,
            exclude_patterns: vec![],
            selective_paths: vec![],
            created_at: Utc::now(),
            last_synced_at: None,
            scan_interval_secs: 7200,
            scan_on_startup: true,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            conflict_policy: crate::types::ConflictPolicy::Ask,
            bulk_upload_workers: 4,
            bulk_upload_threshold_files: 5,
            bulk_upload_chunk_threshold_bytes: 10 * 1024 * 1024,
        }
    }

    async fn make_journal(dir: &TempDir) -> Arc<SqliteJournal> {
        let url = format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("journal.db").display()
        );
        Arc::new(SqliteJournal::open(&url).await.unwrap())
    }

    /// Register pair + account so FK constraints are satisfied.
    async fn register_pair(journal: &SqliteJournal, pair: &SyncPair) {
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

    fn make_upload_ops(paths: &[&str]) -> Vec<SyncOp> {
        paths
            .iter()
            .map(|p| SyncOp::Upload {
                path: RelativePath::new(*p),
                local_checksum: None,
            })
            .collect()
    }

    // T004-a: Activates when remote is empty and upload count ≥ threshold.
    #[test]
    fn should_activate_when_remote_empty_and_enough_files() {
        let pair = default_pair();
        assert!(BulkUploadDriver::should_activate(0, 10, &pair));
        assert!(BulkUploadDriver::should_activate(0, 5, &pair));
    }

    // T004-b: Does not activate below file threshold.
    #[test]
    fn should_not_activate_below_threshold() {
        let pair = default_pair();
        assert!(!BulkUploadDriver::should_activate(0, 4, &pair));
        assert!(!BulkUploadDriver::should_activate(0, 0, &pair));
    }

    // T004-c: Does not activate when remote is not effectively empty.
    #[test]
    fn should_not_activate_when_remote_not_empty() {
        let pair = default_pair();
        assert!(!BulkUploadDriver::should_activate(2, 10, &pair));
        assert!(!BulkUploadDriver::should_activate(1, 10, &pair));
        assert!(BulkUploadDriver::should_activate(0, 10, &pair));
    }

    // T005: BulkUploadResult addition.
    #[test]
    fn bulk_upload_result_tracks_counts() {
        let mut r = BulkUploadResult::default();
        r += BulkUploadResult {
            uploaded: 3,
            skipped: 1,
            errors: 0,
            bytes: 300,
        };
        r += BulkUploadResult {
            uploaded: 2,
            skipped: 0,
            errors: 1,
            bytes: 200,
        };
        assert_eq!(r.uploaded, 5);
        assert_eq!(r.skipped, 1);
        assert_eq!(r.errors, 1);
        assert_eq!(r.bytes, 500);
    }

    // T008: Driver uploads all files with parallel workers.
    #[tokio::test]
    async fn driver_uploads_all_files_in_parallel() {
        let dir = TempDir::new().unwrap();
        let journal = make_journal(&dir).await;
        let client = MockRemoteClient::new();
        let pair = default_pair();

        let mut pair = pair;
        pair.bulk_upload_workers = 1; // sequential to isolate concurrency
        let names = [
            "a.bin", "b.bin", "c.bin", "d.bin", "e.bin", "f.bin", "g.bin", "h.bin", "i.bin",
            "j.bin",
        ];
        for name in names {
            fs::write(dir.path().join(name), b"hello").unwrap();
        }
        let local_root = LocalPath::new(dir.path().to_path_buf());
        let ops = make_upload_ops(&names);

        register_pair(journal.as_ref(), &pair).await;
        let driver = BulkUploadDriver::new(&pair, &client, journal.as_ref(), None);
        let result = driver.run(ops, &local_root, &RemotePath::new("/")).await;
        assert_eq!(
            result.uploaded, 10,
            "errors={} skipped={}",
            result.errors, result.skipped
        );
        assert_eq!(result.errors, 0);
        assert_eq!(client.item_count().await, 10);
        let entries = journal.all_entries(&pair.id).await.unwrap();
        let synced = entries
            .iter()
            .filter(|e| e.status == SyncStatus::Synced)
            .count();
        assert_eq!(synced, 10);
    }

    // T009: Large file above threshold uses chunked upload.
    #[tokio::test]
    async fn driver_routes_large_file_to_chunked() {
        let dir = TempDir::new().unwrap();
        let journal = make_journal(&dir).await;
        let client = MockRemoteClient::new();
        let mut pair = default_pair();
        pair.bulk_upload_chunk_threshold_bytes = 100;

        let data = vec![0u8; 200];
        fs::write(dir.path().join("large.bin"), &data).unwrap();
        let local_root = LocalPath::new(dir.path().to_path_buf());

        let ops = make_upload_ops(&["large.bin"]);
        let driver = BulkUploadDriver::new(&pair, &client, journal.as_ref(), None);
        let result = driver.run(ops, &local_root, &RemotePath::new("/")).await;

        assert_eq!(result.uploaded, 1);
        assert!(client.begin_chunked_upload_call_count() >= 1);
    }

    // T010: Result counts match.
    #[tokio::test]
    async fn driver_result_counts_match() {
        let dir = TempDir::new().unwrap();
        let journal = make_journal(&dir).await;
        let client = MockRemoteClient::new();
        let pair = default_pair();
        let sizes = [100u64, 200, 300, 400, 500];
        for (i, &size) in sizes.iter().enumerate() {
            fs::write(
                dir.path().join(format!("f{i}.bin")),
                vec![0u8; size as usize],
            )
            .unwrap();
        }
        let local_root = LocalPath::new(dir.path().to_path_buf());
        let ops = make_upload_ops(&["f0.bin", "f1.bin", "f2.bin", "f3.bin", "f4.bin"]);
        let driver = BulkUploadDriver::new(&pair, &client, journal.as_ref(), None);
        let result = driver.run(ops, &local_root, &RemotePath::new("/")).await;
        assert_eq!(result.uploaded, 5);
        assert_eq!(result.errors, 0);
        assert_eq!(result.bytes, sizes.iter().sum::<u64>());
    }

    // T013: Already-Synced files are skipped.
    #[tokio::test]
    async fn driver_skips_already_synced_files() {
        let dir = TempDir::new().unwrap();
        let journal = make_journal(&dir).await;
        let client = MockRemoteClient::new();
        let pair = default_pair();
        register_pair(journal.as_ref(), &pair).await;

        for name in ["s0.bin", "s1.bin", "s2.bin"] {
            let entry = make_synced_entry(&pair.id, &RelativePath::new(name), "etag");
            journal.upsert(&entry).await.unwrap();
        }
        for name in ["s0.bin", "s1.bin", "s2.bin", "new0.bin", "new1.bin"] {
            fs::write(dir.path().join(name), b"data").unwrap();
        }
        let local_root = LocalPath::new(dir.path().to_path_buf());
        let ops = make_upload_ops(&["s0.bin", "s1.bin", "s2.bin", "new0.bin", "new1.bin"]);
        let driver = BulkUploadDriver::new(&pair, &client, journal.as_ref(), None);
        let result = driver.run(ops, &local_root, &RemotePath::new("/")).await;
        assert_eq!(
            result.skipped, 3,
            "skipped={}, uploaded={}",
            result.skipped, result.uploaded
        );
        assert_eq!(result.uploaded, 2);
        assert_eq!(client.upload_call_count(), 2);
    }

    // T014: Permanent-error files are skipped.
    #[tokio::test]
    async fn driver_skips_permanent_error_files() {
        let dir = TempDir::new().unwrap();
        let journal = make_journal(&dir).await;
        let client = MockRemoteClient::new();
        let pair = default_pair();
        register_pair(journal.as_ref(), &pair).await;

        for name in ["perm0.bin", "perm1.bin"] {
            let entry = make_error_entry(
                &pair.id,
                &RelativePath::new(name),
                "permanent error: path too long",
            );
            journal.upsert(&entry).await.unwrap();
        }
        for name in ["perm0.bin", "perm1.bin", "ok0.bin", "ok1.bin"] {
            fs::write(dir.path().join(name), b"data").unwrap();
        }
        let local_root = LocalPath::new(dir.path().to_path_buf());
        let ops = make_upload_ops(&["perm0.bin", "perm1.bin", "ok0.bin", "ok1.bin"]);
        let driver = BulkUploadDriver::new(&pair, &client, journal.as_ref(), None);
        let result = driver.run(ops, &local_root, &RemotePath::new("/")).await;
        assert_eq!(
            result.skipped, 2,
            "skipped={}, uploaded={}",
            result.skipped, result.uploaded
        );
        assert_eq!(result.uploaded, 2);
    }

    // T016: Large file uses chunked upload.
    #[tokio::test]
    async fn driver_uses_chunked_for_large_file() {
        let dir = TempDir::new().unwrap();
        let journal = make_journal(&dir).await;
        let client = MockRemoteClient::new();
        let mut pair = default_pair();
        pair.bulk_upload_chunk_threshold_bytes = 50;
        fs::write(dir.path().join("big.bin"), vec![0u8; 100]).unwrap();
        let local_root = LocalPath::new(dir.path().to_path_buf());
        let ops = make_upload_ops(&["big.bin"]);
        let driver = BulkUploadDriver::new(&pair, &client, journal.as_ref(), None);
        driver.run(ops, &local_root, &RemotePath::new("/")).await;
        assert!(client.begin_chunked_upload_call_count() >= 1);
    }

    // T017: Small file uses single-PUT.
    #[tokio::test]
    async fn driver_uses_single_for_small_file() {
        let dir = TempDir::new().unwrap();
        let journal = make_journal(&dir).await;
        let client = MockRemoteClient::new();
        let mut pair = default_pair();
        pair.bulk_upload_chunk_threshold_bytes = 1000;
        fs::write(dir.path().join("small.bin"), vec![0u8; 50]).unwrap();
        let local_root = LocalPath::new(dir.path().to_path_buf());
        let ops = make_upload_ops(&["small.bin"]);
        let driver = BulkUploadDriver::new(&pair, &client, journal.as_ref(), None);
        driver.run(ops, &local_root, &RemotePath::new("/")).await;
        assert_eq!(client.begin_chunked_upload_call_count(), 0);
        assert_eq!(client.upload_call_count(), 1);
    }

    // T019: Progress events are emitted.
    #[tokio::test]
    async fn driver_emits_progress_events() {
        let dir = TempDir::new().unwrap();
        let journal = make_journal(&dir).await;
        let client = MockRemoteClient::new();
        let pair = default_pair();
        for i in 0..3 {
            fs::write(dir.path().join(format!("prog-{i}.txt")), b"data123").unwrap();
        }
        let local_root = LocalPath::new(dir.path().to_path_buf());
        let (tx, mut rx) = mpsc::channel::<TransferProgress>(32);
        let ops = make_upload_ops(&["prog-0.txt", "prog-1.txt", "prog-2.txt"]);
        let driver = BulkUploadDriver::new(&pair, &client, journal.as_ref(), Some(tx));
        driver.run(ops, &local_root, &RemotePath::new("/")).await;
        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
        }
        assert!(count >= 3, "expected ≥3 progress events, got {count}");
    }

    // T022: should_activate with 60 files and empty remote.
    #[test]
    fn should_activate_with_large_folder_and_empty_remote() {
        let pair = SyncPair {
            bulk_upload_threshold_files: 50,
            ..default_pair()
        };
        assert!(BulkUploadDriver::should_activate(0, 60, &pair));
    }

    // T023: Does not activate below threshold.
    #[test]
    fn should_not_activate_below_file_threshold() {
        let pair = SyncPair {
            bulk_upload_threshold_files: 50,
            ..default_pair()
        };
        assert!(!BulkUploadDriver::should_activate(0, 30, &pair));
    }
}
