pub mod fallback;
pub mod mock;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::dirs_cache_dir;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;

// Re-export all shared types from adagio-core::vfs so callers can use either path.
pub use adagio_core::vfs::types::{
    VfsCacheEntry, VfsError, VfsMountHandle, VfsProvider, VfsState, VfsStats,
};

// ── Platform selector ─────────────────────────────────────────────────────────

use std::sync::Arc;

/// Return the appropriate `VfsProvider` for the current platform.
pub fn create_platform_provider() -> Arc<dyn VfsProvider> {
    #[cfg(target_os = "linux")]
    {
        let p = linux::LinuxVfsProvider::new();
        if p.is_supported() {
            return Arc::new(p);
        }
    }
    #[cfg(target_os = "macos")]
    {
        let p = macos::MacosVfsProvider::new();
        if p.is_supported() {
            return Arc::new(p);
        }
    }
    #[cfg(windows)]
    {
        let p = windows::WindowsVfsProvider::new();
        if p.is_supported() {
            return Arc::new(p);
        }
    }
    Arc::new(fallback::FallbackVfsProvider)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use adagio_core::types::{PairId, RelativePath};
    use chrono::Utc;

    // T005-a: State machine behaviour is correct.
    #[test]
    fn vfs_state_transitions_are_valid() {
        let cloud = VfsState::CloudOnly;
        assert!(!cloud.is_cached());
        assert!(!cloud.is_pinned());
        assert!(!cloud.is_auto_evictable());

        let avail = VfsState::LocallyAvailable {
            cached_at: Utc::now(),
            last_accessed: Utc::now(),
        };
        assert!(avail.is_cached());
        assert!(!avail.is_pinned());
        assert!(avail.is_auto_evictable());

        let pinned = VfsState::Pinned {
            cached_at: Utc::now(),
            last_accessed: Utc::now(),
        };
        assert!(pinned.is_cached());
        assert!(pinned.is_pinned());
        assert!(!pinned.is_auto_evictable());
    }

    // T005-b: New cloud-only entry has zero cache bytes.
    #[test]
    fn vfs_cache_entry_default_is_cloud_only() {
        let entry = VfsCacheEntry::new_cloud_only(
            PairId::new(),
            RelativePath::new("docs/file.pdf"),
            1024,
            Some("etag1".to_string()),
            Utc::now(),
        );
        assert_eq!(entry.state, VfsState::CloudOnly);
        assert_eq!(entry.cache_bytes, 0);
    }
}
