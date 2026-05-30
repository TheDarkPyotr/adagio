use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Opaque identifier for a Nextcloud account.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountId(pub String);

impl AccountId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for AccountId {
    fn default() -> Self {
        Self::new()
    }
}

/// Opaque identifier for a sync pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PairId(pub String);

impl PairId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl fmt::Display for PairId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for PairId {
    fn default() -> Self {
        Self::new()
    }
}

/// Opaque identifier for an in-progress transfer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransferId(pub String);

impl TransferId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for TransferId {
    fn default() -> Self {
        Self::new()
    }
}

/// A path relative to the sync pair root (normalized, UTF-8, forward-slash separated).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct RelativePath(pub String);

impl RelativePath {
    /// Normalize separators and strip leading slashes.
    pub fn new(s: impl Into<String>) -> Self {
        let s = s.into().replace('\\', "/");
        let s = s.trim_start_matches('/').to_string();
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RelativePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An absolute path on the local filesystem.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalPath(pub std::path::PathBuf);

impl LocalPath {
    pub fn new(p: impl Into<std::path::PathBuf>) -> Self {
        Self(p.into())
    }
}

/// A path on the remote server (URL-encoded, absolute from server root).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RemotePath(pub String);

impl RemotePath {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RemotePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A file checksum with algorithm tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checksum {
    pub algorithm: ChecksumAlgorithm,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ChecksumAlgorithm {
    Sha256,
    Md5,
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let algo = match self.algorithm {
            ChecksumAlgorithm::Sha256 => "SHA256",
            ChecksumAlgorithm::Md5 => "MD5",
        };
        write!(f, "{}:{}", algo, self.value)
    }
}

/// A Nextcloud account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub display_name: String,
    pub server_url: String,
    pub username: String,
    /// Credentials are stored in the OS keychain; this field holds a service-key
    /// reference, never the plaintext secret.
    pub keychain_service_key: String,
    pub created_at: DateTime<Utc>,
    /// Maximum upload speed in Kbps. 0 = unlimited.
    #[serde(default)]
    pub upload_limit_kbps: u64,
    /// Maximum download speed in Kbps. 0 = unlimited.
    #[serde(default)]
    pub download_limit_kbps: u64,
}

/// A sync pair: local directory ↔ remote directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPair {
    pub id: PairId,
    pub account_id: AccountId,
    pub local_root: LocalPath,
    pub remote_root: RemotePath,
    pub status: PairStatus,
    /// Glob patterns for paths to exclude.
    pub exclude_patterns: Vec<String>,
    /// Relative paths explicitly selected for sync (empty = sync all).
    pub selective_paths: Vec<RelativePath>,
    pub created_at: DateTime<Utc>,
    pub last_synced_at: Option<DateTime<Utc>>,
    /// How often to run a full local scan in seconds (default: 7200 = 2h).
    #[serde(default = "default_scan_interval_secs")]
    pub scan_interval_secs: u64,
    /// Whether to run a full scan immediately on startup (default: true).
    #[serde(default = "default_scan_on_startup")]
    pub scan_on_startup: bool,
    /// Maximum simultaneous uploads for this pair (default: 3, T113).
    #[serde(default = "default_upload_concurrency")]
    pub max_upload_concurrency: u8,
    /// Maximum simultaneous downloads for this pair (default: 3, T113).
    #[serde(default = "default_download_concurrency")]
    pub max_download_concurrency: u8,
    /// How to resolve sync conflicts for this pair (default: Ask).
    #[serde(default = "default_conflict_policy")]
    pub conflict_policy: ConflictPolicy,
    /// Number of parallel upload workers in bulk-upload mode (default: 8, range 1–32).
    #[serde(default = "default_bulk_upload_workers")]
    pub bulk_upload_workers: u8,
    /// Minimum pending-upload count to trigger bulk mode (default: 50).
    #[serde(default = "default_bulk_upload_threshold_files")]
    pub bulk_upload_threshold_files: u32,
    /// File size in bytes above which chunked multipart upload is used (default: 10 MiB).
    #[serde(default = "default_bulk_upload_chunk_threshold_bytes")]
    pub bulk_upload_chunk_threshold_bytes: u64,
    /// Enable VFS (on-demand files) mode for this pair. Default: false (copy sync).
    #[serde(default)]
    pub vfs_enabled: bool,
    /// Maximum bytes of locally-cached VFS content (default: 20 GiB; 0 = unlimited).
    #[serde(default = "default_vfs_cache_max_bytes")]
    pub vfs_cache_max_bytes: u64,
    /// Auto-evict locally-available files when free disk falls below this threshold (default: 5 GiB).
    #[serde(default = "default_vfs_eviction_threshold_bytes")]
    pub vfs_eviction_threshold_bytes: u64,
}

fn default_conflict_policy() -> ConflictPolicy {
    ConflictPolicy::Ask
}
fn default_bulk_upload_workers() -> u8 {
    8
}
fn default_bulk_upload_threshold_files() -> u32 {
    50
}
fn default_bulk_upload_chunk_threshold_bytes() -> u64 {
    10 * 1024 * 1024 // 10 MiB
}
fn default_vfs_cache_max_bytes() -> u64 {
    20 * 1024 * 1024 * 1024 // 20 GiB
}
fn default_vfs_eviction_threshold_bytes() -> u64 {
    5 * 1024 * 1024 * 1024 // 5 GiB free-disk minimum
}

fn default_scan_interval_secs() -> u64 {
    7200
}
fn default_scan_on_startup() -> bool {
    true
}
fn default_upload_concurrency() -> u8 {
    3
}
fn default_download_concurrency() -> u8 {
    3
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairStatus {
    Idle,
    Syncing,
    Paused,
    Error(String),
}

/// The last-known-synced state for a single item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub pair_id: PairId,
    pub path: RelativePath,
    pub file_id: Option<String>,
    pub etag: Option<String>,
    pub checksum: Option<Checksum>,
    pub size: u64,
    pub mtime_local: Option<DateTime<Utc>>,
    pub mtime_remote: Option<DateTime<Utc>>,
    pub status: SyncStatus,
    pub error_message: Option<String>,
    pub retry_count: u32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Synced,
    PendingUpload,
    PendingDownload,
    Conflict,
    Error,
    Excluded,
}

/// An item discovered during the local snapshot scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalItem {
    pub path: RelativePath,
    pub size: u64,
    pub mtime: DateTime<Utc>,
    pub checksum: Option<Checksum>,
    pub is_dir: bool,
}

/// An item returned by the remote PROPFIND scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteItem {
    pub path: RelativePath,
    pub file_id: String,
    pub etag: String,
    pub size: u64,
    pub mtime: DateTime<Utc>,
    pub checksum: Option<Checksum>,
    pub is_dir: bool,
}

impl AsRef<RelativePath> for LocalItem {
    fn as_ref(&self) -> &RelativePath {
        &self.path
    }
}

impl AsRef<RelativePath> for RemoteItem {
    fn as_ref(&self) -> &RelativePath {
        &self.path
    }
}

/// Sub-type of a conflict, describing how the two sides diverged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    /// Both sides modified the file content (classic file conflict).
    #[default]
    ContentModified,
    /// The folder was renamed to different names on each side.
    RenamedBothSides,
    /// Item deleted on one side while the other side added new content or children.
    DeletedWithContent,
}

/// A recorded conflict between local and remote versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictRecord {
    pub id: String,
    pub pair_id: PairId,
    pub path: RelativePath,
    pub local_mtime: DateTime<Utc>,
    pub remote_mtime: DateTime<Utc>,
    pub local_size: u64,
    pub remote_size: u64,
    pub policy: ConflictPolicy,
    pub resolution: Option<ConflictResolution>,
    pub detected_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    /// `true` if this conflict is on a directory (folder conflict).
    #[serde(default)]
    pub is_dir: bool,
    /// Sub-type of conflict describing how the divergence occurred.
    #[serde(default)]
    pub conflict_kind: ConflictKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// Keep both: download remote to its path, rename local to a conflict copy, upload the copy.
    PreserveBoth,
    /// Always prefer local; discard remote version.
    LocalWins,
    /// Always prefer remote; discard local version.
    RemoteWins,
    /// Keep the version with the newer mtime.
    NewestWins,
    /// Ask the user; suspend propagation until answered.
    Ask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    KeptLocal,
    KeptRemote,
    BothKept { conflict_copy_path: RelativePath },
}

/// Which side the user chose in an Ask-policy conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictSide {
    /// Keep the local version.
    Local,
    /// Keep the remote version.
    Remote,
    /// Keep both versions by creating a conflict copy alongside the remote version.
    Both,
}

/// A byte range for partial (resume) downloads.
#[derive(Debug, Clone, Copy)]
pub struct ByteRange {
    pub start: u64,
    pub end: Option<u64>,
}

/// Server capability flags returned by the Nextcloud capabilities endpoint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub max_chunk_size: u64,
    pub supports_chunked_upload: bool,
    pub supports_dav_checksum: bool,
    pub server_version: String,
}

#[cfg(test)]
mod conflict_type_tests {
    use super::*;

    // T002 — ConflictKind must exist with three snake_case-serialised variants.
    #[test]
    fn conflict_kind_serializes_snake_case() {
        let content: ConflictKind = serde_json::from_str(r#""content_modified""#).unwrap();
        assert!(matches!(content, ConflictKind::ContentModified));
        let renamed: ConflictKind = serde_json::from_str(r#""renamed_both_sides""#).unwrap();
        assert!(matches!(renamed, ConflictKind::RenamedBothSides));
        let deleted: ConflictKind = serde_json::from_str(r#""deleted_with_content""#).unwrap();
        assert!(matches!(deleted, ConflictKind::DeletedWithContent));
        assert_eq!(
            serde_json::to_string(&ConflictKind::ContentModified).unwrap(),
            r#""content_modified""#
        );
    }

    // T003 — ConflictRecord gains is_dir (default false) and conflict_kind.
    #[test]
    fn conflict_record_has_is_dir_defaults_false() {
        use crate::types::{ConflictPolicy, PairId, RelativePath};
        use chrono::Utc;
        let record = ConflictRecord {
            id: "test-id".to_string(),
            pair_id: PairId::new(),
            path: RelativePath::new("file.txt"),
            local_mtime: Utc::now(),
            remote_mtime: Utc::now(),
            local_size: 0,
            remote_size: 0,
            policy: ConflictPolicy::Ask,
            resolution: None,
            detected_at: Utc::now(),
            resolved_at: None,
            is_dir: false,
            conflict_kind: ConflictKind::ContentModified,
        };
        assert!(!record.is_dir);
        assert!(matches!(
            record.conflict_kind,
            ConflictKind::ContentModified
        ));

        // Round-trip through serde — is_dir and conflict_kind survive.
        let json = serde_json::to_string(&record).unwrap();
        let back: ConflictRecord = serde_json::from_str(&json).unwrap();
        assert!(!back.is_dir);
    }

    // T004 — ConflictSide must have a Both variant distinct from Local/Remote.
    #[test]
    fn conflict_side_has_both_variant() {
        let local = ConflictSide::Local;
        let remote = ConflictSide::Remote;
        let both = ConflictSide::Both;
        assert_ne!(local, remote);
        assert_ne!(local, both);
        assert_ne!(remote, both);
    }
}
