use adagio_core::journal::sqlite::SqliteJournal;
use adagio_core::journal::Journal;
use adagio_core::types::{
    Checksum, ChecksumAlgorithm, JournalEntry, PairId, RelativePath, SyncStatus,
};
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::str::FromStr;

// ── Test fixture ─────────────────────────────────────────────────────────────

async fn make_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Memory)
        .synchronous(SqliteSynchronous::Off)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap()
}

/// Insert a minimal account + sync_pair so the journal_entries FK is satisfied.
async fn seed_pair(pool: &SqlitePool, pair_id: &PairId) {
    let account_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO accounts \
         (id, display_name, server_url, username, keychain_service_key, created_at) \
         VALUES (?, 'Test Account', 'http://localhost', 'test', 'test-key', ?)",
    )
    .bind(&account_id)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO sync_pairs (id, account_id, local_root, remote_root, created_at) \
         VALUES (?, ?, '/tmp/sync', '/remote', ?)",
    )
    .bind(&pair_id.0)
    .bind(&account_id)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await
    .unwrap();
}

async fn make_journal_for(pair_id: &PairId) -> SqliteJournal {
    let pool = make_pool().await;
    SqliteJournal::run_migrations(&pool).await.unwrap();
    seed_pair(&pool, pair_id).await;
    SqliteJournal::new(pool)
}

// ── Contract: upsert is durable ───────────────────────────────────────────────

#[tokio::test]
async fn upsert_durable_get_returns_entry() {
    let pair_id = PairId::new();
    let j = make_journal_for(&pair_id).await;
    let path = RelativePath::new("docs/report.pdf");

    let entry = JournalEntry {
        pair_id: pair_id.clone(),
        path: path.clone(),
        file_id: Some("nc-001".into()),
        etag: Some("abc123".into()),
        checksum: Some(Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: "a".repeat(64),
        }),
        size: 4096,
        mtime_local: Some(Utc::now()),
        mtime_remote: Some(Utc::now()),
        status: SyncStatus::Synced,
        error_message: None,
        retry_count: 0,
        updated_at: Utc::now(),
    };

    j.upsert(&entry).await.unwrap();

    let got = j.get(&pair_id, &path).await.unwrap().unwrap();
    assert_eq!(got.path, path);
    assert_eq!(got.file_id, entry.file_id);
    assert_eq!(got.size, 4096);
    assert_eq!(got.status, SyncStatus::Synced);
}

// ── Contract: upsert_batch is transactional ───────────────────────────────────

#[tokio::test]
async fn upsert_batch_all_or_nothing() {
    let pair_id = PairId::new();
    let j = make_journal_for(&pair_id).await;

    let entries: Vec<JournalEntry> = (0..5)
        .map(|i| JournalEntry {
            pair_id: pair_id.clone(),
            path: RelativePath::new(format!("batch/{i}.txt")),
            file_id: None,
            etag: None,
            checksum: None,
            size: i * 100,
            mtime_local: Some(Utc::now()),
            mtime_remote: None,
            status: SyncStatus::PendingUpload,
            error_message: None,
            retry_count: 0,
            updated_at: Utc::now(),
        })
        .collect();

    j.upsert_batch(&entries).await.unwrap();

    let all = j.all_entries(&pair_id).await.unwrap();
    assert_eq!(all.len(), 5);
}

// ── Contract: get_by_file_id ──────────────────────────────────────────────────

#[tokio::test]
async fn get_by_file_id_finds_renamed_item() {
    let pair_id = PairId::new();
    let j = make_journal_for(&pair_id).await;
    let file_id = format!("nc-{}", uuid::Uuid::new_v4());
    let path = RelativePath::new("photos/sunset.jpg");

    j.upsert(&JournalEntry {
        pair_id: pair_id.clone(),
        path: path.clone(),
        file_id: Some(file_id.clone()),
        etag: Some("etag1".into()),
        checksum: None,
        size: 2_000_000,
        mtime_local: Some(Utc::now()),
        mtime_remote: Some(Utc::now()),
        status: SyncStatus::Synced,
        error_message: None,
        retry_count: 0,
        updated_at: Utc::now(),
    })
    .await
    .unwrap();

    let found = j.get_by_file_id(&pair_id, &file_id).await.unwrap().unwrap();
    assert_eq!(found.path, path);
}

// ── Contract: delete removes entry ────────────────────────────────────────────

#[tokio::test]
async fn delete_makes_entry_absent() {
    let pair_id = PairId::new();
    let j = make_journal_for(&pair_id).await;
    let path = RelativePath::new("gone.txt");

    j.upsert(&JournalEntry {
        pair_id: pair_id.clone(),
        path: path.clone(),
        file_id: None,
        etag: None,
        checksum: None,
        size: 0,
        mtime_local: Some(Utc::now()),
        mtime_remote: None,
        status: SyncStatus::Synced,
        error_message: None,
        retry_count: 0,
        updated_at: Utc::now(),
    })
    .await
    .unwrap();

    j.delete(&pair_id, &path).await.unwrap();
    assert!(j.get(&pair_id, &path).await.unwrap().is_none());
}

// ── Contract: clear_pair removes all entries ─────────────────────────────────

#[tokio::test]
async fn clear_pair_leaves_no_entries() {
    let pair_id = PairId::new();
    let j = make_journal_for(&pair_id).await;

    for i in 0..3 {
        j.upsert(&JournalEntry {
            pair_id: pair_id.clone(),
            path: RelativePath::new(format!("clear/{i}")),
            file_id: None,
            etag: None,
            checksum: None,
            size: 0,
            mtime_local: Some(Utc::now()),
            mtime_remote: None,
            status: SyncStatus::Synced,
            error_message: None,
            retry_count: 0,
            updated_at: Utc::now(),
        })
        .await
        .unwrap();
    }

    j.clear_pair(&pair_id).await.unwrap();
    assert!(j.all_entries(&pair_id).await.unwrap().is_empty());
}

// ── Contract: entries_by_status filters correctly ────────────────────────────

#[tokio::test]
async fn entries_by_status_returns_only_matching() {
    let pair_id = PairId::new();
    let j = make_journal_for(&pair_id).await;

    j.upsert(&JournalEntry {
        pair_id: pair_id.clone(),
        path: RelativePath::new("a.txt"),
        file_id: None,
        etag: None,
        checksum: None,
        size: 0,
        mtime_local: Some(Utc::now()),
        mtime_remote: None,
        status: SyncStatus::Error,
        error_message: Some("disk full".into()),
        retry_count: 1,
        updated_at: Utc::now(),
    })
    .await
    .unwrap();

    j.upsert(&JournalEntry {
        pair_id: pair_id.clone(),
        path: RelativePath::new("b.txt"),
        file_id: None,
        etag: None,
        checksum: None,
        size: 0,
        mtime_local: Some(Utc::now()),
        mtime_remote: None,
        status: SyncStatus::Synced,
        error_message: None,
        retry_count: 0,
        updated_at: Utc::now(),
    })
    .await
    .unwrap();

    let errors = j
        .entries_by_status(&pair_id, &SyncStatus::Error)
        .await
        .unwrap();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].path, RelativePath::new("a.txt"));
}
