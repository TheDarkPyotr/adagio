use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use adagio_core::journal::Journal;
use adagio_core::remote::RemoteClient;
use adagio_core::types::{PairId, RelativePath, SyncPair};

use crate::{VfsCacheEntry, VfsError, VfsMountHandle, VfsProvider};

/// In-test mock that records every call for assertion.
#[derive(Debug, Default)]
pub struct MockVfsProvider {
    pub placeholders_updated: Mutex<Vec<Vec<VfsCacheEntry>>>,
    pub locally_available_calls: Mutex<Vec<RelativePath>>,
    pub pinned_calls: Mutex<Vec<RelativePath>>,
    pub cloud_only_calls: Mutex<Vec<RelativePath>>,
    pub mount_count: Mutex<u32>,
    pub supported: bool,
}

impl MockVfsProvider {
    pub fn new() -> Self {
        Self {
            supported: true,
            ..Default::default()
        }
    }

    pub fn unsupported() -> Self {
        Self {
            supported: false,
            ..Default::default()
        }
    }
}

#[async_trait]
impl VfsProvider for MockVfsProvider {
    fn is_supported(&self) -> bool {
        self.supported
    }

    async fn mount(
        &self,
        _mount_point: &Path,
        pair: &SyncPair,
        _client: Arc<dyn RemoteClient>,
        _journal: Arc<dyn Journal>,
    ) -> Result<VfsMountHandle, VfsError> {
        *self.mount_count.lock().unwrap() += 1;
        let (tx, _rx) = tokio::sync::oneshot::channel();
        Ok(VfsMountHandle::new(pair.id.clone(), tx))
    }

    async fn unmount(&self, _pair_id: &PairId) -> Result<(), VfsError> {
        Ok(())
    }

    async fn update_placeholders(&self, entries: &[VfsCacheEntry]) -> Result<(), VfsError> {
        self.placeholders_updated
            .lock()
            .unwrap()
            .push(entries.to_vec());
        Ok(())
    }

    async fn set_locally_available(&self, path: &RelativePath) -> Result<(), VfsError> {
        self.locally_available_calls
            .lock()
            .unwrap()
            .push(path.clone());
        Ok(())
    }

    async fn set_pinned(&self, path: &RelativePath) -> Result<(), VfsError> {
        self.pinned_calls.lock().unwrap().push(path.clone());
        Ok(())
    }

    async fn set_cloud_only(&self, path: &RelativePath) -> Result<(), VfsError> {
        self.cloud_only_calls.lock().unwrap().push(path.clone());
        Ok(())
    }
}
