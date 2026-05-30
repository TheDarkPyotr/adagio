use crate::error::SyncError;
use crate::types::{AccountId, ConflictPolicy, PairId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Application-wide runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// How often to poll the remote for changes in the absence of local events.
    #[serde(with = "duration_secs")]
    pub remote_poll_interval: Duration,

    /// Conflict handling policy applied when both sides have changed.
    pub conflict_policy: ConflictPolicy,

    /// Bandwidth throttle in bytes/sec per pair. None = unlimited.
    pub bandwidth_cap: Option<u64>,

    /// Maximum number of concurrent upload/download operations across all pairs.
    pub max_concurrent_transfers: usize,

    /// Log level filter string (e.g. "adagio=debug,warn").
    pub log_level: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            remote_poll_interval: Duration::from_secs(30),
            conflict_policy: ConflictPolicy::NewestWins,
            bandwidth_cap: None,
            max_concurrent_transfers: 4,
            log_level: "adagio=info,warn".to_string(),
        }
    }
}

/// Per-pair configuration overrides (merged on top of AppConfig).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PairConfig {
    pub pair_id: PairId,
    pub conflict_policy: Option<ConflictPolicy>,
    pub bandwidth_cap: Option<u64>,
}

/// Configuration for creating or validating a new sync pair.
#[derive(Debug, Clone)]
pub struct SyncPairConfig {
    pub local_root: PathBuf,
    pub remote_root: String,
    pub account_id: AccountId,
}

impl SyncPairConfig {
    /// Validate the proposed pair configuration.
    ///
    /// Rejects: non-writable local root, or root that cannot be stat'd.
    pub fn validate(&self) -> Result<(), SyncError> {
        // Check the path exists and is a directory.
        let meta = std::fs::metadata(&self.local_root).map_err(|e| {
            SyncError::Permanent(format!("cannot stat local root {:?}: {e}", self.local_root))
        })?;
        if !meta.is_dir() {
            return Err(SyncError::Permanent(format!(
                "local root {:?} is not a directory",
                self.local_root
            )));
        }
        // Check writability by probing with a temp file.
        let probe = self.local_root.join(".adagio_write_probe");
        std::fs::write(&probe, b"").map_err(|_| {
            SyncError::Permanent(format!("local root {:?} is not writable", self.local_root))
        })?;
        let _ = std::fs::remove_file(&probe);
        Ok(())
    }
}

/// An exclude glob pattern. Builtin patterns cannot be removed by callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExcludePattern {
    pub pattern: String,
    pub is_builtin: bool,
}

/// Manages sync pairs: creation, update, deletion, and exclude-pattern governance.
pub struct SyncPairManager {
    patterns: Vec<ExcludePattern>,
    /// Registered pair local roots, keyed by PairId. Used by delete_pair.
    pair_roots: HashMap<PairId, PathBuf>,
    /// Full SyncPair objects, keyed by PairId.
    pairs: HashMap<PairId, crate::types::SyncPair>,
}

impl SyncPairManager {
    pub fn new() -> Self {
        Self {
            patterns: builtin_patterns(),
            pair_roots: HashMap::new(),
            pairs: HashMap::new(),
        }
    }

    /// Register a pair's local root so delete_pair can find it.
    ///
    /// Deprecated: use `register_full_pair` for new code.
    pub fn register_pair(&mut self, pair_id: PairId, local_root: PathBuf) {
        self.pair_roots.insert(pair_id, local_root);
    }

    /// Register a full `SyncPair`.
    ///
    /// Also updates the legacy `pair_roots` index so `delete_pair` continues to work.
    pub fn register_full_pair(&mut self, pair: crate::types::SyncPair) {
        self.pair_roots
            .insert(pair.id.clone(), pair.local_root.0.clone());
        self.pairs.insert(pair.id.clone(), pair);
    }

    /// Return a reference to the pair with `id`, or `None` if not registered.
    pub fn get_pair(&self, id: &PairId) -> Option<&crate::types::SyncPair> {
        self.pairs.get(id)
    }

    /// Return references to all registered `SyncPair`s.
    pub fn all_pairs(&self) -> Vec<&crate::types::SyncPair> {
        self.pairs.values().collect()
    }

    /// Remove a pair from all internal maps. Does not touch the filesystem.
    pub fn remove_pair(&mut self, id: &PairId) {
        self.pairs.remove(id);
        self.pair_roots.remove(id);
    }

    pub fn exclude_patterns(&self) -> &[ExcludePattern] {
        &self.patterns
    }

    /// Replace the exclude pattern list.
    ///
    /// Rejects the update if any currently-builtin pattern is absent from `new_patterns`.
    pub fn update_exclude_patterns(
        &mut self,
        new_patterns: Vec<ExcludePattern>,
    ) -> Result<(), SyncError> {
        let new_set: std::collections::HashSet<&str> =
            new_patterns.iter().map(|p| p.pattern.as_str()).collect();
        for builtin in self.patterns.iter().filter(|p| p.is_builtin) {
            if !new_set.contains(builtin.pattern.as_str()) {
                return Err(SyncError::Permanent(format!(
                    "builtin exclude pattern {:?} cannot be removed",
                    builtin.pattern
                )));
            }
        }
        self.patterns = new_patterns;
        Ok(())
    }

    /// Delete a sync pair, optionally removing the local root directory.
    pub fn delete_pair(&self, pair_id: &PairId, delete_local_files: bool) -> Result<(), SyncError> {
        if delete_local_files {
            if let Some(root) = self.pair_roots.get(pair_id) {
                if root.exists() {
                    std::fs::remove_dir_all(root)?;
                }
            }
        }
        Ok(())
    }
}

impl Default for SyncPairManager {
    fn default() -> Self {
        Self::new()
    }
}

fn builtin_patterns() -> Vec<ExcludePattern> {
    const BUILTINS: &[&str] = &[".DS_Store", "Thumbs.db", "~$*", "*.tmp", ".~lock.*"];
    BUILTINS
        .iter()
        .map(|p| ExcludePattern {
            pattern: p.to_string(),
            is_builtin: true,
        })
        .collect()
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(d.as_secs())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::from_secs(u64::deserialize(d)?))
    }
}

// ── Selective sync management (T084) ─────────────────────────────────────────

impl crate::types::SyncPair {
    /// Add a path to the selective sync list.
    ///
    /// If `selective_paths` is empty it means "sync everything"; adding a path
    /// switches it to opt-in mode. Duplicate entries are silently ignored.
    pub fn add_selective_path(&mut self, path: crate::types::RelativePath) {
        if !self.selective_paths.contains(&path) {
            self.selective_paths.push(path);
        }
    }

    /// Remove a path from the selective sync list.
    ///
    /// If the list becomes empty after removal, all paths are synced again.
    pub fn remove_selective_path(&mut self, path: &crate::types::RelativePath) {
        self.selective_paths.retain(|p| p != path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── T021: SyncPairConfig validation ──────────────────────────────────────

    #[test]
    fn validate_accepts_writable_directory() {
        let dir = TempDir::new().unwrap();
        let cfg = SyncPairConfig {
            local_root: dir.path().to_path_buf(),
            remote_root: "/remote/docs".to_string(),
            account_id: AccountId::new(),
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_rejects_non_writable_directory() {
        let dir = TempDir::new().unwrap();
        // Make the directory read-only.
        let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o444);
        }
        std::fs::set_permissions(dir.path(), perms).unwrap();
        let cfg = SyncPairConfig {
            local_root: dir.path().to_path_buf(),
            remote_root: "/remote/docs".to_string(),
            account_id: AccountId::new(),
        };
        // Should fail: non-writable root.
        assert!(
            cfg.validate().is_err(),
            "expected validation error for non-writable directory"
        );
        // Restore permissions so TempDir cleanup works.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = std::fs::metadata(dir.path()).unwrap().permissions();
            p.set_mode(0o755);
            std::fs::set_permissions(dir.path(), p).unwrap();
        }
    }

    #[test]
    fn validate_rejects_nested_roots() {
        let parent = TempDir::new().unwrap();
        let child = parent.path().join("sub");
        std::fs::create_dir_all(&child).unwrap();
        let cfg_parent = SyncPairConfig {
            local_root: parent.path().to_path_buf(),
            remote_root: "/remote/a".to_string(),
            account_id: AccountId::new(),
        };
        let cfg_child = SyncPairConfig {
            local_root: child.clone(),
            remote_root: "/remote/b".to_string(),
            account_id: AccountId::new(),
        };
        // Neither stub validates nesting yet, but this documents the expected behaviour.
        // These will pass once T028 is implemented.
        let _ = cfg_parent.validate();
        // Child inside parent — validate_against_existing should reject this.
        let existing_roots = vec![parent.path().to_path_buf()];
        assert!(
            validate_not_nested(&cfg_child.local_root, &existing_roots).is_err(),
            "expected nested root rejection"
        );
    }

    // ── T110: builtin exclude-pattern guard ───────────────────────────────────

    #[test]
    fn update_exclude_patterns_accepts_custom_pattern() {
        let mut mgr = SyncPairManager::new();
        let mut patterns = mgr.exclude_patterns().to_vec();
        patterns.push(ExcludePattern {
            pattern: "*.log".to_string(),
            is_builtin: false,
        });
        assert!(mgr.update_exclude_patterns(patterns).is_ok());
        assert!(mgr.exclude_patterns().iter().any(|p| p.pattern == "*.log"));
    }

    #[test]
    fn update_exclude_patterns_rejects_removal_of_builtin() {
        let mut mgr = SyncPairManager::new();
        // Strip all builtins from the new list.
        let only_custom = vec![ExcludePattern {
            pattern: "*.log".to_string(),
            is_builtin: false,
        }];
        assert!(
            mgr.update_exclude_patterns(only_custom).is_err(),
            "expected error when builtins are stripped from the pattern list"
        );
    }

    // ── T105: SyncPairManager::delete_pair ───────────────────────────────────

    #[test]
    fn delete_pair_leaves_local_files_intact() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("keep.txt");
        std::fs::write(&file, b"data").unwrap();
        let mgr = SyncPairManager::new();
        let pair_id = PairId::new();
        mgr.delete_pair(&pair_id, false).unwrap();
        assert!(
            file.exists(),
            "delete_pair(false) must not remove local files"
        );
    }

    #[test]
    fn delete_pair_with_flag_removes_local_root() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("remove.txt");
        std::fs::write(&file, b"data").unwrap();
        let local_root = dir.path().to_path_buf();
        let mut mgr = SyncPairManager::new();
        let pair_id = PairId::new();
        mgr.register_pair(pair_id.clone(), local_root.clone());
        // Forget about dir so TempDir doesn't try to clean it up after we delete it.
        let _ = dir.keep();
        mgr.delete_pair(&pair_id, true).unwrap();
        // delete_local_files=true should have removed the root.
        assert!(
            !local_root.exists(),
            "delete_pair(true) must recursively remove the local root"
        );
    }

    // ── T003: SyncPairManager full-pair methods ───────────────────────────────

    #[test]
    fn register_full_pair_and_get_round_trips() {
        let mut mgr = SyncPairManager::new();
        let pair = make_test_pair();
        let id = pair.id.clone();
        mgr.register_full_pair(pair.clone());
        let found = mgr.get_pair(&id).expect("pair should be registered");
        assert_eq!(found.id, id);
        assert_eq!(found.account_id, pair.account_id);
    }

    #[test]
    fn all_pairs_returns_every_registered_pair() {
        let mut mgr = SyncPairManager::new();
        let p1 = make_test_pair();
        let p2 = make_test_pair();
        let id1 = p1.id.clone();
        let id2 = p2.id.clone();
        mgr.register_full_pair(p1);
        mgr.register_full_pair(p2);
        let all: Vec<_> = mgr.all_pairs();
        let ids: Vec<_> = all.iter().map(|p| p.id.clone()).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn get_pair_returns_none_for_unknown_id() {
        let mgr = SyncPairManager::new();
        assert!(mgr.get_pair(&PairId::new()).is_none());
    }

    // ── T084: selective sync add/remove ───────────────────────────────────────

    fn make_test_pair() -> crate::types::SyncPair {
        use crate::types::*;
        use std::path::PathBuf;
        SyncPair {
            id: PairId::new(),
            account_id: AccountId::new(),
            local_root: LocalPath::new(PathBuf::from("/tmp/test")),
            remote_root: RemotePath::new("remote/"),
            status: PairStatus::Idle,
            exclude_patterns: vec![],
            selective_paths: vec![],
            created_at: chrono::Utc::now(),
            last_synced_at: None,
            scan_interval_secs: 7200,
            scan_on_startup: true,
            max_upload_concurrency: 3,
            max_download_concurrency: 3,
            conflict_policy: ConflictPolicy::Ask,
            bulk_upload_workers: 8,
            bulk_upload_threshold_files: 50,
            bulk_upload_chunk_threshold_bytes: 10 * 1024 * 1024,
        }
    }

    #[test]
    fn add_selective_path_enables_opt_in_mode() {
        use crate::types::RelativePath;
        let mut pair = make_test_pair();
        assert!(pair.selective_paths.is_empty(), "starts with sync-all");
        pair.add_selective_path(RelativePath::new("docs"));
        assert_eq!(pair.selective_paths.len(), 1);
        assert_eq!(pair.selective_paths[0].as_str(), "docs");
    }

    #[test]
    fn add_selective_path_is_idempotent() {
        use crate::types::RelativePath;
        let mut pair = make_test_pair();
        pair.add_selective_path(RelativePath::new("photos"));
        pair.add_selective_path(RelativePath::new("photos"));
        assert_eq!(pair.selective_paths.len(), 1, "duplicate not added");
    }

    #[test]
    fn remove_selective_path_reverts_to_sync_all_when_empty() {
        use crate::types::RelativePath;
        let mut pair = make_test_pair();
        let p = RelativePath::new("docs");
        pair.add_selective_path(p.clone());
        pair.remove_selective_path(&p);
        assert!(pair.selective_paths.is_empty(), "empty list → sync all");
    }
}

/// Check that `candidate` is not inside any path in `existing`.
/// Stub: not yet implemented — T028 wires this into validate().
pub fn validate_not_nested(
    candidate: &std::path::Path,
    existing: &[std::path::PathBuf],
) -> Result<(), SyncError> {
    for root in existing {
        if candidate.starts_with(root) || root.starts_with(candidate) {
            return Err(SyncError::Permanent(format!(
                "local root {:?} is nested inside or contains an existing root {:?}",
                candidate, root
            )));
        }
    }
    Ok(())
}
