use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;

use adagio_core::journal::Journal;
use adagio_core::remote::RemoteClient;
use adagio_core::types::{PairId, RelativePath, SyncPair};

use crate::{VfsCacheEntry, VfsError, VfsMountHandle, VfsProvider};

/// Linux VFS driver using FUSE3.
///
/// Full implementation in future sprint (Phase 4 — US2). This stub is
/// `is_supported()` = true only when `/dev/fuse` is present; all mount/read
/// operations are deferred to the FUSE3 implementation.
#[derive(Debug, Default)]
pub struct LinuxVfsProvider;

impl LinuxVfsProvider {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl VfsProvider for LinuxVfsProvider {
    fn is_supported(&self) -> bool {
        std::path::Path::new("/dev/fuse").exists()
    }

    async fn mount(
        &self,
        _mount_point: &Path,
        _pair: &SyncPair,
        _client: Arc<dyn RemoteClient>,
        _journal: Arc<dyn Journal>,
    ) -> Result<VfsMountHandle, VfsError> {
        // TODO(US2): spawn fuse3 session task
        Err(VfsError::Other(
            "FUSE3 mount not yet implemented (Phase 4)".to_string(),
        ))
    }

    async fn unmount(&self, _pair_id: &PairId) -> Result<(), VfsError> {
        Ok(())
    }

    async fn update_placeholders(&self, _entries: &[VfsCacheEntry]) -> Result<(), VfsError> {
        // Placeholders on Linux are represented as regular files with zero size.
        // The FUSE handler serves the correct metadata from the journal.
        // TODO(US2): signal running FUSE session
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
