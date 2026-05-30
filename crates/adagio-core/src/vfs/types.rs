use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::Path;
use std::sync::Arc;

use crate::journal::Journal;
use crate::remote::RemoteClient;
use crate::types::{PairId, RelativePath, SyncPair};

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum VfsError {
    #[error("VFS is not supported on this platform or OS version")]
    NotSupported,
    #[error("mount failed: {0}")]
    MountFailed(String),
    #[error("path is pinned — unpin before evicting")]
    PathIsPinned,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("download failed: {0}")]
    DownloadFailed(String),
    #[error("{0}")]
    Other(String),
}

// ── File state ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VfsState {
    CloudOnly,
    LocallyAvailable {
        cached_at: DateTime<Utc>,
        last_accessed: DateTime<Utc>,
    },
    Pinned {
        cached_at: DateTime<Utc>,
        last_accessed: DateTime<Utc>,
    },
}

impl VfsState {
    pub fn is_cached(&self) -> bool {
        !matches!(self, VfsState::CloudOnly)
    }
    pub fn is_pinned(&self) -> bool {
        matches!(self, VfsState::Pinned { .. })
    }
    pub fn is_auto_evictable(&self) -> bool {
        matches!(self, VfsState::LocallyAvailable { .. })
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            VfsState::CloudOnly => "cloud_only",
            VfsState::LocallyAvailable { .. } => "locally_available",
            VfsState::Pinned { .. } => "pinned",
        }
    }
}

// ── Cache entry ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct VfsCacheEntry {
    pub pair_id: PairId,
    pub path: RelativePath,
    pub remote_size: u64,
    pub remote_etag: Option<String>,
    pub remote_mtime: DateTime<Utc>,
    pub state: VfsState,
    pub cache_bytes: u64,
}

impl VfsCacheEntry {
    pub fn new_cloud_only(
        pair_id: PairId,
        path: RelativePath,
        remote_size: u64,
        remote_etag: Option<String>,
        remote_mtime: DateTime<Utc>,
    ) -> Self {
        Self {
            pair_id,
            path,
            remote_size,
            remote_etag,
            remote_mtime,
            state: VfsState::CloudOnly,
            cache_bytes: 0,
        }
    }
}

// ── Stats ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct VfsStats {
    pub pair_id: String,
    pub cloud_only_count: u64,
    pub locally_available_count: u64,
    pub pinned_count: u64,
    pub cached_bytes: u64,
    pub cache_max_bytes: u64,
    pub last_eviction_at: Option<DateTime<Utc>>,
}

// ── Mount handle ──────────────────────────────────────────────────────────────

pub struct VfsMountHandle {
    pub pair_id: PairId,
    pub(crate) _unmount: Option<tokio::sync::oneshot::Sender<()>>,
}

impl VfsMountHandle {
    pub fn new(pair_id: PairId, tx: tokio::sync::oneshot::Sender<()>) -> Self {
        Self {
            pair_id,
            _unmount: Some(tx),
        }
    }
    pub fn noop(pair_id: PairId) -> Self {
        Self {
            pair_id,
            _unmount: None,
        }
    }
}

// ── Provider trait ────────────────────────────────────────────────────────────

/// Platform-specific VFS driver (FUSE3 / FileProvider / CfApi).
#[async_trait]
pub trait VfsProvider: Send + Sync + 'static {
    fn is_supported(&self) -> bool;
    async fn mount(
        &self,
        mount_point: &Path,
        pair: &SyncPair,
        client: Arc<dyn RemoteClient>,
        journal: Arc<dyn Journal>,
    ) -> Result<VfsMountHandle, VfsError>;
    async fn unmount(&self, pair_id: &PairId) -> Result<(), VfsError>;
    async fn update_placeholders(&self, entries: &[VfsCacheEntry]) -> Result<(), VfsError>;
    async fn set_locally_available(&self, path: &RelativePath) -> Result<(), VfsError>;
    async fn set_pinned(&self, path: &RelativePath) -> Result<(), VfsError>;
    async fn set_cloud_only(&self, path: &RelativePath) -> Result<(), VfsError>;
}
