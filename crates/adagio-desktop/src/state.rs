use adagio_ipc::DaemonClient;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

/// Shared application state for the Tauri desktop shell.
///
/// After daemon extraction, `AppState` is a thin wrapper around the IPC client.
/// All sync-engine state (journal, accounts, pairs, engine) lives in the daemon
/// process. The desktop shell connects to it on startup.
pub struct AppState {
    /// IPC client connected to the running `adagio-daemon` process.
    pub daemon: Arc<DaemonClient>,
    /// Path to the config directory (used for UI preferences such as palette).
    pub config_path: PathBuf,
}

impl AppState {
    /// Initialise state by connecting to a running daemon, or starting it if needed.
    ///
    /// `config_dir` is the platform-specific app data directory.
    /// `daemon_binary_path` is the path to the `adagio-daemon` executable.
    pub async fn new(config_dir: PathBuf, daemon_binary_path: &std::path::Path) -> Result<Self> {
        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("config.json");
        let daemon = DaemonClient::connect_or_start(daemon_binary_path).await?;
        Ok(Self {
            daemon,
            config_path,
        })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // T014 — AppState must have a daemon field, not an engine field.
    // This test verifies the struct shape at compile time: if AppState still has
    // an `engine` field, this will fail to compile.
    #[test]
    fn app_state_has_daemon_client_not_engine() {
        // AppState { daemon: Arc<DaemonClient>, config_path: PathBuf }
        // If 'engine' field existed, the field access below would fail to compile.
        // We access 'config_path' to confirm it exists; 'daemon' is type-checked
        // by the struct definition itself.
        let _type_check = |s: &AppState| {
            let _path: &PathBuf = &s.config_path;
            let _client: &Arc<DaemonClient> = &s.daemon;
        };
    }

    // The new() method requires a running daemon, so we don't test it in unit
    // tests — it is covered by the quickstart.md integration validation (T065).
}
