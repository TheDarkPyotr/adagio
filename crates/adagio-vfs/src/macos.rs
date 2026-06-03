use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

use adagio_core::journal::Journal;
use adagio_core::remote::RemoteClient;
use adagio_core::types::{PairId, RelativePath, SyncPair};

use crate::{VfsCacheEntry, VfsError, VfsMountHandle, VfsProvider};

/// macOS VFS driver using Apple FileProvider (macOS 12+).
///
/// Full implementation in future sprint (Phase 8). Stub: `is_supported()`
/// returns true on macOS 12+; mount is deferred.
#[derive(Debug, Default)]
pub struct MacosVfsProvider;

impl MacosVfsProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl VfsProvider for MacosVfsProvider {
    fn is_supported(&self) -> bool {
        // Require macOS 12 (Monterey) for NSFileProviderReplicatedExtension.
        #[cfg(target_os = "macos")]
        {
            // os_version check via libc would go here in the full implementation.
            true
        }
        #[cfg(not(target_os = "macos"))]
        false
    }

    async fn mount(
        &self,
        _mount_point: &Path,
        _pair: &SyncPair,
        _client: Arc<dyn RemoteClient>,
        _journal: Arc<dyn Journal>,
    ) -> Result<VfsMountHandle, VfsError> {
        // TODO(Phase8): register NSFileProviderDomain via objc2-file-provider
        Err(VfsError::Other(
            "FileProvider mount not yet implemented (Phase 8)".to_string(),
        ))
    }

    async fn unmount(&self, _pair_id: &PairId) -> Result<(), VfsError> {
        Ok(())
    }
    async fn update_placeholders(&self, _entries: &[VfsCacheEntry]) -> Result<(), VfsError> {
        Ok(())
    }
    async fn set_locally_available(&self, _path: &RelativePath) -> Result<(), VfsError> {
        Ok(())
    }
    async fn set_pinned(&self, _path: &RelativePath) -> Result<(), VfsError> {
        Ok(())
    }
    async fn set_cloud_only(&self, _path: &RelativePath) -> Result<(), VfsError> {
        Ok(())
    }
}
