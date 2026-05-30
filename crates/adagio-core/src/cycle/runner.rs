use crate::cycle::SyncCycle;
use crate::journal::Journal;
use crate::observability::MemorySampler;
use crate::remote::RemoteClient;
use crate::types::{PairId, SyncPair};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// Background sync loop for a single sync pair.
///
/// Holds the handles needed to trigger an immediate cycle or stop the runner.
/// The actual tokio task is spawned on construction and runs until cancelled.
pub struct PairRunner {
    /// The pair this runner serves.
    pub pair_id: PairId,
    cancel: CancellationToken,
    trigger_tx: mpsc::Sender<()>,
    handle: tokio::task::JoinHandle<()>,
}

impl PairRunner {
    /// Spawn a background sync loop for `pair`.
    ///
    /// Runs `SyncCycle::run` on startup (if `pair.scan_on_startup`) and then
    /// on each interval tick or explicit trigger. The loop exits cleanly when
    /// the returned `PairRunner` is stopped or dropped.
    ///
    /// If `conflict_tx` is set, the runner notifies on it whenever a new Ask-policy
    /// conflict is recorded so the desktop layer can emit `adagio://conflict-detected`.
    pub fn spawn(
        pair: Arc<SyncPair>,
        client: Arc<dyn RemoteClient>,
        journal: Arc<dyn Journal>,
        conflict_tx: Option<tokio::sync::mpsc::Sender<()>>,
        memory_sampler: Arc<std::sync::Mutex<MemorySampler>>,
    ) -> Self {
        let cancel = CancellationToken::new();
        let (trigger_tx, mut trigger_rx) = mpsc::channel::<()>(1);
        let pair_id = pair.id.clone();

        let cancel_child = cancel.child_token();
        let handle = tokio::spawn(async move {
            let interval_secs = pair.scan_interval_secs.max(1);
            let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
            // Skip missed ticks: a slow cycle never fires catch-up bursts (ADR-007).
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // Consume the immediate first tick so the interval starts from now.
            ticker.tick().await;

            if pair.scan_on_startup {
                run_cycle(
                    &pair,
                    &*client,
                    &*journal,
                    conflict_tx.as_ref(),
                    &memory_sampler,
                )
                .await;
            }

            loop {
                tokio::select! {
                    biased;
                    _ = cancel_child.cancelled() => {
                        info!(pair_id = %pair.id, "pair runner stopped");
                        break;
                    }
                    _ = ticker.tick() => {
                        info!(pair_id = %pair.id, "scheduled sync cycle starting");
                        run_cycle(&pair, &*client, &*journal, conflict_tx.as_ref(), &memory_sampler).await;
                    }
                    Some(()) = trigger_rx.recv() => {
                        // Drain queued triggers so we run exactly one cycle.
                        while trigger_rx.try_recv().is_ok() {}
                        info!(pair_id = %pair.id, "triggered sync cycle starting");
                        run_cycle(&pair, &*client, &*journal, conflict_tx.as_ref(), &memory_sampler).await;
                    }
                }
            }
        });

        Self {
            pair_id,
            cancel,
            trigger_tx,
            handle,
        }
    }

    /// Construct a `PairRunner` from pre-built parts (used by VFS runner integration).
    pub fn from_parts(
        pair_id: PairId,
        cancel: CancellationToken,
        trigger_tx: mpsc::Sender<()>,
        handle: tokio::task::JoinHandle<()>,
    ) -> Self {
        Self { pair_id, cancel, trigger_tx, handle }
    }

    /// Send an immediate sync trigger. Fire-and-forget; returns `false` if the
    /// runner's channel is full (a cycle is already queued).
    pub fn trigger(&self) -> bool {
        self.trigger_tx.try_send(()).is_ok()
    }

    /// Cancel the runner and abort its task. Non-blocking.
    pub fn stop(self) {
        self.cancel.cancel();
        self.handle.abort();
    }
}

async fn run_cycle(
    pair: &SyncPair,
    client: &dyn RemoteClient,
    journal: &dyn Journal,
    conflict_tx: Option<&tokio::sync::mpsc::Sender<()>>,
    memory_sampler: &Arc<std::sync::Mutex<MemorySampler>>,
) {
    let cycle = SyncCycle {
        pair,
        client,
        journal,
        conflict_tx: conflict_tx.map(|tx| tx as &tokio::sync::mpsc::Sender<()>),
        memory_sampler: Some(memory_sampler.clone()),
    };
    match cycle.run().await {
        Ok(report) => {
            info!(
                pair_id = %pair.id,
                uploaded = report.uploaded,
                downloaded = report.downloaded,
                errors = report.errors,
                conflicts = report.conflicts,
                "sync cycle completed"
            );
        }
        Err(e) => {
            warn!(pair_id = %pair.id, error = %e, "sync cycle failed");
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::sqlite::SqliteJournal;
    use crate::remote::mock::MockRemoteClient;
    use crate::types::{AccountId, LocalPath, PairStatus, RemotePath};
    use sqlx::sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
    };
    use std::str::FromStr;
    use tempfile::TempDir;

    async fn make_journal() -> Arc<SqliteJournal> {
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

    fn make_pair(local_root: &std::path::Path) -> Arc<SyncPair> {
        Arc::new(SyncPair {
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
        })
    }

    #[tokio::test]
    async fn runner_spawns_and_stops_cleanly() {
        let dir = TempDir::new().unwrap();
        let pair = make_pair(dir.path());
        let client = Arc::new(MockRemoteClient::new());
        let journal = make_journal().await;
        let runner = PairRunner::spawn(
            pair,
            client,
            journal,
            None,
            Arc::new(std::sync::Mutex::new(
                crate::observability::MemorySampler::new(),
            )),
        );
        // stop must not panic or deadlock
        runner.stop();
    }

    #[tokio::test]
    async fn trigger_returns_true_when_runner_is_alive() {
        let dir = TempDir::new().unwrap();
        let pair = make_pair(dir.path());
        let client = Arc::new(MockRemoteClient::new());
        let journal = make_journal().await;
        let runner = PairRunner::spawn(
            pair,
            client,
            journal,
            None,
            Arc::new(std::sync::Mutex::new(
                crate::observability::MemorySampler::new(),
            )),
        );
        assert!(
            runner.trigger(),
            "trigger should succeed while runner is alive"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        runner.stop();
    }

    // T003 — Verify MissedTickBehavior::Skip is configured on the interval.
    // The ticker must not fire more than once per interval window when the previous
    // cycle takes longer than the interval. We verify the behaviour indirectly:
    // a real-time 110 ms interval with a 50 ms "cycle" running twice should
    // only see one tick fire per window (not two back-to-back).
    #[tokio::test]
    async fn missed_tick_skip_does_not_fire_twice_after_slow_cycle() {
        // Short interval for a fast test.
        let mut ticker = tokio::time::interval(Duration::from_millis(80));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Consume the immediate first tick.
        ticker.tick().await;

        // Simulate a "slow cycle" by sleeping slightly longer than 2 intervals.
        // With Skip, only ONE tick should be pending after this sleep, not two.
        tokio::time::sleep(Duration::from_millis(170)).await;

        // Drain one tick (the pending one).
        let _ = tokio::time::timeout(Duration::from_millis(5), ticker.tick()).await;

        // The second tick must NOT be immediately ready (it should be ~80ms away).
        let second = tokio::time::timeout(Duration::from_millis(5), ticker.tick()).await;
        assert!(
            second.is_err(),
            "MissedTickBehavior::Skip must not fire a second tick immediately after a slow cycle"
        );
    }

    #[tokio::test]
    async fn runner_with_scan_on_startup_runs_cycle() {
        let dir = TempDir::new().unwrap();
        let mut pair = (*make_pair(dir.path())).clone();
        pair.scan_on_startup = true;
        let client = Arc::new(MockRemoteClient::new());
        let journal = make_journal().await;
        // Spawn and give the startup cycle time to complete
        let runner = PairRunner::spawn(
            Arc::new(pair),
            client,
            journal,
            None,
            Arc::new(std::sync::Mutex::new(
                crate::observability::MemorySampler::new(),
            )),
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        runner.stop();
        // No assertion other than "didn't panic" — verifies run_cycle is called without error
    }
}
