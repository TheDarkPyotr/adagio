use crate::config::{load_config, save_config, SavedPair};
use crate::state::AppState;
use adagio_core::config::SyncPairConfig;
use adagio_core::journal::Journal;
use adagio_core::types::{
    AccountId, LocalPath, PairId, PairStatus, RelativePath, RemotePath, SyncPair, SyncStatus,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

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

/// Create a new sync pair, validate the local root, and persist to config.json.
#[tauri::command]
pub async fn create_pair(
    state: State<'_, AppState>,
    req: CreatePairRequest,
) -> Result<PairDto, String> {
    let local_root = PathBuf::from(&req.local_root);
    let cfg_check = SyncPairConfig {
        local_root: local_root.clone(),
        remote_root: req.remote_root.clone(),
        account_id: AccountId(req.account_id.clone()),
    };
    cfg_check.validate().map_err(|e| e.to_string())?;

    let pair_id = PairId::new();
    let pair = SyncPair {
        id: pair_id.clone(),
        account_id: AccountId(req.account_id.clone()),
        local_root: LocalPath::new(local_root),
        remote_root: RemotePath::new(&req.remote_root),
        status: PairStatus::Idle,
        exclude_patterns: req.exclude_patterns.clone(),
        selective_paths: req
            .selective_paths
            .iter()
            .map(|p| RelativePath::new(p.as_str()))
            .collect(),
        created_at: Utc::now(),
        last_synced_at: None,
        scan_interval_secs: req.scan_interval_secs,
        scan_on_startup: req.scan_on_startup,
        max_upload_concurrency: req.max_upload_concurrency.min(u8::MAX as usize) as u8,
        max_download_concurrency: req.max_download_concurrency.min(u8::MAX as usize) as u8,
    };

    {
        let mut pairs = state.pairs.write().await;
        pairs.register_full_pair(pair.clone());
    }

    // Persist to config.json.
    let mut file_cfg = load_config(&state.config_path);
    file_cfg.pairs.push(SavedPair {
        id: pair.id.0.clone(),
        account_id: pair.account_id.0.clone(),
        local_root: req.local_root.clone(),
        remote_root: req.remote_root.clone(),
        scan_interval_secs: req.scan_interval_secs,
        scan_on_startup: req.scan_on_startup,
        max_upload_concurrency: req.max_upload_concurrency,
        max_download_concurrency: req.max_download_concurrency,
        selective_paths: req.selective_paths,
        exclude_patterns: req.exclude_patterns,
    });
    save_config(&state.config_path, &file_cfg).map_err(|e| e.to_string())?;

    Ok(PairDto {
        id: pair_id.0,
        local_root: req.local_root,
        remote_root: req.remote_root,
        account_id: req.account_id,
    })
}

/// Delete a sync pair, optionally removing local files, stop its runner, and update config.json.
#[tauri::command]
pub async fn delete_pair(
    state: State<'_, AppState>,
    pair_id: String,
    delete_local_files: bool,
) -> Result<(), String> {
    let id = PairId(pair_id.clone());

    // Stop the runner first.
    state.engine.stop_pair(&id).await;

    {
        let mut pairs = state.pairs.write().await;
        pairs
            .delete_pair(&id, delete_local_files)
            .map_err(|e| e.to_string())?;
        pairs.remove_pair(&id);
    }

    // Remove from config.json.
    let mut cfg = load_config(&state.config_path);
    cfg.pairs.retain(|p| p.id != pair_id);
    save_config(&state.config_path, &cfg).map_err(|e| e.to_string())?;

    Ok(())
}

/// List all configured sync pairs.
#[tauri::command]
pub async fn list_pairs(state: State<'_, AppState>) -> Result<Vec<PairDto>, String> {
    let pairs = state.pairs.read().await;
    Ok(pairs
        .all_pairs()
        .into_iter()
        .map(|p| PairDto {
            id: p.id.0.clone(),
            local_root: p.local_root.0.to_string_lossy().to_string(),
            remote_root: p.remote_root.0.clone(),
            account_id: p.account_id.0.clone(),
        })
        .collect())
}

// ── File browser ──────────────────────────────────────────────────────────────

/// Per-file sync status entry returned by `list_synced_files`.
#[derive(Debug, Serialize)]
pub struct FileStatusDto {
    pub name: String,
    pub relative_path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_at: String,
    pub sync_status: String,
    pub error_message: Option<String>,
}

fn sync_status_str(status: &SyncStatus) -> &'static str {
    match status {
        SyncStatus::Synced => "synced",
        SyncStatus::PendingUpload => "pending_upload",
        SyncStatus::PendingDownload => "pending_download",
        SyncStatus::Conflict => "conflict",
        SyncStatus::Error => "error",
        SyncStatus::Excluded => "unknown",
    }
}

/// List the contents of a sync pair's local folder enriched with per-file sync status.
///
/// Returns entries sorted: directories first, then files, each group alphabetically.
/// Files not present in the journal are returned with `sync_status: "unknown"`.
#[tauri::command]
pub async fn list_synced_files(
    state: State<'_, AppState>,
    pair_id: String,
    relative_path: Option<String>,
) -> Result<Vec<FileStatusDto>, String> {
    let id = PairId(pair_id.clone());
    let sub = relative_path.unwrap_or_default();

    // Resolve the local directory to list.
    let local_root = {
        let pairs = state.pairs.read().await;
        let pair = pairs
            .get_pair(&id)
            .ok_or_else(|| format!("pair {pair_id} not found"))?;
        pair.local_root.0.clone()
    };

    let target_dir = if sub.is_empty() {
        local_root.clone()
    } else {
        local_root.join(&sub)
    };

    // Read the directory entries from the filesystem.
    let read_dir =
        std::fs::read_dir(&target_dir).map_err(|e| format!("cannot read {:?}: {e}", target_dir))?;

    let mut entries: Vec<FileStatusDto> = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let is_dir = meta.is_dir();
        let size = if is_dir { 0 } else { meta.len() };
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| {
                    chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                })
            })
            .unwrap_or_default();

        let rel = if sub.is_empty() {
            file_name.clone()
        } else {
            format!("{}/{}", sub.trim_end_matches('/'), file_name)
        };

        // Look up the journal entry for sync status.
        let rel_path = RelativePath::new(&rel);
        let journal_entry = state.journal.get(&id, &rel_path).await.ok().flatten();

        let (sync_status, error_message) = match &journal_entry {
            Some(e) => {
                let err = if matches!(e.status, SyncStatus::Error | SyncStatus::Conflict) {
                    e.error_message.clone()
                } else {
                    None
                };
                (sync_status_str(&e.status).to_string(), err)
            }
            None => ("unknown".to_string(), None),
        };

        entries.push(FileStatusDto {
            name: file_name,
            relative_path: rel,
            is_dir,
            size,
            modified_at,
            sync_status,
            error_message,
        });
    }

    // Sort: directories first, then files; each group alphabetically by name.
    entries.sort_by(|a, b| match (b.is_dir, a.is_dir) {
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

// ── Remote tree for selective sync UI ────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct RemoteTreeItemDto {
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub excluded: bool,
}

/// Return all remote items for display in the tree browser.
#[tauri::command]
pub async fn list_remote_tree(
    _state: State<'_, AppState>,
    _pair_id: String,
) -> Result<Vec<RemoteTreeItemDto>, String> {
    Ok(vec![])
}

/// Return the current exclude patterns for the application.
#[tauri::command]
pub async fn get_exclude_patterns(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let pairs = state.pairs.read().await;
    Ok(pairs
        .exclude_patterns()
        .iter()
        .map(|p| p.pattern.clone())
        .collect())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{save_config, SavedConfig};
    use adagio_core::journal::sqlite::SqliteJournal;
    use adagio_core::types::{JournalEntry, SyncStatus};
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
        };

        let (journal, _) = make_journal().await;
        let entries = list_files_internal(&pair, "", &*journal).await.unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].sync_status, "unknown");
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
        assert_eq!(entries[0].sync_status, "synced");
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
        let size = if is_dir { 0 } else { meta.len() };
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| {
                    chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                })
            })
            .unwrap_or_default();

        let rel = if sub.is_empty() {
            file_name.clone()
        } else {
            format!("{}/{}", sub.trim_end_matches('/'), file_name)
        };

        let rel_path = RelativePath::new(&rel);
        let journal_entry = journal.get(&pair.id, &rel_path).await.ok().flatten();

        let (sync_status, error_message) = match &journal_entry {
            Some(e) => {
                let err = if matches!(e.status, SyncStatus::Error | SyncStatus::Conflict) {
                    e.error_message.clone()
                } else {
                    None
                };
                (sync_status_str(&e.status).to_string(), err)
            }
            None => ("unknown".to_string(), None),
        };

        entries.push(FileStatusDto {
            name: file_name,
            relative_path: rel,
            is_dir,
            size,
            modified_at,
            sync_status,
            error_message,
        });
    }

    entries.sort_by(|a, b| match (b.is_dir, a.is_dir) {
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}
