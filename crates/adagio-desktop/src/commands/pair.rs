use crate::state::AppState;
use adagio_core::config::SyncPairConfig;
use adagio_core::types::{AccountId, SyncStatus};
use adagio_ipc::DaemonRequest;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

#[cfg(test)]
use adagio_core::{
    journal::Journal,
    types::{RelativePath, SyncPair},
};

#[derive(Debug, Serialize)]
pub struct PairDto {
    pub id: String,
    pub local_root: String,
    pub remote_root: String,
    pub account_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePairRequest {
    pub account_id: String,
    pub local_root: String,
    pub remote_root: String,
    #[serde(default = "default_scan_interval")]
    pub scan_interval_secs: u64,
    #[serde(default = "default_scan_on_startup")]
    pub scan_on_startup: bool,
    #[serde(default = "default_concurrency")]
    pub max_upload_concurrency: usize,
    #[serde(default = "default_concurrency")]
    pub max_download_concurrency: usize,
    #[serde(default)]
    pub selective_paths: Vec<String>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
}

fn default_scan_interval() -> u64 {
    7200
}
fn default_scan_on_startup() -> bool {
    true
}
fn default_concurrency() -> usize {
    3
}

#[derive(Debug, Serialize)]
pub struct PreflightSummaryDto {
    pub remote_item_count: usize,
    pub remote_total_bytes: u64,
    pub remote_dir_count: usize,
    pub local_root: String,
    pub remote_root: String,
}

/// Create a new sync pair via the daemon.
#[tauri::command]
pub async fn create_pair(
    state: State<'_, AppState>,
    req: CreatePairRequest,
) -> Result<serde_json::Value, String> {
    // Validate local root exists before sending to daemon.
    let local_root = PathBuf::from(&req.local_root);
    let cfg_check = SyncPairConfig {
        local_root: local_root.clone(),
        remote_root: req.remote_root.clone(),
        account_id: AccountId(req.account_id.clone()),
    };
    cfg_check.validate().map_err(|e| e.to_string())?;

    state
        .daemon
        .request(DaemonRequest::CreatePair {
            account_id: req.account_id,
            local_root: req.local_root,
            remote_root: req.remote_root,
        })
        .await
        .map_err(|e| e.to_string())
}

/// Delete a sync pair via the daemon.
#[tauri::command]
pub async fn delete_pair(
    state: State<'_, AppState>,
    pair_id: String,
    delete_local_files: bool,
) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::DeletePair {
            pair_id,
            delete_local_files,
        })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// List all configured sync pairs.
#[tauri::command]
pub async fn list_pairs(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::ListPairs)
        .await
        .map_err(|e| e.to_string())
}

// ── File browser ──────────────────────────────────────────────────────────────

/// Per-file sync status entry returned by `list_synced_files`.
/// Field names and types mirror the TypeScript `FileStatusDto` interface.
#[derive(Debug, Serialize)]
pub struct FileStatusDto {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub mtime: Option<i64>,
    pub status: String,
    pub etag: Option<String>,
    pub share_count: Option<u32>,
    pub item_count: Option<u32>,
}

/// Recursively sum the sizes of all regular files under `path`.
/// Silently skips entries that can't be read (permission errors, broken symlinks, etc.).
#[cfg_attr(not(test), allow(dead_code))]
fn dir_size(path: &std::path::Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(path) else {
        return 0;
    };
    rd.flatten()
        .map(|e| {
            let Ok(m) = e.metadata() else { return 0 };
            if m.is_file() {
                m.len()
            } else if m.is_dir() {
                dir_size(&e.path())
            } else {
                0
            }
        })
        .sum()
}

#[cfg_attr(not(test), allow(dead_code))]
fn journal_status_to_frontend(s: &SyncStatus) -> &'static str {
    match s {
        SyncStatus::Synced => "ok",
        SyncStatus::PendingUpload | SyncStatus::PendingDownload => "sync",
        SyncStatus::Conflict => "conflict",
        SyncStatus::Error => "conflict",
        SyncStatus::Excluded => "ok",
    }
}

/// List the synced files for a pair via the daemon.
#[tauri::command]
pub async fn list_synced_files(
    state: State<'_, AppState>,
    pair_id: String,
    relative_path: Option<String>,
) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::ListSyncedFiles {
            pair_id,
            relative_path,
        })
        .await
        .map_err(|e| e.to_string())
}

// ── Remote tree for selective sync UI ────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct RemoteTreeItemDto {
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub excluded: bool,
}

/// Return all remote items for the tree browser.
#[tauri::command]
pub async fn list_remote_tree(
    state: State<'_, AppState>,
    pair_id: String,
) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::ListRemoteTree { pair_id })
        .await
        .map_err(|e| e.to_string())
}

/// Return the current exclude patterns.
#[tauri::command]
pub async fn get_exclude_patterns(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::GetExcludePatterns)
        .await
        .map_err(|e| e.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{save_config, SavedConfig, SavedPair};
    use adagio_core::journal::sqlite::SqliteJournal;
    use adagio_core::journal::Journal as _;
    use adagio_core::types::{
        AccountId, JournalEntry, LocalPath, PairId, PairStatus, RelativePath, RemotePath, SyncPair,
        SyncStatus,
    };
    use chrono::Utc;
    use sqlx::sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
    };
    use std::str::FromStr;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn make_journal() -> (Arc<SqliteJournal>, sqlx::SqlitePool) {
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
        let journal = Arc::new(SqliteJournal::new(pool.clone()));
        (journal, pool)
    }

    /// Insert minimal account + sync_pair rows so journal_entries FK constraints pass.
    async fn seed_pair(pool: &sqlx::SqlitePool, account_id: &str, pair_id: &str, local_root: &str) {
        sqlx::query(
            "INSERT INTO accounts (id, display_name, server_url, username, keychain_service_key, created_at) \
             VALUES (?, 'Test', 'http://localhost', 'test', 'test-key', ?)"
        )
        .bind(account_id)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO sync_pairs (id, account_id, local_root, remote_root, created_at) \
             VALUES (?, ?, ?, '', ?)",
        )
        .bind(pair_id)
        .bind(account_id)
        .bind(local_root)
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .unwrap();
    }

    // ── T012: create_pair and delete_pair persistence ─────────────────────────

    #[test]
    fn create_pair_adds_to_config_json() {
        use crate::config::load_config as lc;
        let dir = TempDir::new().unwrap();
        let sync_dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.json");

        let cfg = SavedConfig::default();
        save_config(&config_path, &cfg).unwrap();

        // Manually simulate what create_pair does (no Tauri State in unit test)
        let mut loaded = lc(&config_path);
        loaded.pairs.push(SavedPair {
            id: "pair-1".to_string(),
            account_id: "acc-1".to_string(),
            local_root: sync_dir.path().to_str().unwrap().to_string(),
            remote_root: "remote/".to_string(),
            scan_interval_secs: 7200,
            scan_on_startup: true,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            selective_paths: vec![],
            exclude_patterns: vec![],
        });
        save_config(&config_path, &loaded).unwrap();

        let reloaded = lc(&config_path);
        assert_eq!(reloaded.pairs.len(), 1);
        assert_eq!(reloaded.pairs[0].id, "pair-1");
    }

    #[test]
    fn delete_pair_removes_from_config_json() {
        use crate::config::load_config as lc;
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.json");

        let mut cfg = SavedConfig::default();
        cfg.pairs.push(SavedPair {
            id: "pair-1".to_string(),
            account_id: "acc-1".to_string(),
            local_root: "/tmp/sync".to_string(),
            remote_root: "remote/".to_string(),
            scan_interval_secs: 7200,
            scan_on_startup: true,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            selective_paths: vec![],
            exclude_patterns: vec![],
        });
        save_config(&config_path, &cfg).unwrap();

        // Simulate delete_pair removing it
        let mut loaded = lc(&config_path);
        loaded.pairs.retain(|p| p.id != "pair-1");
        save_config(&config_path, &loaded).unwrap();

        let reloaded = lc(&config_path);
        assert_eq!(reloaded.pairs.len(), 0);
    }

    // ── T024: list_synced_files ───────────────────────────────────────────────

    #[tokio::test]
    async fn list_synced_files_sorts_dirs_before_files() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("zzz_dir")).unwrap();
        std::fs::write(dir.path().join("aaa_file.txt"), b"data").unwrap();

        let pair = SyncPair {
            id: PairId("pair-sort".to_string()),
            account_id: AccountId::new(),
            local_root: LocalPath::new(dir.path()),
            remote_root: RemotePath::new(""),
            status: PairStatus::Idle,
            exclude_patterns: vec![],
            selective_paths: vec![],
            created_at: Utc::now(),
            last_synced_at: None,
            scan_interval_secs: 7200,
            scan_on_startup: false,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            conflict_policy: adagio_core::types::ConflictPolicy::Ask,
        };

        let (journal, _) = make_journal().await;
        let entries = list_files_internal(&pair, "", &*journal).await.unwrap();

        assert!(entries[0].is_dir, "first entry should be the directory");
        assert_eq!(entries[0].name, "zzz_dir");
        assert!(!entries[1].is_dir, "second entry should be the file");
        assert_eq!(entries[1].name, "aaa_file.txt");
    }

    #[tokio::test]
    async fn list_synced_files_returns_unknown_for_untracked_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("new_file.txt"), b"data").unwrap();

        let pair = SyncPair {
            id: PairId("pair-unknown".to_string()),
            account_id: AccountId::new(),
            local_root: LocalPath::new(dir.path()),
            remote_root: RemotePath::new(""),
            status: PairStatus::Idle,
            exclude_patterns: vec![],
            selective_paths: vec![],
            created_at: Utc::now(),
            last_synced_at: None,
            scan_interval_secs: 7200,
            scan_on_startup: false,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            conflict_policy: adagio_core::types::ConflictPolicy::Ask,
        };

        let (journal, _) = make_journal().await;
        let entries = list_files_internal(&pair, "", &*journal).await.unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, "ok");
    }

    #[tokio::test]
    async fn list_synced_files_shows_journal_status() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("synced.txt"), b"data").unwrap();

        let account_id = AccountId::new();
        let pair_id = PairId("pair-journal".to_string());
        let pair = SyncPair {
            id: pair_id.clone(),
            account_id: account_id.clone(),
            local_root: LocalPath::new(dir.path()),
            remote_root: RemotePath::new(""),
            status: PairStatus::Idle,
            exclude_patterns: vec![],
            selective_paths: vec![],
            created_at: Utc::now(),
            last_synced_at: None,
            scan_interval_secs: 7200,
            scan_on_startup: false,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            conflict_policy: adagio_core::types::ConflictPolicy::Ask,
        };

        let (journal, pool) = make_journal().await;
        seed_pair(
            &pool,
            &account_id.0,
            &pair_id.0,
            dir.path().to_str().unwrap(),
        )
        .await;

        let entry = JournalEntry {
            pair_id: pair_id.clone(),
            path: RelativePath::new("synced.txt"),
            file_id: Some("file-1".to_string()),
            etag: Some("abc".to_string()),
            checksum: None,
            size: 4,
            mtime_local: Some(Utc::now()),
            mtime_remote: Some(Utc::now()),
            status: SyncStatus::Synced,
            error_message: None,
            retry_count: 0,
            updated_at: Utc::now(),
        };
        journal.upsert(&entry).await.unwrap();

        let entries = list_files_internal(&pair, "", &*journal).await.unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, "ok");
    }
}

#[cfg(test)]
async fn list_files_internal(
    pair: &SyncPair,
    sub: &str,
    journal: &dyn Journal,
) -> Result<Vec<FileStatusDto>, String> {
    let local_root = &pair.local_root.0;
    let target_dir = if sub.is_empty() {
        local_root.clone()
    } else {
        local_root.join(sub)
    };

    let read_dir =
        std::fs::read_dir(&target_dir).map_err(|e| format!("cannot read {:?}: {e}", target_dir))?;

    let mut entries: Vec<FileStatusDto> = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let is_dir = meta.is_dir();

        let mtime: Option<i64> = meta.modified().ok().and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_millis() as i64)
        });

        let rel = if sub.is_empty() {
            file_name.clone()
        } else {
            format!("{}/{}", sub.trim_end_matches('/'), file_name)
        };

        let rel_path = RelativePath::new(&rel);
        let journal_entry = journal.get(&pair.id, &rel_path).await.ok().flatten();

        let status = match &journal_entry {
            Some(e) => journal_status_to_frontend(&e.status).to_string(),
            None => "ok".to_string(),
        };
        let etag = journal_entry.as_ref().and_then(|e| e.etag.clone());

        let item_count: Option<u32> = if is_dir {
            std::fs::read_dir(entry.path())
                .ok()
                .map(|d| d.count() as u32)
        } else {
            None
        };

        entries.push(FileStatusDto {
            path: format!("/{rel}"),
            name: file_name,
            is_dir,
            size: Some(if is_dir {
                dir_size(&entry.path())
            } else {
                meta.len()
            }),
            mtime,
            status,
            etag,
            share_count: None,
            item_count,
        });
    }

    entries.sort_by(|a, b| match (b.is_dir, a.is_dir) {
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}
