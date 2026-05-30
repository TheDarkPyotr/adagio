# Data Model: Conflict Resolution Wizard (005)

## Modified Entities

### ConflictRecord (adagio-core/src/types.rs)

**Existing fields (unchanged):**

| Field | Type | Description |
|---|---|---|
| `id` | `String` | UUID — unique conflict identifier |
| `pair_id` | `PairId` | The sync pair this conflict belongs to |
| `path` | `RelativePath` | Path of the conflicting file or folder relative to the sync root |
| `local_mtime` | `DateTime<Utc>` | Last-modified time of the local version |
| `remote_mtime` | `DateTime<Utc>` | Last-modified time of the remote version |
| `local_size` | `u64` | Byte size of the local version |
| `remote_size` | `u64` | Byte size of the remote version |
| `policy` | `ConflictPolicy` | The pair's configured auto-resolution policy |
| `resolution` | `Option<ConflictResolution>` | `None` = pending; `Some(...)` = resolved |
| `detected_at` | `DateTime<Utc>` | When the conflict was first detected |
| `resolved_at` | `Option<DateTime<Utc>>` | When the user resolved it |

**New fields:**

| Field | Type | Description |
|---|---|---|
| `is_dir` | `bool` | `true` if the conflict is on a directory (folder conflict); `false` for files |
| `conflict_kind` | `ConflictKind` | Sub-type of conflict (see enum below) |

---

### ConflictKind (new enum, adagio-core/src/types.rs)

```
ContentModified   — both sides modified the file content (classic file conflict)
RenamedBothSides  — the folder was renamed differently on each side
DeletedWithContent — item deleted on one side; other side has new content/children
```

---

### ConflictSide (adagio-core/src/types.rs) — modified

Add `Both` variant:

```
Local   — user chose to keep the local version
Remote  — user chose to keep the remote version
Both    — user chose to keep both versions (creates a conflict copy)
```

---

### ConflictDto (adagio-desktop/src/commands/conflicts.rs)

**New fields mirroring ConflictRecord additions:**

| Field | Type | Description |
|---|---|---|
| `is_dir` | `bool` | Mirrors `ConflictRecord.is_dir` |
| `conflict_kind` | `String` | Serialised `ConflictKind` |

---

## Unchanged Entities

### ConflictResolution (no change)

```
KeptLocal                              — local version is canonical
KeptRemote                             — remote version is canonical
BothKept { conflict_copy_path }        — both versions preserved; copy path recorded
```

### ConflictPolicy (no change)

```
PreserveBoth / LocalWins / RemoteWins / NewestWins / Ask
```

---

## State Transitions

```
Conflict lifecycle:

  [detected by propagator]
        │
        ▼
  resolution = None            ← "pending" — shown in wizard, badge visible
        │
   user picks side
        │
        ▼
  resolution = Some(...)       ← "resolved" — badge count decremented
  resolved_at = Some(now)
```

Conflicts are never deleted from the journal — they are permanently retained as a historical log. The UI filters `resolution.is_none()` to find pending items.

---

## Storage

No new database tables. `ConflictRecord` maps to the existing `conflicts` table in `adagio.db` (SQLite). The two new columns (`is_dir`, `conflict_kind`) require an additive SQLite migration (ALTER TABLE ADD COLUMN with defaults: `is_dir = 0`, `conflict_kind = 'content_modified'`).
