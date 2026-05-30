use std::path::PathBuf;

// ── Socket path (platform-specific) ──────────────────────────────────────────

/// Return the platform-specific path for the daemon's IPC socket / named pipe.
///
/// - **Linux**: `$XDG_RUNTIME_DIR/adagio/daemon.sock`
///   (fallback: `/tmp/adagio-<uid>.sock`)
/// - **macOS**: `~/Library/Application Support/adagio/daemon.sock`
/// - **Windows**: (named pipe path returned as a pseudo-path string;
///   callers must use `tokio::net::windows::named_pipe` with the raw string)
pub fn daemon_socket_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let uid = unsafe { libc::getuid() };
                PathBuf::from(format!("/tmp/adagio-{uid}"))
            });
        runtime_dir.join("adagio").join("daemon.sock")
    }

    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));
        home.join("Library")
            .join("Application Support")
            .join("adagio")
            .join("daemon.sock")
    }

    #[cfg(windows)]
    {
        // Named pipe path — callers use `\\.\pipe\adagio-daemon` directly.
        // We return a pseudo absolute path so is_absolute() holds for tests.
        PathBuf::from(r"\\.\pipe\adagio-daemon")
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        PathBuf::from("/tmp/adagio-daemon.sock")
    }
}

/// Ensure the directory containing `socket_path` exists.
///
/// On Linux this creates `$XDG_RUNTIME_DIR/adagio/` with mode 0700.
pub fn ensure_socket_dir(socket_path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
        // On Unix, restrict the socket directory to owner only.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

// ── Config directory (platform-specific) ─────────────────────────────────────

/// Return the platform-specific path for Adagio's configuration and database
/// directory.
///
/// - **Linux**: `$XDG_DATA_HOME/adagio` (fallback: `~/.local/share/adagio`)
/// - **macOS**: `~/Library/Application Support/adagio`
/// - **Windows**: `%APPDATA%\adagio`
///
/// Both `adagio-daemon` and `adagio-cli` use this to locate (or pass as
/// `--config-dir`) the directory that holds `config.json` and `adagio.db`.
pub fn platform_config_dir() -> anyhow::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                PathBuf::from(home).join(".local").join("share")
            });
        Ok(base.join("adagio"))
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME")?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("adagio"))
    }
    #[cfg(windows)]
    {
        let appdata = std::env::var("APPDATA")?;
        Ok(PathBuf::from(appdata).join("adagio"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        Ok(PathBuf::from("/tmp/adagio"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T007 — daemon_socket_path returns an absolute path.
    #[test]
    fn daemon_socket_path_is_absolute() {
        let p = daemon_socket_path();
        assert!(p.is_absolute(), "socket path must be absolute: {p:?}");
    }

    #[test]
    fn daemon_socket_path_contains_adagio() {
        let p = daemon_socket_path();
        let s = p.to_string_lossy();
        assert!(
            s.contains("adagio"),
            "socket path should contain 'adagio': {s}"
        );
    }
}
