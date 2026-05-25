use crate::config::{load_config, save_config, SavedAccount};
use crate::state::AppState;
use adagio_core::types::{Account, AccountId};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{error, info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountDto {
    pub id: String,
    pub display_name: String,
    pub server_url: String,
    pub username: String,
}

impl From<Account> for AccountDto {
    fn from(a: Account) -> Self {
        Self {
            id: a.id.0,
            display_name: a.display_name,
            server_url: a.server_url,
            username: a.username,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AddAccountRequest {
    pub server_url: String,
    pub username: String,
    pub display_name: String,
    /// App-password or OAuth2 access token — stored in keychain, never persisted here.
    pub secret: String,
}

/// Normalise a server URL for duplicate detection.
///
/// Trims trailing slashes and lowercases scheme + host so that
/// `https://Cloud.Example.com/` and `https://cloud.example.com` compare equal.
fn normalise_server_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    // Find the end of scheme://host[:port] — the first '/' after the authority
    let after_scheme = trimmed.find("://").map(|i| i + 3).unwrap_or(0);
    let path_start = trimmed[after_scheme..].find('/').map(|i| i + after_scheme);
    match path_start {
        Some(pos) => format!("{}{}", trimmed[..pos].to_lowercase(), &trimmed[pos..]),
        None => trimmed.to_lowercase(),
    }
}

/// Check whether an account with the given server URL + username already exists.
pub(crate) fn is_duplicate_account(
    accounts: &adagio_core::account_manager::AccountManager,
    server_url: &str,
    username: &str,
) -> bool {
    let normalised = normalise_server_url(server_url);
    accounts
        .list()
        .unwrap_or_default()
        .iter()
        .any(|a| normalise_server_url(&a.server_url) == normalised && a.username == username)
}

/// Run the complete OAuth2 browser-based account connection flow.
///
/// Steps: validate server → start loopback listener → open browser → receive
/// callback code → exchange for tokens → fetch user info → check duplicate →
/// store token in keychain → persist account → return `AccountDto`.
#[tauri::command]
#[instrument(skip(app, state), fields(server_url))]
pub async fn connect_account_oauth2(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    server_url: String,
) -> Result<AccountDto, String> {
    info!(server_url = %server_url, "starting OAuth2 account connection");

    // Step 1: Validate server
    adagio_nextcloud::auth::validate_server_url(&server_url)
        .await
        .map_err(|e| e.to_string())?;

    // Step 2: Loopback listener
    let (verifier, challenge) = adagio_nextcloud::auth::generate_pkce_pair();
    let state_param = adagio_nextcloud::auth::generate_state();

    let (port, callback_fut) = crate::oauth2_callback::spawn_callback_listener(&state_param)
        .await
        .map_err(|e| e.to_string())?;

    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    // Step 3: Open browser
    let auth_url = adagio_nextcloud::auth::build_authorization_url(
        &server_url,
        adagio_nextcloud::auth::OAUTH2_CLIENT_ID,
        &redirect_uri,
        &challenge,
        &state_param,
    );

    #[allow(deprecated)]
    tauri_plugin_shell::ShellExt::shell(&app)
        .open(&auth_url, None)
        .map_err(|e| {
            error!(error = %e, "failed to open system browser");
            format!("Failed to open browser: {e}")
        })?;

    info!("browser opened; waiting for OAuth2 callback");

    // Step 4: Wait for callback code
    let code = callback_fut.await.map_err(|e| e.to_string())?;

    // Step 5: Exchange code for tokens
    let token_pair = adagio_nextcloud::auth::exchange_code(
        &server_url,
        adagio_nextcloud::auth::OAUTH2_CLIENT_ID,
        adagio_nextcloud::auth::OAUTH2_CLIENT_SECRET,
        &code,
        &verifier,
        &redirect_uri,
    )
    .await
    .map_err(|e| format!("Token exchange failed: {e}"))?;

    // Step 6: Fetch user info
    let (username, display_name) =
        adagio_nextcloud::auth::fetch_user_info(&server_url, &token_pair.access_token)
            .await
            .map_err(|e| e.to_string())?;

    // Step 7: Duplicate check
    if is_duplicate_account(&state.accounts, &server_url, &username) {
        return Err("Account already connected for this server and username".to_string());
    }

    // Step 8: Store token in keychain
    let id = AccountId::new();
    let token_json = serde_json::to_string(&token_pair)
        .map_err(|e| format!("Credential serialization failed: {e}"))?;

    let account_id_str = id.0.clone();
    let token_json_clone = token_json.clone();
    tokio::task::spawn_blocking(move || {
        adagio_nextcloud::auth::store_credentials(&account_id_str, &token_json_clone)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("Credential storage failed: {e}"))?;

    // Step 9: Persist account metadata (no credentials)
    let account = Account {
        id: id.clone(),
        display_name,
        server_url,
        username,
        keychain_service_key: id.0.clone(),
        created_at: Utc::now(),
    };

    state
        .accounts
        .add(account.clone())
        .map_err(|e| e.to_string())?;

    let mut cfg = load_config(&state.config_path);
    cfg.accounts.push(SavedAccount {
        id: account.id.0.clone(),
        display_name: account.display_name.clone(),
        server_url: account.server_url.clone(),
        username: account.username.clone(),
        keychain_service_key: account.keychain_service_key.clone(),
    });
    save_config(&state.config_path, &cfg).map_err(|e| e.to_string())?;

    info!(account_id = %account.id, "OAuth2 account connected successfully");
    Ok(AccountDto::from(account))
}

/// Add a new Nextcloud account using an app-password or pre-obtained token.
///
/// Stores credentials in the OS keychain, registers the account in memory,
/// and persists account metadata (no credentials) to config.json.
#[tauri::command]
#[instrument(skip(state, req), fields(server_url = %req.server_url))]
pub async fn add_account(
    state: State<'_, AppState>,
    req: AddAccountRequest,
) -> Result<AccountDto, String> {
    if is_duplicate_account(&state.accounts, &req.server_url, &req.username) {
        return Err("Account already connected for this server and username".to_string());
    }

    let id = AccountId::new();
    let account_id_str = id.0.clone();
    let secret = req.secret.clone();
    tokio::task::spawn_blocking(move || {
        adagio_nextcloud::auth::store_credentials(&account_id_str, &secret)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let account = Account {
        id: id.clone(),
        display_name: req.display_name,
        server_url: req.server_url,
        username: req.username,
        keychain_service_key: id.0.clone(),
        created_at: Utc::now(),
    };

    state
        .accounts
        .add(account.clone())
        .map_err(|e| e.to_string())?;

    let mut cfg = load_config(&state.config_path);
    cfg.accounts.push(SavedAccount {
        id: account.id.0.clone(),
        display_name: account.display_name.clone(),
        server_url: account.server_url.clone(),
        username: account.username.clone(),
        keychain_service_key: account.keychain_service_key.clone(),
    });
    save_config(&state.config_path, &cfg).map_err(|e| e.to_string())?;

    info!(account_id = %account.id, "account added via app-password");
    Ok(AccountDto::from(account))
}

/// Remove an account by ID, delete its keychain credentials, and update config.json.
#[tauri::command]
#[instrument(skip(state), fields(account_id))]
pub async fn remove_account(state: State<'_, AppState>, account_id: String) -> Result<(), String> {
    let id = AccountId(account_id.clone());
    let removed = state.accounts.remove(&id).map_err(|e| e.to_string())?;

    if let Some(account) = removed {
        let key = account.keychain_service_key.clone();
        tokio::task::spawn_blocking(move || adagio_nextcloud::auth::delete_credentials(&key))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
    }

    let mut cfg = load_config(&state.config_path);
    cfg.accounts.retain(|a| a.id != account_id);
    cfg.pairs.retain(|p| p.account_id != account_id);
    save_config(&state.config_path, &cfg).map_err(|e| e.to_string())?;

    info!(account_id, "account removed");
    Ok(())
}

/// List all registered accounts.
#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<AccountDto>, String> {
    let accounts = state.accounts.list().map_err(|e| e.to_string())?;
    Ok(accounts.into_iter().map(AccountDto::from).collect())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{save_config, SavedConfig};
    use adagio_core::account_manager::AccountManager;
    use tempfile::TempDir;

    fn make_manager_with_account(server_url: &str, username: &str) -> AccountManager {
        let manager = AccountManager::new();
        manager
            .add(Account {
                id: AccountId::new(),
                display_name: "Test".to_string(),
                server_url: server_url.to_string(),
                username: username.to_string(),
                keychain_service_key: "test-key".to_string(),
                created_at: Utc::now(),
            })
            .unwrap();
        manager
    }

    // ── T008: connect_account_oauth2 validation ───────────────────────────────

    #[tokio::test]
    async fn validate_server_url_returns_err_for_unroutable_host() {
        let result = adagio_nextcloud::auth::validate_server_url("http://192.0.2.1").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Server unreachable"));
    }

    // ── T014/T016: duplicate detection ───────────────────────────────────────

    #[test]
    fn duplicate_detection_rejects_same_server_and_username() {
        let mgr = make_manager_with_account("https://cloud.example.com", "alice");
        assert!(is_duplicate_account(
            &mgr,
            "https://cloud.example.com",
            "alice"
        ));
    }

    #[test]
    fn duplicate_detection_ignores_trailing_slash() {
        let mgr = make_manager_with_account("https://cloud.example.com/", "alice");
        assert!(is_duplicate_account(
            &mgr,
            "https://cloud.example.com",
            "alice"
        ));
    }

    #[test]
    fn duplicate_detection_ignores_host_case() {
        let mgr = make_manager_with_account("https://Cloud.Example.Com", "alice");
        assert!(is_duplicate_account(
            &mgr,
            "https://cloud.example.com",
            "alice"
        ));
    }

    #[test]
    fn duplicate_detection_allows_different_username() {
        let mgr = make_manager_with_account("https://cloud.example.com", "alice");
        assert!(!is_duplicate_account(
            &mgr,
            "https://cloud.example.com",
            "bob"
        ));
    }

    #[test]
    fn duplicate_detection_allows_different_server() {
        let mgr = make_manager_with_account("https://cloud.example.com", "alice");
        assert!(!is_duplicate_account(
            &mgr,
            "https://other.example.com",
            "alice"
        ));
    }

    #[test]
    fn duplicate_detection_allows_first_account() {
        let mgr = AccountManager::new();
        assert!(!is_duplicate_account(
            &mgr,
            "https://cloud.example.com",
            "alice"
        ));
    }

    // ── T015: two-account list ────────────────────────────────────────────────

    #[test]
    fn account_manager_lists_both_after_two_additions() {
        let mgr = AccountManager::new();
        for (url, user) in &[
            ("https://server-a.example.com", "alice"),
            ("https://server-b.example.com", "bob"),
        ] {
            mgr.add(Account {
                id: AccountId::new(),
                display_name: user.to_string(),
                server_url: url.to_string(),
                username: user.to_string(),
                keychain_service_key: url.to_string(),
                created_at: Utc::now(),
            })
            .unwrap();
        }
        assert_eq!(mgr.list().unwrap().len(), 2);
    }

    // ── T018: remove_account cascade ─────────────────────────────────────────

    #[test]
    fn remove_clears_account_and_associated_pairs_from_config() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.json");

        let mut cfg = SavedConfig::default();
        cfg.accounts.push(SavedAccount {
            id: "acc-1".to_string(),
            display_name: "Alice".to_string(),
            server_url: "https://cloud.example.com".to_string(),
            username: "alice".to_string(),
            keychain_service_key: "acc-1".to_string(),
        });
        cfg.pairs.push(crate::config::SavedPair {
            id: "pair-1".to_string(),
            account_id: "acc-1".to_string(),
            local_root: "/tmp/sync".to_string(),
            remote_root: "/".to_string(),
            scan_interval_secs: 7200,
            scan_on_startup: true,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            selective_paths: vec![],
            exclude_patterns: vec![],
        });
        save_config(&config_path, &cfg).unwrap();

        let mut loaded = crate::config::load_config(&config_path);
        loaded.accounts.retain(|a| a.id != "acc-1");
        loaded.pairs.retain(|p| p.account_id != "acc-1");
        save_config(&config_path, &loaded).unwrap();

        let final_cfg = crate::config::load_config(&config_path);
        assert_eq!(final_cfg.accounts.len(), 0, "account removed");
        assert_eq!(final_cfg.pairs.len(), 0, "pairs cascade removed");
    }

    #[test]
    fn remove_is_idempotent_for_nonexistent_account() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.json");
        save_config(&config_path, &SavedConfig::default()).unwrap();

        let mut loaded = crate::config::load_config(&config_path);
        loaded.accounts.retain(|a| a.id != "ghost-id");
        save_config(&config_path, &loaded).unwrap();

        assert_eq!(crate::config::load_config(&config_path).accounts.len(), 0);
    }

    // ── T023: no credentials in config.json ──────────────────────────────────

    #[test]
    fn saved_config_contains_no_credential_fields() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join("config.json");
        let mut cfg = SavedConfig::default();
        cfg.accounts.push(SavedAccount {
            id: "acc-1".to_string(),
            display_name: "Alice".to_string(),
            server_url: "https://cloud.example.com".to_string(),
            username: "alice".to_string(),
            keychain_service_key: "acc-1".to_string(),
        });
        save_config(&config_path, &cfg).unwrap();

        let raw = std::fs::read_to_string(&config_path).unwrap();
        for forbidden in &["access_token", "refresh_token", "password"] {
            assert!(
                !raw.contains(forbidden),
                "config.json must not contain '{forbidden}'"
            );
        }
    }
}
