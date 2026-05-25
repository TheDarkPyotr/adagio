use super::Journal;
use crate::error::JournalError;
use crate::types::{
    Checksum, ChecksumAlgorithm, ConflictPolicy, ConflictRecord, ConflictResolution, JournalEntry,
    PairId, RelativePath, SyncStatus,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;
use tracing::instrument;

/// SQLite-backed journal. Uses WAL mode for concurrent reads.
pub struct SqliteJournal {
    pool: SqlitePool,
}

impl SqliteJournal {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a connection pool with appropriate pragmas for production use.
    pub async fn open(db_url: &str) -> Result<Self, JournalError> {
        let opts = SqliteConnectOptions::from_str(db_url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        Self::run_migrations(&pool).await?;
        Ok(Self { pool })
    }

    /// Run SQLx migrations against an already-connected pool.
    pub async fn run_migrations(pool: &SqlitePool) -> Result<(), JournalError> {
        sqlx::migrate!("./migrations")
            .run(pool)
            .await
            .map_err(|e| JournalError::Db(e.into()))?;
        Ok(())
    }

    /// Run `PRAGMA integrity_check` and return `Corruption` if the database is damaged.
    pub async fn integrity_check(&self) -> Result<(), JournalError> {
        let row: (String,) = sqlx::query_as("PRAGMA integrity_check")
            .fetch_one(&self.pool)
            .await?;
        if row.0 != "ok" {
            return Err(JournalError::Corruption(format!(
                "PRAGMA integrity_check reported: {}",
                row.0
            )));
        }
        Ok(())
    }

    /// Verify that the expected tables are present (structural schema check).
    ///
    /// Returns `JournalError::Corruption` if any required table is missing.
    pub async fn schema_check(&self) -> Result<(), JournalError> {
        const REQUIRED_TABLES: &[&str] = &[
            "accounts",
            "sync_pairs",
            "journal_entries",
            "conflict_records",
            "transfers",
        ];
        for table in REQUIRED_TABLES {
            let count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?")
                    .bind(table)
                    .fetch_one(&self.pool)
                    .await?;

            if count.0 == 0 {
                return Err(JournalError::Corruption(format!(
                    "required table '{table}' is missing — schema is corrupt"
                )));
            }
        }
        Ok(())
    }

    /// Ensure the `accounts` and `sync_pairs` rows exist for the given pair.
    ///
    /// Uses `INSERT OR IGNORE` so repeated calls and concurrent starts are safe.
    /// Must be called before the first `upsert` for a pair to satisfy FK constraints.
    pub async fn register_pair(
        &self,
        account: &crate::types::Account,
        pair: &crate::types::SyncPair,
    ) -> Result<(), JournalError> {
        sqlx::query(
            "INSERT OR IGNORE INTO accounts
             (id, display_name, server_url, username, keychain_service_key, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&account.id.0)
        .bind(&account.display_name)
        .bind(&account.server_url)
        .bind(&account.username)
        .bind(&account.keychain_service_key)
        .bind(account.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT OR IGNORE INTO sync_pairs
             (id, account_id, local_root, remote_root, status, created_at)
             VALUES (?, ?, ?, ?, 'idle', ?)",
        )
        .bind(&pair.id.0)
        .bind(&account.id.0)
        .bind(pair.local_root.0.to_string_lossy().as_ref())
        .bind(&pair.remote_root.0)
        .bind(pair.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

// ── Mapping helpers ──────────────────────────────────────────────────────────

fn row_to_entry(row: &sqlx::sqlite::SqliteRow) -> Result<JournalEntry, JournalError> {
    let status_str: String = row.try_get("status")?;
    let status = parse_status(&status_str)?;

    let checksum = match (
        row.try_get::<Option<String>, _>("checksum_algo")?,
        row.try_get::<Option<String>, _>("checksum_value")?,
    ) {
        (Some(algo), Some(value)) => Some(Checksum {
            algorithm: parse_algo(&algo)?,
            value,
        }),
        _ => None,
    };

    Ok(JournalEntry {
        pair_id: PairId(row.try_get("pair_id")?),
        path: RelativePath::new(row.try_get::<String, _>("path")?),
        file_id: row.try_get("file_id")?,
        etag: row.try_get("etag")?,
        checksum,
        size: row.try_get::<i64, _>("size")? as u64,
        mtime_local: parse_dt(row.try_get("mtime_local")?)?,
        mtime_remote: parse_dt(row.try_get("mtime_remote")?)?,
        status,
        error_message: row.try_get("error_message")?,
        retry_count: row.try_get::<i64, _>("retry_count")? as u32,
        updated_at: parse_dt(row.try_get("updated_at")?)?.unwrap_or_else(Utc::now),
    })
}

fn parse_status(s: &str) -> Result<SyncStatus, JournalError> {
    match s {
        "synced" => Ok(SyncStatus::Synced),
        "pending_upload" => Ok(SyncStatus::PendingUpload),
        "pending_download" => Ok(SyncStatus::PendingDownload),
        "conflict" => Ok(SyncStatus::Conflict),
        "error" => Ok(SyncStatus::Error),
        "excluded" => Ok(SyncStatus::Excluded),
        other => Err(JournalError::Corruption(format!(
            "unknown status value: {other}"
        ))),
    }
}

fn status_str(s: &SyncStatus) -> &'static str {
    match s {
        SyncStatus::Synced => "synced",
        SyncStatus::PendingUpload => "pending_upload",
        SyncStatus::PendingDownload => "pending_download",
        SyncStatus::Conflict => "conflict",
        SyncStatus::Error => "error",
        SyncStatus::Excluded => "excluded",
    }
}

fn parse_algo(s: &str) -> Result<ChecksumAlgorithm, JournalError> {
    match s {
        "SHA256" => Ok(ChecksumAlgorithm::Sha256),
        "MD5" => Ok(ChecksumAlgorithm::Md5),
        other => Err(JournalError::Corruption(format!(
            "unknown checksum algorithm: {other}"
        ))),
    }
}

fn conflict_policy_str(p: &ConflictPolicy) -> &'static str {
    match p {
        ConflictPolicy::PreserveBoth => "preserve_both",
        ConflictPolicy::LocalWins => "local_wins",
        ConflictPolicy::RemoteWins => "remote_wins",
        ConflictPolicy::NewestWins => "newest_wins",
        ConflictPolicy::Ask => "ask",
    }
}

fn parse_conflict_policy(s: &str) -> Result<ConflictPolicy, JournalError> {
    match s {
        "preserve_both" => Ok(ConflictPolicy::PreserveBoth),
        "local_wins" => Ok(ConflictPolicy::LocalWins),
        "remote_wins" => Ok(ConflictPolicy::RemoteWins),
        "newest_wins" => Ok(ConflictPolicy::NewestWins),
        "ask" => Ok(ConflictPolicy::Ask),
        other => Err(JournalError::Corruption(format!(
            "unknown conflict policy: {other}"
        ))),
    }
}

fn row_to_conflict(row: &sqlx::sqlite::SqliteRow) -> Result<ConflictRecord, JournalError> {
    let policy_str: String = row.try_get("policy")?;
    let policy = parse_conflict_policy(&policy_str)?;

    let resolution: Option<ConflictResolution> = row
        .try_get::<Option<String>, _>("resolution")?
        .map(|s| serde_json::from_str(&s))
        .transpose()
        .map_err(|e| JournalError::Corruption(format!("deserialize resolution: {e}")))?;

    Ok(ConflictRecord {
        id: row.try_get("id")?,
        pair_id: PairId(row.try_get("pair_id")?),
        path: RelativePath::new(row.try_get::<String, _>("path")?),
        local_mtime: row
            .try_get::<String, _>("local_mtime")?
            .parse()
            .map_err(|e| JournalError::Corruption(format!("bad local_mtime: {e}")))?,
        remote_mtime: row
            .try_get::<String, _>("remote_mtime")?
            .parse()
            .map_err(|e| JournalError::Corruption(format!("bad remote_mtime: {e}")))?,
        local_size: row.try_get::<i64, _>("local_size")? as u64,
        remote_size: row.try_get::<i64, _>("remote_size")? as u64,
        policy,
        resolution,
        detected_at: row
            .try_get::<String, _>("detected_at")?
            .parse()
            .map_err(|e| JournalError::Corruption(format!("bad detected_at: {e}")))?,
        resolved_at: row
            .try_get::<Option<String>, _>("resolved_at")?
            .map(|s| s.parse())
            .transpose()
            .map_err(|e| JournalError::Corruption(format!("bad resolved_at: {e}")))?,
    })
}

fn parse_dt(s: Option<String>) -> Result<Option<DateTime<Utc>>, JournalError> {
    match s {
        None => Ok(None),
        Some(s) => s
            .parse::<DateTime<Utc>>()
            .map(Some)
            .map_err(|e| JournalError::Corruption(format!("bad datetime {s}: {e}"))),
    }
}

// ── Journal impl ─────────────────────────────────────────────────────────────

#[async_trait]
impl Journal for SqliteJournal {
    #[instrument(err, skip(self), fields(pair_id = %pair_id, path = %path))]
    async fn get(
        &self,
        pair_id: &PairId,
        path: &RelativePath,
    ) -> Result<Option<JournalEntry>, JournalError> {
        let row = sqlx::query("SELECT * FROM journal_entries WHERE pair_id = ? AND path = ?")
            .bind(&pair_id.0)
            .bind(path.as_str())
            .fetch_optional(&self.pool)
            .await?;

        row.map(|r| row_to_entry(&r)).transpose()
    }

    #[instrument(err, skip(self), fields(pair_id = %pair_id, file_id))]
    async fn get_by_file_id(
        &self,
        pair_id: &PairId,
        file_id: &str,
    ) -> Result<Option<JournalEntry>, JournalError> {
        let row = sqlx::query("SELECT * FROM journal_entries WHERE pair_id = ? AND file_id = ?")
            .bind(&pair_id.0)
            .bind(file_id)
            .fetch_optional(&self.pool)
            .await?;

        row.map(|r| row_to_entry(&r)).transpose()
    }

    #[instrument(err, skip(self, entry), fields(pair_id = %entry.pair_id, path = %entry.path))]
    async fn upsert(&self, entry: &JournalEntry) -> Result<(), JournalError> {
        let (algo, val) = entry
            .checksum
            .as_ref()
            .map(|c| {
                let a = match c.algorithm {
                    ChecksumAlgorithm::Sha256 => "SHA256",
                    ChecksumAlgorithm::Md5 => "MD5",
                };
                (Some(a.to_string()), Some(c.value.clone()))
            })
            .unwrap_or((None, None));

        sqlx::query(
            r#"
            INSERT INTO journal_entries
                (pair_id, path, file_id, etag, checksum_algo, checksum_value,
                 size, mtime_local, mtime_remote, status, error_message,
                 retry_count, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(pair_id, path) DO UPDATE SET
                file_id        = excluded.file_id,
                etag           = excluded.etag,
                checksum_algo  = excluded.checksum_algo,
                checksum_value = excluded.checksum_value,
                size           = excluded.size,
                mtime_local    = excluded.mtime_local,
                mtime_remote   = excluded.mtime_remote,
                status         = excluded.status,
                error_message  = excluded.error_message,
                retry_count    = excluded.retry_count,
                updated_at     = excluded.updated_at
            "#,
        )
        .bind(&entry.pair_id.0)
        .bind(entry.path.as_str())
        .bind(&entry.file_id)
        .bind(&entry.etag)
        .bind(algo)
        .bind(val)
        .bind(entry.size as i64)
        .bind(entry.mtime_local.map(|d| d.to_rfc3339()))
        .bind(entry.mtime_remote.map(|d| d.to_rfc3339()))
        .bind(status_str(&entry.status))
        .bind(&entry.error_message)
        .bind(entry.retry_count as i64)
        .bind(entry.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[instrument(err, skip(self), fields(pair_id = %pair_id, path = %path))]
    async fn delete(&self, pair_id: &PairId, path: &RelativePath) -> Result<(), JournalError> {
        sqlx::query("DELETE FROM journal_entries WHERE pair_id = ? AND path = ?")
            .bind(&pair_id.0)
            .bind(path.as_str())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    #[instrument(err, skip(self), fields(pair_id = %pair_id))]
    async fn all_entries(&self, pair_id: &PairId) -> Result<Vec<JournalEntry>, JournalError> {
        let rows = sqlx::query("SELECT * FROM journal_entries WHERE pair_id = ?")
            .bind(&pair_id.0)
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_entry).collect()
    }

    #[instrument(err, skip(self), fields(pair_id = %pair_id, status = ?status))]
    async fn entries_by_status(
        &self,
        pair_id: &PairId,
        status: &SyncStatus,
    ) -> Result<Vec<JournalEntry>, JournalError> {
        let rows = sqlx::query("SELECT * FROM journal_entries WHERE pair_id = ? AND status = ?")
            .bind(&pair_id.0)
            .bind(status_str(status))
            .fetch_all(&self.pool)
            .await?;

        rows.iter().map(row_to_entry).collect()
    }

    #[instrument(err, skip(self, entries), fields(count = entries.len()))]
    async fn upsert_batch(&self, entries: &[JournalEntry]) -> Result<(), JournalError> {
        let mut tx = self.pool.begin().await?;

        for entry in entries {
            let (algo, val) = entry
                .checksum
                .as_ref()
                .map(|c| {
                    let a = match c.algorithm {
                        ChecksumAlgorithm::Sha256 => "SHA256",
                        ChecksumAlgorithm::Md5 => "MD5",
                    };
                    (Some(a.to_string()), Some(c.value.clone()))
                })
                .unwrap_or((None, None));

            sqlx::query(
                r#"
                INSERT INTO journal_entries
                    (pair_id, path, file_id, etag, checksum_algo, checksum_value,
                     size, mtime_local, mtime_remote, status, error_message,
                     retry_count, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(pair_id, path) DO UPDATE SET
                    file_id        = excluded.file_id,
                    etag           = excluded.etag,
                    checksum_algo  = excluded.checksum_algo,
                    checksum_value = excluded.checksum_value,
                    size           = excluded.size,
                    mtime_local    = excluded.mtime_local,
                    mtime_remote   = excluded.mtime_remote,
                    status         = excluded.status,
                    error_message  = excluded.error_message,
                    retry_count    = excluded.retry_count,
                    updated_at     = excluded.updated_at
                "#,
            )
            .bind(&entry.pair_id.0)
            .bind(entry.path.as_str())
            .bind(&entry.file_id)
            .bind(&entry.etag)
            .bind(algo)
            .bind(val)
            .bind(entry.size as i64)
            .bind(entry.mtime_local.map(|d| d.to_rfc3339()))
            .bind(entry.mtime_remote.map(|d| d.to_rfc3339()))
            .bind(status_str(&entry.status))
            .bind(&entry.error_message)
            .bind(entry.retry_count as i64)
            .bind(entry.updated_at.to_rfc3339())
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    #[instrument(err, skip(self), fields(pair_id = %pair_id))]
    async fn clear_pair(&self, pair_id: &PairId) -> Result<(), JournalError> {
        sqlx::query("DELETE FROM journal_entries WHERE pair_id = ?")
            .bind(&pair_id.0)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Conflict persistence ─────────────────────────────────────────────────

    #[instrument(err, skip(self, record), fields(id = %record.id, path = %record.path))]
    async fn upsert_conflict(&self, record: &ConflictRecord) -> Result<(), JournalError> {
        let policy = conflict_policy_str(&record.policy);
        let resolution_json = record
            .resolution
            .as_ref()
            .map(|r| serde_json::to_string(r).unwrap_or_default());

        sqlx::query(
            r#"
            INSERT INTO conflict_records
                (id, pair_id, path, local_mtime, remote_mtime, local_size, remote_size,
                 policy, resolution, detected_at, resolved_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                resolution  = excluded.resolution,
                resolved_at = excluded.resolved_at
            "#,
        )
        .bind(&record.id)
        .bind(&record.pair_id.0)
        .bind(record.path.as_str())
        .bind(record.local_mtime.to_rfc3339())
        .bind(record.remote_mtime.to_rfc3339())
        .bind(record.local_size as i64)
        .bind(record.remote_size as i64)
        .bind(policy)
        .bind(resolution_json)
        .bind(record.detected_at.to_rfc3339())
        .bind(record.resolved_at.map(|d| d.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[instrument(err, skip(self, resolution), fields(id))]
    async fn resolve_conflict(
        &self,
        id: &str,
        resolution: ConflictResolution,
    ) -> Result<(), JournalError> {
        let resolution_json = serde_json::to_string(&resolution)
            .map_err(|e| JournalError::Corruption(format!("serialize resolution: {e}")))?;

        sqlx::query("UPDATE conflict_records SET resolution = ?, resolved_at = ? WHERE id = ?")
            .bind(resolution_json)
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    #[instrument(err, skip(self), fields(pair_id = %pair_id))]
    async fn list_conflicts(&self, pair_id: &PairId) -> Result<Vec<ConflictRecord>, JournalError> {
        let rows = sqlx::query(
            "SELECT * FROM conflict_records WHERE pair_id = ? ORDER BY detected_at DESC",
        )
        .bind(&pair_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_conflict).collect()
    }
}
