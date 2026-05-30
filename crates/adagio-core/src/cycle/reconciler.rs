use crate::types::{Checksum, ConflictPolicy, JournalEntry, LocalItem, RelativePath, RemoteItem};
use chrono::{DateTime, Utc};

// ── Content-match dedup (T022/T030) ──────────────────────────────────────────

/// Determine whether a first-sync upload can be skipped because the remote
/// already holds a file with the same content.
///
/// Both the algorithm and value must match; a missing remote checksum is not a match.
pub fn should_skip_upload(local: &Checksum, remote: Option<&Checksum>) -> bool {
    match remote {
        Some(r) => r.algorithm == local.algorithm && r.value == local.value,
        None => false,
    }
}

// ── Reconciler (T036/T043) ────────────────────────────────────────────────────

/// A single operation produced by the reconciler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOp {
    /// File is in sync — no action needed.
    NoOp { path: RelativePath },
    /// Local file was created or modified; upload it.
    Upload {
        path: RelativePath,
        local_checksum: Option<crate::types::Checksum>,
    },
    /// Remote file was created or modified; download it.
    Download {
        path: RelativePath,
        etag: String,
        remote_checksum: Option<crate::types::Checksum>,
    },
    /// Local file was deleted; delete the remote copy.
    DeleteRemote { path: RelativePath },
    /// Remote file was deleted; delete the local copy.
    DeleteLocal { path: RelativePath },
    /// Both sides have changed since the last sync — conflict resolution required.
    Conflict {
        path: RelativePath,
        /// etag of the remote version that caused the conflict.
        remote_etag: String,
        /// mtime of the remote version (used by NewestWins).
        remote_mtime: DateTime<Utc>,
        /// Size of the remote version in bytes.
        remote_size: u64,
        /// Policy to apply when resolving (from the pair's configuration).
        policy: ConflictPolicy,
    },
    /// File exists on both sides with identical content; record as Synced without transfer.
    Adopt {
        path: RelativePath,
        etag: String,
        local_checksum: Option<crate::types::Checksum>,
    },
    /// Remote item was renamed/moved (detected via stable file ID); rename locally.
    MoveLocal {
        from: RelativePath,
        to: RelativePath,
    },
    /// Local item was renamed/moved; rename on the remote.
    MoveRemote {
        from: RelativePath,
        to: RelativePath,
    },
}

impl SyncOp {
    /// Return the primary path associated with this operation, if any.
    pub fn path(&self) -> Option<&RelativePath> {
        match self {
            SyncOp::NoOp { path }
            | SyncOp::Upload { path, .. }
            | SyncOp::Download { path, .. }
            | SyncOp::DeleteRemote { path }
            | SyncOp::DeleteLocal { path }
            | SyncOp::Conflict { path, .. }
            | SyncOp::Adopt { path, .. } => Some(path),
            SyncOp::MoveLocal { to, .. } | SyncOp::MoveRemote { to, .. } => Some(to),
        }
    }
}

/// The full set of operations to execute in one sync cycle for a pair.
#[derive(Debug, Default)]
pub struct OperationPlan {
    pub ops: Vec<SyncOp>,
}

impl OperationPlan {
    pub fn uploads(&self) -> Vec<&SyncOp> {
        self.ops
            .iter()
            .filter(|op| matches!(op, SyncOp::Upload { .. }))
            .collect()
    }

    pub fn downloads(&self) -> Vec<&SyncOp> {
        self.ops
            .iter()
            .filter(|op| matches!(op, SyncOp::Download { .. }))
            .collect()
    }

    pub fn conflicts(&self) -> Vec<&SyncOp> {
        self.ops
            .iter()
            .filter(|op| matches!(op, SyncOp::Conflict { .. }))
            .collect()
    }
}

/// Three-way reconciliation: local snapshot × remote snapshot × journal state.
///
/// Implements all 10 change categories from FR-020:
/// 1. Unchanged  → NoOp
/// 2. Local-only change → Upload
/// 3. Remote-only change → Download
/// 4. Local creation (not in journal/remote) → Upload
/// 5. Remote creation (not in journal/local) → Download
/// 6. Local deletion (in journal + remote, absent locally) → DeleteRemote
/// 7. Remote deletion (in journal + local, absent remotely) → DeleteLocal
/// 8. Both sides changed → Conflict
/// 9. Remote rename (same file_id, new path) → MoveLocal
/// 10. Local rename (same checksum, old path gone, new path appears) → MoveRemote
pub fn reconcile(
    local: &[LocalItem],
    remote: &[RemoteItem],
    journal: &[JournalEntry],
    conflict_policy: ConflictPolicy,
) -> OperationPlan {
    use std::collections::HashMap;

    // Build lookup maps keyed by path string.
    let local_by_path: HashMap<&str, &LocalItem> =
        local.iter().map(|i| (i.path.as_str(), i)).collect();
    let remote_by_path: HashMap<&str, &RemoteItem> =
        remote.iter().map(|i| (i.path.as_str(), i)).collect();
    let journal_by_path: HashMap<&str, &JournalEntry> =
        journal.iter().map(|e| (e.path.as_str(), e)).collect();

    // Secondary index: remote items by file_id (for rename detection).
    let remote_by_file_id: HashMap<&str, &RemoteItem> =
        remote.iter().map(|i| (i.file_id.as_str(), i)).collect();

    // Collect all paths mentioned across all three snapshots.
    let mut all_paths: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for i in local {
        all_paths.insert(i.path.as_str());
    }
    for i in remote {
        all_paths.insert(i.path.as_str());
    }
    for e in journal {
        all_paths.insert(e.path.as_str());
    }

    // Track paths handled by rename detection to avoid double-emitting.
    let mut handled: std::collections::HashSet<&str> = std::collections::HashSet::new();

    let mut ops: Vec<SyncOp> = Vec::new();

    // ── Pass 1: Remote renames (category 9) ──────────────────────────────────
    // A remote rename is detected when the journal knows a file_id at path A,
    // but the remote now has that same file_id at path B.
    for entry in journal {
        if let Some(fid) = &entry.file_id {
            if let Some(r) = remote_by_file_id.get(fid.as_str()) {
                let old = entry.path.as_str();
                let new = r.path.as_str();
                if old != new {
                    // Only emit MoveLocal when the local file still exists at old path.
                    if local_by_path.contains_key(old) {
                        ops.push(SyncOp::MoveLocal {
                            from: entry.path.clone(),
                            to: r.path.clone(),
                        });
                        handled.insert(old);
                        handled.insert(new);
                    }
                }
            }
        }
    }

    // ── Pass 2: Local renames (category 10) ──────────────────────────────────
    // A local rename is detected when a journal entry's path is missing locally
    // but a local file with the same checksum exists at a different path that
    // is not in the journal/remote.
    for entry in journal {
        let old = entry.path.as_str();
        if handled.contains(old) {
            continue;
        }
        if local_by_path.contains_key(old) {
            continue;
        } // still present locally → not a rename

        if let Some(j_ck) = &entry.checksum {
            // Look for a local item with matching checksum at a new path.
            for li in local {
                let new = li.path.as_str();
                if handled.contains(new) {
                    continue;
                }
                if journal_by_path.contains_key(new) {
                    continue;
                } // already known
                if let Some(l_ck) = &li.checksum {
                    if l_ck.algorithm == j_ck.algorithm && l_ck.value == j_ck.value {
                        ops.push(SyncOp::MoveRemote {
                            from: entry.path.clone(),
                            to: li.path.clone(),
                        });
                        handled.insert(old);
                        handled.insert(new);
                        break;
                    }
                }
            }
        }
    }

    // ── Pass 3: Per-path reconciliation (categories 1-8) ─────────────────────
    for path in &all_paths {
        if handled.contains(path) {
            continue;
        }

        let l = local_by_path.get(path);
        let r = remote_by_path.get(path);
        let j = journal_by_path.get(path);

        let op = match (l, r, j) {
            // Category 4: local creation — new local file, no journal, no remote.
            (Some(li), None, None) => SyncOp::Upload {
                path: li.path.clone(),
                local_checksum: li.checksum.clone(),
            },

            // Category 5: remote creation — new remote file, no journal, no local.
            (None, Some(ri), None) => SyncOp::Download {
                path: ri.path.clone(),
                etag: ri.etag.clone(),
                remote_checksum: ri.checksum.clone(),
            },

            // Category 6: local deletion — was synced (journal + remote), now gone locally.
            // If the remote also changed since the journal, this is a delete-vs-change conflict.
            (None, Some(ri), Some(j)) => {
                if etag_differs(ri.etag.as_str(), j.etag.as_deref()) {
                    SyncOp::Conflict {
                        path: ri.path.clone(),
                        remote_etag: ri.etag.clone(),
                        remote_mtime: ri.mtime,
                        remote_size: ri.size,
                        policy: conflict_policy.clone(),
                    }
                } else {
                    SyncOp::DeleteRemote {
                        path: ri.path.clone(),
                    }
                }
            }

            // Category 7: remote deletion — was synced (journal + local), now gone remotely.
            // If the local also changed since the journal, this is a delete-vs-change conflict.
            (Some(li), None, Some(j)) => {
                // Guard: if the journal entry is a permanent error (e.g. path-too-long),
                // the remote was never actually in sync — emitting DeleteLocal would
                // incorrectly destroy local data and the propagator would re-park the
                // path on every cycle. Emit NoOp so the path is quietly skipped until
                // the user resolves the underlying compat issue.
                use crate::types::SyncStatus;
                if j.status == SyncStatus::Error
                    && j.error_message
                        .as_deref()
                        .map_or(false, |m| m.starts_with("permanent error:"))
                {
                    SyncOp::NoOp {
                        path: li.path.clone(),
                    }
                } else {
                    // Only flag conflict when we have a checksum baseline confirming local change.
                    // Without one (j.checksum = None), can't tell — DeleteLocal wins.
                    let local_changed =
                        j.checksum.is_some() && checksum_differs(&li.checksum, j.checksum.as_ref());
                    if local_changed {
                        SyncOp::Conflict {
                            path: li.path.clone(),
                            remote_etag: String::new(),
                            remote_mtime: Utc::now(),
                            remote_size: 0,
                            policy: conflict_policy.clone(),
                        }
                    } else {
                        SyncOp::DeleteLocal {
                            path: li.path.clone(),
                        }
                    }
                }
            }

            // Both local and remote present; journal may or may not exist.
            (Some(li), Some(ri), j_opt) => {
                // No journal: we have no baseline to detect changes from.
                // Only treat as a genuine conflict when BOTH sides have checksums
                // and they provably differ. Otherwise adopt conservatively.
                if j_opt.is_none() {
                    match (&li.checksum, &ri.checksum) {
                        (Some(l_ck), Some(r_ck))
                            if l_ck.algorithm != r_ck.algorithm || l_ck.value != r_ck.value =>
                        {
                            // Both checksums present and different → genuine divergence.
                            // Fall through to the local_changed / remote_changed logic.
                        }
                        _ => {
                            // Checksums match, or at least one is missing.
                            // Can't confirm divergence → adopt without transfer.
                            ops.push(SyncOp::Adopt {
                                path: li.path.clone(),
                                etag: ri.etag.clone(),
                                local_checksum: li.checksum.clone(),
                            });
                            continue;
                        }
                    }
                }

                // When the journal has a checksum baseline, compare against it.
                // When no baseline exists (j.checksum = None), we can't detect local changes
                // — treat as unchanged. Only flag true when there is genuinely no journal
                // entry at all (j_opt = None), which means both checksums were present and
                // differed (handled by the j_opt.is_none() block above).
                let local_changed = match j_opt.and_then(|j| j.checksum.as_ref()) {
                    Some(j_ck) => checksum_differs(&li.checksum, Some(j_ck)),
                    None => j_opt.is_none(),
                };
                let remote_changed = etag_differs(
                    ri.etag.as_str(),
                    j_opt.map(|j| j.etag.as_deref()).unwrap_or(None),
                );

                match (local_changed, remote_changed) {
                    (false, false) => SyncOp::NoOp {
                        path: li.path.clone(),
                    }, // category 1
                    (true, false) => SyncOp::Upload {
                        path: li.path.clone(),
                        local_checksum: li.checksum.clone(),
                    }, // category 2
                    (false, true) => SyncOp::Download {
                        path: li.path.clone(),
                        etag: ri.etag.clone(),
                        remote_checksum: ri.checksum.clone(),
                    }, // category 3
                    (true, true) => SyncOp::Conflict {
                        // category 8
                        path: li.path.clone(),
                        remote_etag: ri.etag.clone(),
                        remote_mtime: ri.mtime,
                        remote_size: ri.size,
                        policy: conflict_policy.clone(),
                    },
                }
            }

            // Both absent and no journal entry — nothing to do.
            (None, None, _) => continue,
        };

        ops.push(op);
    }

    OperationPlan { ops }
}

fn checksum_differs(
    local_ck: &Option<crate::types::Checksum>,
    journal_ck: Option<&crate::types::Checksum>,
) -> bool {
    match (local_ck, journal_ck) {
        (Some(l), Some(j)) => l.algorithm != j.algorithm || l.value != j.value,
        (Some(_), None) => true, // new file, no journal
        (None, _) => false,      // no local checksum → treat as unchanged
    }
}

fn etag_differs(remote_etag: &str, journal_etag: Option<&str>) -> bool {
    match journal_etag {
        Some(j) => remote_etag != j,
        None => true, // no journal → treat remote as new
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Checksum, ChecksumAlgorithm, ConflictPolicy, JournalEntry, LocalItem, PairId, RelativePath,
        RemoteItem, SyncStatus,
    };
    use chrono::Utc;

    fn path(s: &str) -> RelativePath {
        RelativePath::new(s)
    }

    fn sha256(v: &str) -> Checksum {
        Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: v.to_string(),
        }
    }

    fn local_item(p: &str, size: u64, cksum: &str) -> LocalItem {
        LocalItem {
            path: path(p),
            size,
            mtime: Utc::now(),
            checksum: Some(sha256(cksum)),
            is_dir: false,
        }
    }

    fn remote_item(p: &str, size: u64, etag: &str, file_id: &str, cksum: &str) -> RemoteItem {
        RemoteItem {
            path: path(p),
            file_id: file_id.to_string(),
            etag: etag.to_string(),
            size,
            mtime: Utc::now(),
            checksum: Some(sha256(cksum)),
            is_dir: false,
        }
    }

    fn journal_entry(p: &str, file_id: &str, etag: &str, cksum: &str) -> JournalEntry {
        JournalEntry {
            pair_id: PairId::new(),
            path: path(p),
            file_id: Some(file_id.to_string()),
            etag: Some(etag.to_string()),
            checksum: Some(sha256(cksum)),
            size: 100,
            mtime_local: Some(Utc::now()),
            mtime_remote: Some(Utc::now()),
            status: SyncStatus::Synced,
            error_message: None,
            retry_count: 0,
            updated_at: Utc::now(),
        }
    }

    // ── T036 tests ────────────────────────────────────────────────────────────

    // 1. Unchanged
    #[test]
    fn reconcile_unchanged_produces_noop() {
        let j = journal_entry("doc.txt", "fid1", "etag1", "aaaa");
        let l = local_item("doc.txt", 100, "aaaa");
        let r = remote_item("doc.txt", 100, "etag1", "fid1", "aaaa");
        let plan = reconcile(&[l], &[r], &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops.iter().any(|op| matches!(op, SyncOp::NoOp { .. })),
            "unchanged item should produce NoOp"
        );
    }

    // 2. Local-only change → Upload
    #[test]
    fn reconcile_local_change_produces_upload() {
        let j = journal_entry("doc.txt", "fid1", "etag1", "aaaa");
        // Local checksum differs → local changed
        let l = local_item("doc.txt", 200, "bbbb");
        let r = remote_item("doc.txt", 100, "etag1", "fid1", "aaaa");
        let plan = reconcile(&[l], &[r], &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops
                .iter()
                .any(|op| matches!(op, SyncOp::Upload { .. })),
            "local change should produce Upload"
        );
    }

    // 3. Remote-only change → Download
    #[test]
    fn reconcile_remote_change_produces_download() {
        let j = journal_entry("doc.txt", "fid1", "etag1", "aaaa");
        let l = local_item("doc.txt", 100, "aaaa");
        // Remote etag differs → remote changed
        let r = remote_item("doc.txt", 200, "etag2", "fid1", "cccc");
        let plan = reconcile(&[l], &[r], &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops
                .iter()
                .any(|op| matches!(op, SyncOp::Download { .. })),
            "remote change should produce Download"
        );
    }

    // 4. Local creation (new local, not in journal, not in remote) → Upload
    #[test]
    fn reconcile_local_creation_produces_upload() {
        let l = local_item("new_local.txt", 50, "dddd");
        // No journal entry, no remote item
        let plan = reconcile(&[l], &[], &[], ConflictPolicy::NewestWins);
        assert!(
            plan.ops
                .iter()
                .any(|op| matches!(op, SyncOp::Upload { .. })),
            "new local file should produce Upload"
        );
    }

    // 5. Remote creation (new remote, not in journal, not locally) → Download
    #[test]
    fn reconcile_remote_creation_produces_download() {
        let r = remote_item("new_remote.txt", 50, "etag5", "fid5", "eeee");
        // No journal entry, no local item
        let plan = reconcile(&[], &[r], &[], ConflictPolicy::NewestWins);
        assert!(
            plan.ops
                .iter()
                .any(|op| matches!(op, SyncOp::Download { .. })),
            "new remote file should produce Download"
        );
    }

    // 6. Local deletion → DeleteRemote
    #[test]
    fn reconcile_local_deletion_produces_delete_remote() {
        let j = journal_entry("deleted_local.txt", "fid6", "etag6", "ffff");
        let r = remote_item("deleted_local.txt", 100, "etag6", "fid6", "ffff");
        // No local item
        let plan = reconcile(&[], &[r], &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops
                .iter()
                .any(|op| matches!(op, SyncOp::DeleteRemote { .. })),
            "locally deleted file should produce DeleteRemote"
        );
    }

    // 7. Remote deletion → DeleteLocal
    #[test]
    fn reconcile_remote_deletion_produces_delete_local() {
        let j = journal_entry("deleted_remote.txt", "fid7", "etag7", "gggg");
        let l = local_item("deleted_remote.txt", 100, "gggg");
        // No remote item
        let plan = reconcile(&[l], &[], &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops
                .iter()
                .any(|op| matches!(op, SyncOp::DeleteLocal { .. })),
            "remotely deleted file should produce DeleteLocal"
        );
    }

    // 8. Both-sides changed → Conflict
    #[test]
    fn reconcile_both_sides_changed_produces_conflict() {
        let j = journal_entry("conflict.txt", "fid8", "etag8", "hhhh");
        // Both local and remote checksums differ from journal
        let l = local_item("conflict.txt", 150, "iiii");
        let r = remote_item("conflict.txt", 200, "etag9", "fid8", "jjjj");
        let plan = reconcile(&[l], &[r], &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops
                .iter()
                .any(|op| matches!(op, SyncOp::Conflict { .. })),
            "both-sides-changed should produce Conflict"
        );
    }

    // 9. Remote rename (same file_id, new path) → MoveLocal
    #[test]
    fn reconcile_remote_rename_produces_move_local() {
        // Journal knows file as "old_name.txt" with fid9
        let j = journal_entry("old_name.txt", "fid9", "etag9", "kkkk");
        let l = local_item("old_name.txt", 100, "kkkk");
        // Remote now has same file_id at a new path
        let r = remote_item("new_name.txt", 100, "etag9", "fid9", "kkkk");
        let plan = reconcile(&[l], &[r], &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops.iter().any(|op| matches!(
                op,
                SyncOp::MoveLocal {
                    from,
                    to
                } if from.as_str() == "old_name.txt" && to.as_str() == "new_name.txt"
            )),
            "remote rename (same file_id, new path) should produce MoveLocal"
        );
    }

    // 10. Local rename (file at new path, old path gone from journal) → MoveRemote
    #[test]
    fn reconcile_local_rename_produces_move_remote() {
        // Journal: "old_local.txt" → fid10
        let j = journal_entry("old_local.txt", "fid10", "etag10", "llll");
        // Local: "old_local.txt" is gone, "new_local.txt" appears with same checksum
        let l = local_item("new_local.txt", 100, "llll");
        // Remote: still has old name
        let r = remote_item("old_local.txt", 100, "etag10", "fid10", "llll");
        let plan = reconcile(&[l], &[r], &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops.iter().any(|op| matches!(
                op,
                SyncOp::MoveRemote {
                    from,
                    to
                } if from.as_str() == "old_local.txt" && to.as_str() == "new_local.txt"
            )),
            "local rename (checksum match, old path absent) should produce MoveRemote"
        );
    }

    // ── T055: delete-vs-change conflict tests ────────────────────────────────

    // T055-a: local deleted + remote changed → Conflict (not DeleteRemote)
    #[test]
    fn reconcile_local_deleted_and_remote_changed_produces_conflict() {
        let j = journal_entry("doc.txt", "fid1", "old_etag", "aaaa");
        // Local: absent (deleted)
        // Remote: etag changed → remote modified after last sync
        let r = remote_item("doc.txt", 200, "new_etag", "fid1", "bbbb");
        let plan = reconcile(&[], &[r], &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops
                .iter()
                .any(|op| matches!(op, SyncOp::Conflict { .. })),
            "local-deleted + remote-changed should produce Conflict, got: {:?}",
            plan.ops
        );
    }

    // T055-b: remote deleted + local changed → Conflict (not DeleteLocal)
    #[test]
    fn reconcile_remote_deleted_and_local_changed_produces_conflict() {
        let j = journal_entry("doc.txt", "fid2", "old_etag", "aaaa");
        // Local: checksum changed → local modified after last sync
        let l = local_item("doc.txt", 200, "cccc");
        // Remote: absent (deleted)
        let plan = reconcile(&[l], &[], &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops
                .iter()
                .any(|op| matches!(op, SyncOp::Conflict { .. })),
            "remote-deleted + local-changed should produce Conflict, got: {:?}",
            plan.ops
        );
    }

    // ── T022/T030 dedup tests ─────────────────────────────────────────────────

    #[test]
    fn skip_upload_when_checksums_match() {
        let cksum = sha256("abc123");
        assert!(
            should_skip_upload(&cksum, Some(&cksum)),
            "identical checksums should skip the upload"
        );
    }

    #[test]
    fn do_not_skip_when_checksums_differ() {
        let local = sha256("abc123");
        let remote = sha256("def456");
        assert!(
            !should_skip_upload(&local, Some(&remote)),
            "different checksums must not skip the upload"
        );
    }

    #[test]
    fn do_not_skip_when_no_remote_checksum() {
        let local = sha256("abc123");
        assert!(
            !should_skip_upload(&local, None),
            "no remote checksum means we cannot confirm match — must not skip"
        );
    }

    #[test]
    fn do_not_skip_when_algorithms_differ() {
        use crate::types::ChecksumAlgorithm;
        let local = Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: "abc123".to_string(),
        };
        let remote = Checksum {
            algorithm: ChecksumAlgorithm::Md5,
            value: "abc123".to_string(),
        };
        assert!(
            !should_skip_upload(&local, Some(&remote)),
            "same value but different algorithms must not skip"
        );
    }

    // T086: Newly-excluded item (absent from filtered remote snapshot) → DeleteLocal.
    #[test]
    fn reconcile_newly_excluded_path_produces_delete_local() {
        use crate::detection::exclusion::filter_selective;
        use crate::types::RelativePath;

        // "docs/report.pdf" was synced before (in journal + locally).
        let j = journal_entry("docs/report.pdf", "fid_ex", "etag_ex", "ex_hash");
        let l = local_item("docs/report.pdf", 500, "ex_hash");

        // Remote still has it, but we apply selective filter that excludes docs/.
        let r = remote_item("docs/report.pdf", 500, "etag_ex", "fid_ex", "ex_hash");
        let all_remote = vec![r];
        // Filter: only "photos/" is selected → docs/ is filtered out.
        let selective = vec![RelativePath::new("photos")];
        let filtered_remote = filter_selective(all_remote, &selective);

        let plan = reconcile(&[l], &filtered_remote, &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops
                .iter()
                .any(|op| matches!(op, SyncOp::DeleteLocal { .. })),
            "item excluded by selective_sync filter should produce DeleteLocal"
        );
    }

    // Regression: permanently-errored paths (e.g. path too long) must produce
    // NoOp, not DeleteLocal, so they are not re-queued every cycle.
    #[test]
    fn reconcile_permanent_error_journal_entry_produces_noop() {
        // Simulate a path that failed with "permanent error: path too long".
        let mut j = journal_entry("very/long/path.txt", "", "", "aaaa");
        j.status = SyncStatus::Error;
        j.etag = Some("".to_string()); // never uploaded
        j.error_message = Some(
            "permanent error: path too long (300 chars, max 259): very/long/path.txt".to_string(),
        );

        // File still exists locally; remote doesn't have it (upload never succeeded).
        let l = local_item("very/long/path.txt", 100, "aaaa");

        let plan = reconcile(&[l], &[], &[j], ConflictPolicy::NewestWins);
        assert!(
            plan.ops.iter().all(|op| matches!(op, SyncOp::NoOp { .. })),
            "permanently-errored path should produce NoOp, not DeleteLocal or Upload; got: {:?}",
            plan.ops
        );
    }
}
