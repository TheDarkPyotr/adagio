# Data Model: VFS On-demand Files (012)

## New database tables (SQLite migration 003)

### `vfs_cache_metadata`

Tracks download state and LRU eviction order for every file in a VFS-mode pair.

```sql
CREATE TABLE IF NOT EXISTS vfs_cache_metadata (
    pair_id          TEXT NOT NULL REFERENCES sync_pairs(id) ON DELETE CASCADE,
    path             TEXT NOT NULL,        -- relative path within the pair
    remote_size      INTEGER NOT NULL,     -- file size on Nextcloud (bytes)
    remote_etag      TEXT,                 -- current remote version tag
    remote_mtime     TEXT NOT NULL,        -- ISO-8601 UTC; shown in file manager
    state            TEXT NOT NULL DEFAULT 'cloud_only',
                                           -- cloud_only | locally_available | pinned
    cached_at        TEXT,                 -- ISO-8601 UTC; null if cloud_only
    last_accessed_at TEXT,                 -- ISO-8601 UTC; updated on every read()
    cache_bytes      INTEGER NOT NULL DEFAULT 0,
                                           -- bytes of local content (0 if cloud_only)
    PRIMARY KEY (pair_id, path)
);

CREATE INDEX IF NOT EXISTS idx_vfs_lru
    ON vfs_cache_metadata(pair_id, last_accessed_at)
    WHERE state = 'locally_available';

CREATE INDEX IF NOT EXISTS idx_vfs_state
    ON vfs_cache_metadata(pair_id, state);
```

**State transitions:**
```
cloud_only ──(read())──→ locally_available
locally_available ──(pin)──→ pinned
pinned ──(unpin)──→ locally_available
locally_available ──(evict/auto)──→ cloud_only
pinned ──(explicit evict)──→ cloud_only
```

---

### `vfs_pinned_paths`

Normalized list of user-pinned paths. A pinned directory entry covers all files within it.

```sql
CREATE TABLE IF NOT EXISTS vfs_pinned_paths (
    pair_id   TEXT NOT NULL REFERENCES sync_pairs(id) ON DELETE CASCADE,
    path      TEXT NOT NULL,        -- may be a file or a directory prefix
    pinned_at TEXT NOT NULL,        -- ISO-8601 UTC
    PRIMARY KEY (pair_id, path)
);
```

**Query pattern for "is path P pinned?":**
```sql
SELECT 1 FROM vfs_pinned_paths
WHERE pair_id = ?
  AND (path = ? OR ? LIKE (path || '/%'))
LIMIT 1;
```

---

## New / modified Rust types

### `VfsState` (enum, added to `adagio-core`)
```rust
pub enum VfsState {
    CloudOnly,
    LocallyAvailable { cached_at: DateTime<Utc>, last_accessed: DateTime<Utc> },
    Pinned           { cached_at: DateTime<Utc>, last_accessed: DateTime<Utc> },
}
```

### `VfsCacheEntry` (mirrors `vfs_cache_metadata` row)
```rust
pub struct VfsCacheEntry {
    pub pair_id:         PairId,
    pub path:            RelativePath,
    pub remote_size:     u64,
    pub remote_etag:     Option<String>,
    pub remote_mtime:    DateTime<Utc>,
    pub state:           VfsState,
    pub cache_bytes:     u64,
}
```

### `VfsStats` (runtime summary, not persisted)
```rust
pub struct VfsStats {
    pub pair_id:                  PairId,
    pub cloud_only_count:         u64,
    pub locally_available_count:  u64,
    pub pinned_count:             u64,
    pub cached_bytes:             u64,
    pub last_eviction_at:         Option<DateTime<Utc>>,
}
```

### Config additions to `SyncPair`
```rust
// Added fields with #[serde(default)]:
pub vfs_enabled:                bool,          // default false
pub vfs_cache_max_bytes:        u64,           // default 20 GiB
pub vfs_eviction_threshold_bytes: u64,         // default 5 GiB free-disk minimum
```

Same three fields mirrored on `SavedPair` in `adagio-desktop`.

---

## `VfsProvider` trait (platform abstraction)

```rust
/// Platform-specific VFS driver (FUSE3 / FileProvider / CfApi).
#[async_trait]
pub trait VfsProvider: Send + Sync {
    /// Mount the virtual folder and start serving requests.
    async fn mount(&self, mount_point: &Path, pair_id: &PairId) -> Result<(), VfsError>;
    /// Unmount and clean up kernel resources.
    async fn unmount(&self, pair_id: &PairId) -> Result<(), VfsError>;
    /// Populate placeholder entries for all cloud-only files.
    async fn update_placeholders(&self, entries: &[VfsCacheEntry]) -> Result<(), VfsError>;
    /// Mark a file as locally available (content now on disk).
    async fn set_locally_available(&self, path: &RelativePath) -> Result<(), VfsError>;
    /// Mark a file as pinned.
    async fn set_pinned(&self, path: &RelativePath) -> Result<(), VfsError>;
    /// Mark a file as cloud-only (content evicted).
    async fn set_cloud_only(&self, path: &RelativePath) -> Result<(), VfsError>;
}
```

---

## New IPC variants

```rust
// Added to DaemonRequest:
GetVfsStats   { pair_id: String },
SetVfsPin     { pair_id: String, path: String, pinned: bool },
EvictVfsFile  { pair_id: String, path: String },

// Internal (FUSE handler → daemon, not exposed via CLI):
FetchOnDemand { pair_id: String, path: String, offset: u64, length: u64 },
```

---

## Eviction algorithm

```
SELECT path, cache_bytes FROM vfs_cache_metadata
WHERE pair_id = ? AND state = 'locally_available'
ORDER BY last_accessed_at ASC;

-- evict entries until:
--   SUM(cache_bytes_removed) >= bytes_to_free
--   OR no more evictable entries remain
```

Runs when:
- Manual "Free up space" request
- Daemon detects free disk < `vfs_eviction_threshold_bytes` (checked after every sync cycle)
