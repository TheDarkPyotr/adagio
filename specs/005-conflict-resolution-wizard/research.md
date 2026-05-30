# Research: Conflict Resolution Wizard (005)

## Existing Infrastructure Inventory

The following already exists and can be reused:

| Asset | Location | Notes |
|---|---|---|
| `ConflictRecord` | `adagio-core/src/types.rs:270` | Missing `is_dir` field |
| `ConflictPolicy` enum | `adagio-core/src/types.rs:286` | PreserveBoth / LocalWins / RemoteWins / NewestWins / Ask |
| `ConflictResolution` enum | `adagio-core/src/types.rs:301` | KeptLocal / KeptRemote / BothKept |
| `ConflictSide` enum | `adagio-core/src/types.rs:309` | Local / Remote — **missing Both** |
| `conflict::resolve()` | `adagio-core/src/conflict.rs:57` | Performs actual file I/O per policy |
| `conflict_copy_path()` | `adagio-core/src/conflict.rs:23` | Naming for conflict copies |
| `Journal::list_conflicts` | `adagio-core/src/journal/mod.rs:76` | Returns all conflicts (resolved + pending) |
| `Journal::resolve_conflict` | `adagio-core/src/journal/mod.rs:69` | Persists resolution |
| `ConflictDto` | `adagio-desktop/src/commands/conflicts.rs:10` | IPC DTO |
| `list_conflicts` command | `adagio-desktop/src/commands/conflicts.rs:51` | Registered in lib.rs |
| `resolve_conflict` command | `adagio-desktop/src/commands/conflicts.rs:64` | **Broken: missing "both", no file I/O** |
| `NextcloudClient` construction pattern | `adagio-desktop/src/lifecycle.rs:71` | Keychain → client |
| Tauri event delivery | `adagio-desktop/src/lib.rs:33` | `app_handle.emit(...)` already used |

---

## Decision 1: Resolution Execution Model

**Question**: When the user picks a side in the wizard, should `resolve_conflict` perform the file operations immediately (synchronous in the command) or queue it for the propagator to process on the next sync cycle?

**Decision**: Immediate execution within the Tauri command.

**Rationale**: Conflict resolution is an explicit user action. The user clicks "Keep Local Version" and expects to see the conflict cleared immediately. Queuing the operation through the propagator would introduce indeterminate latency and require the UI to poll for completion — making the UX feel unreliable. The `NextcloudClient` construction pattern (`lifecycle.rs:71`) already demonstrates how commands obtain credentials and build a live client.

**How to apply**: `resolve_conflict` looks up the conflict record + pair + account, retrieves credentials from the keychain via `spawn_blocking`, builds a `NextcloudClient`, then calls `conflict::resolve()` with an override policy matching the user's chosen side. After the file ops succeed, marks the journal via `resolve_conflict`.

**Alternatives considered**:
- Propagator re-queue: More architecturally elegant but adds latency + polling complexity for a user-facing action.
- Mark-only (current broken state): Only updates the journal without performing file I/O — leaves files in an inconsistent state.

---

## Decision 2: "Both" Handling

**Question**: How should the wizard's "Keep Both" choice map to the existing `conflict::resolve()` machinery?

**Decision**: Add `ConflictSide::Both` to the `ConflictSide` enum, add handling in `conflict::resolve()`'s `Ask` branch, and also wire it directly in `resolve_conflict` (which bypasses the policy dispatch via an override).

**Rationale**: The `ConflictPolicy::PreserveBoth` branch in `conflict::resolve()` already implements the full "keep both" logic (rename local to conflict copy, download remote, upload copy). Reusing that code is correct. The `ConflictSide` enum needs `Both` so the user's choice can be conveyed idiomatically through the API.

**How to apply**: `resolve_conflict("both")` → constructs a temporary `ConflictRecord` with `policy: ConflictPolicy::PreserveBoth` and calls `conflict::resolve()`. (The pair's saved policy is irrelevant when the user has made an explicit choice.)

**Alternatives considered**:
- Duplicate `PreserveBoth` logic inline in the command — rejected (DRY, already tested).
- Add a separate `resolve_conflict_both` command — rejected (unnecessary proliferation).

---

## Decision 3: Folder Conflict Support

**Question**: What does "folder conflict" mean concretely, and how is it detected and resolved?

**Decision**: Scope v1 to two folder conflict sub-types:
1. **Rename conflict**: folder renamed on both sides → user picks which name wins, or keeps both as sibling folders.
2. **Delete-vs-content conflict**: folder deleted on one side, new files added on the other → user sees a file count warning and picks a side.

**How to detect**: Add `is_dir: bool` to `ConflictRecord`. The propagator diff step already has `LocalItem.is_dir` / `RemoteItem.is_dir`; a folder conflict is raised when an `is_dir = true` item exists on both sides with different states.

**Resolution mechanics**:
- **Keep Local (rename conflict)**: overwrite server folder name with local name via a MOVE operation.
- **Keep Server (rename conflict)**: rename local folder to match server name.
- **Keep Both (rename conflict)**: local folder keeps its name; download the server-named folder as a new sibling.
- **Keep Local (delete-vs-content)**: if local deleted it, remove new server files; server folder is deleted.
- **Keep Server (delete-vs-content)**: restore the folder and download all its new files locally. Impact warning lists the files that will appear on disk.

**Folder conflict copy naming**: `FolderName (server copy)` — same pattern as file conflicts, but without a file extension.

**Out-of-scope for v1**: True merge of divergent children (both sides added different files to the same folder). This is tracked as a potential v2 scenario.

**Alternatives considered**:
- Defer folder conflicts entirely to v2 — rejected per spec (user explicitly chose Option B).

---

## Decision 4: Conflict Detection → UI Notification

**Question**: How does the React UI learn that a new conflict exists?

**Decision**: Tauri event `adagio://conflict-detected` with payload `{ count: number }` (total pending count), emitted each time the propagator persists a new conflict.

**Rationale**: Push events are more responsive and efficient than frontend polling. The `app_handle.emit(...)` pattern is already used in `lib.rs` for the tray window. React listens via `@tauri-apps/api` `listen("adagio://conflict-detected", ...)`.

**How to apply**: After `journal.upsert_conflict(record)` succeeds in the propagator, `conflict::record_conflict()` accepts an optional `AppHandle` parameter (or the caller emits directly) to fire the event. The pending count is computed as `list_conflicts.filter(resolution.is_none()).len()`.

**Alternatives considered**:
- Frontend polling every 5s — rejected (conflict badge would lag by up to 5s; polling adds background load).
- Tauri Plugin notification — too heavyweight; in-app badge is sufficient.

---

## Decision 5: Conflict Badge Placement

**Question**: Where in the UI should the persistent conflict indicator live?

**Decision**: Add a conflict badge to the existing `Chrome.tsx` right-side button cluster. When `pendingConflicts > 0`, a dedicated conflict icon (or the bell icon with a red dot) shows the count. Clicking opens the `ConflictWizard` overlay.

**Rationale**: Conflicts are time-sensitive; they block sync for the affected files. Surface them in the always-visible chrome rather than burying them in the Sidebar or Activity feed. The chrome already has the settings and bell buttons; adding one more is consistent with the pattern.

**Alternatives considered**:
- Sidebar badge on "All files" — too easy to miss.
- Activity feed entry only — conflicts are actionable items, not merely informational events.

---

## ADR

→ See `docs/adr/006-conflict-resolution-execution.md` (to be created in Phase 1).
