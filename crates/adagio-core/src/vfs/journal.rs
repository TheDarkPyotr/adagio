use chrono::{DateTime, Utc};

use crate::error::JournalError;
use crate::journal::sqlite::SqliteJournal;
use crate::types::PairId;
use crate::vfs::types::{VfsCacheEntry, VfsState};

// ── VfsJournal extension ──────────────────────────────────────────────────────

impl SqliteJournal {
    /// Sync remote metadata (size, etag, mtime) for a file.
    ///
    /// INSERT as cloud_only for new files. For existing files, only updates the
    /// remote_* columns — never touches state, cached_at, last_accessed_at, or
    /// cache_bytes. This is the correct call for the metadata-sync loop so that
    /// locally-available and pinned states are never overwritten by a poll cycle.
    pub async fn sync_vfs_remote_metadata(
        &self,
        entry: &VfsCacheEntry,
    ) -> Result<(), JournalError> {
        sqlx::query(
            "INSERT INTO vfs_cache_metadata
             (pair_id, path, remote_size, remote_etag, remote_mtime,
              state, cached_at, last_accessed_at, cache_bytes)
             VALUES (?, ?, ?, ?, ?, 'cloud_only', NULL, NULL, 0)
             ON CONFLICT(pair_id, path) DO UPDATE SET
               remote_size  = excluded.remote_size,
               remote_etag  = excluded.remote_etag,
               remote_mtime = excluded.remote_mtime",
        )
        .bind(&entry.pair_id.0)
        .bind(entry.path.as_str())
        .bind(entry.remote_size as i64)
        .bind(&entry.remote_etag)
        .bind(entry.remote_mtime.to_rfc3339())
        .execute(self.pool())
        .await
        .map_err(JournalError::Db)?;
        Ok(())
    }

    /// Insert or update a VFS cache metadata row (full update — all fields).
    pub async fn upsert_vfs_entry(&self, entry: &VfsCacheEntry) -> Result<(), JournalError> {
        let (state_str, cached_at, last_accessed_at) = match &entry.state {
            VfsState::CloudOnly => ("cloud_only", None::<String>, None::<String>),
            VfsState::LocallyAvailable {
                cached_at,
                last_accessed,
            } => (
                "locally_available",
                Some(cached_at.to_rfc3339()),
                Some(last_accessed.to_rfc3339()),
            ),
            VfsState::Pinned {
                cached_at,
                last_accessed,
            } => (
                "pinned",
                Some(cached_at.to_rfc3339()),
                Some(last_accessed.to_rfc3339()),
            ),
        };

        sqlx::query(
            "INSERT INTO vfs_cache_metadata
             (pair_id, path, remote_size, remote_etag, remote_mtime,
              state, cached_at, last_accessed_at, cache_bytes)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(pair_id, path) DO UPDATE SET
               remote_size      = excluded.remote_size,
               remote_etag      = excluded.remote_etag,
               remote_mtime     = excluded.remote_mtime,
               state            = excluded.state,
               cached_at        = excluded.cached_at,
               last_accessed_at = excluded.last_accessed_at,
               cache_bytes      = excluded.cache_bytes",
        )
        .bind(&entry.pair_id.0)
        .bind(entry.path.as_str())
        .bind(entry.remote_size as i64)
        .bind(&entry.remote_etag)
        .bind(entry.remote_mtime.to_rfc3339())
        .bind(state_str)
        .bind(cached_at)
        .bind(last_accessed_at)
        .bind(entry.cache_bytes as i64)
        .execute(self.pool())
        .await
        .map_err(JournalError::Db)?;

        Ok(())
    }

    /// Return all VFS cache entries for a pair.
    pub async fn all_vfs_entries(
        &self,
        pair_id: &PairId,
    ) -> Result<Vec<VfsCacheEntry>, JournalError> {
        let rows = sqlx::query(
            "SELECT pair_id, path, remote_size, remote_etag, remote_mtime,
                    state, cached_at, last_accessed_at, cache_bytes
             FROM vfs_cache_metadata WHERE pair_id = ?",
        )
        .bind(&pair_id.0)
        .fetch_all(self.pool())
        .await
        .map_err(JournalError::Db)?;

        rows.iter().map(row_to_entry).collect()
    }

    /// Return a single VFS cache entry, or None if not found.
    pub async fn get_vfs_entry(
        &self,
        pair_id: &PairId,
        path: &str,
    ) -> Result<Option<VfsCacheEntry>, JournalError> {
        let row = sqlx::query(
            "SELECT pair_id, path, remote_size, remote_etag, remote_mtime,
                    state, cached_at, last_accessed_at, cache_bytes
             FROM vfs_cache_metadata WHERE pair_id = ? AND path = ?",
        )
        .bind(&pair_id.0)
        .bind(path)
        .fetch_optional(self.pool())
        .await
        .map_err(JournalError::Db)?;

        row.as_ref().map(row_to_entry).transpose()
    }

    /// Return least-recently-accessed locally-available entries for LRU eviction.
    pub async fn vfs_lru_candidates(
        &self,
        pair_id: &PairId,
        limit: usize,
    ) -> Result<Vec<VfsCacheEntry>, JournalError> {
        let rows = sqlx::query(
            "SELECT pair_id, path, remote_size, remote_etag, remote_mtime,
                    state, cached_at, last_accessed_at, cache_bytes
             FROM vfs_cache_metadata
             WHERE pair_id = ? AND state = 'locally_available'
             ORDER BY last_accessed_at ASC
             LIMIT ?",
        )
        .bind(&pair_id.0)
        .bind(limit as i64)
        .fetch_all(self.pool())
        .await
        .map_err(JournalError::Db)?;

        rows.iter().map(row_to_entry).collect()
    }

    /// True if the path (or any of its parents) is in `vfs_pinned_paths`.
    pub async fn is_path_pinned(&self, pair_id: &PairId, path: &str) -> Result<bool, JournalError> {
        let row: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM vfs_pinned_paths
             WHERE pair_id = ?
               AND (path = ? OR ? LIKE (path || '/%'))
             LIMIT 1",
        )
        .bind(&pair_id.0)
        .bind(path)
        .bind(path)
        .fetch_optional(self.pool())
        .await
        .map_err(JournalError::Db)?;

        Ok(row.is_some())
    }

    /// Insert a pinned-path row (idempotent).
    pub async fn pin_path(
        &self,
        pair_id: &PairId,
        path: &str,
        pinned_at: DateTime<Utc>,
    ) -> Result<(), JournalError> {
        sqlx::query(
            "INSERT OR IGNORE INTO vfs_pinned_paths (pair_id, path, pinned_at) VALUES (?, ?, ?)",
        )
        .bind(&pair_id.0)
        .bind(path)
        .bind(pinned_at.to_rfc3339())
        .execute(self.pool())
        .await
        .map_err(JournalError::Db)?;
        Ok(())
    }

    /// Remove a pinned-path row.
    pub async fn unpin_path(&self, pair_id: &PairId, path: &str) -> Result<(), JournalError> {
        sqlx::query("DELETE FROM vfs_pinned_paths WHERE pair_id = ? AND path = ?")
            .bind(&pair_id.0)
            .bind(path)
            .execute(self.pool())
            .await
            .map_err(JournalError::Db)?;
        Ok(())
    }

    /// Aggregate VFS cache stats for a pair.
    pub async fn vfs_stats(&self, pair_id: &PairId) -> Result<(u64, u64, u64, u64), JournalError> {
        // Returns (cloud_only, locally_available, pinned, cached_bytes)
        let row: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
               SUM(CASE WHEN state = 'cloud_only' THEN 1 ELSE 0 END),
               SUM(CASE WHEN state = 'locally_available' THEN 1 ELSE 0 END),
               SUM(CASE WHEN state = 'pinned' THEN 1 ELSE 0 END),
               SUM(cache_bytes)
             FROM vfs_cache_metadata WHERE pair_id = ?",
        )
        .bind(&pair_id.0)
        .fetch_one(self.pool())
        .await
        .map_err(JournalError::Db)?;

        Ok((row.0 as u64, row.1 as u64, row.2 as u64, row.3 as u64))
    }

    /// Delete a VFS cache metadata row (used after eviction).
    pub async fn delete_vfs_entry(&self, pair_id: &PairId, path: &str) -> Result<(), JournalError> {
        sqlx::query("DELETE FROM vfs_cache_metadata WHERE pair_id = ? AND path = ?")
            .bind(&pair_id.0)
            .bind(path)
            .execute(self.pool())
            .await
            .map_err(JournalError::Db)?;
        Ok(())
    }
}

// ── Row → entry conversion ────────────────────────────────────────────────────

fn row_to_entry(row: &sqlx::sqlite::SqliteRow) -> Result<VfsCacheEntry, JournalError> {
    use crate::types::{PairId, RelativePath};
    use sqlx::Row;

    let pair_id_str: String = row.try_get("pair_id").map_err(JournalError::Db)?;
    let path_str: String = row.try_get("path").map_err(JournalError::Db)?;
    let remote_size: i64 = row.try_get("remote_size").map_err(JournalError::Db)?;
    let remote_etag: Option<String> = row.try_get("remote_etag").map_err(JournalError::Db)?;
    let remote_mtime_str: String = row.try_get("remote_mtime").map_err(JournalError::Db)?;
    let state_str: String = row.try_get("state").map_err(JournalError::Db)?;
    let cached_at_str: Option<String> = row.try_get("cached_at").map_err(JournalError::Db)?;
    let last_accessed_str: Option<String> =
        row.try_get("last_accessed_at").map_err(JournalError::Db)?;
    let cache_bytes: i64 = row.try_get("cache_bytes").map_err(JournalError::Db)?;

    let remote_mtime = remote_mtime_str
        .parse::<DateTime<Utc>>()
        .unwrap_or(Utc::now());

    let parse_dt = |s: Option<String>| {
        s.and_then(|s| s.parse::<DateTime<Utc>>().ok())
            .unwrap_or(Utc::now())
    };

    let state = match state_str.as_str() {
        "locally_available" => VfsState::LocallyAvailable {
            cached_at: parse_dt(cached_at_str),
            last_accessed: parse_dt(last_accessed_str),
        },
        "pinned" => VfsState::Pinned {
            cached_at: parse_dt(cached_at_str),
            last_accessed: parse_dt(last_accessed_str),
        },
        _ => VfsState::CloudOnly,
    };

    Ok(VfsCacheEntry {
        pair_id: PairId(pair_id_str),
        path: RelativePath::new(&path_str),
        remote_size: remote_size as u64,
        remote_etag,
        remote_mtime,
        state,
        cache_bytes: cache_bytes as u64,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::sqlite::SqliteJournal;
    use crate::types::{PairId, RelativePath};
    use crate::vfs::types::VfsCacheEntry;

    async fn make_journal() -> SqliteJournal {
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
        SqliteJournal::new(pool)
    }

    async fn register_pair(journal: &SqliteJournal, pair_id: &PairId) {
        use crate::types::{Account, AccountId};
        use chrono::Utc;
        let acct = Account {
            id: AccountId::new(),
            display_name: "test".to_string(),
            server_url: "https://test.example.com".to_string(),
            username: "test".to_string(),
            keychain_service_key: "test".to_string(),
            created_at: Utc::now(),
            upload_limit_kbps: 0,
            download_limit_kbps: 0,
        };
        use crate::types::{ConflictPolicy, LocalPath, PairStatus, RemotePath, SyncPair};
        let pair = SyncPair {
            id: pair_id.clone(),
            account_id: acct.id.clone(),
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
            conflict_policy: ConflictPolicy::Ask,
            bulk_upload_workers: 8,
            bulk_upload_threshold_files: 50,
            bulk_upload_chunk_threshold_bytes: 10 * 1024 * 1024,
            vfs_enabled: false,
            vfs_cache_max_bytes: 20 * 1024 * 1024 * 1024,
            vfs_eviction_threshold_bytes: 5 * 1024 * 1024 * 1024,
            e2ee_enabled: false,
            e2ee_account_id: None,
        };
        journal.register_pair(&acct, &pair).await.unwrap();
    }

    // T006-a: Upsert writes an entry and get_vfs_entry returns it.
    #[tokio::test]
    async fn vfs_journal_upsert_returns_entry() {
        let journal = make_journal().await;
        let pair_id = PairId::new();
        register_pair(&journal, &pair_id).await;

        let entry = VfsCacheEntry::new_cloud_only(
            pair_id.clone(),
            RelativePath::new("docs/file.pdf"),
            1024,
            Some("etag1".to_string()),
            Utc::now(),
        );
        journal.upsert_vfs_entry(&entry).await.unwrap();

        let loaded = journal
            .get_vfs_entry(&pair_id, "docs/file.pdf")
            .await
            .unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.remote_size, 1024);
        assert_eq!(loaded.state, VfsState::CloudOnly);
        assert_eq!(loaded.cache_bytes, 0);
    }

    // T006-b: LRU query orders by last_accessed_at ascending.
    #[tokio::test]
    async fn vfs_journal_lru_query_orders_by_last_accessed() {
        let journal = make_journal().await;
        let pair_id = PairId::new();
        register_pair(&journal, &pair_id).await;

        let base = Utc::now();
        for i in 0..5 {
            let mut entry = VfsCacheEntry::new_cloud_only(
                pair_id.clone(),
                RelativePath::new(format!("file-{i}.bin")),
                100,
                None,
                base,
            );
            let accessed = base + chrono::Duration::seconds(i * 100);
            entry.state = VfsState::LocallyAvailable {
                cached_at: base,
                last_accessed: accessed,
            };
            entry.cache_bytes = 100;
            journal.upsert_vfs_entry(&entry).await.unwrap();
        }

        let candidates = journal.vfs_lru_candidates(&pair_id, 3).await.unwrap();
        assert_eq!(candidates.len(), 3);
        // First candidate should be the oldest (file-0.bin, accessed = base + 0s)
        assert_eq!(candidates[0].path.as_str(), "file-0.bin");
        assert_eq!(candidates[2].path.as_str(), "file-2.bin");
    }
}
