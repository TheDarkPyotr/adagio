# Feature Specification: Nextcloud File Synchronization

**Feature Branch**: `001-nextcloud-file-sync`

**Created**: 2026-05-24

**Status**: Draft

## User Scenarios & Testing

### User Story 1 - Account Setup and First-Time Sync (Priority: P1)

A user adds their Nextcloud account credentials, configures a sync pair linking a
local directory to a remote Nextcloud directory, reviews a pre-flight summary of what
will be synchronized, optionally excludes large subdirectories via selective sync, and
starts the initial sync. Progress is visible throughout, including estimated time
remaining once a representative sample of items has been processed.

**Why this priority**: Without a working account setup and initial sync, no other
capability is accessible. This is the entry point for all sync functionality.

**Independent Test**: Can be fully tested by configuring an account, creating a sync
pair, and verifying the initial sync completes with the expected files present on both
sides.

**Acceptance Scenarios**:

1. **Given** a user has Nextcloud server credentials, **When** they configure a new
   account and sync pair, **Then** the system displays a pre-flight summary showing
   remote item count, total size, and local destination before any data moves.
2. **Given** the local sync root already contains files, **When** first sync begins,
   **Then** the system matches local files against remote by content to avoid redundant
   uploads rather than blindly overwriting.
3. **Given** the user enables selective sync before first sync starts, **When** they
   exclude large subdirectories, **Then** no data from those subdirectories is
   downloaded to the local device.
4. **Given** a first sync is in progress, **When** a representative sample of items
   has been processed, **Then** an estimated time remaining is displayed and updates
   as the sync progresses.

---

### User Story 2 - Ongoing Bidirectional Synchronization (Priority: P1)

During normal operation, file and folder changes made on the local device are
propagated to the server, and changes made on the server (by the user or another client)
are propagated to the local device. Changes are detected and propagated within the
configured intervals without user intervention.

**Why this priority**: This is the core daily value of the sync client. All other
features depend on reliable bidirectional sync.

**Independent Test**: Can be fully tested by modifying files on both sides independently
and verifying propagation completes within the expected window.

**Acceptance Scenarios**:

1. **Given** the client is running, **When** a file is created, modified, or deleted
   locally, **Then** the change is detected within the filesystem event debounce window
   (2–5 seconds of quiescence) and propagated to the server.
2. **Given** the client is running, **When** a file is created, modified, or deleted
   on the server, **Then** the change is detected within the configured polling interval
   (default: 30 seconds during active use) and propagated locally.
3. **Given** the client was not running when local changes occurred, **When** the
   client restarts, **Then** it detects the missed changes during startup and
   propagates them.
4. **Given** a file is renamed or moved on the server, **When** the next sync cycle
   runs, **Then** the client issues a local rename/move rather than a
   delete-plus-recreate.
5. **Given** a folder is renamed or moved locally, **When** the next sync cycle runs,
   **Then** the client issues a server-side rename/move rather than
   delete-plus-recreate.
6. **Given** a file's content is unchanged on both sides since last sync, **When** a
   sync cycle runs, **Then** no transfer is initiated for that item.

---

### User Story 3 - Conflict Detection and Resolution (Priority: P2)

When the same file is modified on both the local device and the server since the last
successful sync, the system detects the conflict and applies the configured resolution
policy. By default, both versions are preserved with neither silently discarded. The
user can view all current and historical conflicts.

**Why this priority**: Conflicts are inevitable in multi-device or multi-user scenarios.
Silently overwriting user data is unacceptable.

**Independent Test**: Can be fully tested by modifying the same file on both sides,
triggering a sync cycle, and verifying both versions are preserved (or the configured
policy is applied) with the conflict surfaced in the UI.

**Acceptance Scenarios**:

1. **Given** the default "preserve both" policy, **When** a conflict is detected,
   **Then** the remote version is downloaded to its normal local path and the local
   version is renamed with a conflict suffix containing the device name and timestamp
   (e.g., `report (conflicted copy from <device> 2026-05-24 14-32-15).docx`).
2. **Given** the "preserve both" policy, **When** the conflict copy is created,
   **Then** the renamed conflict file is uploaded to the server as a new file so both
   versions exist on both sides.
3. **Given** a "remote wins" policy, **When** a conflict is detected, **Then** the
   remote version replaces the local version without creating a conflict copy.
4. **Given** an "ask" policy, **When** a conflict is detected, **Then** propagation
   pauses for that item, the user is prompted to choose a resolution, and the rest of
   the sync cycle continues for unaffected items.
5. **Given** conflicts have occurred, **When** the user opens the conflicts view,
   **Then** all current and historical conflicts are listed with their paths,
   timestamps, and devices involved.

---

### User Story 4 - Large File Transfer and Bandwidth Management (Priority: P2)

Users can transfer files of any size. Large files are transferred in chunks that are
resumable if interrupted. Users can configure upload and download bandwidth limits and
schedule throttled time windows.

**Why this priority**: A sync client without reliable large-file support and bandwidth
control is unusable in professional environments.

**Independent Test**: Can be fully tested by uploading a file above the chunked-upload
threshold, interrupting mid-transfer, resuming, and verifying the complete file arrives
intact with no retransmission of already-transferred portions.

**Acceptance Scenarios**:

1. **Given** a file at or above the chunked-upload threshold (default: 10 MB), **When**
   it is uploaded, **Then** it is transferred in chunks and assembled server-side;
   file contents are not fully loaded into memory at any point.
2. **Given** a chunked upload is interrupted, **When** the client resumes, **Then**
   only the remaining chunks are transferred.
3. **Given** a download is interrupted, **When** the client resumes, **Then** it
   resumes from the interruption point rather than restarting from the beginning.
4. **Given** a user-configured upload limit of 500 KB/s during working hours, **When**
   a sync transfer occurs during those hours, **Then** upload throughput does not
   exceed the configured limit.
5. **Given** the device is detected as being on a metered connection, **When** a sync
   cycle would start, **Then** non-essential sync is suspended by default until the
   connection type changes or the user overrides.
6. **Given** battery level drops below the configured threshold, **When** a sync
   cycle would start, **Then** sync is suspended until battery recovers above the
   threshold.

---

### User Story 5 - Error Visibility and Recovery (Priority: P2)

When errors occur, the user can see all failed items with their cause and category.
Transient errors are retried automatically with backoff. Permanent errors require user
action. No sync error is silent, and no failed item blocks others from syncing.

**Why this priority**: Silent failures destroy trust. Users need full visibility into
what has and hasn't synced.

**Independent Test**: Can be fully tested by inducing transient and permanent errors and
verifying auto-retry behavior, the errors view content, and that unaffected items
continue to sync independently.

**Acceptance Scenarios**:

1. **Given** a transient error (network timeout, server 5xx) on an item, **When** the
   system retries with exponential backoff, **Then** the item is retried up to 5
   attempts before being parked in error state.
2. **Given** a permanent error (permission denied, invalid filename) on one item,
   **When** the sync cycle continues, **Then** all unrelated items complete
   successfully without being delayed.
3. **Given** one or more items have failed, **When** the user opens the errors view,
   **Then** each entry shows the item path, operation attempted, error category, and
   underlying cause.
4. **Given** credentials become invalid (HTTP 401), **When** the system detects this,
   **Then** sync pauses for that account and a re-authentication prompt is surfaced
   without retrying.
5. **Given** a disk-full condition occurs during a download, **When** the download
   fails, **Then** the existing local file at the destination is left intact and
   uncorrupted.

---

### User Story 6 - Selective Sync (Priority: P3)

Users can exclude specific remote subdirectories from local materialization. Excluded
directories remain on the server and are visible in the application's remote browser,
but consume no local storage. Users can change their selections at any time.

**Why this priority**: Large remote directories can exceed available local storage;
users need fine-grained control to manage local disk usage.

**Independent Test**: Can be fully tested by excluding a subdirectory before or after
first sync and verifying no local files are created or are removed for it, then
including it and verifying the files are downloaded.

**Acceptance Scenarios**:

1. **Given** the user excludes a remote subdirectory, **When** the next sync cycle
   runs, **Then** no files from that subdirectory are downloaded locally; if they were
   previously downloaded, they are removed.
2. **Given** the user re-includes a previously excluded subdirectory, **When** the
   next sync cycle runs, **Then** the directory and its contents are downloaded.
3. **Given** a directory is excluded via selective sync, **When** the user browses the
   remote tree in the application, **Then** the excluded directory is still visible
   there, reflecting its server-side presence.

---

### Edge Cases

- What happens when a local file is still being written by another application during
  checksumming? (Upload deferred to next cycle.)
- How does the system handle paths valid on the server but containing characters invalid
  on the local filesystem? (Surfaced as a permanent error; no silent rename.)
- What happens if two server-side files differ only in case on a case-insensitive local
  filesystem? (Detected as a path-compatibility conflict and surfaced as a permanent
  error.)
- What happens if the local sync root becomes inaccessible (unmounted, deleted)?
  (Sync pair transitions to error state and user is notified.)
- How does the system handle a file deleted locally and modified remotely simultaneously?
  (Treated as a conflict; the modified remote version wins by default, or the configured
  conflict policy applies.)
- What happens if the journal is corrupted or lost? (System rebuilds the journal from a
  full discovery pass, treating the situation as first-time sync.)

## Requirements

### Functional Requirements

**Account Management**

- **FR-001**: The system MUST support one or more configured Nextcloud accounts, each
  uniquely identified by server URL and authenticated user identity.
- **FR-002**: The system MUST store account credentials exclusively in the device's
  native credential storage; credentials MUST NOT appear in plain-text configuration
  files or logs.
- **FR-003**: The system MUST support app-password authentication and OAuth2
  authentication flows as offered by the Nextcloud server.
- **FR-004**: The system MUST verify connectivity and authentication before each sync
  cycle; if credentials become invalid (HTTP 401), sync MUST pause for that account
  and a re-authentication prompt MUST be surfaced before retrying.

**Sync Pair Configuration**

- **FR-005**: Users MUST be able to create, modify, pause, resume, and delete sync
  pairs.
- **FR-006**: The system MUST validate that a sync pair's local root is writable, exists
  or can be created, and is not nested inside another active sync pair's local root;
  violations MUST be rejected with a clear explanation.
- **FR-007**: Users MUST be able to configure selective sync to designate remote
  subdirectories that will not be downloaded locally.
- **FR-008**: The system MUST ship with a default exclude list covering common OS and
  editor metadata patterns (`.DS_Store`, `Thumbs.db`, `~$*`, `*.tmp`, `.~lock.*`);
  users MUST be able to add exclude patterns but MUST NOT be able to remove patterns
  that protect system-managed files (e.g., the journal).
- **FR-009**: Deleting a sync pair MUST remove the journal and system metadata but MUST
  NOT delete the user's local or remote files unless the user explicitly opts in.

**Journal**

- **FR-010**: The system MUST maintain a persistent journal per sync pair recording, for
  every known sync item: relative path, type, size, local modification time, local
  content checksum, remote etag, server file ID, last successful sync timestamp, and
  current sync status.
- **FR-011**: Journal writes that record completed sync operations MUST be durable before
  any subsequent operation that depends on that state begins.
- **FR-012**: The journal MUST NOT be stored within the sync pair's local root directory.
- **FR-013**: If the journal is lost, corrupted, or detected as inconsistent, the system
  MUST rebuild it via a full discovery and reconciliation pass.

**Change Detection**

- **FR-014**: The system MUST monitor the local sync root for filesystem events (create,
  modify, delete, rename), coalescing events over a 2–5 second debounce window before
  triggering propagation.
- **FR-015**: The system MUST perform periodic full local scans at a configurable interval
  (default: every 2 hours and at every application startup).
- **FR-016**: The system MUST confirm a local content change by comparing checksums when
  a file's modification time or size differs from the journal entry.
- **FR-017**: The system MUST detect remote changes by comparing server-reported etags
  against journaled values; a differing etag indicates modification, an absent item
  indicates deletion, an unknown item indicates creation.
- **FR-018**: The system MUST use the server-assigned file ID as the primary identity for
  matching items across cycles, enabling correct detection of server-side renames and
  moves.

**Sync Cycle**

- **FR-019**: A sync cycle MUST consist of three sequential phases — discovery,
  reconciliation, propagation — where reconciliation produces only a plan and performs
  no I/O against the server and makes no local modifications.
- **FR-020**: The reconciliation phase MUST classify every item into a defined change
  category and produce a deterministic operation plan covering: unchanged, local-only
  change, remote-only change, local creation, remote creation, local deletion, remote
  deletion, both-sides changed (conflict), and move/rename.
- **FR-021**: Folder creations MUST complete before any operations on their contents;
  folder deletions MUST occur after all child operations within them have completed.
- **FR-022**: Only one sync cycle per sync pair MUST be active at any given time; a
  new-cycle request while one is running MUST be coalesced or queued.
- **FR-023**: Propagation MUST apply bounded concurrency (default: 3 concurrent uploads,
  3 concurrent downloads, total ≤ 6 active transfers); limits MUST be user-configurable.
- **FR-024**: After each individual operation completes, the journal MUST be updated to
  reflect the new state before any dependent operation proceeds.

**File Transfer**

- **FR-025**: Files at or above the chunked-upload threshold (configurable; default:
  10 MB) MUST be uploaded using Nextcloud's chunked upload protocol; uploads MUST stream
  from disk and MUST NOT load entire file contents into memory.
- **FR-026**: Interrupted chunked uploads MUST be resumable from the last successfully
  uploaded chunk without retransmitting earlier chunks.
- **FR-027**: Downloads MUST stream to a temporary file in the same directory as the
  final destination and be atomically renamed into place only after successful completion
  and checksum verification.
- **FR-028**: Interrupted downloads MUST support resumption using range requests.
- **FR-029**: The system MUST verify file checksums on both upload acknowledgment and
  download completion; mismatches MUST cause the transfer to be discarded and retried;
  repeated mismatches MUST surface as errors.

**Bandwidth and Resource Controls**

- **FR-030**: Users MUST be able to configure separate upload and download bandwidth
  limits in KB/s; a value of zero or "unlimited" disables the cap.
- **FR-031**: Users MUST be able to schedule time-window-based bandwidth limits (e.g.,
  throttled during working hours, unlimited overnight).
- **FR-032**: The system MUST suspend non-essential sync when the device is on a metered
  connection, when battery is below a configurable threshold, and when the user has
  manually paused sync.

**Conflict Handling**

- **FR-033**: The system MUST detect conflicts (same item modified on both sides since
  last sync, or item deleted on one side and changed on the other) and apply the
  configured resolution policy.
- **FR-034**: The default resolution policy MUST be "preserve both": the remote version
  is downloaded to its normal local path; the local version is renamed with a conflict
  suffix (`<name> (conflicted copy from <device> <YYYY-MM-DD HH-MM-SS>)<ext>`); the
  renamed version is uploaded to the server as a new file.
- **FR-035**: The following alternative resolution policies MUST be selectable per sync
  pair: "local wins", "remote wins", "newest wins" (by modification timestamp), "ask"
  (pause item and prompt user while remaining sync continues).
- **FR-036**: Users MUST be able to view a list of all current and historical conflicts,
  including item path, conflicting modification timestamps, resolution applied, and
  outcome file paths.

**Error Handling**

- **FR-037**: Transient errors (network timeouts, HTTP 5xx, HTTP 429, temporary local
  I/O errors) MUST be retried with exponential backoff (starting at 1 second, doubling,
  capped at 5 minutes, with jitter), with a per-item retry cap (default: 5 attempts)
  before the item is parked in error state.
- **FR-038**: Permanent errors (HTTP 4xx excluding 401/408/423/429, local permission
  denials, disk-full, path-too-long, invalid filenames) MUST NOT be retried automatically
  and MUST be surfaced to the user with path, operation, error category, and cause.
- **FR-039**: A failed item MUST NOT block other unrelated items from syncing in the
  same cycle or subsequent cycles.
- **FR-040**: The system MUST maintain an errors view showing every item that did not
  successfully sync, with path, attempted operation, error category, and underlying cause.

**Path Compatibility**

- **FR-041**: Before propagation, the system MUST detect path incompatibilities including
  reserved Windows filenames (CON, PRN, AUX, NUL, COM1–COM9, LPT1–LPT9), disallowed
  characters on the local filesystem, trailing dots or spaces on Windows, paths exceeding
  platform length limits, and case-only collisions on case-insensitive filesystems.
- **FR-042**: Detected path incompatibilities MUST be surfaced as permanent errors; the
  system MUST NOT silently rename or modify filenames to work around them.

**Observability**

- **FR-043**: The system MUST report the current sync status for each sync pair at all
  times: idle, scanning, syncing (with item count and byte progress), paused, or error.
- **FR-044**: The system MUST maintain a recent activity log of the last N completed
  operations (default: 500), each recording timestamp, item path, operation type,
  direction, and result.
- **FR-045**: The system MUST produce structured logs at configurable verbosity levels;
  logs MUST NOT contain credentials, file contents, or server URLs with embedded tokens;
  logs MUST be rotated to bound disk usage.
- **FR-046**: Users MUST be able to export a diagnostic bundle containing logs, redacted
  configuration, and journal statistics.

**Safety Guarantees**

- **FR-047**: The system MUST be safe to terminate at any time; the next startup MUST
  reconcile the journal against the actual state of both sides and resume without data
  loss.
- **FR-048**: The system MUST defer uploading a local file if an in-progress write by
  another process is detected (e.g., size or modification time changes during
  checksumming); the upload MUST be rescheduled for the next cycle.

### Key Entities

- **Account**: A Nextcloud user identity bound to a server URL; holds authentication
  state (stored in OS credential store) and server-reported capabilities.
- **Sync Pair**: The binding between one local root directory and one remote root
  directory for one account; holds exclude patterns, selective sync configuration,
  conflict policy, bandwidth limits, and current status.
- **Journal Entry**: The last-known-synced state of a single sync item (path, type,
  size, modification time, checksum, remote etag, server file ID, last sync timestamp,
  status), enabling change detection across cycles.
- **Sync Item**: Any file or folder within a sync pair's scope, identified by its path
  relative to the sync root.
- **Conflict Record**: A record of a detected conflict — item path, conflicting
  modification timestamps on each side, resolution policy applied, and resulting
  file paths after resolution.
- **Transfer**: An in-progress upload or download, including its progress, chunk state,
  and retry history.

## Success Criteria

### Measurable Outcomes

- **SC-001**: Local filesystem changes (create, modify, delete) are detected and queued
  for propagation within 5 seconds of the last write to that item completing.
- **SC-002**: Remote changes are detected and queued for propagation within the
  configured polling interval (default: 30 seconds during active use).
- **SC-003**: Every uploaded and downloaded file passes checksum verification; zero
  silent data-integrity failures occur under normal operating conditions.
- **SC-004**: After an abrupt client termination during any sync operation, the next
  startup completes reconciliation and resumes with no data loss or corruption.
- **SC-005**: Conflicts are never silently discarded; 100% of detected conflicts result
  in either the user being notified, both versions being preserved, or an explicit
  user-chosen resolution being applied.
- **SC-006**: A large-file transfer interrupted at any point resumes from the
  interruption point without retransmitting already-transferred data.
- **SC-007**: A failed sync item does not delay or block other items; unaffected items
  in the same sync cycle complete within their normal time bounds.
- **SC-008**: The idle client stays within the constitution's resource budget: under
  100 MB RSS at rest and under 5% CPU during steady-state background sync.

## Assumptions

- Users have a reachable Nextcloud server (supporting WebDAV and the Nextcloud
  capabilities endpoint) accessible from their device.
- The local device has sufficient storage for files the user has not excluded via
  selective sync; the system surfaces disk-full conditions but does not prevent the user
  from syncing more data than they have space for.
- Mobile and tablet platforms are out of scope; the sync client targets desktop operating
  systems (Linux, macOS, Windows).
- Virtual files (placeholder-based on-demand materialization) mode is out of scope for
  this specification phase and will be addressed in a separate feature.
- End-to-end encryption, sharing and permission management, server-side trash/version
  history integration, and public link creation are explicitly out of scope.
- The "ask" conflict policy requires a UI component capable of presenting the conflict
  and receiving a user choice; the data and states that UI must display are in scope,
  but the specific UI presentation details are not.
- Bandwidth scheduling uses the device's local clock; no server-side time coordination
  is required.
