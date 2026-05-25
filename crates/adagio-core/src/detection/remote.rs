use crate::error::DetectorError;
use crate::remote::RemoteClient;
use crate::types::{JournalEntry, RelativePath, RemoteItem, RemotePath};
use std::collections::HashMap;
use std::time::Duration;
use tracing::instrument;

/// A rename detected by comparing journal file_ids against the current remote snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedRename {
    pub from: RelativePath,
    pub to: RelativePath,
    pub file_id: String,
}

/// Compare journal entries against the current remote snapshot to find items
/// that have been moved or renamed server-side (same file_id, different path).
///
/// Returns one `DetectedRename` for every journal entry whose file_id is found
/// in the remote snapshot at a *different* path.
pub fn detect_renames(journal: &[JournalEntry], remote: &[RemoteItem]) -> Vec<DetectedRename> {
    use std::collections::HashMap;

    // Build remote index: file_id → RemoteItem
    let remote_by_file_id: HashMap<&str, &RemoteItem> =
        remote.iter().map(|r| (r.file_id.as_str(), r)).collect();

    let mut renames = Vec::new();

    for entry in journal {
        if let Some(fid) = &entry.file_id {
            if let Some(r) = remote_by_file_id.get(fid.as_str()) {
                if r.path != entry.path {
                    renames.push(DetectedRename {
                        from: entry.path.clone(),
                        to: r.path.clone(),
                        file_id: fid.clone(),
                    });
                }
            }
        }
    }

    renames
}

// ── Remote PROPFIND poller (T041) ─────────────────────────────────────────────

/// Polling intervals: 30 s when the engine is active, 5 min when idle.
pub const ACTIVE_POLL_INTERVAL: Duration = Duration::from_secs(30);
pub const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(300);

/// Fetch a full remote snapshot for the given root path.
///
/// Wraps `client.list_recursive` and surface client errors as `DetectorError`.
///
/// Directory entries (`is_dir = true`) are stripped from the result — parent
/// directories are created implicitly by the download code and do not need
/// their own reconcile ops.
#[instrument(skip(client), fields(remote_root = %remote_root))]
pub async fn fetch_remote_snapshot(
    client: &dyn RemoteClient,
    remote_root: &RemotePath,
) -> Result<Vec<RemoteItem>, DetectorError> {
    let items = client
        .list_recursive(remote_root)
        .await
        .map_err(|e| DetectorError::Remote(e.to_string()))?;
    Ok(items.into_iter().filter(|i| !i.is_dir).collect())
}

/// Compare two remote snapshots and return items whose etag has changed.
///
/// Items only in `current` (not in `previous`) are included (new files).
/// Items only in `previous` (not in `current`) are returned as deleted.
pub fn diff_remote_snapshots(previous: &[RemoteItem], current: &[RemoteItem]) -> RemoteDiff {
    let prev_by_path: HashMap<&str, &RemoteItem> =
        previous.iter().map(|i| (i.path.as_str(), i)).collect();
    let curr_by_path: HashMap<&str, &RemoteItem> =
        current.iter().map(|i| (i.path.as_str(), i)).collect();

    let mut changed = Vec::new();
    let mut added = Vec::new();
    let mut deleted = Vec::new();

    for curr in current {
        match prev_by_path.get(curr.path.as_str()) {
            Some(prev) if prev.etag != curr.etag => changed.push(curr.clone()),
            None => added.push(curr.clone()),
            _ => {}
        }
    }

    for prev in previous {
        if !curr_by_path.contains_key(prev.path.as_str()) {
            deleted.push(prev.clone());
        }
    }

    RemoteDiff {
        changed,
        added,
        deleted,
    }
}

/// Summary of changes between two consecutive remote snapshots.
#[derive(Debug, Default)]
pub struct RemoteDiff {
    /// Items present in both snapshots but with a different etag.
    pub changed: Vec<RemoteItem>,
    /// Items present only in the current snapshot (new on the server).
    pub added: Vec<RemoteItem>,
    /// Items present only in the previous snapshot (deleted on the server).
    pub deleted: Vec<RemoteItem>,
}

impl RemoteDiff {
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty() && self.deleted.is_empty()
    }
}

// ── Tests (T037) ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Checksum, ChecksumAlgorithm, JournalEntry, PairId, RelativePath, RemoteItem, SyncStatus,
    };
    use chrono::Utc;

    fn path(s: &str) -> RelativePath {
        RelativePath::new(s)
    }

    fn remote_item(p: &str, file_id: &str) -> RemoteItem {
        RemoteItem {
            path: path(p),
            file_id: file_id.to_string(),
            etag: "etag1".to_string(),
            size: 100,
            mtime: Utc::now(),
            checksum: Some(Checksum {
                algorithm: ChecksumAlgorithm::Sha256,
                value: "abc".to_string(),
            }),
            is_dir: false,
        }
    }

    fn journal_entry(p: &str, file_id: &str) -> JournalEntry {
        JournalEntry {
            pair_id: PairId::new(),
            path: path(p),
            file_id: Some(file_id.to_string()),
            etag: Some("etag1".to_string()),
            checksum: Some(Checksum {
                algorithm: ChecksumAlgorithm::Sha256,
                value: "abc".to_string(),
            }),
            size: 100,
            mtime_local: Some(Utc::now()),
            mtime_remote: Some(Utc::now()),
            status: SyncStatus::Synced,
            error_message: None,
            retry_count: 0,
            updated_at: Utc::now(),
        }
    }

    // 1. No renames when paths match.
    #[test]
    fn detect_renames_none_when_paths_match() {
        let j = journal_entry("doc.txt", "fid1");
        let r = remote_item("doc.txt", "fid1");
        let renames = detect_renames(&[j], &[r]);
        assert!(
            renames.is_empty(),
            "no renames when file_id is at same path"
        );
    }

    // 2. Detect rename when file_id moved to a new path.
    #[test]
    fn detect_renames_finds_moved_file_id() {
        let j = journal_entry("old.txt", "fid2");
        let r = remote_item("new.txt", "fid2");
        let renames = detect_renames(&[j], &[r]);
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].from.as_str(), "old.txt");
        assert_eq!(renames[0].to.as_str(), "new.txt");
        assert_eq!(renames[0].file_id, "fid2");
    }

    // 3. Multiple renames detected simultaneously.
    #[test]
    fn detect_renames_multiple_simultaneous() {
        let journal = vec![
            journal_entry("a.txt", "fid-a"),
            journal_entry("b.txt", "fid-b"),
        ];
        let remote = vec![
            remote_item("a_new.txt", "fid-a"),
            remote_item("b_new.txt", "fid-b"),
        ];
        let renames = detect_renames(&journal, &remote);
        assert_eq!(renames.len(), 2);
    }

    // 4. Journal entries without a file_id are ignored.
    #[test]
    fn detect_renames_ignores_entries_without_file_id() {
        let mut j = journal_entry("no_id.txt", "unused");
        j.file_id = None;
        let r = remote_item("no_id.txt", "fid_x");
        let renames = detect_renames(&[j], &[r]);
        assert!(
            renames.is_empty(),
            "entry without file_id should be skipped"
        );
    }

    // 5. No rename when file_id no longer present in remote (deleted remotely).
    #[test]
    fn detect_renames_skips_deleted_remote_files() {
        let j = journal_entry("gone.txt", "fid-gone");
        // Remote has no item with fid-gone
        let r = remote_item("other.txt", "fid-other");
        let renames = detect_renames(&[j], &[r]);
        assert!(
            renames.is_empty(),
            "missing file_id in remote should produce no rename"
        );
    }

    // 6. Detect move into subdirectory.
    #[test]
    fn detect_renames_move_into_subdirectory() {
        let j = journal_entry("file.txt", "fid-sub");
        let r = remote_item("subdir/file.txt", "fid-sub");
        let renames = detect_renames(&[j], &[r]);
        assert_eq!(renames.len(), 1);
        assert_eq!(renames[0].from.as_str(), "file.txt");
        assert_eq!(renames[0].to.as_str(), "subdir/file.txt");
    }

    // ── T041 diff_remote_snapshots tests ─────────────────────────────────────

    fn remote_item_etag(p: &str, etag: &str) -> RemoteItem {
        RemoteItem {
            path: path(p),
            file_id: "fid".to_string(),
            etag: etag.to_string(),
            size: 100,
            mtime: Utc::now(),
            checksum: None,
            is_dir: false,
        }
    }

    #[test]
    fn diff_remote_unchanged_is_empty() {
        let snap = vec![remote_item_etag("a.txt", "e1")];
        let diff = diff_remote_snapshots(&snap, &snap);
        assert!(diff.is_empty());
    }

    #[test]
    fn diff_remote_detects_etag_change() {
        let prev = vec![remote_item_etag("a.txt", "e1")];
        let curr = vec![remote_item_etag("a.txt", "e2")];
        let diff = diff_remote_snapshots(&prev, &curr);
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].path.as_str(), "a.txt");
    }

    #[test]
    fn diff_remote_detects_new_file() {
        let prev: Vec<RemoteItem> = vec![];
        let curr = vec![remote_item_etag("new.txt", "e1")];
        let diff = diff_remote_snapshots(&prev, &curr);
        assert_eq!(diff.added.len(), 1);
    }

    #[test]
    fn diff_remote_detects_deleted_file() {
        let prev = vec![remote_item_etag("gone.txt", "e1")];
        let curr: Vec<RemoteItem> = vec![];
        let diff = diff_remote_snapshots(&prev, &curr);
        assert_eq!(diff.deleted.len(), 1);
        assert_eq!(diff.deleted[0].path.as_str(), "gone.txt");
    }
}
