use adagio_ipc::{transport::daemon_socket_path, DaemonClient};
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

/// Connect to a running `adagio-daemon` or spawn it, then upgrade the
/// provided stub client with the real socket connection.
///
/// `config_dir` is passed to the daemon as `--config-dir` so both processes
/// use the same data directory (avoids Tauri `app_config_dir` vs XDG mismatch).
pub async fn connect_or_upgrade(
    stub: &Arc<DaemonClient>,
    config_dir: &std::path::Path,
) -> Result<()> {
    let daemon_path = daemon_binary_path()?;
    let socket_path = daemon_socket_path();

    // Fast path: daemon already running.
    if stub.upgrade_connection(&socket_path).await.is_ok() {
        info!("upgraded to existing adagio-daemon");
        return Ok(());
    }

    // Spawn daemon with the desktop's config dir.
    info!(daemon_path = %daemon_path.display(), config_dir = %config_dir.display(), "spawning adagio-daemon");
    spawn_daemon_with_config(&daemon_path, config_dir)?;

    // Retry for up to 5 s.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        if stub.upgrade_connection(&socket_path).await.is_ok() {
            info!("upgraded to newly started adagio-daemon");
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("adagio-daemon did not start within 5 seconds");
        }
    }
}

/// Derive the daemon binary path from the current executable's location.
pub fn daemon_binary_path() -> Result<std::path::PathBuf> {
    let current = std::env::current_exe()?;
    let parent = current
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot determine daemon path: no parent dir"))?;

    #[cfg(windows)]
    let daemon = parent.join("adagio-daemon.exe");
    #[cfg(not(windows))]
    let daemon = parent.join("adagio-daemon");

    Ok(daemon)
}

/// Override daemon binary path (for testing / CI).
pub fn daemon_binary_path_or(override_path: Option<&Path>) -> Result<std::path::PathBuf> {
    if let Some(p) = override_path {
        return Ok(p.to_path_buf());
    }
    daemon_binary_path()
}

/// Spawn the daemon as a detached process, passing the config directory.
fn spawn_daemon_with_config(
    daemon_path: &std::path::Path,
    config_dir: &std::path::Path,
) -> Result<()> {
    #[cfg(unix)]
    {
        std::process::Command::new(daemon_path)
            .arg("--config-dir")
            .arg(config_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        std::process::Command::new(daemon_path)
            .arg("--config-dir")
            .arg(config_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()?;
    }
    #[cfg(not(any(unix, windows)))]
    {
        std::process::Command::new(daemon_path)
            .arg("--config-dir")
            .arg(config_dir)
            .spawn()?;
    }
    Ok(())
}
