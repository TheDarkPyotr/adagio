use adagio_core::network::NetworkPolicy;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// On-disk representation of all user configuration.
///
/// Written atomically (write temp + rename) to `config.json` in the platform
/// app-config directory. Never contains credentials — only keychain lookup keys.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavedConfig {
    /// Schema version for future migration support.
    #[serde(default)]
    pub version: u32,
    /// All registered Nextcloud accounts (no credentials).
    #[serde(default)]
    pub accounts: Vec<SavedAccount>,
    /// All configured sync pairs.
    #[serde(default)]
    pub pairs: Vec<SavedPair>,
    /// Active color palette name. `None` means the frontend applies its default
    /// (sienna, or ink if OS dark-mode is active). Never store credentials here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
    /// Network awareness policy (metered/battery/SSID rules). Default = all Allow.
    #[serde(default)]
    pub network_policy: NetworkPolicy,
    /// User-defined custom color palettes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_palettes: Vec<CustomPaletteEntry>,
}

/// A user-defined colour palette stored alongside built-in themes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPaletteEntry {
    /// Unique identifier, e.g. `"custom-my-theme"`.
    pub id: String,
    /// Human-readable name chosen by the user.
    pub name: String,
    /// Background colour as a CSS hex string, e.g. `"#f5f1ea"`.
    pub cream: String,
    /// Text colour as a CSS hex string.
    pub ink: String,
    /// Accent / action colour as a CSS hex string.
    pub accent: String,
}

/// A Nextcloud account as persisted on disk.
///
/// The actual credential (app-password or OAuth2 token) lives in the OS keychain;
/// only the keychain lookup key is stored here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedAccount {
    pub id: String,
    pub display_name: String,
    pub server_url: String,
    pub username: String,
    /// Key used to retrieve credentials from the OS keychain.
    pub keychain_service_key: String,
    /// Maximum upload speed in Kbps. 0 = unlimited.
    #[serde(default)]
    pub upload_limit_kbps: u64,
    /// Maximum download speed in Kbps. 0 = unlimited.
    #[serde(default)]
    pub download_limit_kbps: u64,
}

/// A sync pair as persisted on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedPair {
    pub id: String,
    pub account_id: String,
    /// Absolute path of the local sync folder.
    pub local_root: String,
    /// User-relative remote path (e.g. `Documents/Adagio`).
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
    /// Number of parallel upload workers in bulk mode (default: 8).
    #[serde(default = "default_bulk_upload_workers")]
    pub bulk_upload_workers: u8,
    /// Minimum pending-upload count to trigger bulk mode (default: 50).
    #[serde(default = "default_bulk_upload_threshold_files")]
    pub bulk_upload_threshold_files: u32,
    /// File size in bytes above which chunked upload is used (default: 10 MiB).
    #[serde(default = "default_bulk_upload_chunk_threshold_bytes")]
    pub bulk_upload_chunk_threshold_bytes: u64,
}

fn default_scan_interval() -> u64 {
    7200
}
fn default_scan_on_startup() -> bool {
    true
}
fn default_bulk_upload_workers() -> u8 {
    8
}
fn default_bulk_upload_threshold_files() -> u32 {
    50
}
fn default_bulk_upload_chunk_threshold_bytes() -> u64 {
    10 * 1024 * 1024
}
fn default_concurrency() -> usize {
    3
}

/// Load configuration from `path`.
///
/// Returns a default empty config if the file does not exist. Logs a warning
/// and returns empty config if the file is present but cannot be parsed, so
/// the app starts cleanly rather than crashing (satisfies SC-006).
pub fn load_config(path: &Path) -> SavedConfig {
    if !path.exists() {
        info!(path = %path.display(), "config file not found; starting with empty config");
        return SavedConfig::default();
    }
    let contents = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            warn!(path = %path.display(), error = %e, "failed to read config file; starting with empty config");
            return SavedConfig::default();
        }
    };
    match serde_json::from_str::<SavedConfig>(&contents) {
        Ok(cfg) => {
            info!(
                path = %path.display(),
                accounts = cfg.accounts.len(),
                pairs = cfg.pairs.len(),
                "config loaded"
            );
            cfg
        }
        Err(e) => {
            warn!(path = %path.display(), error = %e, "config file is corrupt; starting with empty config");
            SavedConfig::default()
        }
    }
}

/// Persist `config` to `path` atomically (write temp file, then rename).
///
/// The rename ensures that a crash mid-write never leaves a partial file.
pub fn save_config(path: &Path, config: &SavedConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(config)?;

    // Write to a sibling temp file first, then rename atomically.
    let tmp_path: PathBuf = path.with_extension("json.tmp");
    fs::write(&tmp_path, &json)?;
    fs::rename(&tmp_path, path)?;

    info!(
        path = %path.display(),
        accounts = config.accounts.len(),
        pairs = config.pairs.len(),
        "config saved"
    );
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_account(id: &str) -> SavedAccount {
        SavedAccount {
            id: id.to_string(),
            display_name: "Test User".to_string(),
            server_url: "https://cloud.example.com".to_string(),
            username: "testuser".to_string(),
            keychain_service_key: format!("adagio/{id}"),
            upload_limit_kbps: 0,
            download_limit_kbps: 0,
        }
    }

    fn make_pair(id: &str, account_id: &str) -> SavedPair {
        SavedPair {
            id: id.to_string(),
            account_id: account_id.to_string(),
            local_root: "/tmp/sync".to_string(),
            remote_root: "Documents/Adagio".to_string(),
            scan_interval_secs: 7200,
            scan_on_startup: true,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            selective_paths: vec![],
            exclude_patterns: vec![],
            bulk_upload_workers: 8,
            bulk_upload_threshold_files: 50,
            bulk_upload_chunk_threshold_bytes: 10 * 1024 * 1024,
        }
    }

    // ── T009: SavedConfig round-trip tests ────────────────────────────────────

    #[test]
    fn empty_config_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        let cfg = SavedConfig::default();
        save_config(&path, &cfg).unwrap();
        let loaded = load_config(&path);
        assert_eq!(loaded.accounts.len(), 0);
        assert_eq!(loaded.pairs.len(), 0);
    }

    #[test]
    fn config_with_accounts_and_pairs_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        let cfg = SavedConfig {
            version: 1,
            accounts: vec![make_account("acc-1"), make_account("acc-2")],
            pairs: vec![make_pair("pair-1", "acc-1")],
            palette: None,
            network_policy: Default::default(),
        };
        save_config(&path, &cfg).unwrap();
        let loaded = load_config(&path);
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.accounts.len(), 2);
        assert_eq!(loaded.accounts[0].id, "acc-1");
        assert_eq!(loaded.pairs.len(), 1);
        assert_eq!(loaded.pairs[0].account_id, "acc-1");
    }

    #[test]
    fn load_config_returns_default_when_file_absent() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");
        let cfg = load_config(&path);
        assert_eq!(cfg.accounts.len(), 0);
        assert_eq!(cfg.pairs.len(), 0);
    }

    #[test]
    fn load_config_returns_default_on_corrupt_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, b"not valid json {{{").unwrap();
        let cfg = load_config(&path);
        assert_eq!(
            cfg.accounts.len(),
            0,
            "corrupt file should produce empty config"
        );
    }

    #[test]
    fn save_config_creates_parent_directories() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested/dirs/config.json");
        let cfg = SavedConfig::default();
        save_config(&path, &cfg).unwrap();
        assert!(path.exists(), "config file should be created");
    }

    #[test]
    fn credentials_never_appear_in_saved_config() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.json");
        let cfg = SavedConfig {
            version: 1,
            accounts: vec![make_account("acc-1")],
            pairs: vec![],
            palette: None,
            network_policy: Default::default(),
        };
        save_config(&path, &cfg).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(
            !raw.contains("password") && !raw.contains("secret") && !raw.contains("token"),
            "no credential fields should appear in config.json"
        );
        assert!(
            raw.contains("keychain_service_key"),
            "keychain key reference must be present"
        );
    }
}
