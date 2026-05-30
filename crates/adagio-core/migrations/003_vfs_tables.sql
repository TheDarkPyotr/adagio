-- VFS on-demand files (Feature 012) — migration 003
-- Adds vfs_cache_metadata and vfs_pinned_paths tables.
-- Existing tables are unchanged.

CREATE TABLE IF NOT EXISTS vfs_cache_metadata (
    pair_id          TEXT NOT NULL REFERENCES sync_pairs(id) ON DELETE CASCADE,
    path             TEXT NOT NULL,
    remote_size      INTEGER NOT NULL DEFAULT 0,
    remote_etag      TEXT,
    remote_mtime     TEXT NOT NULL DEFAULT '',
    state            TEXT NOT NULL DEFAULT 'cloud_only',
    cached_at        TEXT,
    last_accessed_at TEXT,
    cache_bytes      INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (pair_id, path)
);

-- LRU eviction: order by last_accessed_at for locally_available files.
CREATE INDEX IF NOT EXISTS idx_vfs_lru
    ON vfs_cache_metadata (pair_id, last_accessed_at)
    WHERE state = 'locally_available';

-- Fast state lookups.
CREATE INDEX IF NOT EXISTS idx_vfs_state
    ON vfs_cache_metadata (pair_id, state);

-- User-pinned paths (files or directory prefixes).
CREATE TABLE IF NOT EXISTS vfs_pinned_paths (
    pair_id   TEXT NOT NULL REFERENCES sync_pairs(id) ON DELETE CASCADE,
    path      TEXT NOT NULL,
    pinned_at TEXT NOT NULL,
    PRIMARY KEY (pair_id, path)
);
