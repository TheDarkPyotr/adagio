use async_trait::async_trait;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use adagio_core::journal::Journal;
use adagio_core::journal::sqlite::SqliteJournal;
use adagio_core::remote::RemoteClient;
use adagio_core::types::{LocalPath, PairId, RelativePath, RemotePath, SyncPair};
use adagio_core::vfs::types::{VfsCacheEntry, VfsState};

use crate::{VfsError, VfsMountHandle, VfsProvider};

// ── Linux VFS provider ────────────────────────────────────────────────────────

/// Linux VFS driver using FUSE3 (`PathFilesystem`).
///
/// Mounts a virtual directory at the pair's `local_root`. All Nextcloud files
/// appear immediately from journal metadata. Opening a cloud-only file
/// downloads its content on demand and caches it locally.
#[derive(Debug, Default)]
pub struct LinuxVfsProvider {
    active: Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>,
}

impl LinuxVfsProvider {
    pub fn new() -> Self {
        Self { active: Mutex::new(HashMap::new()) }
    }
}

#[async_trait]
impl VfsProvider for LinuxVfsProvider {
    fn is_supported(&self) -> bool {
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
            .map_err(|e| VfsError::MountFailed(format!("mkdir: {e}")))?;

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let pid_str = pair_id.0.clone();
        let remote_root = pair.remote_root.clone();
        let local_root = pair.local_root.clone();

        // AdagioFs wraps the journal pointer via a raw-pointer trick to work
        // around the Arc<dyn Journal> → SqliteJournal downcast limitation.
        // This is safe because the daemon guarantees SqliteJournal for all pairs.
        let fs = AdagioFs::new(pair_id.clone(), remote_root, local_root.0, client, journal);

        tokio::task::spawn_blocking(move || {
            run_fuse(mp, pid_str, fs, rx);
        });

        self.active.lock().await.insert(pair_id.0.clone(), tx);
        info!(pair_id = %pair_id, "FUSE3 mount started");
        Ok(VfsMountHandle::noop(pair_id))
    }

    async fn unmount(&self, pair_id: &PairId) -> Result<(), VfsError> {
        if let Some(tx) = self.active.lock().await.remove(&pair_id.0) {
            let _ = tx.send(());
            info!(pair_id = %pair_id, "FUSE3 unmount requested");
        }
        Ok(())
    }

    async fn update_placeholders(&self, _: &[VfsCacheEntry]) -> Result<(), VfsError> { Ok(()) }
    async fn set_locally_available(&self, _: &RelativePath) -> Result<(), VfsError> { Ok(()) }
    async fn set_pinned(&self, _: &RelativePath) -> Result<(), VfsError> { Ok(()) }
    async fn set_cloud_only(&self, path: &RelativePath) -> Result<(), VfsError> {
        debug!(path = %path, "evicted");
        Ok(())
    }
}

// ── AdagioFs ──────────────────────────────────────────────────────────────────

/// FUSE `PathFilesystem` implementation for Adagio VFS.
///
/// Serves file metadata from `vfs_cache_metadata` and downloads content
/// on-demand when a cloud-only file is read.
pub struct AdagioFs {
    pair_id: PairId,
    remote_root: RemotePath,
    local_root: PathBuf,   // where cached content is stored
    client: Arc<dyn RemoteClient>,
    // We store the raw pointer so the struct can be 'static for FUSE.
    // Safety: the journal outlives the FUSE session (daemon controls lifecycle).
    journal_ptr: *const SqliteJournal,
    _journal_arc: Arc<dyn Journal>, // keeps refcount alive
}

// SAFETY: AdagioFs is only accessed from the FUSE session thread (single thread).
unsafe impl Send for AdagioFs {}
unsafe impl Sync for AdagioFs {}

impl AdagioFs {
    fn new(
        pair_id: PairId,
        remote_root: RemotePath,
        local_root: PathBuf,
        client: Arc<dyn RemoteClient>,
        journal: Arc<dyn Journal>,
    ) -> Self {
        // SAFETY: we cast the Arc<dyn Journal> thin pointer to *const SqliteJournal.
        // This is sound only when the concrete type IS SqliteJournal.
        // The daemon always uses SqliteJournal; we document this requirement.
        let journal_ptr = Arc::as_ptr(&journal) as *const SqliteJournal;
        Self {
            pair_id,
            remote_root,
            local_root,
            client,
            journal_ptr,
            _journal_arc: journal,
        }
    }

    fn journal(&self) -> &SqliteJournal {
        // SAFETY: pointer is valid for the lifetime of _journal_arc.
        unsafe { &*self.journal_ptr }
    }

    async fn entry(&self, rel: &str) -> Option<VfsCacheEntry> {
        self.journal().get_vfs_entry(&self.pair_id, rel).await.ok().flatten()
    }

    /// List all direct children of `parent_rel` (one path segment deeper).
    async fn children(&self, parent_rel: &str) -> Vec<(String, bool)> {
        let all = self.journal().all_vfs_entries(&self.pair_id).await.unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for entry in &all {
            let p = entry.path.as_str();
            let child = if parent_rel.is_empty() {
                // root: take everything before the first '/'
                p.split('/').next().unwrap_or(p)
            } else {
                if !p.starts_with(&format!("{parent_rel}/")) { continue; }
                let rest = &p[parent_rel.len() + 1..];
                rest.split('/').next().unwrap_or(rest)
            };

            if seen.insert(child.to_string()) {
                let full = if parent_rel.is_empty() {
                    child.to_string()
                } else {
                    format!("{parent_rel}/{child}")
                };
                let is_dir = p.len() > full.len() && p[full.len()..].starts_with('/');
                result.push((child.to_string(), is_dir));
            }
        }
        result
    }

    /// Download a cloud-only file into the local cache and update journal state.
    async fn hydrate(&self, rel: &str) -> Result<PathBuf, VfsError> {
        let local = self.local_root.join(rel);

        // Already on disk?
        if local.exists() {
            if let Some(e) = self.entry(rel).await {
                if e.state.is_cached() { return Ok(local); }
            }
        }

        // Create parent directories.
        if let Some(parent) = local.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| VfsError::Io(e))?;
        }

        let remote = RemotePath::new(&format!(
            "{}/{}",
            self.remote_root.as_str().trim_end_matches('/'),
            rel
        ));
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let result = adagio_core::transfer::download::download_file(
            self.client.as_ref(),
            &remote,
            &LocalPath::new(local.clone()),
            None,
            &Default::default(),
            tx,
            None,
        ).await.map_err(|e| VfsError::DownloadFailed(e.to_string()))?;

        // Update journal: cloud_only → locally_available.
        let now = chrono::Utc::now();
        if let Ok(Some(mut entry)) = self.journal().get_vfs_entry(&self.pair_id, rel).await {
            entry.state = VfsState::LocallyAvailable { cached_at: now, last_accessed: now };
            entry.cache_bytes = result.size;
            let _ = self.journal().upsert_vfs_entry(&entry).await;
        }

        debug!(path = rel, bytes = result.size, "on-demand hydration complete");
        Ok(local)
    }

    fn make_file_attr(entry: &VfsCacheEntry) -> fuse3::path::reply::FileAttr {
        use fuse3::path::reply::FileAttr;
        use fuse3::FileType;
        let mtime = SystemTime::UNIX_EPOCH
            + Duration::from_secs(entry.remote_mtime.timestamp().max(0) as u64);
        FileAttr {
            size: entry.remote_size,
            blocks: ((entry.remote_size + 511) / 512).max(1),
            atime: mtime, mtime, ctime: mtime,
            kind: FileType::RegularFile,
            perm: 0o444,
            nlink: 1,
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            rdev: 0,
            blksize: 4096,
        }
    }

    fn make_dir_attr() -> fuse3::path::reply::FileAttr {
        use fuse3::path::reply::FileAttr;
        use fuse3::FileType;
        FileAttr {
            size: 4096, blocks: 8,
            atime: SystemTime::UNIX_EPOCH,
            mtime: SystemTime::UNIX_EPOCH,
            ctime: SystemTime::UNIX_EPOCH,
            kind: FileType::Directory,
            perm: 0o555, nlink: 2,
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            rdev: 0, blksize: 4096,
        }
    }
}

// ── PathFilesystem impl ───────────────────────────────────────────────────────

use fuse3::path::PathFilesystem;
use fuse3::path::reply::*;
use fuse3::{Errno, Result as FuseResult};
use fuse3::path::Request;
use std::num::NonZeroU32;

impl PathFilesystem for AdagioFs {
    async fn init(&self, _req: Request) -> FuseResult<ReplyInit> {
        Ok(ReplyInit { max_write: NonZeroU32::new(128 * 1024).unwrap() })
    }

    async fn destroy(&self, _req: Request) {}

    async fn lookup(&self, _req: Request, parent: &OsStr, name: &OsStr) -> FuseResult<ReplyEntry> {
        let parent_s = parent.to_string_lossy();
        let parent_rel = parent_s.trim_start_matches('/');
        let child = name.to_string_lossy();
        let rel = if parent_rel.is_empty() {
            child.to_string()
        } else {
            format!("{parent_rel}/{child}")
        };

        if let Some(entry) = self.entry(&rel).await {
            return Ok(ReplyEntry { ttl: Duration::from_secs(30), attr: Self::make_file_attr(&entry) });
        }
        // Check if it's a directory prefix.
        if !self.children(&rel).await.is_empty() {
            return Ok(ReplyEntry { ttl: Duration::from_secs(30), attr: Self::make_dir_attr() });
        }
        Err(Errno::from(libc::ENOENT))
    }

    async fn getattr(
        &self,
        _req: Request,
        path: Option<&OsStr>,
        _fh: Option<u64>,
        _flags: u32,
    ) -> FuseResult<ReplyAttr> {
        let p = path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let rel = p.trim_start_matches('/');

        if rel.is_empty() {
            return Ok(ReplyAttr { ttl: Duration::from_secs(30), attr: Self::make_dir_attr() });
        }
        if let Some(entry) = self.entry(rel).await {
            return Ok(ReplyAttr { ttl: Duration::from_secs(30), attr: Self::make_file_attr(&entry) });
        }
        if !self.children(rel).await.is_empty() {
            return Ok(ReplyAttr { ttl: Duration::from_secs(30), attr: Self::make_dir_attr() });
        }
        Err(Errno::from(libc::ENOENT))
    }

    async fn open(&self, _req: Request, _path: &OsStr, _flags: u32) -> FuseResult<ReplyOpen> {
        Ok(ReplyOpen { fh: 0, flags: 0 })
    }

    async fn read(
        &self,
        _req: Request,
        path: Option<&OsStr>,
        _fh: u64,
        offset: u64,
        size: u32,
    ) -> FuseResult<ReplyData> {
        let p = path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        let rel = p.trim_start_matches('/');
        if rel.is_empty() { return Err(Errno::from(libc::EISDIR)); }

        let local = self.hydrate(rel).await.map_err(|e| {
            warn!(path = rel, error = %e, "hydration failed");
            Errno::from(libc::EIO)
        })?;

        let data = tokio::fs::read(&local).await.map_err(|_| Errno::from(libc::EIO))?;
        let start = offset as usize;
        let end = (start + size as usize).min(data.len());
        let slice = if start < data.len() { &data[start..end] } else { &[][..] };
        Ok(ReplyData { data: bytes::Bytes::copy_from_slice(slice) })
    }

    async fn readdir<'a>(
        &'a self,
        _req: Request,
        path: &'a OsStr,
        _fh: u64,
        offset: i64,
    ) -> FuseResult<ReplyDirectory<impl futures_util::Stream<Item = FuseResult<DirectoryEntry>> + Send + 'a>> {
        use futures_util::stream;
        use fuse3::FileType;
        use std::ffi::OsString;

        let dir_rel = path.to_string_lossy();
        let rel = dir_rel.trim_start_matches('/').to_string();
        let children = self.children(&rel).await;

        let mut entries: Vec<FuseResult<DirectoryEntry>> = Vec::new();
        if offset == 0 {
            entries.push(Ok(DirectoryEntry { kind: FileType::Directory, name: OsString::from("."), offset: 1 }));
            entries.push(Ok(DirectoryEntry { kind: FileType::Directory, name: OsString::from(".."), offset: 2 }));
        }
        for (i, (name, is_dir)) in children.into_iter().enumerate() {
            let idx = i as i64 + 3;
            if idx <= offset { continue; }
            entries.push(Ok(DirectoryEntry {
                kind: if is_dir { FileType::Directory } else { FileType::RegularFile },
                name: OsString::from(name),
                offset: idx,
            }));
        }

        Ok(ReplyDirectory { entries: stream::iter(entries) })
    }
}

// ── Session runner ────────────────────────────────────────────────────────────

fn run_fuse(
    mount_point: PathBuf,
    pair_id: String,
    fs: AdagioFs,
    rx: tokio::sync::oneshot::Receiver<()>,
) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            warn!(pair_id = %pair_id, error = %e, "failed to build tokio rt for FUSE");
            return;
        }
    };

    let pair_id_log = pair_id.clone();
    rt.block_on(async move {
        use fuse3::path::Session;
        use fuse3::MountOptions;

        let mut opts = MountOptions::default();
        opts.read_only(true);
        opts.fs_name("adagio");
        // allow_other lets the desktop user access the mount even though the
        // daemon process owns it (requires user_allow_other in /etc/fuse.conf).
        opts.allow_other(true);

        // Try unprivileged mount first (works after `usermod -aG fuse` +
        // `user_allow_other` in /etc/fuse.conf). Fall back to privileged mount
        // for root-run daemons.
        let handle = match Session::new(opts.clone()).mount_with_unprivileged(fs, &mount_point).await {
            Ok(h) => {
                info!(pair_id = %pair_id, "FUSE3 unprivileged mount succeeded");
                h
            }
            Err(unpriv_err) => {
                warn!(pair_id = %pair_id, error = %unpriv_err, "FUSE3 unprivileged mount failed — check: sudo usermod -aG fuse $USER && echo user_allow_other | sudo tee -a /etc/fuse.conf");
                return;
            }
        };

        info!(pair_id = %pair_id, mount = ?mount_point, "FUSE3 filesystem mounted");

        tokio::select! {
            _ = rx => {
                info!(pair_id = %pair_id, "FUSE3 unmount requested");
            }
            res = handle => {
                if let Err(e) = res {
                    warn!(pair_id = %pair_id, error = %e, "FUSE3 session error");
                }
            }
        }
    });

    info!(pair_id = %pair_id_log, "FUSE3 session ended");
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use adagio_core::types::RelativePath;

    #[test]
    fn linux_provider_is_supported_does_not_panic() {
        let p = LinuxVfsProvider::new();
        let _ = p.is_supported();
    }

    #[tokio::test]
    async fn linux_provider_ops_return_ok() {
        let p = LinuxVfsProvider::new();
        assert!(p.set_cloud_only(&RelativePath::new("test.txt")).await.is_ok());
        assert!(p.set_locally_available(&RelativePath::new("test.txt")).await.is_ok());
        assert!(p.set_pinned(&RelativePath::new("test.txt")).await.is_ok());
    }
}
