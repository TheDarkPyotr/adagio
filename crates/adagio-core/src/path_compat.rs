use crate::error::SyncError;

/// Reserved filenames on Windows (case-insensitive, with or without extension).
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Characters that are disallowed in Windows filenames.
const WINDOWS_DISALLOWED: &[char] = &[
    '<', '>', ':', '"', '/', '\\', '|', '?', '*', '\x00', '\x01', '\x02', '\x03', '\x04', '\x05',
    '\x06', '\x07', '\x08', '\x09', '\x0a', '\x0b', '\x0c', '\x0d', '\x0e', '\x0f', '\x10', '\x11',
    '\x12', '\x13', '\x14', '\x15', '\x16', '\x17', '\x18', '\x19', '\x1a', '\x1b', '\x1c', '\x1d',
    '\x1e', '\x1f',
];

/// Maximum total path length (Windows MAX_PATH = 260).
pub const MAX_PATH_LEN: usize = 259;

/// Check whether `path` is safe to use on all supported platforms.
///
/// Returns `Ok(())` if the path is compatible, or `Err(SyncError::Permanent(reason))`
/// describing the first violation found.
pub fn check_path_compat(path: &str) -> Result<(), SyncError> {
    if path.len() > MAX_PATH_LEN {
        return Err(SyncError::Permanent(format!(
            "path too long ({} chars, max {MAX_PATH_LEN}): {path}",
            path.len()
        )));
    }

    for segment in path.split('/') {
        if segment.is_empty() {
            continue;
        }

        // Check reserved Windows names (stem without extension).
        let stem = segment.split('.').next().unwrap_or(segment);
        if WINDOWS_RESERVED
            .iter()
            .any(|r| stem.eq_ignore_ascii_case(r))
        {
            return Err(SyncError::Permanent(format!(
                "reserved Windows filename in path: {segment}"
            )));
        }

        // Check for disallowed characters.
        if let Some(bad) = segment.chars().find(|c| WINDOWS_DISALLOWED.contains(c)) {
            return Err(SyncError::Permanent(format!(
                "disallowed character {bad:?} in path segment: {segment}"
            )));
        }

        // Trailing dot or space is not allowed on Windows.
        let last = segment.chars().last();
        if matches!(last, Some('.') | Some(' ')) {
            return Err(SyncError::Permanent(format!(
                "path segment ends with dot or space (not allowed on Windows): {segment}"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // T094-1: Normal path passes.
    #[test]
    fn normal_path_accepted() {
        assert!(check_path_compat("docs/report.pdf").is_ok());
        assert!(check_path_compat("photos/2024/img.jpg").is_ok());
    }

    // T094-2: Reserved names are rejected.
    #[test]
    fn reserved_windows_names_rejected() {
        assert!(check_path_compat("CON").is_err());
        assert!(check_path_compat("con.txt").is_err());
        assert!(check_path_compat("docs/NUL").is_err());
        assert!(check_path_compat("COM3.log").is_err());
        assert!(check_path_compat("LPT9").is_err());
    }

    // T094-3: Disallowed characters are rejected.
    #[test]
    fn disallowed_chars_rejected() {
        assert!(check_path_compat("file<name.txt").is_err());
        assert!(check_path_compat("docs/file:info.txt").is_err());
        assert!(check_path_compat("hello?world").is_err());
        assert!(check_path_compat("file|pipe").is_err());
    }

    // T094-4: Trailing dot and space rejected.
    #[test]
    fn trailing_dot_and_space_rejected() {
        assert!(check_path_compat("file.").is_err(), "trailing dot");
        assert!(check_path_compat("file ").is_err(), "trailing space");
    }

    // T094-5: Path too long.
    #[test]
    fn path_too_long_rejected() {
        let long = "a".repeat(MAX_PATH_LEN + 1);
        assert!(check_path_compat(&long).is_err());
    }

    // T094-6: Normal names that look like reserved are OK with extra chars.
    #[test]
    fn con_prefix_in_longer_name_accepted() {
        assert!(check_path_compat("console.txt").is_ok());
        assert!(check_path_compat("contracts/deal.pdf").is_ok());
    }
}
