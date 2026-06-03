use adagio_ipc::{transport::daemon_socket_path, DaemonClient};
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

// ── Tray position helper ──────────────────────────────────────────────────────

/// Compute the screen position for the tray popover window.
///
/// Positions the popover below the click point for top panels and above it for
/// bottom taskbars. When `click` is `(0.0, 0.0)` (Wayland compositors that do
/// not expose cursor position), falls back to the top-right corner of the
/// primary monitor at `(screen_w - win_w - 16, 48)`.
pub fn compute_popover_position(
    click: (f64, f64),
    win_size: (u32, u32),
    screen_w: u32,
    screen_h: u32,
) -> (i32, i32) {
    let (cx, cy) = click;
    let (win_w, win_h) = win_size;

    // Position (0,0): Wayland compositors that don't expose cursor position, or
    // Linux AppIndicator synthetic events after menu activation. Fall back to
    // top-right corner (the natural tray icon area on most Linux DEs).
    if cx == 0.0 && cy == 0.0 {
        tracing::debug!("tray click position is (0,0) — using top-right fallback");
        return ((screen_w as i32) - (win_w as i32) - 16, 48);
    }

    let is_top_panel = cy < (screen_h as f64) / 2.0;
    if is_top_panel {
        // Popover opens below the icon (top panel, e.g. GNOME/Ubuntu).
        let x = ((cx as i32) - (win_w as i32 / 2)).max(0);
        let y = cy as i32 + 8;
        (x, y)
    } else {
        // Popover opens above the icon (bottom taskbar, e.g. KDE Plasma default).
        let x = ((cx as i32) - (win_w as i32 / 2)).max(0);
        let y = ((cy as i32) - (win_h as i32) - 8).max(0);
        (x, y)
    }
}

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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // T003 — popover is positioned BELOW the click on a top panel.
    #[test]
    fn compute_tray_position_top_panel() {
        let pos = compute_popover_position((760.0, 30.0), (380, 520), 1920, 1080);
        // y must be greater than the click y (popover below the icon)
        assert!(
            pos.1 > 30,
            "expected popover below click (top panel), got y={}",
            pos.1
        );
        assert!(pos.0 >= 0, "x must not be negative");
    }

    // T004 — popover is positioned ABOVE the click on a bottom taskbar.
    #[test]
    fn compute_tray_position_bottom_panel() {
        let pos = compute_popover_position((760.0, 1050.0), (380, 520), 1920, 1080);
        // y must be less than the click y (popover above the icon)
        assert!(
            pos.1 < 1050,
            "expected popover above click (bottom panel), got y={}",
            pos.1
        );
        assert!(pos.0 >= 0);
    }

    // T005 — Wayland fallback when click position is (0, 0).
    #[test]
    fn compute_tray_position_wayland_fallback() {
        let pos = compute_popover_position((0.0, 0.0), (380, 520), 1920, 1080);
        assert_eq!(pos.0, 1920 - 380 - 16, "expected top-right fallback x");
        assert_eq!(pos.1, 48, "expected top-right fallback y");
    }
}
