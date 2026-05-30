# Data Model: UI Refactor — Design Handoff Implementation

**Feature**: 004-ui-refactor-handoff
**Date**: 2026-05-25

---

This document covers the **frontend data model** — the TypeScript types used by the Svelte component layer and the Tauri IPC boundary. Backend Rust types (in `adagio-core` and `adagio-nextcloud`) are not changed by this feature except where new IPC commands require new DTOs.

---

## 1. Design Token Types

### `PaletteName`

The eight selectable color palettes shipped with Adagio.

```typescript
type PaletteName =
  | 'sienna'   // canonical warm earth (default)
  | 'sage'
  | 'slate'
  | 'ocean'
  | 'forest'
  | 'rose'
  | 'ink'      // dark mode
  | 'dusk';
```

### `ThemeState`

Held in the `lib/stores/theme.ts` reactive store.

```typescript
interface ThemeState {
  palette: PaletteName;   // currently active palette
  isDark: boolean;         // true when palette is a dark palette (currently only 'ink')
  systemPrefersDark: boolean; // reflects OS prefers-color-scheme
}
```

**State transitions**:
- On app launch: read `get_palette()` from config. If null, check `systemPrefersDark` → set `'ink'` if true, `'sienna'` if false.
- On manual change: call `set_palette(name)` and update store.
- On OS preference change: if user has never manually set a palette, follow OS preference.

---

## 2. File Browser Types

### `SyncStatus`

Possible per-file sync states.

```typescript
type SyncStatus = 'ok' | 'sync' | 'cloud' | 'pin' | 'conflict';
```

| Value | Meaning | Visual indicator |
|-------|---------|-----------------|
| `ok` | Fully synced | 14px green circle with check |
| `sync` | Transfer in progress | 14px clay spinner ring |
| `cloud` | Cloud-only, not downloaded | 16px cloud outline, muted |
| `pin` | Pinned for offline use | 14px dark circle with cream check |
| `conflict` | Version conflict detected | 16px danger triangle |

### `FileKind`

File type used to select the file glyph icon.

```typescript
type FileKind =
  | 'folder'
  | 'pdf' | 'md' | 'fig' | 'zip' | 'svg' | 'wav' | 'txt'
  | string; // generic fallback
```

### `FileNode`

A single row in the file browser table.

```typescript
interface FileNode {
  path: string;           // full relative path from sync root
  name: string;           // display name (last segment)
  kind: FileKind;
  size: number | null;    // bytes; null for folders
  mtime: number;          // unix ms
  status: SyncStatus;
  shareCount?: number;    // number of active shares (shows people pill)
  itemCount?: number;     // child item count (folders only)
  etag?: string;
}
```

**Derived from**: `FileStatusDto` returned by `list_synced_files`. The existing `FileStatusDto` in `crates/adagio-desktop/src/commands/pair.rs` maps directly to this interface; the frontend type is a re-declaration for TypeScript clarity.

### `FileSortKey`

Sortable column identifiers.

```typescript
type FileSortKey = 'name' | 'size' | 'mtime' | 'status';
type SortDirection = 'asc' | 'desc';
```

### `FileBrowserState`

Held in `lib/stores/files.ts`.

```typescript
interface FileBrowserState {
  pairId: string;
  currentPath: string;          // relative path from pair root
  breadcrumbs: BreadcrumbSegment[];
  items: FileNode[];
  selectedPaths: Set<string>;
  sortKey: FileSortKey;
  sortDirection: SortDirection;
  loading: boolean;
  error: string | null;
}

interface BreadcrumbSegment {
  label: string;
  path: string;     // absolute path to navigate to
}
```

---

## 3. Activity Feed Types

### `ActivityKind`

Used for filter pill grouping.

```typescript
type ActivityKind = 'edit' | 'share' | 'sync' | 'conflict';
```

### `ActivityVerb`

```typescript
type ActivityVerb =
  | 'edited' | 'shared' | 'pulled' | 'flagged'
  | 'pinned' | 'added' | 'declined';
```

### `ActivityEvent`

A single entry in the activity feed. Maps to the extended `ActivityEntryDto`.

```typescript
interface ActivityEvent {
  id: string;
  who: string;           // display name or 'You' or 'Auto-sync' or 'Conflict'
  verb: ActivityVerb;
  target: string;        // file or folder name
  withWhom?: string;     // e.g., 'Yui Tanaka' (for share events)
  where: string;         // folder path, e.g., '/Sound'
  at: number;            // unix ms
  kind: ActivityKind;    // for filter pill
}
```

**Time buckets** (derived, not stored):
- `at` within today's date → "TODAY"
- `at` within yesterday → "YESTERDAY"
- `at` within last 7 days → "EARLIER THIS WEEK"
- older → "EARLIER"

### `ActivityFeedState`

Held in `lib/stores/activity.ts`.

```typescript
interface ActivityFeedState {
  events: ActivityEvent[];
  activeFilter: ActivityKind | 'all';
  filterCounts: Record<ActivityKind | 'all', number>;
  loading: boolean;
  error: string | null;
}
```

---

## 4. Share Dialog Types

### `SharePermission`

```typescript
type SharePermission = 'view' | 'comment' | 'edit';
```

### `ShareExpiry`

```typescript
type ShareExpiry = '24h' | '7d' | '30d' | 'none';
```

### `ShareRecipient`

A resolved recipient shown as a chip in the share dialog.

```typescript
interface ShareRecipient {
  userId: string;
  displayName: string;
  initials: string;        // 1-2 chars, derived from displayName
  permission: SharePermission;
}
```

### `UserSearchResult`

Returned by `search_users` for the autocomplete dropdown.

```typescript
interface UserSearchResult {
  userId: string;
  displayName: string;
  initials: string;
}
```

### `CreateShareRequest`

Sent to `create_share`.

```typescript
interface CreateShareRequest {
  accountId: string;
  path: string;
  recipients: Array<{ userId: string; permission: SharePermission }>;
  expiry: ShareExpiry;
  linkPassword?: string;
  hideDownload: boolean;
  notifyOnOpen: boolean;
  note?: string;
}
```

### `ShareResult`

Returned by `create_share`.

```typescript
interface ShareResult {
  shareId: string;
  shareUrl: string;
}
```

### `ShareDialogState`

Local component state (not a global store — modal is transient).

```typescript
interface ShareDialogState {
  targetPath: string;
  targetName: string;
  targetSize: number | null;
  targetMtime: number;
  recipients: ShareRecipient[];
  permission: SharePermission;
  expiry: ShareExpiry;
  shareUrl: string | null;
  linkPassword: string;
  hideDownload: boolean;
  notifyOnOpen: boolean;
  note: string;
  searchQuery: string;
  searchResults: UserSearchResult[];
  submitting: boolean;
  error: string | null;
}
```

---

## 5. Sync State Types

### `SyncStatusDto`

Already returned by `get_status`. Used by sidebar footer, status bar, and tray.

```typescript
// Existing — confirmed from commands/sync.rs
interface SyncStatusDto {
  status: 'idle' | 'syncing' | 'paused' | 'error';
  active_file_count: number;
  total_bytes: number;
  transferred_bytes: number;
  eta_seconds: number | null;
  last_sync_at: number | null;  // unix ms
}
```

### `PairSyncSummary`

Per-pair sync state for the sidebar's pinned-folder dots.

```typescript
interface PairSyncSummary {
  pairId: string;
  name: string;
  status: 'ok' | 'sync' | 'error';
}
```

**Derived from**: `list_pairs` + `get_status`. The pair list provides names; the global status plus `get_error_items` per pair gives per-folder state.

### `SyncState`

Global reactive state held in `lib/stores/sync.ts`.

```typescript
interface SyncState {
  overview: SyncStatusDto | null;
  pairs: PairDto[];
  errorItemsByPair: Record<string, ErrorItemDto[]>;
  lastUpdatedAt: number;
}
```

Refreshed by polling `get_status` + `list_pairs` every 2 seconds when window is focused.

---

## 6. Tray Types

### `TrayState`

Held in `lib/stores/tray.ts` (used only when running in tray-window mode).

```typescript
interface TrayState {
  syncStatus: SyncStatusDto | null;
  recentEvents: ActivityEvent[];   // last 3
  loading: boolean;
}
```

---

## 7. Account Types

### `AccountDto`

Already returned by `list_accounts`. Augmented with derived display fields.

```typescript
// Existing DTO (confirmed)
interface AccountDto {
  id: string;
  display_name: string;
  server_url: string;
  username: string;
}

// Derived UI fields (computed in the component)
interface AccountDisplayInfo {
  initial: string;          // first char of display_name, uppercase
  colorVar: string;         // CSS var name, e.g., '--forest' (assigned by index)
}
```

---

## 8. Preference Types

### `AppPreferences`

The subset of `config.json` that the frontend reads/writes for UI preferences.

```typescript
interface AppPreferences {
  palette: PaletteName;
}
```

**Persisted via**: new `get_palette(): Promise<string>` and `set_palette(name: string): Promise<void>` Tauri commands. The Rust side reads/writes the `palette` key in `config.json` using the existing `SavedConfig` struct.

---

## 9. Entity Relationships

```
Account (1) ──── (*) SyncPair (PairDto)
                        │
                        └── (*) FileNode  (via list_synced_files)
                        └── (*) ActivityEvent (via get_activity_log)
                        └── (*) ErrorItemDto (via get_error_items)

FileNode (1) ──── (0-1) ShareResult (via create_share)
FileNode (*) ──── ShareRecipient (via create_share request)

SyncStatusDto ──── aggregated from all active SyncPairs

ThemeState ──── stored in config.json (palette key)
              └── read from OS (systemPrefersDark)
```
