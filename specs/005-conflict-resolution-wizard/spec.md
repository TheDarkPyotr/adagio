# Feature Specification: Conflict Resolution Wizard

**Feature Branch**: `005-conflict-resolution-wizard`

**Created**: 2026-05-27

**Status**: Draft

**Input**: User description: "When a true sync conflict happens, the official client just dumps a duplicate file with a timestamp in the folder. In Adagio, create a popup that allows the user to see a side-by-side comparison (or at least metadata like file size and modified date) and click Keep Local Version, Keep Server Version, or Keep Both right then and there."

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Resolve a Conflict When Detected (Priority: P1)

During a background sync run, Adagio detects that the same file was modified both locally and on the server since the last successful sync. Rather than silently creating a duplicate file with a cryptic timestamp suffix, Adagio surfaces a conflict resolution dialog. The user sees the two versions side by side (local vs. server), their file sizes, and last-modified timestamps. They click one button to resolve the conflict permanently.

**Why this priority**: This is the core value of the feature. Without it, conflicts remain invisible or confusing. Every sync client user eventually hits a conflict; this is the minimum viable resolution experience.

**Independent Test**: Can be fully tested by simulating a conflict (modify a file locally and update the same file on the server), triggering a sync, and verifying the resolution dialog appears with correct metadata and that each resolution button produces the correct file outcome.

**Acceptance Scenarios**:

1. **Given** a file has been modified both locally and on the server since the last sync, **When** Adagio performs a sync scan, **Then** the conflict is detected and the user is notified that a conflict requires their attention.
2. **Given** a conflict notification is shown, **When** the user opens the conflict resolution dialog, **Then** both the local version and the server version are shown with their file name, size, and last-modified date/time.
3. **Given** the conflict dialog is open, **When** the user clicks "Keep Local Version", **Then** the server file is overwritten with the local version, the local file is unchanged, and the conflict is cleared.
4. **Given** the conflict dialog is open, **When** the user clicks "Keep Server Version", **Then** the local file is replaced with the server version and the conflict is cleared.
5. **Given** the conflict dialog is open, **When** the user clicks "Keep Both", **Then** the local file remains at its original path and the server version is saved alongside it with a clear distinguishing name (e.g., `filename (server copy).ext`), and the conflict is cleared.
6. **Given** a folder has been renamed on both sides, **When** the user opens the conflict dialog, **Then** both names are shown ("Local: 'Projects 2026' / Server: 'Work Projects'") and the user can choose which name wins or keep both as separate folders.
7. **Given** a folder was deleted locally but new files were added to it on the server, **When** the user clicks "Keep Server Version", **Then** the dialog shows a warning listing the files that will be restored to disk before the user confirms.

---

### User Story 2 — Work Through Multiple Queued Conflicts (Priority: P2)

A user returns after several days offline. On reconnecting, Adagio detects multiple conflicts across different files. Rather than requiring the user to find each one manually, the wizard presents them in sequence — "Conflict 1 of 3" — allowing rapid resolution without hunting through folders.

**Why this priority**: Multiple conflicts are common after offline periods. Without a queue, a user who dismisses one conflict could lose track of others.

**Independent Test**: Can be tested by simulating two or more conflicts simultaneously and verifying that after resolving the first, the dialog automatically advances to the next, and the counter updates correctly.

**Acceptance Scenarios**:

1. **Given** three files have conflicts, **When** the user opens the conflict resolution dialog, **Then** it shows a progress indicator ("1 of 3") and the first conflict's details.
2. **Given** the user resolves the first conflict, **When** the resolution is applied, **Then** the dialog automatically advances to the second conflict ("2 of 3").
3. **Given** the user resolves all conflicts, **When** the last resolution is applied, **Then** the dialog closes and the activity feed shows all three conflicts as resolved.

---

### User Story 3 — Dismiss and Resolve Conflicts Later (Priority: P3)

A user is in the middle of something and does not want to be interrupted. They dismiss the conflict notification for now. Later, they notice a conflict indicator in the app and return to resolve the pending conflicts at their convenience.

**Why this priority**: Forced resolution interrupts workflows. Users should be able to defer without losing the conflict state. Unresolved conflicts should remain visible so nothing is silently lost.

**Independent Test**: Can be tested by dismissing the conflict dialog and verifying the conflict badge/indicator persists, then re-opening the dialog from the indicator and successfully resolving.

**Acceptance Scenarios**:

1. **Given** a conflict notification appears, **When** the user dismisses it without resolving, **Then** the conflict is marked as pending and a conflict badge or indicator remains visible in the app chrome or activity feed.
2. **Given** there are pending unresolved conflicts, **When** the user clicks the conflict indicator, **Then** the conflict resolution dialog reopens with all pending conflicts.
3. **Given** the user has resolved all pending conflicts, **When** the last conflict is resolved, **Then** the conflict indicator disappears.

---

### Edge Cases

- What happens when a file is deleted locally but modified on the server (or vice versa)? The dialog must clearly indicate "deleted locally" vs. "modified on server" so the user understands what each choice means.
- What if a conflict file is currently open in another application when the user chooses "Keep Server Version"? The system should warn the user that the file is in use and the replacement may not take effect until the application releases it.
- What if the user loses their connection between opening the conflict dialog and confirming a resolution? The conflict must remain pending and the local file must remain untouched until the resolution can be committed successfully.
- What happens if "Keep Both" is chosen and a file named `filename (server copy).ext` already exists? A further unique suffix must be applied to avoid a second collision.
- What if a conflict exists for a folder (not a file)? Folder-level conflicts (e.g., renamed on both sides, deleted locally but files added on server) are in scope for v1. The dialog must clearly describe what happened on each side and present the same three choices — with "Keep Both" meaning both folder states are preserved where possible. If the conflict is irreconcilable without cascading consequences (e.g., folder deleted locally but contains new server files), the dialog must explain the downstream impact before the user confirms.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When a sync scan detects a true file conflict (both local and server versions modified since last sync), the system MUST surface a notification to the user that requires action.
- **FR-002**: The conflict resolution dialog MUST display, for each conflicting version (local and server): the file name, file size, and last-modified date and time.
- **FR-003**: The user MUST be presented with exactly three resolution options for each conflict: "Keep Local Version", "Keep Server Version", and "Keep Both".
- **FR-004**: Selecting "Keep Local Version" MUST result in the server copy being replaced by the local version, with the local file remaining unchanged.
- **FR-005**: Selecting "Keep Server Version" MUST result in the local file being replaced by the server version.
- **FR-006**: Selecting "Keep Both" MUST preserve the local file at its original path and save the server version alongside it under a clearly labeled name (e.g., `<filename> (server copy).<ext>`). If that name already exists, an additional unique suffix must be applied.
- **FR-007**: After any resolution is applied, the conflict MUST be removed from the pending conflicts queue and the outcome MUST be recorded in the activity log.
- **FR-008**: If multiple conflicts exist simultaneously, the dialog MUST present them sequentially with a counter showing the user's progress (e.g., "Conflict 2 of 4").
- **FR-009**: The user MUST be able to dismiss the conflict dialog without resolving; dismissed conflicts MUST be retained as pending.
- **FR-010**: As long as at least one conflict is pending, a visible indicator (badge, label, or icon) MUST be shown persistently in the app so the user can return to it.
- **FR-011**: Clicking the pending-conflict indicator MUST re-open the conflict resolution dialog with all unresolved conflicts.
- **FR-012**: The system MUST NOT silently create duplicate files with timestamp suffixes as a result of a conflict; all conflict outcomes must be the result of an explicit user choice.
- **FR-013**: The conflict resolution dialog MUST handle folder-level conflicts in addition to file-level conflicts. For folder conflicts, the dialog MUST clearly describe what changed on each side (e.g., "renamed locally to X / renamed on server to Y", "deleted locally / new files added on server").
- **FR-014**: For folder conflicts where "Keep Both" would result in cascading side effects (e.g., undeleting files, moving nested content), the dialog MUST display a plain-language summary of the downstream impact before the user confirms the choice.

### Key Entities

- **Conflict**: A record of a file that has been modified both locally and on the server since the last successful sync. Attributes: file path, local version metadata (size, modified-at), server version metadata (size, modified-at), detected-at timestamp, resolution status (pending / resolved), resolution chosen (local / server / both).
- **Conflict Queue**: The ordered collection of all pending conflicts for the current session. Supports sequential traversal and persistence across app restarts.
- **Resolution**: The user's chosen outcome for a specific conflict. Once applied, it triggers the appropriate file operation and updates the activity log.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can open a conflict, review both versions, and resolve it in under 30 seconds.
- **SC-002**: 100% of detected conflicts are surfaced to the user — zero conflicts are silently resolved or silently discarded without user action.
- **SC-003**: After a resolution is applied, the file on disk and on the server matches the user's chosen outcome within one sync cycle (no residual duplicate files, no lingering conflict state).
- **SC-004**: When a user dismisses a conflict and reopens the app later, all unresolved conflicts remain accessible and no conflict data is lost.
- **SC-005**: The conflict indicator disappears within one sync cycle after all conflicts are resolved.

---

## Assumptions

- **File and folder scope**: This feature addresses conflicts on both individual files and folders. Folder conflict resolution is included in v1 with impact warnings for cascading operations.
- **Metadata-only comparison**: The conflict dialog shows file size and last-modified date for each version. A content diff (line-by-line comparison) is out of scope for v1; it may be added in a future iteration.
- **Single account**: Conflicts are scoped to the active sync pair. Multi-account conflict scenarios are not addressed in this version.
- **Non-blocking notification**: The conflict notification does not block the rest of the app. The sync engine continues syncing non-conflicted files while conflicts await user resolution.
- **Conflict persistence**: Pending conflicts survive app restarts — they are stored on disk so the user does not lose them if the app is closed before resolving.
- **"Keep Both" naming**: The server copy is named `<filename> (server copy).<ext>`. This is a user-visible name chosen for clarity; no timestamp suffix is used (avoiding the problem this feature is designed to solve).
