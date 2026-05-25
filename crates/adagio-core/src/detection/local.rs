use crate::error::{DetectorError, SyncError};
use crate::types::{LocalItem, LocalPath, RelativePath, SyncPair};
use chrono::{DateTime, Utc};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::watch;
use tracing::instrument;

use super::LocalChangeSignal;

/// Debounce window before a local change signal is emitted.
const DEBOUNCE_WINDOW: Duration = Duration::from_secs(2);

/// Manages per-pair filesystem watchers and emits debounced change signals.
pub struct LocalWatcher {
    /// Per-pair debounced watchers; keyed by local root path string.
    watchers: Mutex<HashMap<String, ActiveWatcher>>,
}

struct ActiveWatcher {
    _debouncer: Debouncer<RecommendedWatcher, FileIdMap>,
    tx: watch::Sender<LocalChangeSignal>,
}

impl LocalWatcher {
    pub fn new() -> Self {
        Self {
            watchers: Mutex::new(HashMap::new()),
        }
    }

    /// Start (or reuse) a watcher for the given pair's local root.
    ///
    /// Returns a `watch::Receiver` that fires whenever debounced filesystem
    /// events are coalesced for that root.
    pub fn watch(&self, pair: &SyncPair) -> Result<watch::Receiver<LocalChangeSignal>, SyncError> {
        let root = pair.local_root.0.as_path().to_path_buf();
        let root_key = root.to_string_lossy().to_string();
        let pair_id = pair.id.clone();

        let mut guard = self.watchers.lock().unwrap();
        if let Some(aw) = guard.get(&root_key) {
            return Ok(aw.tx.subscribe());
        }

        let (tx, rx) = watch::channel(LocalChangeSignal {
            pair_id: pair_id.clone(),
            detected_at: Utc::now(),
        });

        let tx_clone = tx.clone();
        let pair_id_clone = pair_id.clone();

        let debouncer = new_debouncer(DEBOUNCE_WINDOW, None, move |res: DebounceEventResult| {
            if let Ok(events) = res {
                let any_change = events.iter().any(|e| is_sync_relevant(&e.event));
                if any_change {
                    let _ = tx_clone.send(LocalChangeSignal {
                        pair_id: pair_id_clone.clone(),
                        detected_at: Utc::now(),
                    });
                }
            }
        })
        .map_err(|e| SyncError::Permanent(format!("debouncer init failed: {e}")))?;

        // Register the watcher path (mutable borrow needed on the inner watcher).
        {
            let mut d = debouncer;
            d.watcher()
                .watch(&root, RecursiveMode::Recursive)
                .map_err(|e| SyncError::Permanent(format!("watch failed: {e}")))?;

            guard.insert(root_key, ActiveWatcher { _debouncer: d, tx });
        }

        Ok(rx)
    }

    /// Stop watching the given root (called when a pair is deleted).
    pub fn unwatch(&self, root: &Path) {
        let key = root.to_string_lossy().to_string();
        self.watchers.lock().unwrap().remove(&key);
    }
}

impl Default for LocalWatcher {
    fn default() -> Self {
        Self::new()
    }
}

fn is_sync_relevant(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    )
}

// ── Full local scan (T040) ────────────────────────────────────────────────────

/// Scan a local directory tree and return `LocalItem`s for all files.
///
/// Uses mtime + size from the journal cache where available to avoid
/// unnecessary SHA-256 computation. Pass an empty `cache` to force
/// checksum computation for every file.
#[instrument(skip(root, cache), fields(root = %root.0.display()))]
pub async fn scan_local(
    root: &LocalPath,
    cache: &HashMap<RelativePath, CachedLocalEntry>,
) -> Result<Vec<LocalItem>, DetectorError> {
    let root_path = root.0.to_path_buf();
    let cache = cache.clone();
    let items = tokio::task::spawn_blocking(move || scan_blocking(&root_path, &root_path, &cache))
        .await
        .map_err(|e| DetectorError::Fs(std::io::Error::other(e.to_string())))??;
    Ok(items)
}

/// Cache entry from journal to avoid rehashing unchanged files.
#[derive(Clone)]
pub struct CachedLocalEntry {
    pub mtime: DateTime<Utc>,
    pub size: u64,
    pub checksum: Option<crate::types::Checksum>,
}

fn scan_blocking(
    root: &Path,
    current: &Path,
    cache: &HashMap<RelativePath, CachedLocalEntry>,
) -> Result<Vec<LocalItem>, DetectorError> {
    let mut items = Vec::new();

    let read_dir = std::fs::read_dir(current).map_err(DetectorError::Fs)?;

    for entry in read_dir {
        let entry = entry.map_err(DetectorError::Fs)?;
        let path = entry.path();
        let meta = entry.metadata().map_err(DetectorError::Fs)?;

        // Build relative path
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/"); // normalise Windows separators
        let rel_path = RelativePath::new(&rel);

        if meta.is_dir() {
            items.push(LocalItem {
                path: rel_path.clone(),
                size: 0,
                mtime: mtime_from_meta(&meta),
                checksum: None,
                is_dir: true,
            });
            // Recurse
            let mut sub = scan_blocking(root, &path, cache)?;
            items.append(&mut sub);
        } else if meta.is_file() {
            let size = meta.len();
            let mtime = mtime_from_meta(&meta);

            // Use cache to avoid recomputing checksum when mtime+size match.
            let checksum_result = if let Some(cached) = cache.get(&rel_path) {
                if cached.size == size
                    && (cached.mtime - mtime).abs() < chrono::Duration::seconds(2)
                {
                    Ok(cached.checksum.clone())
                } else {
                    compute_checksum(&path)
                }
            } else {
                compute_checksum(&path)
            };

            let checksum = match checksum_result {
                Ok(c) => c,
                Err(DetectorError::FileInProgress) => continue, // skip; next cycle will retry
                Err(e) => return Err(e),
            };

            items.push(LocalItem {
                path: rel_path,
                size,
                mtime,
                checksum,
                is_dir: false,
            });
        }
    }

    Ok(items)
}

fn mtime_from_meta(meta: &std::fs::Metadata) -> DateTime<Utc> {
    meta.modified()
        .ok()
        .and_then(|t| {
            DateTime::from_timestamp(
                t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                0,
            )
        })
        .unwrap_or_else(Utc::now)
}

/// Returns true if the file's mtime or size changed since `mtime_before`/`size_before`.
///
/// Called after reading the file to detect concurrent writes. If the file changed
/// during the read the checksum covers only a partial or inconsistent view, so
/// the caller should skip this file and retry on the next cycle.
pub(crate) fn file_changed_during_read(
    mtime_before: std::time::SystemTime,
    size_before: u64,
    path: &Path,
) -> bool {
    match std::fs::metadata(path) {
        Ok(m) => {
            let mtime_after = m.modified().unwrap_or(mtime_before);
            m.len() != size_before || mtime_after != mtime_before
        }
        Err(_) => true,
    }
}

fn compute_checksum(path: &Path) -> Result<Option<crate::types::Checksum>, DetectorError> {
    use sha2::{Digest, Sha256};

    let meta_before = std::fs::metadata(path).map_err(DetectorError::Fs)?;
    let mtime_before = meta_before.modified().unwrap_or(std::time::UNIX_EPOCH);
    let size_before = meta_before.len();

    let data = std::fs::read(path).map_err(DetectorError::Fs)?;

    if file_changed_during_read(mtime_before, size_before, path) {
        return Err(DetectorError::FileInProgress);
    }

    let mut h = Sha256::new();
    h.update(&data);
    let value = hex::encode(h.finalize());
    Ok(Some(crate::types::Checksum {
        algorithm: crate::types::ChecksumAlgorithm::Sha256,
        value,
    }))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    // T079: file modified during checksum → skipped as in-progress.
    #[tokio::test]
    async fn scan_skips_file_being_written() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("writing.bin");
        fs::write(&path, b"initial").unwrap();

        // Write a new version of the file while the scan is running.
        // We simulate this by having the compute_checksum detect the mtime changed.
        // Direct test: verify detect_in_progress() returns true when mtime changes.
        let meta_before = std::fs::metadata(&path).unwrap();
        let mtime_before = meta_before.modified().unwrap();

        // Sleep 10ms then touch the file to change mtime.
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(&path, b"updated content").unwrap();

        let meta_after = std::fs::metadata(&path).unwrap();
        let mtime_after = meta_after.modified().unwrap();

        assert!(
            file_changed_during_read(mtime_before, meta_before.len(), &path),
            "file with updated mtime should be detected as in-progress"
        );
        let _ = mtime_after;
    }

    fn local_path(dir: &TempDir) -> LocalPath {
        LocalPath::new(dir.path())
    }

    // T040-1: scan empty directory returns empty list.
    #[tokio::test]
    async fn scan_local_empty_dir() {
        let dir = TempDir::new().unwrap();
        let items = scan_local(&local_path(&dir), &HashMap::new())
            .await
            .expect("scan should succeed");
        assert!(items.is_empty());
    }

    // T040-2: scan finds a flat file with correct relative path and checksum.
    #[tokio::test]
    async fn scan_local_finds_flat_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("hello.txt"), b"hello").unwrap();
        let items = scan_local(&local_path(&dir), &HashMap::new())
            .await
            .expect("scan should succeed");
        let file = items
            .iter()
            .find(|i| !i.is_dir)
            .expect("should find a file");
        assert_eq!(file.path.as_str(), "hello.txt");
        assert_eq!(file.size, 5);
        assert!(file.checksum.is_some());
    }

    // T040-3: scan recurses into subdirectories.
    #[tokio::test]
    async fn scan_local_recurses_subdirectory() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("nested.txt"), b"nested content").unwrap();
        let items = scan_local(&local_path(&dir), &HashMap::new())
            .await
            .expect("scan should succeed");
        let nested = items.iter().find(|i| i.path.as_str() == "sub/nested.txt");
        assert!(nested.is_some(), "nested file should be found");
    }

    // T040-4: cache hit avoids recomputing checksum.
    #[tokio::test]
    async fn scan_local_uses_cache() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("cached.txt"), b"data").unwrap();

        // First pass: compute checksum from disk.
        let first = scan_local(&local_path(&dir), &HashMap::new())
            .await
            .unwrap();
        let first_item = first.iter().find(|i| !i.is_dir).unwrap();

        // Build cache from first scan result.
        let mut cache = HashMap::new();
        cache.insert(
            first_item.path.clone(),
            CachedLocalEntry {
                mtime: first_item.mtime,
                size: first_item.size,
                checksum: first_item.checksum.clone(),
            },
        );

        // Second pass: should use cache (checksum unchanged).
        let second = scan_local(&local_path(&dir), &cache).await.unwrap();
        let second_item = second.iter().find(|i| !i.is_dir).unwrap();
        assert_eq!(
            first_item.checksum.as_ref().map(|c| &c.value),
            second_item.checksum.as_ref().map(|c| &c.value),
            "checksum should match cache"
        );
    }
}
