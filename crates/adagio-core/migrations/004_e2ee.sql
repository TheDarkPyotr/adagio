-- E2EE: per-account key state (one row per Nextcloud account)
CREATE TABLE IF NOT EXISTS e2ee_account_keys (
    account_id      TEXT NOT NULL PRIMARY KEY,
    -- PEM certificate returned by Nextcloud server CA after CSR submission.
    certificate     TEXT NOT NULL,
    -- Hex-encoded SHA-256 fingerprint of the RSA public key.
    key_fingerprint TEXT NOT NULL,
    -- ISO-8601 UTC timestamp when the key pair was first paired on this device.
    paired_at       TEXT NOT NULL,
    created_at      TEXT NOT NULL
);

-- E2EE: per-pair metadata sync state (one row per E2EE-enabled sync pair)
CREATE TABLE IF NOT EXISTS e2ee_folder_state (
    pair_id          TEXT NOT NULL PRIMARY KEY,
    -- Nextcloud WebDAV file-ID of the E2EE root folder (numeric string).
    folder_id        TEXT NOT NULL DEFAULT '',
    -- Metadata format version string reported by server ("2.0", "1.0", …).
    metadata_version TEXT NOT NULL DEFAULT '2.0',
    -- Last-known committed counter; next write must use counter + 1.
    counter          INTEGER NOT NULL DEFAULT 0,
    -- Space-separated hex-encoded SHA-256 checksums of all metadata keys ever used.
    key_checksums    TEXT NOT NULL DEFAULT '',
    updated_at       TEXT NOT NULL
);
