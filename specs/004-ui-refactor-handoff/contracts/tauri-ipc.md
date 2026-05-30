# Tauri IPC Contracts: UI Refactor — Design Handoff Implementation

**Feature**: 004-ui-refactor-handoff
**Date**: 2026-05-25

---

## Overview

This document defines the IPC boundary between the Svelte frontend and the Rust backend for this feature. It covers:

1. **Existing commands** whose TypeScript bindings need to be added/updated in `lib/tauri.ts`
2. **New commands** that must be implemented in Rust

All commands are invoked via `invoke(command_name, args)` from the frontend.

---

## Existing Commands (Bindings Required / Already Exist)

These commands exist in Rust and only need TypeScript binding declarations added or verified in `src-ui/src/lib/tauri.ts`.

### `list_accounts() → AccountDto[]`

```typescript
export interface AccountDto {
  id: string;
  display_name: string;
  server_url: string;
  username: string;
}

export async function listAccounts(): Promise<AccountDto[]> {
  return await invoke('list_accounts');
}
```

### `list_pairs() → PairDto[]`

```typescript
export interface PairDto {
  id: string;
  account_id: string;
  local_root: string;
  remote_root: string;
  scan_interval_secs: number;
  selective_paths: string[];
}

export async function listPairs(): Promise<PairDto[]> {
  return await invoke('list_pairs');
}
```

### `list_synced_files(pair_id, relative_path?) → FileStatusDto[]`

```typescript
export interface FileStatusDto {
  path: string;
  name: string;
  is_dir: boolean;
  size: number | null;
  mtime: number | null;
  status: 'ok' | 'sync' | 'cloud' | 'pin' | 'conflict';
  etag?: string;
  share_count?: number;
  item_count?: number;
}

export async function listSyncedFiles(
  pairId: string,
  relativePath?: string
): Promise<FileStatusDto[]> {
  return await invoke('list_synced_files', { pairId, relativePath });
}
```

### `get_status() → SyncStatusDto`

```typescript
export interface SyncStatusDto {
  status: 'idle' | 'syncing' | 'paused' | 'error';
  active_file_count: number;
  total_bytes: number;
  transferred_bytes: number;
  eta_seconds: number | null;
  last_sync_at: number | null;
}

export async function getStatus(): Promise<SyncStatusDto> {
  return await invoke('get_status');
}
```

### `get_activity_log(limit?) → ActivityEntryDto[]` (extended)

The existing command is extended with an optional `filter` parameter.

```typescript
export interface ActivityEntryDto {
  id: string;
  who: string;
  verb: string;
  target: string;
  with_whom: string | null;
  where_path: string;
  at: number;              // unix ms
  kind: 'edit' | 'share' | 'sync' | 'conflict';
}

export async function getActivityLog(
  limit?: number,
  filter?: 'edit' | 'share' | 'sync' | 'conflict'
): Promise<ActivityEntryDto[]> {
  return await invoke('get_activity_log', { limit, filter });
}
```

**Rust change required**: Add `filter: Option<String>` parameter to the existing `get_activity_log` command. The filter is applied before returning results.

### `pause_sync() → void`

```typescript
export async function pauseSync(): Promise<void> {
  return await invoke('pause_sync');
}
```

### `resume_sync() → void`

```typescript
export async function resumeSync(): Promise<void> {
  return await invoke('resume_sync');
}
```

### `trigger_sync(pair_id) → void`

```typescript
export async function triggerSync(pairId: string): Promise<void> {
  return await invoke('trigger_sync', { pairId });
}
```

### `get_error_items(pair_id) → ErrorItemDto[]`

```typescript
export interface ErrorItemDto {
  path: string;
  message: string;
  retry_count: number;
}

export async function getErrorItems(pairId: string): Promise<ErrorItemDto[]> {
  return await invoke('get_error_items', { pairId });
}
```

### `list_conflicts(pair_id) → ConflictDto[]`

```typescript
export interface ConflictDto {
  id: string;
  pair_id: string;
  path: string;
  local_mtime: number;
  remote_mtime: number;
  resolved: boolean;
  resolution?: 'local' | 'remote';
}

export async function listConflicts(pairId: string): Promise<ConflictDto[]> {
  return await invoke('list_conflicts', { pairId });
}
```

### `resolve_conflict(id, side) → void`

```typescript
export async function resolveConflict(
  id: string,
  side: 'local' | 'remote'
): Promise<void> {
  return await invoke('resolve_conflict', { id, side });
}
```

---

## New Commands (Must Be Implemented)

### `search_users(account_id, query) → UserSearchResult[]`

**Purpose**: Searches the Nextcloud user/group directory for recipients to add to a share. Called from the Share dialog as the user types.

**Frontend binding**:
```typescript
export interface UserSearchResult {
  user_id: string;
  display_name: string;
}

export async function searchUsers(
  accountId: string,
  query: string
): Promise<UserSearchResult[]> {
  return await invoke('search_users', { accountId, query });
}
```

**Rust implementation** (`crates/adagio-desktop/src/commands/sharing.rs`):
```rust
#[tauri::command]
pub async fn search_users(
    state: tauri::State<'_, AppState>,
    account_id: String,
    query: String,
) -> Result<Vec<UserSearchResultDto>, String> { ... }
```

**Nextcloud OCS call**: `GET /ocs/v2.php/apps/files_sharing/api/v1/sharees?format=json&search={query}&itemType=file`

Response path: `ocs.data.users[].value.shareWith` + `ocs.data.users[].label`

**Error cases**:
- Account not found → `Err("Account not found")`
- Network error → `Err("Search failed: {message}")`
- Empty query → returns `Ok(vec![])` immediately without network call

---

### `create_share(request) → ShareResult`

**Purpose**: Creates a Nextcloud share (user share + public link) for a file or folder.

**Frontend binding**:
```typescript
export interface CreateShareRequest {
  account_id: string;
  path: string;              // path on the Nextcloud server (remote path)
  recipients: Array<{ user_id: string; permission: number }>;
  expiry_date?: string;      // ISO date string or null
  link_password?: string;
  hide_download: boolean;
  notify_on_open: boolean;
  note?: string;
}

export interface ShareResult {
  share_id: string;
  share_url: string;
}

export async function createShare(
  request: CreateShareRequest
): Promise<ShareResult> {
  return await invoke('create_share', { request });
}
```

**Nextcloud permission bit mapping** (Nextcloud uses bitmask permissions):
- view: `1` (READ)
- comment: `17` (READ + COMMENT)
- edit: `31` (READ + UPDATE + CREATE + DELETE + COMMENT)

**Nextcloud OCS call**: `POST /ocs/v2.php/apps/files_sharing/api/v1/shares`

Request body (form data):
```
path={path}&shareType=0&shareWith={userId}&permissions={permission}
```

For public link:
```
path={path}&shareType=3&permissions=1
```

**Rust implementation** (`crates/adagio-desktop/src/commands/sharing.rs`):
```rust
#[tauri::command]
pub async fn create_share(
    state: tauri::State<'_, AppState>,
    request: CreateShareRequestDto,
) -> Result<ShareResultDto, String> { ... }
```

**Error cases**:
- Account not found → `Err("Account not found")`
- Path not found on server → `Err("File not found on server")`
- Permission denied → `Err("You don't have permission to share this item")`
- Network error → `Err("Share failed: {message}")`

---

### `get_palette() → string`

**Purpose**: Reads the persisted palette name from `config.json`.

**Frontend binding**:
```typescript
export async function getPalette(): Promise<string> {
  return await invoke('get_palette');
}
```

**Rust implementation** (`crates/adagio-desktop/src/commands/prefs.rs`):
```rust
#[tauri::command]
pub async fn get_palette(
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    // Reads state.config.palette or returns "sienna" as default
}
```

Returns: palette name string (e.g., `"sienna"`, `"ink"`). Returns `"sienna"` if not set.

---

### `set_palette(name) → void`

**Purpose**: Persists the selected palette name to `config.json`.

**Frontend binding**:
```typescript
export async function setPalette(name: string): Promise<void> {
  return await invoke('set_palette', { name });
}
```

**Rust implementation** (`crates/adagio-desktop/src/commands/prefs.rs`):
```rust
#[tauri::command]
pub async fn set_palette(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    // Validates palette name, updates state.config.palette, calls save_config
}
```

**Valid values**: `sienna | sage | slate | ocean | forest | rose | ink | dusk`
**Error cases**: Invalid palette name → `Err("Unknown palette: {name}")`

---

## New Rust Modules Required

| Module | Location | Commands |
|--------|----------|----------|
| `sharing` | `crates/adagio-desktop/src/commands/sharing.rs` | `search_users`, `create_share` |
| `prefs` | `crates/adagio-desktop/src/commands/prefs.rs` | `get_palette`, `set_palette` |

Both modules follow the existing command module pattern (state access via `tauri::State<AppState>`, error type `String`, tracing instrumentation).

## Nextcloud API Layer Extensions

| Module | Location | New Functions |
|--------|----------|--------------|
| `sharing` | `crates/adagio-nextcloud/src/sharing.rs` | `search_sharees`, `create_share` |

The `adagio-nextcloud` sharing module calls OCS endpoints authenticated with Bearer token (same pattern as `fetch_user_info` in `auth.rs`).

## Command Registration

All new commands must be added to the `.invoke_handler()` chain in `crates/adagio-desktop/src/lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
  // ... existing commands ...
  commands::sharing::search_users,
  commands::sharing::create_share,
  commands::prefs::get_palette,
  commands::prefs::set_palette,
])
```

---

## Tray Window IPC Contract

The tray window (when rendered at `/?tray=1`) uses the same set of commands as the main window. No tray-specific IPC is needed — the tray reads state via `getStatus()` and `getActivityLog(3)`, and calls `pauseSync()` / `resumeSync()` / `triggerSync()` / `openSyncFolder()` directly.

`openSyncFolder()` and `openInBrowser()` are handled via `@tauri-apps/plugin-shell`'s `open()` API (already installed) — not via a custom Tauri command.

---

## Frontend Polling Strategy

Since Tauri 2 does not yet have a built-in push event mechanism for sync state, the frontend polls:

| Data | Interval | Command |
|------|----------|---------|
| Sync status (sidebar footer, status bar, tray header) | 2s when focused | `get_status` |
| File list (file browser) | on-navigate + 5s when focused | `list_synced_files` |
| Activity feed | 10s when Activity tab open | `get_activity_log` |
| Pair list (pinned folders) | 10s | `list_pairs` |

All polling is paused when the window loses focus (`document.addEventListener('visibilitychange')`).
