use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

use adagio_core::journal::Journal;
use adagio_core::remote::RemoteClient;
use adagio_core::types::{PairId, RelativePath, SyncPair};

use crate::{VfsCacheEntry, VfsError, VfsMountHandle, VfsProvider};

/// No-op provider for unsupported platforms.
///
/// `is_supported()` returns false; all operations return `VfsError::NotSupported`.
#[derive(Debug, Default)]
pub struct FallbackVfsProvider;

#[async_trait]
impl VfsProvider for FallbackVfsProvider {
    fn is_supported(&self) -> bool {
        false
    }

    async fn mount(
        &self,
        _mount_point: &Path,
        _pair: &SyncPair,
        _client: Arc<dyn RemoteClient>,
        _journal: Arc<dyn Journal>,
    ) -> Result<VfsMountHandle, VfsError> {
        Err(VfsError::NotSupported)
    }

    async fn unmount(&self, _pair_id: &PairId) -> Result<(), VfsError> {
        Err(VfsError::NotSupported)
    }

    async fn update_placeholders(&self, _entries: &[VfsCacheEntry]) -> Result<(), VfsError> {
        Err(VfsError::NotSupported)
    }

    async fn set_locally_available(&self, _path: &RelativePath) -> Result<(), VfsError> {
        Err(VfsError::NotSupported)
    }

    async fn set_pinned(&self, _path: &RelativePath) -> Result<(), VfsError> {
        Err(VfsError::NotSupported)
    }

    async fn set_cloud_only(&self, _path: &RelativePath) -> Result<(), VfsError> {
        Err(VfsError::NotSupported)
    }
}
