-- Registered Nextcloud accounts
-- Note: journal_mode=WAL, foreign_keys=ON, synchronous=NORMAL are set via
-- SqliteConnectOptions, not here, because PRAGMAs cannot run inside a transaction.
-- Credentials are stored in the OS keychain; only the service key reference is here
CREATE TABLE IF NOT EXISTS accounts (
    id                  TEXT PRIMARY KEY NOT NULL,
    display_name        TEXT NOT NULL,
    server_url          TEXT NOT NULL,
    username            TEXT NOT NULL,
    keychain_service_key TEXT NOT NULL,
    created_at          TEXT NOT NULL  -- ISO-8601 UTC
);

-- Sync pairs: local directory <-> remote directory
CREATE TABLE IF NOT EXISTS sync_pairs (
    id              TEXT PRIMARY KEY NOT NULL,
    account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    local_root      TEXT NOT NULL,
    remote_root     TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'idle',
    exclude_patterns TEXT NOT NULL DEFAULT '[]',  -- JSON array
    selective_paths  TEXT NOT NULL DEFAULT '[]',  -- JSON array
    created_at      TEXT NOT NULL,
    last_synced_at  TEXT
);

CREATE INDEX IF NOT EXISTS idx_sync_pairs_account ON sync_pairs(account_id);

-- Journal: last-known-synced state per item per pair
CREATE TABLE IF NOT EXISTS journal_entries (
    pair_id         TEXT NOT NULL REFERENCES sync_pairs(id) ON DELETE CASCADE,
    path            TEXT NOT NULL,
    file_id         TEXT,
    etag            TEXT,
    checksum_algo   TEXT,   -- 'SHA256' | 'MD5' | NULL
    checksum_value  TEXT,
    size            INTEGER NOT NULL DEFAULT 0,
    mtime_local     TEXT,   -- ISO-8601 UTC
    mtime_remote    TEXT,
    status          TEXT NOT NULL DEFAULT 'synced',
    error_message   TEXT,
    retry_count     INTEGER NOT NULL DEFAULT 0,
    updated_at      TEXT NOT NULL,
    PRIMARY KEY (pair_id, path)
);

CREATE INDEX IF NOT EXISTS idx_journal_file_id ON journal_entries(pair_id, file_id)
    WHERE file_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_journal_status ON journal_entries(pair_id, status);

-- Conflict records
CREATE TABLE IF NOT EXISTS conflict_records (
    id              TEXT PRIMARY KEY NOT NULL,
    pair_id         TEXT NOT NULL REFERENCES sync_pairs(id) ON DELETE CASCADE,
    path            TEXT NOT NULL,
    local_mtime     TEXT NOT NULL,
    remote_mtime    TEXT NOT NULL,
    local_size      INTEGER NOT NULL,
    remote_size     INTEGER NOT NULL,
    policy          TEXT NOT NULL,
    resolution      TEXT,   -- JSON | NULL
    detected_at     TEXT NOT NULL,
    resolved_at     TEXT
);

CREATE INDEX IF NOT EXISTS idx_conflicts_pair ON conflict_records(pair_id, detected_at DESC);
CREATE INDEX IF NOT EXISTS idx_conflicts_unresolved ON conflict_records(pair_id)
    WHERE resolution IS NULL;

-- Active and historical transfer records
CREATE TABLE IF NOT EXISTS transfers (
    id              TEXT PRIMARY KEY NOT NULL,
    pair_id         TEXT NOT NULL REFERENCES sync_pairs(id) ON DELETE CASCADE,
    path            TEXT NOT NULL,
    direction       TEXT NOT NULL,  -- 'upload' | 'download'
    status          TEXT NOT NULL DEFAULT 'pending',
    bytes_total     INTEGER NOT NULL DEFAULT 0,
    bytes_done      INTEGER NOT NULL DEFAULT 0,
    session_url     TEXT,   -- chunked upload session URL
    started_at      TEXT NOT NULL,
    completed_at    TEXT
);

CREATE INDEX IF NOT EXISTS idx_transfers_pair ON transfers(pair_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_transfers_active ON transfers(pair_id)
    WHERE status IN ('pending', 'in_progress', 'interrupted');
