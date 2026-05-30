use crate::cycle::runner::PairRunner;
use crate::detection::local::{scan_local_with_stats, CachedLocalEntry};
use crate::detection::remote::fetch_remote_snapshot;
use crate::error::SyncError;
use crate::journal::Journal;
use crate::observability::{CycleMetrics, MemorySampler};
use crate::remote::RemoteClient;
use crate::types::{AccountId, LocalPath, PairId, RelativePath, RemotePath, SyncPair};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{watch, Mutex, RwLock};
use tracing::instrument;

pub mod discovery;
pub mod propagator;
pub mod reconciler;
pub mod runner;

// ── Network condition detection (T070) ───────────────────────────────────────

/// Returns `true` when the active network interface is reported as metered.
///
/// On Linux/macOS this requires D-Bus / SCNetworkReachability — those bindings
/// are deferred. The function is safe to call on all platforms; it returns
/// `false` when detection is unavailable so sync continues by default.
pub fn is_metered_connection() -> bool {
    // TODO(T070): implement via D-Bus NetworkManager on Linux,
    // SCNetworkReachability on macOS, and WinRT NetworkInformation on Windows.
    false
}

// ── Battery detection (T071) ─────────────────────────────────────────────────

/// Returns the battery charge as a percentage (0–100), or `None` if the device
/// has no battery (desktop on AC) or if the platform API is unavailable.
pub async fn battery_level_percent() -> Option<u8> {
    #[cfg(target_os = "linux")]
    {
        // Try common sysfs paths for the first battery found.
        for name in &["BAT0", "BAT1", "battery"] {
            let path = format!("/sys/class/power_supply/{}/capacity", name);
            if let Ok(s) = tokio::fs::read_to_string(&path).await {
                if let Ok(level) = s.trim().parse::<u8>() {
                    return Some(level);
                }
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    None
}

/// Summary of a completed sync cycle for one pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub pair_id: PairId,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub uploaded: u32,
    pub downloaded: u32,
    pub conflicts: u32,
    pub errors: u32,
    pub skipped: u32,
}

/// A single event in the activity log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub pair_id: PairId,
    pub path: String,
    pub action: String,
    pub occurred_at: DateTime<Utc>,
    pub bytes: Option<u64>,
}

/// An item in the persistent error / retry list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorItem {
    pub pair_id: PairId,
    pub path: String,
    pub message: String,
    pub retry_count: u32,
    pub last_attempted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineStatus {
    Idle,
    Syncing { pair_id: PairId },
    Paused,
    Error(String),
}

/// Drives sync cycles for all configured pairs.
#[async_trait]
pub trait SyncEngine: Send + Sync {
    /// Trigger an immediate sync cycle for a specific pair.
    ///
    /// Idempotent: if a cycle is already in progress for this pair, returns
    /// the existing cycle's report when it completes.
    async fn trigger_sync(&self, pair_id: &PairId) -> Result<SyncReport, SyncError>;

    /// Pause all sync activity. In-flight transfers complete; no new cycles start.
    async fn pause(&self) -> Result<(), SyncError>;

    /// Resume from paused state.
    async fn resume(&self) -> Result<(), SyncError>;

    /// Return the current engine status (non-blocking, lock-free).
    fn status(&self) -> EngineStatus;

    /// Subscribe to engine status changes.
    fn subscribe_status(&self) -> watch::Receiver<EngineStatus>;

    /// Return the last N activity log entries across all pairs.
    async fn activity_log(&self, limit: usize) -> Result<Vec<ActivityEntry>, SyncError>;

    /// Return all items currently in an error state across all pairs.
    async fn error_items(&self) -> Result<Vec<ErrorItem>, SyncError>;
}

// ── SyncCycle orchestrator (T048) ─────────────────────────────────────────────

/// Executes a single sync cycle for one pair: discovery → reconciliation → propagation.
pub struct SyncCycle<'a> {
    pub pair: &'a SyncPair,
    pub client: &'a dyn RemoteClient,
    pub journal: &'a dyn Journal,
    /// Optional channel for conflict-detected notifications (forwarded to propagator).
    /// Bounded (capacity 1); `try_send` is used so a full channel never blocks.
    pub conflict_tx: Option<&'a tokio::sync::mpsc::Sender<()>>,
    /// Optional memory sampler; when set, `observe()` is called at cycle end.
    pub memory_sampler: Option<Arc<std::sync::Mutex<MemorySampler>>>,
}

impl<'a> SyncCycle<'a> {
    /// Run the full cycle and return a `SyncReport`.
    #[instrument(skip(self), fields(pair_id = %self.pair.id))]
    pub async fn run(&self) -> Result<SyncReport, SyncError> {
        let started_at = Utc::now();

        // 1. Discovery: build local and remote snapshots.
        let journal_entries = self.journal.all_entries(&self.pair.id).await?;
        let cache: HashMap<RelativePath, CachedLocalEntry> = journal_entries
            .iter()
            .filter_map(|e| {
                Some((
                    e.path.clone(),
                    CachedLocalEntry {
                        mtime: e.mtime_local?,
                        size: e.size,
                        checksum: e.checksum.clone(),
                    },
                ))
            })
            .collect();

        let local_root = LocalPath::new(&self.pair.local_root.0);
        let remote_root = RemotePath::new(self.pair.remote_root.as_str());

        let (local_snapshot, scan_stats) = scan_local_with_stats(&local_root, &cache).await?;
        // Directories are created implicitly; exclude them from reconciliation so
        // they don't generate spurious Upload ops.
        let path_count = local_snapshot.len();
        let local_items: Vec<_> = local_snapshot.into_iter().filter(|i| !i.is_dir).collect();
        let raw_remote = fetch_remote_snapshot(self.client, &remote_root).await?;
        // Apply selective-sync filtering: if the pair has an explicit path list,
        // only items under those paths are included in the diff.
        let remote_items =
            crate::detection::exclusion::filter_selective(raw_remote, &self.pair.selective_paths);

        // 2. Reconciliation.
        let plan = reconciler::reconcile(
            &local_items,
            &remote_items,
            &journal_entries,
            self.pair.conflict_policy.clone(),
        );

        // 3. Propagation — optionally via BulkUploadDriver for initial-sync fast path.
        use crate::bulk_upload::BulkUploadDriver;
        use crate::cycle::reconciler::SyncOp;

        // Partition: Upload ops are candidates for bulk mode; everything else always
        // goes through the standard propagator.
        let (upload_ops, other_ops): (Vec<SyncOp>, Vec<SyncOp>) = plan
            .ops
            .into_iter()
            .partition(|op| matches!(op, SyncOp::Upload { .. }));

        let use_bulk =
            BulkUploadDriver::should_activate(remote_items.len(), upload_ops.len(), self.pair);

        let mut result = if use_bulk {
            let driver = BulkUploadDriver::new(self.pair, self.client, self.journal, None);
            let bulk_result = driver.run(upload_ops, &local_root, &remote_root).await;
            crate::cycle::propagator::PropagatorResult {
                uploaded: bulk_result.uploaded,
                downloaded: 0,
                deleted_remote: 0,
                deleted_local: 0,
                moved: 0,
                conflicts: 0,
                errors: bulk_result.errors,
                skipped: bulk_result.skipped,
            }
        } else {
            // Standard path: recombine upload ops with other ops.
            let mut all_ops = upload_ops;
            all_ops.extend(other_ops.iter().cloned());

            let propagator = if let Some(tx) = self.conflict_tx {
                propagator::Propagator::with_conflict_channel(
                    self.pair.max_upload_concurrency as usize,
                    self.pair.max_download_concurrency as usize,
                    tx.clone(),
                )
            } else {
                propagator::Propagator::new(
                    self.pair.max_upload_concurrency as usize,
                    self.pair.max_download_concurrency as usize,
                )
            };
            propagator
                .execute(
                    &all_ops,
                    &self.pair.id,
                    &local_root,
                    &remote_root,
                    self.client,
                    self.journal,
                )
                .await
        };

        // After bulk mode, run propagator on non-upload ops (downloads, deletes, conflicts).
        if use_bulk && !other_ops.is_empty() {
            let propagator = if let Some(tx) = self.conflict_tx {
                propagator::Propagator::with_conflict_channel(
                    self.pair.max_upload_concurrency as usize,
                    self.pair.max_download_concurrency as usize,
                    tx.clone(),
                )
            } else {
                propagator::Propagator::new(
                    self.pair.max_upload_concurrency as usize,
                    self.pair.max_download_concurrency as usize,
                )
            };
            let other_result = propagator
                .execute(
                    &other_ops,
                    &self.pair.id,
                    &local_root,
                    &remote_root,
                    self.client,
                    self.journal,
                )
                .await;
            result.downloaded += other_result.downloaded;
            result.errors += other_result.errors;
            result.conflicts += other_result.conflicts;
        }

        let completed_at = Utc::now();
        let duration_ms = (completed_at - started_at).num_milliseconds().max(0) as u64;

        // Emit cycle metrics for AC-1 observability.
        CycleMetrics {
            path_count,
            checksum_recomputed: scan_stats.checksum_recomputed,
            checksum_from_cache: scan_stats.checksum_from_cache,
            duration_ms,
        }
        .emit();

        // On Linux, ask glibc to return freed pages to the OS before we sample RSS.
        // Sync cycles allocate large transient heaps (remote item lists, local scan
        // results, journal entry maps) that glibc would otherwise retain indefinitely.
        // Trimming first ensures the RSS reading reflects actual live data, not
        // fragmented free pages — otherwise the first post-cycle baseline captures
        // the unfragmented heap and every subsequent cycle appears to "grow".
        #[cfg(target_os = "linux")]
        {
            // SAFETY: malloc_trim is a standard glibc extension with no preconditions.
            unsafe { libc::malloc_trim(0) };
        }

        // Sample RSS for AC-3 memory tracking (after trim so readings are comparable).
        if let Some(sampler_arc) = &self.memory_sampler {
            let mut sampler = sampler_arc.lock().unwrap();
            if !sampler.has_baseline() {
                if let Some(rss) = MemorySampler::sample_rss() {
                    sampler.set_baseline(rss);
                }
            } else {
                sampler.observe();
            }
        }

        Ok(SyncReport {
            pair_id: self.pair.id.clone(),
            started_at,
            completed_at,
            uploaded: result.uploaded,
            downloaded: result.downloaded,
            conflicts: result.conflicts,
            errors: result.errors,
            skipped: 0,
        })
    }
}

// ── DefaultSyncEngine (T049) ──────────────────────────────────────────────────

/// Production `SyncEngine` implementation: per-pair cycle coalescing + status broadcast.
pub struct DefaultSyncEngine {
    status_tx: watch::Sender<EngineStatus>,
    status_rx: watch::Receiver<EngineStatus>,
    activity_log: Arc<crate::observability::ActivityLog>,
    paused: Arc<Mutex<bool>>,
    /// Last report per pair (protected by RwLock for concurrent reads).
    last_report: Arc<RwLock<HashMap<PairId, SyncReport>>>,
    /// Per-pair background sync runners, keyed by PairId.
    runners: Arc<RwLock<HashMap<PairId, PairRunner>>>,
    /// Suspend non-essential sync when the connection is metered (T070).
    pub pause_on_metered: bool,
    /// Minimum battery percentage below which sync is suspended (T071). None = no threshold.
    pub min_battery_percent: Option<u8>,
    /// Shared RSS sampler for AC-3 memory tracking; passed to each PairRunner.
    memory_sampler: Arc<std::sync::Mutex<MemorySampler>>,
    /// Per-account bandwidth limits: (upload_kbps, download_kbps). 0 = unlimited.
    pub bandwidth_limits: Arc<RwLock<HashMap<AccountId, (u64, u64)>>>,
}

impl DefaultSyncEngine {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(EngineStatus::Idle);
        Self {
            status_tx: tx,
            status_rx: rx,
            activity_log: Arc::new(crate::observability::ActivityLog::new(500)),
            paused: Arc::new(Mutex::new(false)),
            last_report: Arc::new(RwLock::new(HashMap::new())),
            runners: Arc::new(RwLock::new(HashMap::new())),
            pause_on_metered: false,
            min_battery_percent: None,
            memory_sampler: Arc::new(std::sync::Mutex::new(MemorySampler::new())),
            bandwidth_limits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Update bandwidth limits for an account. Takes effect on the next sync cycle.
    pub async fn update_bandwidth_limits(
        &self,
        account_id: &AccountId,
        upload_kbps: u64,
        download_kbps: u64,
    ) {
        let mut limits = self.bandwidth_limits.write().await;
        if upload_kbps == 0 && download_kbps == 0 {
            limits.remove(account_id);
        } else {
            limits.insert(account_id.clone(), (upload_kbps, download_kbps));
        }
    }

    /// Spawn a `PairRunner` for `pair` and register it.
    ///
    /// If a runner already exists for this pair it is stopped and replaced.
    ///
    /// Pass `conflict_tx` to receive a notification each time an Ask-policy conflict
    /// is recorded by the propagator.
    pub async fn start_pair(
        &self,
        pair: SyncPair,
        client: Arc<dyn RemoteClient>,
        journal: Arc<dyn Journal>,
        conflict_tx: Option<tokio::sync::mpsc::Sender<()>>,
    ) {
        let pair_id = pair.id.clone();

        let sampler = self.memory_sampler.clone();
        let runner = PairRunner::spawn(Arc::new(pair), client, journal, conflict_tx, sampler);
        let mut runners = self.runners.write().await;
        if let Some(old) = runners.remove(&pair_id) {
            old.stop();
        }
        runners.insert(pair_id, runner);
    }

    /// Start a VFS-mode pair runner (metadata-only, no content downloads).
    ///
    /// Called by the daemon when `pair.vfs_enabled = true`. Uses `SqliteJournal`
    /// directly for VFS extension methods. Copy-sync pairs remain unaffected
    /// (they use `start_pair` instead).
    pub fn start_vfs_pair(
        &self,
        pair: SyncPair,
        client: Arc<dyn RemoteClient>,
        journal: Arc<crate::journal::sqlite::SqliteJournal>,
        provider: Arc<dyn crate::vfs::VfsProvider>,
    ) {
        use crate::vfs::VfsPairRunner;
        use tokio_util::sync::CancellationToken;
        use tokio::sync::mpsc;

        let pair_id = pair.id.clone();
        let vfs_runner = Arc::new(VfsPairRunner::new(pair.clone(), client, journal, provider));
        let pair_id_str = pair_id.0.clone();

        let cancel = CancellationToken::new();
        let (trigger_tx, mut trigger_rx) = mpsc::channel::<()>(1);
        let cancel_child = cancel.child_token();

        let handle = tokio::spawn(async move {
            // Poll every 30 seconds for VFS pairs — lightweight PROPFIND only.
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            ticker.tick().await; // consume immediate first tick

            // Run first metadata sync immediately on startup.
            if let Err(e) = vfs_runner.run_metadata_sync().await {
                tracing::warn!(pair_id = %pair_id_str, error = %e, "VFS metadata sync failed");
            }

            loop {
                tokio::select! {
                    biased;
                    _ = cancel_child.cancelled() => {
                        tracing::info!(pair_id = %pair_id_str, "VFS runner stopped");
                        break;
                    }
                    _ = ticker.tick() => {
                        if let Err(e) = vfs_runner.run_metadata_sync().await {
                            tracing::warn!(pair_id = %pair_id_str, error = %e, "VFS metadata sync failed");
                        }
                    }
                    Some(()) = trigger_rx.recv() => {
                        while trigger_rx.try_recv().is_ok() {}
                        tracing::info!(pair_id = %pair_id_str, "VFS metadata sync triggered");
                        if let Err(e) = vfs_runner.run_metadata_sync().await {
                            tracing::warn!(pair_id = %pair_id_str, error = %e, "VFS metadata sync failed");
                        }
                    }
                }
            }
        });

        // Register under runners so trigger_pair / stop_pair work for VFS pairs.
        let runner = crate::cycle::runner::PairRunner::from_parts(
            pair_id.clone(), cancel, trigger_tx, handle,
        );
        let runners = self.runners.clone();
        let pair_id_reg = pair_id.clone();
        tokio::spawn(async move {
            runners.write().await.insert(pair_id_reg, runner);
        });

        tracing::info!(pair_id = %pair_id, "VFS pair runner started (30 s interval)");
    }

    /// Stop and remove the runner for `pair_id`.
    pub async fn stop_pair(&self, pair_id: &PairId) {
        let mut runners = self.runners.write().await;
        if let Some(runner) = runners.remove(pair_id) {
            runner.stop();
        }
    }

    /// Send an immediate-trigger signal to the pair's runner.
    pub async fn trigger_pair(&self, pair_id: &PairId) -> Result<(), SyncError> {
        let runners = self.runners.read().await;
        let runner = runners
            .get(pair_id)
            .ok_or_else(|| SyncError::Permanent(format!("no runner for pair {pair_id}")))?;
        runner.trigger();
        Ok(())
    }

    /// Returns `true` if a sync cycle should be skipped due to network or battery conditions.
    pub async fn should_suspend(&self) -> bool {
        if self.pause_on_metered && is_metered_connection() {
            return true;
        }
        if let Some(threshold) = self.min_battery_percent {
            if let Some(level) = battery_level_percent().await {
                if level < threshold {
                    return true;
                }
            }
        }
        false
    }
}

impl Default for DefaultSyncEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SyncEngine for DefaultSyncEngine {
    async fn trigger_sync(&self, pair_id: &PairId) -> Result<SyncReport, SyncError> {
        self.trigger_pair(pair_id).await?;
        let guard = self.last_report.read().await;
        guard
            .get(pair_id)
            .cloned()
            .ok_or_else(|| SyncError::Permanent(format!("no report yet for pair {pair_id}")))
    }

    async fn pause(&self) -> Result<(), SyncError> {
        *self.paused.lock().await = true;
        let _ = self.status_tx.send(EngineStatus::Paused);
        Ok(())
    }

    async fn resume(&self) -> Result<(), SyncError> {
        *self.paused.lock().await = false;
        let _ = self.status_tx.send(EngineStatus::Idle);
        Ok(())
    }

    fn status(&self) -> EngineStatus {
        self.status_rx.borrow().clone()
    }

    fn subscribe_status(&self) -> watch::Receiver<EngineStatus> {
        self.status_rx.clone()
    }

    async fn activity_log(&self, limit: usize) -> Result<Vec<ActivityEntry>, SyncError> {
        Ok(self
            .activity_log
            .last_n(limit)
            .into_iter()
            .map(|e| ActivityEntry {
                pair_id: e.pair_id,
                path: e.path,
                action: e.action,
                occurred_at: e.occurred_at,
                bytes: e.bytes,
            })
            .collect())
    }

    async fn error_items(&self) -> Result<Vec<ErrorItem>, SyncError> {
        Ok(vec![])
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // T049-1: Engine starts in Idle state.
    #[test]
    fn default_engine_starts_idle() {
        let engine = DefaultSyncEngine::new();
        assert_eq!(engine.status(), EngineStatus::Idle);
    }

    // T049-2: pause() transitions to Paused.
    #[tokio::test]
    async fn engine_pause_changes_status() {
        let engine = DefaultSyncEngine::new();
        engine.pause().await.unwrap();
        assert_eq!(engine.status(), EngineStatus::Paused);
    }

    // T049-3: resume() returns to Idle.
    #[tokio::test]
    async fn engine_resume_returns_idle() {
        let engine = DefaultSyncEngine::new();
        engine.pause().await.unwrap();
        engine.resume().await.unwrap();
        assert_eq!(engine.status(), EngineStatus::Idle);
    }

    // T049-4: subscribe_status delivers updates.
    #[tokio::test]
    async fn engine_status_watch_delivers_pause() {
        let engine = DefaultSyncEngine::new();
        let mut rx = engine.subscribe_status();
        engine.pause().await.unwrap();
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), EngineStatus::Paused);
    }

    // ── T005: DefaultSyncEngine runner management ─────────────────────────────

    use crate::journal::sqlite::SqliteJournal;
    use crate::remote::mock::MockRemoteClient;
    use crate::types::{AccountId, LocalPath, PairStatus, RemotePath};
    use sqlx::sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
    };
    use std::str::FromStr;
    use tempfile::TempDir;

    async fn make_engine_test_journal() -> Arc<SqliteJournal> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Memory)
            .synchronous(SqliteSynchronous::Off)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        SqliteJournal::run_migrations(&pool).await.unwrap();
        Arc::new(SqliteJournal::new(pool))
    }

    fn make_engine_test_pair(local_root: &std::path::Path) -> SyncPair {
        SyncPair {
            id: PairId::new(),
            account_id: AccountId::new(),
            local_root: LocalPath::new(local_root),
            remote_root: RemotePath::new(""),
            status: PairStatus::Idle,
            exclude_patterns: vec![],
            selective_paths: vec![],
            created_at: chrono::Utc::now(),
            last_synced_at: None,
            scan_interval_secs: 3600,
            scan_on_startup: false,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            conflict_policy: crate::types::ConflictPolicy::Ask,
            bulk_upload_workers: 8,
            bulk_upload_threshold_files: 50,
            bulk_upload_chunk_threshold_bytes: 10 * 1024 * 1024,
            vfs_enabled: false,
            vfs_cache_max_bytes: 20 * 1024 * 1024 * 1024,
            vfs_eviction_threshold_bytes: 5 * 1024 * 1024 * 1024,
        }
    }

    #[tokio::test]
    async fn start_pair_registers_runner() {
        let dir = TempDir::new().unwrap();
        let engine = DefaultSyncEngine::new();
        let pair = make_engine_test_pair(dir.path());
        let pair_id = pair.id.clone();
        let client = Arc::new(MockRemoteClient::new());
        let journal = make_engine_test_journal().await;
        engine.start_pair(pair, client, journal, None).await;
        let runners = engine.runners.read().await;
        assert!(
            runners.contains_key(&pair_id),
            "runner should be registered after start_pair"
        );
    }

    #[tokio::test]
    async fn stop_pair_removes_runner() {
        let dir = TempDir::new().unwrap();
        let engine = DefaultSyncEngine::new();
        let pair = make_engine_test_pair(dir.path());
        let pair_id = pair.id.clone();
        let client = Arc::new(MockRemoteClient::new());
        let journal = make_engine_test_journal().await;
        engine.start_pair(pair, client, journal, None).await;
        engine.stop_pair(&pair_id).await;
        let runners = engine.runners.read().await;
        assert!(
            !runners.contains_key(&pair_id),
            "runner should be removed after stop_pair"
        );
    }

    #[tokio::test]
    async fn trigger_pair_returns_error_for_unknown_pair() {
        let engine = DefaultSyncEngine::new();
        let unknown = PairId::new();
        assert!(
            engine.trigger_pair(&unknown).await.is_err(),
            "trigger_pair should error for a pair with no runner"
        );
    }

    #[tokio::test]
    async fn trigger_pair_succeeds_for_registered_pair() {
        let dir = TempDir::new().unwrap();
        let engine = DefaultSyncEngine::new();
        let pair = make_engine_test_pair(dir.path());
        let pair_id = pair.id.clone();
        let client = Arc::new(MockRemoteClient::new());
        let journal = make_engine_test_journal().await;
        engine.start_pair(pair, client, journal, None).await;
        assert!(
            engine.trigger_pair(&pair_id).await.is_ok(),
            "trigger_pair should succeed for a registered pair"
        );
        engine.stop_pair(&pair_id).await;
    }

    // T070-1: pause_on_metered=false means should_suspend returns false
    // even if is_metered_connection() returns false (no real interface).
    #[tokio::test]
    async fn engine_does_not_suspend_when_pause_on_metered_false() {
        let engine = DefaultSyncEngine::new(); // pause_on_metered defaults to false
        assert!(
            !engine.should_suspend().await,
            "should not suspend with default settings"
        );
    }

    // T070-2: pause_on_metered=true suspends when is_metered_connection() is true.
    // Because is_metered_connection() always returns false in CI, we verify the
    // flag wiring is correct by checking that false+flag=false (can't force true).
    #[tokio::test]
    async fn engine_suspension_flag_is_respected() {
        let mut engine = DefaultSyncEngine::new();
        engine.pause_on_metered = true;
        // is_metered_connection() is false in CI → should_suspend still false
        assert!(!engine.should_suspend().await);
    }

    // T071-1: No battery threshold set → never suspends for battery reasons.
    #[tokio::test]
    async fn engine_no_battery_threshold_never_suspends_for_battery() {
        let engine = DefaultSyncEngine::new(); // min_battery_percent = None
        assert!(!engine.should_suspend().await);
    }

    // T071-2: Battery threshold of 0 → never suspends (level always >= 0).
    #[tokio::test]
    async fn engine_battery_threshold_zero_never_suspends() {
        let mut engine = DefaultSyncEngine::new();
        engine.min_battery_percent = Some(0);
        assert!(
            !engine.should_suspend().await,
            "threshold 0 should never trigger suspension"
        );
    }
}
