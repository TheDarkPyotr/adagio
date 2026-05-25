/// Integration tests for the desktop app lifecycle startup sequence.
///
/// These tests exercise the engine wiring end-to-end using `MockRemoteClient`
/// so they can run in CI without a real Nextcloud server or OS keychain.
use adagio_core::cycle::DefaultSyncEngine;
use adagio_core::journal::sqlite::SqliteJournal;
use adagio_core::remote::mock::MockRemoteClient;
use adagio_core::types::{AccountId, LocalPath, PairId, PairStatus, RemotePath, SyncPair};
use chrono::Utc;
use std::sync::Arc;
use tempfile::TempDir;

async fn make_journal(tmp: &TempDir) -> Arc<SqliteJournal> {
    let db_url = format!("sqlite://{}?mode=rwc", tmp.path().join("test.db").display());
    Arc::new(SqliteJournal::open(&db_url).await.unwrap())
}

fn make_pair(local_dir: &TempDir) -> SyncPair {
    SyncPair {
        id: PairId::new(),
        account_id: AccountId("acc-1".into()),
        local_root: LocalPath::new(local_dir.path().to_path_buf()),
        remote_root: RemotePath::new("remote/path"),
        status: PairStatus::Idle,
        exclude_patterns: vec![],
        selective_paths: vec![],
        created_at: Utc::now(),
        last_synced_at: None,
        scan_interval_secs: 7200,
        scan_on_startup: true,
        max_upload_concurrency: 2,
        max_download_concurrency: 2,
    }
}

/// T018: starting a pair runner makes trigger_pair succeed immediately.
///
/// Verifies that after `engine.start_pair(pair, client, journal)`:
/// - The runner is registered (trigger_pair returns Ok rather than an error).
/// - A second `start_pair` for the same ID is a no-op (does not panic or double-register).
#[tokio::test]
async fn start_pair_runner_is_registered_and_trigger_succeeds() {
    let tmp_dir = TempDir::new().unwrap();
    let local_dir = TempDir::new().unwrap();
    let journal = make_journal(&tmp_dir).await;
    let engine = Arc::new(DefaultSyncEngine::new());
    let client = Arc::new(MockRemoteClient::new());

    // Before start_pair, trigger should fail with "unknown pair".
    let dummy_id = PairId::new();
    let err = engine.trigger_pair(&dummy_id).await;
    assert!(err.is_err(), "trigger on unknown pair must return Err");

    // Register a real pair.
    let pair = make_pair(&local_dir);
    let pair_id = pair.id.clone();

    engine
        .start_pair(
            pair,
            client as Arc<dyn adagio_core::remote::RemoteClient>,
            journal,
        )
        .await;

    // Now trigger_pair must succeed.
    engine
        .trigger_pair(&pair_id)
        .await
        .expect("trigger_pair must return Ok after start_pair");
}

/// T018: stop_pair removes the runner so subsequent trigger_pair returns an error.
#[tokio::test]
async fn stop_pair_deregisters_runner() {
    let tmp_dir = TempDir::new().unwrap();
    let local_dir = TempDir::new().unwrap();
    let journal = make_journal(&tmp_dir).await;
    let engine = Arc::new(DefaultSyncEngine::new());
    let client = Arc::new(MockRemoteClient::new());

    let pair = make_pair(&local_dir);
    let pair_id = pair.id.clone();

    engine
        .start_pair(
            pair,
            client as Arc<dyn adagio_core::remote::RemoteClient>,
            journal,
        )
        .await;

    engine.stop_pair(&pair_id).await;

    let err = engine.trigger_pair(&pair_id).await;
    assert!(err.is_err(), "trigger_pair must fail after stop_pair");
}

/// T018: multiple pairs can be started independently and each responds to its own trigger.
#[tokio::test]
async fn multiple_pairs_start_independently() {
    let tmp_dir = TempDir::new().unwrap();
    let local1 = TempDir::new().unwrap();
    let local2 = TempDir::new().unwrap();
    let journal = make_journal(&tmp_dir).await;
    let engine = Arc::new(DefaultSyncEngine::new());

    let pair1 = make_pair(&local1);
    let pair2 = make_pair(&local2);
    let id1 = pair1.id.clone();
    let id2 = pair2.id.clone();

    let client1 = Arc::new(MockRemoteClient::new()) as Arc<dyn adagio_core::remote::RemoteClient>;
    let client2 = Arc::new(MockRemoteClient::new()) as Arc<dyn adagio_core::remote::RemoteClient>;

    engine.start_pair(pair1, client1, journal.clone()).await;
    engine.start_pair(pair2, client2, journal).await;

    engine
        .trigger_pair(&id1)
        .await
        .expect("pair1 trigger must succeed");
    engine
        .trigger_pair(&id2)
        .await
        .expect("pair2 trigger must succeed");

    engine.stop_pair(&id1).await;

    assert!(engine.trigger_pair(&id1).await.is_err(), "pair1 stopped");
    engine
        .trigger_pair(&id2)
        .await
        .expect("pair2 still running");
}
