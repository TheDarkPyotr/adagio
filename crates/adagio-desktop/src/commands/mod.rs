pub mod account;
pub mod bandwidth;
pub mod conflicts;
pub mod daemon;
pub mod e2ee;
pub mod files;
pub mod network;
pub mod onboarding;
pub mod pair;
pub mod prefs;
pub mod sharing;
pub mod sync;
pub mod vfs;

/// Returns the current operating system identifier.
///
/// Used by the tray popover to display platform-appropriate keyboard modifier
/// labels (`⌘` on macOS, `Ctrl+` on Linux and Windows).
#[tauri::command]
pub fn get_platform() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        "linux"
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // T006 — get_platform returns a recognised platform string on all CI targets.
    #[test]
    fn get_platform_is_valid_string() {
        let valid = ["linux", "macos", "windows"];
        assert!(
            valid.contains(&get_platform()),
            "get_platform() returned unexpected value: {}",
            get_platform()
        );
    }
}
