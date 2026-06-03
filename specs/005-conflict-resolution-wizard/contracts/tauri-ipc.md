# IPC Contracts: Conflict Resolution Wizard (005)

All commands are invoked via `@tauri-apps/api/core` `invoke(cmd, args)`.

---

## Modified Commands

### `list_conflicts`

**Already registered.** No signature change — only the response DTO gains two new fields.

```
invoke("list_conflicts", { pairId: string }) → ConflictDto[]
```

**ConflictDto** (updated):
```typescript
interface ConflictDto {
  id: string;
  pair_id: string;
  path: string;
  local_mtime: string;   // ISO-8601
  remote_mtime: string;  // ISO-8601
  local_size: number;    // bytes
  remote_size: number;   // bytes
  policy: string;        // e.g. "preserve_both" | "local_wins" | "remote_wins" | "newest_wins" | "ask"
  resolution: string | null;  // null = pending; "kept_local" | "kept_remote" | "both_kept{...}"
  detected_at: string;   // ISO-8601
  resolved_at: string | null;
  is_dir: boolean;       // NEW: true for folder conflicts
  conflict_kind: string; // NEW: "content_modified" | "renamed_both_sides" | "deleted_with_content"
}
```

**Filtering for pending conflicts** (frontend):
```typescript
const pending = conflicts.filter(c => c.resolution === null);
```

---

### `resolve_conflict`

**Already registered.** Signature unchanged — behaviour significantly extended.

```
invoke("resolve_conflict", { id: string, side: "local" | "remote" | "both" }) → void
```

**Previous behaviour** (broken): only updated the journal; did not perform file I/O; "both" returned an error.

**New behaviour**:
1. Fetches the `ConflictRecord` from the journal.
2. Fetches the `SyncPair` and associated `Account`.
3. Retrieves credentials from the OS keychain via `spawn_blocking`.
4. Builds a `NextcloudClient`.
5. Executes the appropriate file operation:
   - `"local"` → uploads local file to remote path (`upload_single`).
   - `"remote"` → downloads remote file to local path (`download_file`).
   - `"both"` → runs `PreserveBoth` logic: renames local to conflict-copy path, downloads remote, uploads the conflict copy.
6. Persists the resulting `ConflictResolution` to the journal.
7. Emits `adagio://conflict-resolved` event with updated pending count.

**Error conditions**:
- Conflict ID not found → `"conflict not found: {id}"`
- Pair or account not found → `"pair/account not found for conflict {id}"`
- Keychain error → `"keychain error: {detail}"`
- File I/O or network error → `"transfer error: {detail}"`

---

## New Tauri Events (push, no invoke)

### `adagio://conflict-detected`

Emitted by the Rust propagator each time a new conflict is written to the journal.

**Payload**:
```typescript
{ pending_count: number }  // total unresolved conflicts across all pairs
```

**React listener**:
```typescript
import { listen } from '@tauri-apps/api/event';
const unlisten = await listen<{ pending_count: number }>(
  'adagio://conflict-detected',
  (event) => setPendingConflicts(event.payload.pending_count)
);
```

### `adagio://conflict-resolved`

Emitted by the Rust command after `resolve_conflict` succeeds.

**Payload**:
```typescript
{
  id: string;
  pending_count: number;  // remaining unresolved conflicts
}
```

---

## Unchanged Commands (context only)

`list_pairs`, `get_status`, `get_activity_log` — unchanged; used by existing UI that hosts the badge.
