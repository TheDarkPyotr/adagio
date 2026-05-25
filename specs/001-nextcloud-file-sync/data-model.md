# Data Model: Nextcloud File Sync

**Feature**: 001-nextcloud-file-sync
**Date**: 2026-05-24

This document defines the core entities, their fields, relationships, and state
transitions for the Adagio sync engine.

---

## Entity: Account

Represents a single authenticated Nextcloud user on a specific server.

```rust
pub struct Account {
    /// Stable unique ID (UUID v4, generated on creation).
    pub id: AccountId,
    /// Base URL of the Nextcloud server, e.g. "https://cloud.example.com".
    pub server_url: Url,
    /// Nextcloud username.
    pub username: String,
    /// Display name fetched from server (informational).
    pub display_name: Option<String>,
    /// Authentication method in use.
    pub auth_method: AuthMethod,
    /// Server capabilities fetched on last successful connection.
    pub capabilities: Option<ServerCapabilities>,
    /// Current connection/authentication status.
    pub status: AccountStatus,
}

pub enum AuthMethod {
    AppPassword,
    OAuth2 { client_id: String },
}

pub enum AccountStatus {
    Connected,
    /// Credentials invalid; re-authentication required.
    AuthRequired,
    /// Network unreachable.
    Offline,
}
```

**Constraints**:
- Credentials (app-password token or OAuth2 refresh token) are stored in the OS keychain
  under key `adagio/<account_id>`; they MUST NOT appear in this struct.
- `server_url` MUST be validated as HTTPS in production (HTTP allowed for development/
  self-signed setups with explicit flag).
- `id` is immutable after creation.

---

## Entity: SyncPair

The binding between one local root directory and one remote root directory for one
account.

```rust
pub struct SyncPair {
    /// Stable unique ID.
    pub id: PairId,
    /// Owning account.
    pub account_id: AccountId,
    /// Absolute local filesystem path.
    pub local_root: PathBuf,
    /// Remote path relative to the account's Nextcloud home.
    pub remote_root: RemotePath,
    /// Current operational state.
    pub status: PairStatus,
    /// Glob patterns for items to never sync in either direction.
    pub exclude_patterns: Vec<ExcludePattern>,
    /// Paths (relative to remote_root) excluded from local materialization.
    pub selective_sync_exclusions: Vec<RemotePath>,
    /// What to do when the same item is changed on both sides.
    pub conflict_policy: ConflictPolicy,
    /// Optional bandwidth caps.
    pub bandwidth_limits: Option<BandwidthLimits>,
    /// When this pair was created.
    pub created_at: DateTime<Utc>,
    /// When the last successful sync cycle completed.
    pub last_synced_at: Option<DateTime<Utc>>,
}

pub enum PairStatus {
    Idle,
    Scanning,
    Syncing { items_done: u64, items_total: u64, bytes_done: u64, bytes_total: u64 },
    Paused,
    Error { message: String },
}

pub enum ConflictPolicy {
    /// Default: download remote to normal path, rename local with conflict suffix,
    /// upload renamed local as new file.
    PreserveBoth,
    LocalWins,
    RemoteWins,
    /// The version with the later mtime prevails.
    NewestWins,
    /// Pause item, prompt user; rest of sync continues.
    Ask,
}

pub struct BandwidthLimits {
    /// Upload cap in bytes/sec. None = unlimited.
    pub upload_bps: Option<u64>,
    /// Download cap in bytes/sec. None = unlimited.
    pub download_bps: Option<u64>,
    /// Time-window overrides (e.g., throttle during 09:00–18:00).
    pub schedules: Vec<BandwidthSchedule>,
}

pub struct BandwidthSchedule {
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub upload_bps: Option<u64>,
    pub download_bps: Option<u64>,
    /// Days of week this schedule applies (0 = Sunday, 6 = Saturday).
    pub days: Vec<u8>,
}

pub struct ExcludePattern {
    /// Glob pattern, e.g. "*.tmp", ".DS_Store", "~$*".
    pub pattern: String,
    /// If true, this pattern ships with the defaults and cannot be removed by the user.
    pub is_builtin: bool,
}
```

**Constraints**:
- `local_root` MUST be an absolute path; MUST be writable; MUST NOT be nested inside
  another active pair's `local_root`.
- `local_root` MUST NOT be the same directory as the journal storage path.
- At most one `SyncPair` per unique `(account_id, local_root)` pair.

---

## Entity: JournalEntry

Persistent record of the last-known-synced state of a single sync item. One entry per
item per sync pair.

```rust
pub struct JournalEntry {
    pub pair_id: PairId,
    /// Path relative to the sync pair's local root.
    pub relative_path: RelativePath,
    pub item_type: ItemType,
    // --- File-only fields (None for directories) ---
    pub size_bytes: Option<u64>,
    pub local_mtime: Option<DateTime<Utc>>,
    /// SHA-256 or MD5 hex digest of last synced file content.
    pub local_checksum: Option<Checksum>,
    pub remote_etag: Option<String>,
    /// Server-assigned stable file ID (persists across renames).
    pub server_file_id: Option<String>,
    // --- Sync state ---
    pub last_synced_at: Option<DateTime<Utc>>,
    pub sync_status: SyncStatus,
}

pub enum ItemType { File, Directory }

pub struct Checksum {
    pub algorithm: ChecksumAlgorithm,
    pub hex: String,
}

pub enum ChecksumAlgorithm { Sha256, Md5 }

pub enum SyncStatus {
    Synced,
    PendingUpload,
    PendingDownload,
    Error { category: ErrorCategory, message: String, retry_count: u8 },
    Conflict,
    Excluded,
}
```

**Persistence**: Stored in SQLite, table `journal_entries`.
**Index**: `(pair_id, relative_path)` — primary lookup key.
**Durability**: writes use `BEGIN IMMEDIATE` + `COMMIT` with WAL mode; the journal
MUST be fsynced before the next dependent operation begins.

---

## Entity: SyncItem (runtime, not persisted separately)

Represents one item in either the local or remote snapshot during a sync cycle. Built
during the Discovery phase from filesystem scan + PROPFIND results and compared against
journal entries during Reconciliation.

```rust
pub struct LocalItem {
    pub relative_path: RelativePath,
    pub item_type: ItemType,
    pub size_bytes: Option<u64>,
    pub mtime: DateTime<Utc>,
    /// Populated lazily during reconciliation if mtime/size differ from journal.
    pub checksum: Option<Checksum>,
}

pub struct RemoteItem {
    pub relative_path: RelativePath,
    pub item_type: ItemType,
    pub size_bytes: Option<u64>,
    pub etag: String,
    /// Server-assigned stable file ID.
    pub file_id: Option<String>,
    pub last_modified: DateTime<Utc>,
}
```

---

## Entity: OperationPlan (runtime, output of Reconciliation)

Produced by the Reconciler from the three-way comparison (local snapshot, remote
snapshot, journal). This is the sole output of the Reconciliation phase; no I/O occurs
until Propagation.

```rust
pub struct OperationPlan {
    pub pair_id: PairId,
    pub operations: Vec<PlannedOperation>,
}

pub enum PlannedOperation {
    Upload { path: RelativePath, local_mtime: DateTime<Utc> },
    Download { path: RelativePath, remote_etag: String },
    DeleteRemote { path: RelativePath },
    DeleteLocal { path: RelativePath },
    MoveRemote { from: RelativePath, to: RelativePath },
    MoveLocal { from: RelativePath, to: RelativePath },
    CreateFolderRemote { path: RelativePath },
    CreateFolderLocal { path: RelativePath },
    Conflict {
        path: RelativePath,
        local_mtime: DateTime<Utc>,
        remote_etag: String,
        policy: ConflictPolicy,
    },
}
```

---

## Entity: ConflictRecord (persisted)

Permanent audit record of every detected conflict and its resolution outcome.

```rust
pub struct ConflictRecord {
    pub id: ConflictId,
    pub pair_id: PairId,
    /// Original item path.
    pub item_path: RelativePath,
    pub detected_at: DateTime<Utc>,
    pub local_mtime: DateTime<Utc>,
    pub remote_etag: String,
    pub policy_applied: ConflictPolicy,
    pub outcome: ConflictOutcome,
}

pub enum ConflictOutcome {
    PreserveBoth {
        /// Relative path of the conflict copy (local + remote).
        conflict_copy_path: RelativePath,
    },
    LocalWon,
    RemoteWon,
    UserResolved { chosen_side: ConflictSide },
}

pub enum ConflictSide { Local, Remote }
```

**Persistence**: Stored in SQLite, table `conflict_records`.

---

## Entity: Transfer (in-flight state, persisted for resumption)

Tracks an in-progress upload or download to enable resumption after interruption.

```rust
pub struct Transfer {
    pub id: TransferId,
    pub pair_id: PairId,
    pub direction: TransferDirection,
    pub relative_path: RelativePath,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    /// For chunked uploads: which chunk index to resume from.
    pub next_chunk_index: Option<u32>,
    /// Nextcloud upload session ID (for chunked uploads).
    pub upload_session_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub status: TransferStatus,
}

pub enum TransferDirection { Upload, Download }

pub enum TransferStatus {
    InProgress,
    Paused,
    Completed,
    Failed { error: String },
}
```

**Persistence**: In-progress transfers are checkpointed to SQLite. Completed transfers
are removed; failed transfers are archived for diagnostics.

---

## State Transitions

### SyncPair Status Machine

```
           [user creates pair]
                  │
                  ▼
               Idle ──── [user pauses] ───► Paused
                │                              │
         [cycle starts]                 [user resumes]
                │                              │
                ▼                              │
            Scanning ◄─────────────────────────┘
                │
         [scan complete]
                │
                ▼
            Syncing
           /       \
  [all ops done]  [fatal error]
       │                │
       ▼                ▼
     Idle             Error
```

### JournalEntry SyncStatus Transitions

```
   [item discovered]
         │
         ▼
     PendingUpload / PendingDownload
         │
   [op attempted]
         │
    ┌────┴────┐
    │         │
 success   failure
    │         │
    ▼         ▼
  Synced    Error (retry_count++)
              │
     [retry_count > max]
              │
              ▼
    Error (parked, surfaced to user)
```

---

## Relationships

```
Account ──────────────────── has many ──── SyncPair
SyncPair ─────────────────── has many ──── JournalEntry
SyncPair ─────────────────── has many ──── ConflictRecord
SyncPair ─────────────────── has at most N ─ Transfer (active)
JournalEntry ─────────────── tracks ────── SyncItem (runtime)
OperationPlan ────────────── produced from ─ (LocalSnapshot, RemoteSnapshot, Journal)
ConflictRecord ──────────── produced from ─ OperationPlan.Conflict entries
```

---

## SQLite Schema (canonical)

```sql
CREATE TABLE accounts (
    id          TEXT PRIMARY KEY,   -- UUID
    server_url  TEXT NOT NULL,
    username    TEXT NOT NULL,
    display_name TEXT,
    auth_method TEXT NOT NULL,      -- "app_password" | "oauth2"
    status      TEXT NOT NULL,      -- "connected" | "auth_required" | "offline"
    capabilities_json TEXT,
    created_at  TEXT NOT NULL       -- ISO 8601
);

CREATE TABLE sync_pairs (
    id              TEXT PRIMARY KEY,
    account_id      TEXT NOT NULL REFERENCES accounts(id),
    local_root      TEXT NOT NULL,
    remote_root     TEXT NOT NULL,
    status          TEXT NOT NULL,
    conflict_policy TEXT NOT NULL,
    config_json     TEXT NOT NULL,   -- serialized SyncPairConfig (excludes, bandwidth, etc.)
    created_at      TEXT NOT NULL,
    last_synced_at  TEXT
);

CREATE TABLE journal_entries (
    pair_id         TEXT NOT NULL,
    relative_path   TEXT NOT NULL,
    item_type       TEXT NOT NULL,   -- "file" | "directory"
    size_bytes      INTEGER,
    local_mtime     TEXT,
    local_checksum  TEXT,            -- "<algorithm>:<hex>"
    remote_etag     TEXT,
    server_file_id  TEXT,
    last_synced_at  TEXT,
    sync_status     TEXT NOT NULL,
    sync_status_detail TEXT,         -- JSON for error details
    PRIMARY KEY (pair_id, relative_path),
    FOREIGN KEY (pair_id) REFERENCES sync_pairs(id)
);

CREATE TABLE conflict_records (
    id              TEXT PRIMARY KEY,
    pair_id         TEXT NOT NULL REFERENCES sync_pairs(id),
    item_path       TEXT NOT NULL,
    detected_at     TEXT NOT NULL,
    local_mtime     TEXT NOT NULL,
    remote_etag     TEXT NOT NULL,
    policy_applied  TEXT NOT NULL,
    outcome_json    TEXT NOT NULL
);

CREATE TABLE transfers (
    id                  TEXT PRIMARY KEY,
    pair_id             TEXT NOT NULL REFERENCES sync_pairs(id),
    direction           TEXT NOT NULL,
    relative_path       TEXT NOT NULL,
    total_bytes         INTEGER NOT NULL,
    transferred_bytes   INTEGER NOT NULL DEFAULT 0,
    next_chunk_index    INTEGER,
    upload_session_id   TEXT,
    started_at          TEXT NOT NULL,
    status              TEXT NOT NULL,
    error_detail        TEXT
);

CREATE INDEX idx_journal_pair ON journal_entries(pair_id);
CREATE INDEX idx_journal_file_id ON journal_entries(server_file_id) WHERE server_file_id IS NOT NULL;
CREATE INDEX idx_conflicts_pair ON conflict_records(pair_id);
CREATE INDEX idx_transfers_pair ON transfers(pair_id, status);
```
