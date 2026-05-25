# Data Model: Desktop App Lifecycle

**Feature**: 002-desktop-app-lifecycle
**Date**: 2026-05-24

---

## New Entities

### SavedConfig

The on-disk representation of all user configuration. Written to `config.json` in the
platform app-config directory. Never contains credentials (passwords / tokens).

| Field    | Type                 | Notes                                      |
|----------|----------------------|--------------------------------------------|
| version  | u32                  | Schema version for future migration support |
| accounts | Vec\<SavedAccount\>  | All registered Nextcloud accounts           |
| pairs    | Vec\<SavedPair\>     | All configured sync pairs                   |

---

### SavedAccount

A Nextcloud account as persisted on disk. The actual credential (app-password or token)
lives in the OS keychain; only the lookup key is stored here.

| Field               | Type   | Notes                                                 |
|---------------------|--------|-------------------------------------------------------|
| id                  | String | UUID; stable across restarts                          |
| display_name        | String | Human label shown in the UI                           |
| server_url          | String | Base URL of the Nextcloud server                      |
| username            | String | Nextcloud login name                                  |
| keychain_service_key| String | Key used to retrieve credentials from the OS keychain |

---

### SavedPair

A sync pair as persisted on disk. Maps one local folder to one remote Nextcloud folder.

| Field                    | Type         | Notes                                              |
|--------------------------|--------------|----------------------------------------------------|
| id                       | String       | UUID; stable across restarts                       |
| account_id               | String       | References a `SavedAccount.id`                     |
| local_root               | String       | Absolute path of the local sync folder             |
| remote_root              | String       | User-relative remote path (e.g. `Documents/Adagio`)|
| scan_interval_secs       | u64          | Seconds between automatic sync cycles; default 7200|
| scan_on_startup          | bool         | Whether to run a cycle immediately on app launch   |
| max_upload_concurrency   | usize        | Max simultaneous uploads; default 3                |
| max_download_concurrency | usize        | Max simultaneous downloads; default 3              |
| selective_paths          | Vec\<String\>| User-relative paths to include; empty = sync all   |
| exclude_patterns         | Vec\<String\>| Glob patterns to exclude from sync                 |

---

### PairRunner *(runtime only, not persisted)*

Manages the background sync loop for one pair. Lives in `adagio-core/src/cycle/runner.rs`.

| Field          | Type                   | Notes                                             |
|----------------|------------------------|---------------------------------------------------|
| pair_id        | PairId                 | Identifies which pair this runner serves          |
| cancel         | CancellationToken      | Cancelling stops the runner cleanly               |
| trigger_tx     | mpsc::Sender\<()\>     | Sending a message starts an immediate sync cycle  |

**State transitions**:
```
Created → Running (start_pair called) → Idle (waiting for interval/trigger)
                ↓                            ↓
            Syncing (SyncCycle::run)  ← trigger_pair / interval tick
                ↓
            Idle / Stopped (cancel token fired)
```

---

### FileStatusDto *(Tauri IPC response)*

Per-file entry returned by the `list_synced_files` command. Combines filesystem metadata
with journal sync state.

| Field         | Type            | Notes                                                          |
|---------------|-----------------|----------------------------------------------------------------|
| name          | String          | File or folder name (last path component)                      |
| relative_path | String          | Path relative to the sync pair root                            |
| is_dir        | bool            | True for directories                                           |
| size          | u64             | File size in bytes; 0 for directories                          |
| modified_at   | String          | ISO-8601 last-modified timestamp from the filesystem           |
| sync_status   | String          | One of: synced, pending_upload, pending_download, uploading, downloading, conflict, error, unknown |
| error_message | Option\<String\>| Set when sync_status is "error" or "conflict"                  |

---

## Modified Entities

### SyncPairManager (adagio-core)

Currently stores `HashMap<PairId, PathBuf>`. Extended to store full `SyncPair` structs.

**New methods added**:
- `register_full_pair(pair: SyncPair)` — replaces `register_pair(id, local_root)`
- `get_pair(id: &PairId) → Option<&SyncPair>`
- `all_pairs() → Vec<&SyncPair>`

**Backward compatibility**: `register_pair(id, local_root)` retained for existing callers but
marked deprecated; callers migrated to `register_full_pair`.

---

### DefaultSyncEngine (adagio-core)

Extended with per-pair runner management.

**New fields**:
- `runners: Arc<RwLock<HashMap<PairId, PairRunner>>>`

**New methods**:
- `start_pair(pair, client, journal)` — spawns a `PairRunner` background task
- `stop_pair(pair_id)` — cancels and removes the runner
- `trigger_pair(pair_id)` — sends to `trigger_tx` for an immediate cycle

**Updated methods**:
- `trigger_sync(pair_id)` — now delegates to `trigger_pair()` + returns last report
