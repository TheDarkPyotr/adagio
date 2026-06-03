use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

use adagio_core::journal::Journal;
use adagio_core::remote::RemoteClient;
use adagio_core::types::{PairId, RelativePath, SyncPair};

use crate::{VfsCacheEntry, VfsError, VfsMountHandle, VfsProvider};

/// Windows VFS driver using Cloud Files API (CfApi, Windows 10 1709+).
///
/// Full implementation in future sprint (Phase 8). Stub: `is_supported()` = true
/// on Windows; mount is deferred.
#[derive(Debug, Default)]
pub struct WindowsVfsProvider;

impl WindowsVfsProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl VfsProvider for WindowsVfsProvider {
    fn is_supported(&self) -> bool {
        cfg!(windows)
    }

    async fn mount(
        &self,
        _mount_point: &Path,
        _pair: &SyncPair,
        _client: Arc<dyn RemoteClient>,
        _journal: Arc<dyn Journal>,
    ) -> Result<VfsMountHandle, VfsError> {
        // TODO(Phase8): CfRegisterSyncRoot + wincs callback loop
        Err(VfsError::Other(
            "CfApi mount not yet implemented (Phase 8)".to_string(),
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
