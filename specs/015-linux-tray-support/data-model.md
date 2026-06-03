# Data Model: Linux System Tray Support

**Feature**: 015-linux-tray-support
**Date**: 2026-06-03

This feature introduces no new persistent data entities. It consumes existing daemon DTOs and introduces one new Tauri command. The entities below are reproduced here for reference to make the tray window's data dependencies explicit.

---

## Consumed Entities

### SyncStatusDto (existing — `tauri.ts`)

| Field | Type | Description |
|-------|------|-------------|
| `status` | `'idle' \| 'syncing' \| 'paused' \| 'error' \| 'maintenance' \| 'unreachable'` | Current sync engine state |
| `active_file_count` | `number` | Files currently being transferred (only meaningful when `status = 'syncing'`) |
| `total_bytes` | `number` | Total bytes managed in this sync pair |
| `transferred_bytes` | `number` | Bytes transferred in the current session |
| `eta_seconds` | `number \| null` | Estimated seconds to complete current sync run |
| `last_sync_at` | `number \| null` | Unix timestamp (ms) of last completed sync cycle |

**Tray usage**: `status` drives the headline and status dot. `active_file_count` appears in the "Syncing N files" headline. `last_sync_at` formats to `HH:MM` in the status subtitle. `total_bytes` shows as GB.

---

### ActivityEntryDto (existing — `tauri.ts`)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | Unique entry identifier |
| `kind` | `'edit' \| 'share' \| 'sync' \| 'conflict' \| 'add' \| 'remove'` | Activity type |
| `target` | `string` | File path or folder name affected |
| `at` | `number` | Unix timestamp (ms) of the event |

**Tray usage**: The 3 most-recent entries appear in the "Recent" section. `kind` selects the icon glyph. `target` is truncated for display. `at` is formatted as relative time ("4 min ago").

---

### AccountDto (existing — `tauri.ts`)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | Account UUID |
| `display_name` | `string` | User-facing account name |
| `server_url` | `string` | Nextcloud instance URL |
| `username` | `string` | Nextcloud username |

**Tray usage**: `server_url` is the target for "Open in browser".

---

### PairDto (existing — `tauri.ts`)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `string` | Pair UUID |
| `local_root` | `string` | Absolute local filesystem path |
| `remote_root` | `string` | Remote WebDAV path |

**Tray usage**: `local_root` is the target for "Open Adagio folder".

---

## New Entity: Platform string (ephemeral, in-memory)

A new Tauri command `get_platform` returns one of three string literals:

| Value | Meaning |
|-------|---------|
| `"linux"` | Running on a Linux host |
| `"macos"` | Running on a macOS host |
| `"windows"` | Running on a Windows host |

Not persisted; read once at tray window mount time.

---

## Tray Window Configuration (tauri.conf.json — new window entry)

| Property | Value | Notes |
|----------|-------|-------|
| `label` | `"tray"` | Used by `get_webview_window("tray")` in Rust |
| `url` | `"index.html?tray"` | `IS_TRAY` flag in App.tsx routes to TrayPopover |
| `width` | `380` | Matches design spec |
| `height` | `520` | Sufficient for 3 recent rows + actions + footer |
| `visible` | `false` | Hidden at startup; toggled by tray icon click |
| `decorations` | `false` | No title bar or window chrome |
| `alwaysOnTop` | `true` | Must float above other windows |
| `resizable` | `false` | Fixed-size popover |
| `skipTaskbar` | `true` | Must not appear in taskbar/dock |

> `shadow` and `focus` are NOT included — these are not valid Tauri 2 cross-platform window config fields. Drop shadow is handled via CSS `box-shadow` on the popover root; focus is controlled at runtime via `tray_win.set_focus()`.
