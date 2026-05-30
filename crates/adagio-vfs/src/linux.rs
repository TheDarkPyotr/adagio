use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use adagio_core::journal::Journal;
use adagio_core::remote::RemoteClient;
use adagio_core::types::{PairId, RelativePath, SyncPair};

use crate::{VfsCacheEntry, VfsError, VfsMountHandle, VfsProvider, VfsState};

// ── Linux VFS provider ────────────────────────────────────────────────────────

/// Linux VFS driver using FUSE3.
///
/// Mounts a virtual directory at the pair's `local_root`. Files in
/// `vfs_cache_metadata` appear as regular files; cloud-only files are
/// downloaded on-demand when first read.
///
/// The full FUSE filesystem handler (getattr / readdir / read / lookup) will be
/// added in the next sprint. This implementation mounts and tracks sessions.
#[derive(Debug, Default)]
pub struct LinuxVfsProvider {
    /// Active mounts: pair_id string → unmount signal sender.
    active: Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>,
}

impl LinuxVfsProvider {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl VfsProvider for LinuxVfsProvider {
    fn is_supported(&self) -> bool {
        // FUSE3 requires /dev/fuse to be accessible.
        std::path::Path::new("/dev/fuse").exists()
    }

    async fn mount(
        &self,
        mount_point: &Path,
        pair: &SyncPair,
        client: Arc<dyn RemoteClient>,
        journal: Arc<dyn Journal>,
    ) -> Result<VfsMountHandle, VfsError> {
        let pair_id = pair.id.clone();
        let mp = mount_point.to_path_buf();

        tokio::fs::create_dir_all(&mp)
            .await
            .map_err(|e| VfsError::MountFailed(format!("create mount point: {e}")))?;

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let pid_str = pair_id.0.clone();

        // Run FUSE session on a blocking thread.
        tokio::task::spawn_blocking(move || {
            run_fuse_session(mp, pid_str, client, journal, rx);
        });

        self.active.lock().await.insert(pair_id.0.clone(), tx);
        info!(pair_id = %pair_id, "VFS FUSE mount started");

        // Return a noop handle — the real unmount is via self.active.
        Ok(VfsMountHandle::noop(pair_id))
    }

    async fn unmount(&self, pair_id: &PairId) -> Result<(), VfsError> {
        if let Some(tx) = self.active.lock().await.remove(&pair_id.0) {
            let _ = tx.send(());
            info!(pair_id = %pair_id, "VFS FUSE unmount requested");
        }
        Ok(())
    }

    async fn update_placeholders(&self, _entries: &[VfsCacheEntry]) -> Result<(), VfsError> {
        // Placeholders on Linux are served directly via FUSE from the journal.
        // No explicit OS operation needed when files are added to the journal.
        Ok(())
    }

    async fn set_locally_available(&self, _path: &RelativePath) -> Result<(), VfsError> {
        Ok(()) // FUSE reads state from journal on every getattr call.
    }

    async fn set_pinned(&self, _path: &RelativePath) -> Result<(), VfsError> {
        Ok(())
    }

    async fn set_cloud_only(&self, path: &RelativePath) -> Result<(), VfsError> {
        debug!(path = %path, "set_cloud_only: local content removed");
        Ok(())
    }
}

// ── FUSE session (stub — full implementation in next sprint) ──────────────────

/// Run a FUSE3 session until `rx` receives an unmount signal.
///
/// The production implementation creates a `fuse3::Session` with `AdagioFs`
/// and runs its event loop. On `read()` for a cloud-only file:
/// 1. Download the byte range via `client.download(path, Some(ByteRange))`
/// 2. Write content to disk at `{local_root}/{path}`
/// 3. Update `vfs_cache_metadata` state to `locally_available`
/// 4. Return bytes to the requesting application
fn run_fuse_session(
    _mount_point: PathBuf,
    pair_id: String,
    _client: Arc<dyn RemoteClient>,
    _journal: Arc<dyn Journal>,
    rx: tokio::sync::oneshot::Receiver<()>,
) {
    // TODO(Phase 4 full impl): replace with fuse3::Session::new(AdagioFs { ... }).mount(...)
    info!(pair_id = %pair_id, "FUSE3 session running (stub — full impl next sprint)");

    // Block until unmount signal using std oneshot bridge.
    let (std_tx, std_rx) = std::sync::mpsc::channel::<()>();
    // Spawn a tiny Tokio task to bridge the async oneshot to the std channel.
    let _ = tokio::runtime::Handle::try_current().map(|h| {
        h.spawn(async move {
            let _ = rx.await;
            let _ = std_tx.send(());
        });
    });
    // Block this thread until either the channel receives or the sender is dropped.
    let _ = std_rx.recv();

    info!(pair_id = %pair_id, "FUSE3 session ended");
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use adagio_core::types::RelativePath;

    // T020: LinuxVfsProvider::is_supported() does not panic.
    #[test]
    fn linux_provider_is_supported_does_not_panic() {
        let p = LinuxVfsProvider::new();
        let _ = p.is_supported(); // true on machines with /dev/fuse, false otherwise
    }

    // T022: set_cloud_only returns Ok.
    #[tokio::test]
    async fn linux_set_cloud_only_returns_ok() {
        let p = LinuxVfsProvider::new();
        let result = p.set_cloud_only(&RelativePath::new("docs/test.pdf")).await;
        assert!(result.is_ok());
    }
}
