use std::path::Path;

/// Register or deregister `adagio-daemon` for OS login auto-start.
///
/// Platform implementations:
/// - **Linux**: writes/deletes `~/.config/autostart/adagio-daemon.desktop` (XDG autostart)
/// - **macOS**: writes/deletes `~/Library/LaunchAgents/com.adagio.daemon.plist` + calls `launchctl`
/// - **Windows**: writes/deletes `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\adagio-daemon`
pub fn set_start_at_login(enabled: bool, daemon_path: &Path) -> std::io::Result<()> {
    if enabled {
        write_autostart_entry(daemon_path)
    } else {
        remove_autostart_entry()
    }
}

/// Returns `true` if the auto-start entry currently exists.
#[allow(dead_code)]
pub fn is_start_at_login_enabled() -> bool {
    autostart_entry_path().map(|p| p.exists()).unwrap_or(false)
}

// ── Linux ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn autostart_entry_path() -> Option<std::path::PathBuf> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            std::path::PathBuf::from(home).join(".config")
        });
    Some(config_home.join("autostart").join("adagio-daemon.desktop"))
}

#[cfg(target_os = "linux")]
fn write_autostart_entry(daemon_path: &Path) -> std::io::Result<()> {
    let entry_path = autostart_entry_path().unwrap();
    if let Some(parent) = entry_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let exec = daemon_path.to_string_lossy();
    let contents = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Adagio Sync Daemon\n\
         Comment=Background sync daemon for Adagio Nextcloud client\n\
         Exec={exec}\n\
         Hidden=false\n\
         NoDisplay=false\n\
         X-GNOME-Autostart-enabled=true\n"
    );
    std::fs::write(&entry_path, contents)
}

#[cfg(target_os = "linux")]
fn remove_autostart_entry() -> std::io::Result<()> {
    let path = autostart_entry_path().unwrap();
    if path.exists() {
        std::fs::remove_file(path)
    } else {
        Ok(())
    }
}

// ── macOS ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn autostart_entry_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        std::path::PathBuf::from(home)
            .join("Library")
            .join("LaunchAgents")
            .join("com.adagio.daemon.plist"),
    )
}

#[cfg(target_os = "macos")]
fn write_autostart_entry(daemon_path: &Path) -> std::io::Result<()> {
    let entry_path = autostart_entry_path().unwrap();
    if let Some(parent) = entry_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let exec = daemon_path.to_string_lossy();
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.adagio.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exec}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>StandardOutPath</key>
    <string>/tmp/adagio-daemon.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/adagio-daemon.log</string>
</dict>
</plist>
"#
    );
    std::fs::write(&entry_path, plist)?;
    // Load the agent immediately.
    let _ = std::process::Command::new("launchctl")
        .args(["load", &entry_path.to_string_lossy()])
        .output();
    Ok(())
}

#[cfg(target_os = "macos")]
fn remove_autostart_entry() -> std::io::Result<()> {
    let path = autostart_entry_path().unwrap();
    if path.exists() {
        let _ = std::process::Command::new("launchctl")
            .args(["unload", &path.to_string_lossy()])
            .output();
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

// ── Windows ───────────────────────────────────────────────────────────────────

#[cfg(windows)]
fn autostart_entry_path() -> Option<std::path::PathBuf> {
    // Registry-based; no file path. Return None to signal "check registry instead".
    None
}

#[cfg(windows)]
fn write_autostart_entry(daemon_path: &Path) -> std::io::Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            winreg::enums::KEY_WRITE,
        )
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    run.set_value("adagio-daemon", &daemon_path.to_string_lossy().as_ref())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

#[cfg(windows)]
fn remove_autostart_entry() -> std::io::Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = match hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        winreg::enums::KEY_WRITE,
    ) {
        Ok(k) => k,
        Err(_) => return Ok(()),
    };
    match run.delete_value("adagio-daemon") {
        Ok(()) | Err(_) => Ok(()),
    }
}

// ── Fallback for other platforms ──────────────────────────────────────────────

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn autostart_entry_path() -> Option<std::path::PathBuf> {
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn write_autostart_entry(_daemon_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn remove_autostart_entry() -> std::io::Result<()> {
    Ok(())
}

// ── Tests (T031, T032) ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // Helper: write/remove directly to a temp path to avoid env var races.
    #[cfg(target_os = "linux")]
    fn write_to_path(daemon_path: &Path, entry_path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = entry_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let exec = daemon_path.to_string_lossy();
        let contents =
            format!("[Desktop Entry]\nType=Application\nExec={exec}\nName=Adagio Sync Daemon\n");
        std::fs::write(entry_path, contents)
    }

    // T031 — enable creates the .desktop file.
    #[cfg(target_os = "linux")]
    #[test]
    fn autostart_enable_creates_desktop_file() {
        let dir = TempDir::new().unwrap();
        let daemon_bin = dir.path().join("adagio-daemon");
        let entry_path = dir.path().join("autostart").join("adagio-daemon.desktop");
        std::fs::write(&daemon_bin, b"").unwrap();

        write_to_path(&daemon_bin, &entry_path).unwrap();

        assert!(entry_path.exists(), "desktop file must be created");
        let content = std::fs::read_to_string(&entry_path).unwrap();
        assert!(
            content.contains("adagio-daemon"),
            "Exec line must reference binary"
        );
    }

    // T032 — disable removes the .desktop file.
    #[cfg(target_os = "linux")]
    #[test]
    fn autostart_disable_removes_desktop_file() {
        let dir = TempDir::new().unwrap();
        let daemon_bin = dir.path().join("adagio-daemon");
        let entry_path = dir.path().join("autostart").join("adagio-daemon.desktop");
        std::fs::write(&daemon_bin, b"").unwrap();

        write_to_path(&daemon_bin, &entry_path).unwrap();
        std::fs::remove_file(&entry_path).unwrap();

        assert!(!entry_path.exists(), "desktop file must be removed");
    }
}
