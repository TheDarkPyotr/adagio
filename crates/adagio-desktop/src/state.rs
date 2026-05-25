use crate::config::{load_config, SavedConfig};
use adagio_core::account_manager::AccountManager;
use adagio_core::config::SyncPairManager;
use adagio_core::cycle::DefaultSyncEngine;
use adagio_core::journal::sqlite::SqliteJournal;
use adagio_core::types::{
    Account, AccountId, LocalPath, PairId, PairStatus, RelativePath, RemotePath, SyncPair,
};
use adagio_core::AppConfig;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared application state managed by Tauri.
///
/// Held in a `tauri::State<AppState>` and passed to all command handlers.
pub struct AppState {
    pub config: Arc<RwLock<AppConfig>>,
    pub accounts: Arc<AccountManager>,
    pub pairs: Arc<RwLock<SyncPairManager>>,
    pub engine: Arc<DefaultSyncEngine>,
    pub journal: Arc<SqliteJournal>,
    /// Path to the on-disk config file — used by commands that mutate state.
    pub config_path: PathBuf,
}

impl AppState {
    /// Initialise state, loading persisted config from `config_dir/config.json`
    /// and opening the journal at `config_dir/adagio.db`.
    pub fn new(config_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&config_dir)?;
        let db_path = config_dir.join("adagio.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
        let journal = tauri::async_runtime::block_on(async { SqliteJournal::open(&db_url).await })?;

        let config_path = config_dir.join("config.json");
        let saved = load_config(&config_path);

        let accounts = AccountManager::new();
        restore_accounts(&accounts, &saved);

        let mut pair_manager = SyncPairManager::new();
        restore_pairs(&mut pair_manager, &saved);

        Ok(Self {
            config: Arc::new(RwLock::new(AppConfig::default())),
            accounts: Arc::new(accounts),
            pairs: Arc::new(RwLock::new(pair_manager)),
            engine: Arc::new(DefaultSyncEngine::new()),
            journal: Arc::new(journal),
            config_path,
        })
    }
}

fn restore_accounts(manager: &AccountManager, saved: &SavedConfig) {
    use chrono::Utc;
    for sa in &saved.accounts {
        let account = Account {
            id: AccountId(sa.id.clone()),
            display_name: sa.display_name.clone(),
            server_url: sa.server_url.clone(),
            username: sa.username.clone(),
            keychain_service_key: sa.keychain_service_key.clone(),
            created_at: Utc::now(),
        };
        if let Err(e) = manager.add(account) {
            tracing::warn!(account_id = %sa.id, error = %e, "failed to restore account");
        }
    }
}

fn restore_pairs(manager: &mut SyncPairManager, saved: &SavedConfig) {
    use chrono::Utc;
    for sp in &saved.pairs {
        let pair = SyncPair {
            id: PairId(sp.id.clone()),
            account_id: AccountId(sp.account_id.clone()),
            local_root: LocalPath::new(PathBuf::from(&sp.local_root)),
            remote_root: RemotePath::new(&sp.remote_root),
            status: PairStatus::Idle,
            exclude_patterns: sp.exclude_patterns.clone(),
            selective_paths: sp
                .selective_paths
                .iter()
                .map(|p| RelativePath::new(p.as_str()))
                .collect(),
            created_at: Utc::now(),
            last_synced_at: None,
            scan_interval_secs: sp.scan_interval_secs,
            scan_on_startup: sp.scan_on_startup,
            max_upload_concurrency: sp.max_upload_concurrency.min(u8::MAX as usize) as u8,
            max_download_concurrency: sp.max_download_concurrency.min(u8::MAX as usize) as u8,
        };
        manager.register_full_pair(pair);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{save_config, SavedAccount, SavedConfig as Cfg, SavedPair};
    use tempfile::TempDir;

    fn make_saved_account(id: &str) -> SavedAccount {
        SavedAccount {
            id: id.to_string(),
            display_name: "Test".to_string(),
            server_url: "https://cloud.example.com".to_string(),
            username: "user".to_string(),
            keychain_service_key: format!("adagio/{id}"),
        }
    }

    fn make_saved_pair(id: &str, account_id: &str, local_root: &str) -> SavedPair {
        SavedPair {
            id: id.to_string(),
            account_id: account_id.to_string(),
            local_root: local_root.to_string(),
            remote_root: "remote/path".to_string(),
            scan_interval_secs: 7200,
            scan_on_startup: false,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            selective_paths: vec![],
            exclude_patterns: vec![],
        }
    }

    // ── T010: AppState::new with config_dir ───────────────────────────────────

    #[test]
    fn new_with_empty_config_dir_starts_cleanly() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).unwrap();
        assert_eq!(state.accounts.list().unwrap().len(), 0);
        let pairs = tauri::async_runtime::block_on(async { state.pairs.read().await });
        assert_eq!(pairs.all_pairs().len(), 0);
    }

    #[test]
    fn new_restores_accounts_from_config() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.json");
        let saved = Cfg {
            version: 1,
            accounts: vec![make_saved_account("acc-1"), make_saved_account("acc-2")],
            pairs: vec![],
        };
        save_config(&config_path, &saved).unwrap();
        let state = AppState::new(dir.path().to_path_buf()).unwrap();
        assert_eq!(state.accounts.list().unwrap().len(), 2);
    }

    #[test]
    fn new_restores_pairs_from_config() {
        let dir = TempDir::new().unwrap();
        // pair needs an existing local_root for register to work (register_full_pair doesn't validate)
        let local_dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.json");
        let saved = Cfg {
            version: 1,
            accounts: vec![make_saved_account("acc-1")],
            pairs: vec![make_saved_pair(
                "pair-1",
                "acc-1",
                local_dir.path().to_str().unwrap(),
            )],
        };
        save_config(&config_path, &saved).unwrap();
        let state = AppState::new(dir.path().to_path_buf()).unwrap();
        let pairs = tauri::async_runtime::block_on(async { state.pairs.read().await });
        assert_eq!(pairs.all_pairs().len(), 1);
        assert_eq!(pairs.all_pairs()[0].id.0, "pair-1");
    }

    #[test]
    fn config_path_is_set_correctly() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).unwrap();
        assert_eq!(state.config_path, dir.path().join("config.json"));
    }
}
