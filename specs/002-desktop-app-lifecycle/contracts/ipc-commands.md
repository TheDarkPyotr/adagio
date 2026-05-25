# IPC Command Contracts: Desktop App Lifecycle

**Feature**: 002-desktop-app-lifecycle
**Date**: 2026-05-24

These contracts define the Tauri IPC commands introduced or modified by this feature.
The frontend (Svelte) calls these commands via `invoke()`; the backend (Rust) implements them.

---

## New Commands

### `list_synced_files`

Returns the contents of a folder within a sync pair's local directory, enriched with
per-file sync status from the sync history.

**Request**:
```typescript
{
  pair_id: string,          // UUID of the sync pair
  relative_path?: string    // Sub-folder to list; omit or "" for the pair root
}
```

**Response** (`FileStatusDto[]`):
```typescript
{
  name: string,             // File or folder name
  relative_path: string,    // Path relative to the pair root
  is_dir: boolean,
  size: number,             // Bytes; 0 for directories
  modified_at: string,      // ISO-8601
  sync_status: "synced" | "pending_upload" | "pending_download" |
               "uploading" | "downloading" | "conflict" | "error" | "unknown",
  error_message?: string    // Only set for "error" and "conflict"
}
```

**Error**: Returns a string error message if the pair does not exist, the local folder
cannot be read, or the sync history is unavailable.

**Contract invariants**:
- The response MUST include all entries in the local folder (files and sub-directories)
- Entries with no journal record MUST return `sync_status: "unknown"`
- Directories MUST NOT have their contents recursively expanded in the response
- Results MUST be sorted: directories first, then files, each group alphabetically

---

## Modified Commands

### `create_pair` (modified)

Existing behaviour: creates a pair in memory.
**New behaviour**: additionally persists the full pair configuration to `config.json`.

No change to request or response shape.

**Post-condition**: `config.json` contains the new pair; restarting the app restores it.

---

### `delete_pair` (modified)

Existing behaviour: removes a pair from memory.
**New behaviour**: additionally removes the pair from `config.json` and stops its sync runner.

No change to request or response shape.

---

### `add_account` (modified)

Existing behaviour: stores credentials in the keychain, registers account in memory.
**New behaviour**: additionally persists the account metadata (no credentials) to `config.json`.

No change to request or response shape.

**Post-condition**: `config.json` contains the new account; restarting the app restores it.

---

### `remove_account` (modified)

Existing behaviour: deletes keychain credentials, removes from memory.
**New behaviour**: additionally removes the account from `config.json`.

No change to request or response shape.

---

### `trigger_sync` (modified)

Existing behaviour: returns last cached report (stub).
**New behaviour**: sends an immediate-trigger signal to the pair's background runner,
causing a sync cycle to start within 1 second.

No change to request or response shape.
Returns `Ok(())` immediately (the cycle runs asynchronously).

---

## Internal Startup Sequence (not IPC)

The following is not an IPC command but documents the startup contract between the
Tauri setup hook and the sync engine:

1. `AppState::new(config_dir)` — loads `config.json`; populates `accounts` and `pairs`
2. `lifecycle::start_engine_for_all_pairs(&state, &app_handle)` — for each loaded pair:
   a. Retrieves credentials from OS keychain via `spawn_blocking`
   b. Constructs a `NextcloudClient`
   c. Calls `engine.start_pair(pair, client, journal)` to spawn the runner
3. If any pair fails to start (e.g., missing credentials), it is skipped with a warning;
   other pairs continue to start normally
