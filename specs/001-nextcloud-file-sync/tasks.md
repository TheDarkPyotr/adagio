---
description: "Task list for Adagio — Nextcloud File Sync"
---

# Tasks: Nextcloud File Sync

**Input**: Design documents from `/specs/001-nextcloud-file-sync/`

**Prerequisites**: plan.md ✅ | spec.md ✅ | data-model.md ✅ | contracts/ ✅ | research.md ✅

**Testing**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins. The Red-Green-Refactor cycle is strictly enforced.

**Organization**: Tasks are grouped by user story to enable independent implementation
and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US6)
- Include exact file paths in all descriptions

## Path Conventions

- Cargo workspace root: `adagio/`
- Core sync engine: `crates/adagio-core/src/`
- Nextcloud protocol: `crates/adagio-nextcloud/src/`
- Desktop shell (Rust): `crates/adagio-desktop/src/`
- Desktop shell (Svelte UI): `crates/adagio-desktop/src-ui/`
- Integration tests: `tests/integration/`
- Contract tests: `tests/contract/`
- Benchmarks: `benches/`

---

## Phase 1: Setup (Project Initialization)

**Purpose**: Establish the Cargo workspace, crate skeletons, and CI pipeline.

- [X] T001 Initialize Cargo workspace manifest at `Cargo.toml` with `[workspace]` members (`adagio-core`, `adagio-nextcloud`, `adagio-desktop`) and `resolver = "2"`
- [X] T002 Create `crates/adagio-core/Cargo.toml` with tokio, sqlx (sqlite), tracing, sha2, notify, notify-debouncer-full, serde, thiserror dependencies
- [X] T003 [P] Create `crates/adagio-nextcloud/Cargo.toml` with reqwest (rustls-tls), quick-xml, keyring, serde dependencies and adagio-core path dependency
- [X] T004 [P] Create `crates/adagio-desktop/Cargo.toml` + `crates/adagio-desktop/tauri.conf.json` with Tauri 2.x dependencies and adagio-core/adagio-nextcloud path dependencies
- [X] T005 [P] Initialize Svelte 5 + TypeScript + Vite frontend in `crates/adagio-desktop/src-ui/` (`npm create vite@latest` with svelte-ts template)
- [X] T006 Configure GitHub Actions CI matrix in `.github/workflows/ci.yml` with jobs for Linux x86_64, macOS arm64, Windows x86_64 running `cargo test --workspace`
- [X] T007 [P] Add `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, and `cargo doc --workspace --no-deps` steps to `.github/workflows/ci.yml`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core type definitions, trait contracts, journal implementation, and
infrastructure that all user stories depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T008 Define shared primitive types (`AccountId`, `PairId`, `RelativePath`, `RemotePath`, `Checksum`, `ChecksumAlgorithm`, `ItemType`, `TransferId`) in `crates/adagio-core/src/types.rs`
- [X] T009 [P] Define `SyncError` (Transient/Permanent/Fatal variants) and `ErrorCategory` enum in `crates/adagio-core/src/error.rs`
- [X] T010 [P] Define `Journal` trait and `JournalError` enum in `crates/adagio-core/src/journal/mod.rs` per `contracts/journal.md`
- [X] T011 [P] Define `RemoteClient` trait with `list`, `list_recursive`, `upload`, `begin_chunked_upload`, `upload_chunk`, `finalize_chunked_upload`, `list_uploaded_chunks`, `download`, `delete`, `move_item`, `create_dir`, `capabilities` in `crates/adagio-core/src/remote.rs` per `contracts/remote-client.md`
- [X] T012 [P] Define `SyncEngine` trait with `trigger_sync`, `pause`, `resume`, `status`, `subscribe_status`, `activity_log`, `error_items` in `crates/adagio-core/src/cycle/mod.rs` per `contracts/sync-engine.md`
- [X] T013 [P] Define `ChangeDetector` trait with `local_snapshot`, `remote_snapshot`, `subscribe_local_events` in `crates/adagio-core/src/detection/mod.rs` per `contracts/change-detector.md`
- [X] T014 [P] Define `TransferManager` trait with `upload`, `download`, `cancel` and associated types in `crates/adagio-core/src/transfer/mod.rs` per `contracts/transfer-manager.md`
- [X] T015 Write contract tests for `Journal` trait using an in-memory mock impl in `tests/contract/journal.rs` — 6 tests all pass ✓
- [X] T016 [P] Write contract tests for `RemoteClient` trait using a mock server impl in `tests/contract/remote_client.rs` — 8 tests all pass ✓
- [X] T017 Create SQLite migrations for all five tables (accounts, sync_pairs, journal_entries, conflict_records, transfers) in `crates/adagio-core/migrations/`
- [X] T018 Implement `SqliteJournal` satisfying `Journal` trait (upsert with `BEGIN IMMEDIATE`/WAL flush, `get`, `get_by_file_id`, `all_entries`, `entries_by_status`, `upsert_batch`, `clear_pair`) in `crates/adagio-core/src/journal/sqlite.rs`
- [X] T019 [P] Configure `tracing` + `tracing-subscriber` with JSON formatter for production and pretty-print for dev; add `init_tracing()` in `crates/adagio-core/src/telemetry.rs`
- [X] T020 [P] Define `Account`, `SyncPair`, `PairStatus`, `ConflictPolicy`, `BandwidthLimits`, `BandwidthSchedule`, `ExcludePattern` config types in `crates/adagio-core/src/config.rs`
- [X] T098 Implement `AppState` struct holding `Arc<dyn SyncEngine>`, `Arc<AccountManager>`, and `Arc<SyncPairManager>`; register as Tauri managed state in `crates/adagio-desktop/src/lib.rs`; implement `crates/adagio-desktop/src/state.rs`
- [X] T099 [P] Write unit tests for `SqliteJournal` corruption detection (schema-version mismatch, `PRAGMA integrity_check` failure) in `crates/adagio-core/src/journal/corruption.rs` — tests passed Red before T100
- [X] T100 Implement journal corruption detection in `SqliteJournal`: `integrity_check()` and `schema_check()` methods in `crates/adagio-core/src/journal/sqlite.rs`
- [X] T114 [P] Add `#[instrument]` timing spans to all `SqliteJournal` methods and cycle entry point stubs (Constitution Principle III) in `crates/adagio-core/src/journal/sqlite.rs` and `crates/adagio-core/src/cycle/mod.rs`

**Checkpoint**: Foundation ready — all traits defined, journal implemented, CI green

---

## Phase 3: User Story 1 — Account Setup and First-Time Sync (Priority: P1) 🎯 MVP

**Goal**: User configures a Nextcloud account, creates a sync pair, reviews a pre-flight
summary, optionally enables selective sync, and starts the initial sync with progress
visible throughout.

**Independent Test**: Configure an account against the Docker Nextcloud instance, create
a sync pair, run first sync, verify files are present on both sides.

### Tests for User Story 1 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T021 [P] [US1] Write unit tests for `SyncPairConfig` validation (not writable, nested roots rejected) in `crates/adagio-core/src/config.rs`
- [X] T022 [P] [US1] Write unit tests for first-sync content-match deduplication logic in `crates/adagio-core/src/cycle/reconciler.rs`
- [X] T023 [P] [US1] Write integration test skeleton in `tests/integration/sync_cycle.rs` — 2 tests marked `#[ignore]`, require live Nextcloud (see quickstart.md)

### Implementation for User Story 1

- [X] T024 [P] [US1] Implement `keyring`-backed credential storage: store/retrieve/delete credentials under key `adagio/<account_id>` in `crates/adagio-nextcloud/src/auth.rs`
- [X] T025 [US1] Implement `AccountManager` with add/remove/list/get in `crates/adagio-core/src/account_manager.rs` — 4 tests pass ✓
- [X] T026 [US1] Implement `NextcloudClient` satisfying `RemoteClient` trait: PROPFIND, PUT, GET, DELETE, MOVE, MKCOL in `crates/adagio-nextcloud/src/client.rs` + XML parsing in `xml.rs`
- [X] T027 [P] [US1] Implement `ServerCapabilities` fetch from `/ocs/v2.php/cloud/capabilities` in `crates/adagio-nextcloud/src/client.rs` (`capabilities()` method)
- [X] T028 [P] [US1] Implement `SyncPairManager` with create/update/delete and local-root validation in `crates/adagio-core/src/config.rs`
- [X] T029 [US1] Implement first-sync pre-flight summary (remote item count, total size) in `crates/adagio-core/src/cycle/discovery.rs`
- [X] T030 [P] [US1] Implement content-based deduplication during first sync (checksum match → skip upload) in `crates/adagio-core/src/cycle/reconciler.rs`
- [X] T031 [P] [US1] Implement selective sync exclusion filtering via glob patterns in `crates/adagio-core/src/detection/exclusion.rs` — 6 tests pass ✓
- [X] T032 [US1] Implement sync progress reporting with item count, bytes done/total, ETA, and ActivityLog ring buffer in `crates/adagio-core/src/observability.rs` — 3 tests pass ✓
- [X] T033 [US1] Implement Tauri IPC commands for account management (`add_account`, `remove_account`, `list_accounts`, `initiate_oauth2`) in `crates/adagio-desktop/src/commands/account.rs`
- [X] T034 [US1] Implement Tauri IPC commands for pair management (`create_pair`, `delete_pair`, `get_exclude_patterns`) in `crates/adagio-desktop/src/commands/pair.rs`
- [X] T035 [US1] Implement Svelte onboarding flow (account setup form, pair config form, pre-flight summary review screen) in `crates/adagio-desktop/src-ui/src/routes/Onboarding.svelte` + `Pairs.svelte`
- [X] T101 [P] [US1] Write unit tests for OAuth2 PKCE: authorization URL generation (correct `code_challenge` S256 encoding, `state` param) in `crates/adagio-nextcloud/src/auth.rs` — 8 tests, all pass ✓
- [X] T102 [US1] Implement OAuth2 PKCE: `generate_pkce_pair()`, `generate_state()`, `build_authorization_url()` in `crates/adagio-nextcloud/src/auth.rs`
- [X] T103 [P] [US1] Implement OAuth2 access-token refresh (`refresh_access_token`, `exchange_code`) in `crates/adagio-nextcloud/src/auth.rs`
- [X] T104 [P] [US1] Implement `initiate_oauth2` Tauri IPC command (opens browser, returns PKCE verifier) in `crates/adagio-desktop/src/commands/account.rs`
- [X] T105 [P] [US1] Write unit tests for `SyncPairManager::delete_pair()`: local files intact + recursive delete path in `crates/adagio-core/src/config.rs` — tests pass ✓
- [X] T106 [US1] Implement `SyncPairManager::delete_pair(pair_id, delete_local_files)`: recursively removes local root when flag is true in `crates/adagio-core/src/config.rs`
- [X] T110 [P] [US1] Write unit tests for `SyncPairManager::update_exclude_patterns()`: custom pattern accepted; builtin stripped → error in `crates/adagio-core/src/config.rs` — tests pass ✓
- [X] T111 [P] [US1] Implement builtin exclude guard in `SyncPairManager::update_exclude_patterns()`; initialise builtin list in `SyncPairManager::new()` in `crates/adagio-core/src/config.rs`

**Checkpoint**: User Story 1 fully functional and independently testable

---

## Phase 4: User Story 2 — Ongoing Bidirectional Synchronization (Priority: P1)

**Goal**: Changes on either side (create/modify/delete/rename) propagate to the other
within configured detection intervals, with correct move detection using server file IDs.

**Independent Test**: Modify files on both sides independently; verify propagation within
5 s (local) and 30 s (remote) with correct outcomes including rename detection.

### Tests for User Story 2 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T036 [P] [US2] Write unit tests for all 10 `Reconciler` change categories in `crates/adagio-core/src/cycle/reconciler.rs`
- [X] T037 [P] [US2] Write unit tests for server file ID rename detection in `crates/adagio-core/src/detection/remote.rs`
- [X] T038 [P] [US2] Write integration tests for create/modify/delete in each direction in `tests/integration/sync_cycle.rs`

### Implementation for User Story 2

- [X] T039 [US2] Implement local filesystem watcher with `notify` + `notify-debouncer-full` (2–5 s quiescence window) in `crates/adagio-core/src/detection/local.rs` — design the checksum entry point to accept a "defer" return value so that T079 can signal "file still being written, skip this cycle" without requiring a structural change later
- [X] T040 [P] [US2] Implement periodic full local scan (at startup + configurable interval, default 2 h, driven by `SyncPair.scan_interval_secs` added in T112) in `crates/adagio-core/src/detection/local.rs` — **this startup scan, combined with T041 (remote PROPFIND) and T043 (reconciler), is the crash-recovery path for FR-047 / SC-004**: the first sync cycle after an abrupt termination reconciles journal state against reality; no separate recovery codepath is required
- [X] T041 [US2] Implement remote PROPFIND polling (30 s active / 5 min idle) with etag comparison in `crates/adagio-core/src/detection/remote.rs`
- [X] T042 [P] [US2] Implement server file ID rename/move detection in `crates/adagio-core/src/detection/remote.rs`
- [X] T043 [US2] Implement `Reconciler` producing `OperationPlan` from three-way diff (local snapshot, remote snapshot, journal) covering all 10 change categories in `crates/adagio-core/src/cycle/reconciler.rs`
- [X] T044 [P] [US2] Implement single-PUT upload with OC-Checksum header and server etag verification in `crates/adagio-core/src/transfer/upload.rs`
- [X] T045 [P] [US2] Implement streaming download to `.adagio_tmp_<uuid>` temp file: compute checksum incrementally during stream (SHA-256 or server-preferred algorithm per capabilities), verify against server-provided `OC-Checksum` / `ETag` value on completion; delete temp file and enqueue retry on mismatch; surface repeated mismatches as permanent errors; atomically rename temp file to final path only on verified success in `crates/adagio-core/src/transfer/download.rs`
- [X] T046 [US2] Implement `Propagator` with bounded concurrency (3 up + 3 down via `tokio::sync::Semaphore`), folder ordering, and journal update after each op in `crates/adagio-core/src/cycle/propagator.rs`
- [X] T047 [P] [US2] Implement server-side MOVE for remote renames; local `fs::rename` for local renames in `crates/adagio-core/src/cycle/propagator.rs`
- [X] T048 [US2] Implement `SyncCycle` orchestrator: run discovery → reconciliation → propagation in sequence in `crates/adagio-core/src/cycle/mod.rs`
- [X] T049 [US2] Implement `DefaultSyncEngine` satisfying `SyncEngine` trait: per-pair cycle coalescing, `tokio::sync::watch` status broadcast in `crates/adagio-core/src/cycle/mod.rs`
- [X] T050 [P] [US2] Implement in-memory `ActivityLog` (ring buffer of last 500 entries) in `crates/adagio-core/src/observability.rs`
- [X] T051 [P] [US2] Implement Tauri IPC commands for sync control and status (`get_status`, `pause_sync`, `resume_sync`, `get_activity_log`) in `crates/adagio-desktop/src/commands/sync.rs`
- [X] T052 [US2] Implement sync status dashboard in Svelte (per-pair status badge, progress bar, activity log feed) in `crates/adagio-desktop/src-ui/routes/Dashboard.svelte`
- [X] T112 [P] [US2] Add `scan_interval_secs: u64` (default 7200) and `scan_on_startup: bool` (default true) fields to `SyncPair` config in `crates/adagio-core/src/config.rs`; wire values into T040's scan scheduler; expose a "Scan interval" input (in minutes) in the pair settings screen in `crates/adagio-desktop/src-ui/routes/Settings.svelte`

**Checkpoint**: User Story 2 fully functional and independently testable

---

## Phase 5: User Story 3 — Conflict Detection and Resolution (Priority: P2)

**Goal**: Same-item concurrent modifications produce deterministic, policy-driven
outcomes with no silent data loss. Conflicts are surfaced and logged.

**Independent Test**: Modify same file on both sides; verify "preserve both" policy
produces conflict copy with correct naming; verify conflicts view lists the record.

### Tests for User Story 3 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T053 [P] [US3] Write unit tests for conflict suffix name format in `crates/adagio-core/src/conflict.rs`
- [X] T054 [P] [US3] Write integration tests for all 5 conflict policies in `tests/integration/conflict.rs`

### Implementation for User Story 3

- [X] T055 [US3] Implement conflict detection in `Reconciler` (both-sides-changed and delete-vs-change categories) in `crates/adagio-core/src/cycle/reconciler.rs`
- [X] T056 [P] [US3] Implement `ConflictResolver` for all 5 policies in `crates/adagio-core/src/conflict.rs`: PreserveBoth (download remote to normal path + rename local + upload copy), LocalWins, RemoteWins, NewestWins (compare mtimes), Ask (send event to UI channel, pause item)
- [X] T057 [P] [US3] Implement conflict copy naming: `<stem> (conflicted copy from <device> <YYYY-MM-DD HH-MM-SS>)<ext>` in `crates/adagio-core/src/conflict.rs`
- [X] T058 [P] [US3] Persist `ConflictRecord` to SQLite after each resolution in `crates/adagio-core/src/conflict.rs`
- [X] T059 [US3] Implement Tauri IPC commands `list_conflicts` (current + historical) and `resolve_conflict` (for Ask policy) in `crates/adagio-desktop/src/commands/conflicts.rs`
- [X] T060 [P] [US3] Implement Svelte conflicts view: list with path, devices, timestamps, policy applied in `crates/adagio-desktop/src-ui/routes/Conflicts.svelte`
- [X] T107 [P] [US3] Write unit test for "Ask" conflict policy: configure a pair with `ConflictPolicy::Ask`, trigger a conflict, verify the `Propagator` emits a `pending_conflict` status and does NOT resolve the item until a `resolve_conflict` call fires the oneshot receiver in `crates/adagio-core/src/cycle/propagator.rs` — verify test FAILS before T108
- [X] T108 [US3] Implement "Ask" policy suspension in `Propagator`: for each `Conflict { policy: Ask }` item, create a `tokio::sync::oneshot::channel`, store the sender in a `pending_conflicts: DashMap<RelativePath, oneshot::Sender<ConflictSide>>` on `DefaultSyncEngine`, update pair status to include the blocked path, then `oneshot::receiver.await` the resolution; when `resolve_conflict` Tauri command is called, retrieve the sender by path and send the `ConflictSide` choice to unblock propagation in `crates/adagio-core/src/cycle/propagator.rs` and `crates/adagio-core/src/cycle/mod.rs`
- [X] T109 [P] [US3] Implement real-time conflict resolution modal in `crates/adagio-desktop/src-ui/lib/ConflictModal.svelte`: shown automatically when `PairStatus` contains a `pending_conflict` path; display both file versions (local mtime, remote mtime, file size); provide "Keep local", "Keep remote", and "Keep both" choices; send the choice via the `resolve_conflict` Tauri command; dismiss on resolution; distinct from the history list in T060

**Checkpoint**: User Story 3 fully functional and independently testable

---

## Phase 6: User Story 4 — Large File Transfer and Bandwidth Management (Priority: P2)

**Goal**: Files of any size transfer reliably. Interrupted transfers resume from where
they stopped. Bandwidth limits apply at configured rates and scheduled windows.

**Independent Test**: Upload a 100 MB file, kill the process mid-upload, restart, verify
resume from last chunk with no retransmission. Verify bandwidth cap is respected.

### Tests for User Story 4 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T061 [P] [US4] Write integration tests for chunked upload (> 10 MB), mid-upload interruption, and resume in `tests/integration/transfer.rs`
- [X] T062 [P] [US4] Write integration tests for download interruption and range-request resume in `tests/integration/transfer.rs`
- [X] T063 [P] [US4] Write unit tests for token-bucket rate limiter behavior in `crates/adagio-core/src/bandwidth.rs`

### Implementation for User Story 4

- [X] T064 [US4] Implement Nextcloud chunked upload: create `.uploads/<session_id>/` on server, PUT each chunk, MOVE to final path in `crates/adagio-nextcloud/src/chunked.rs`
- [X] T065 [P] [US4] Implement chunked upload resume: list existing chunks via PROPFIND on session dir, skip already-uploaded chunk indices in `crates/adagio-nextcloud/src/chunked.rs`
- [X] T066 [P] [US4] Implement download range-request resume (`Range: bytes=<offset>-`) appending to existing temp file in `crates/adagio-core/src/transfer/download.rs`
- [X] T067 [US4] Implement `TransferManager` routing: single-PUT for files below threshold, chunked for files at/above threshold; wire progress events via `mpsc` in `crates/adagio-core/src/transfer/mod.rs`
- [X] T068 [P] [US4] Implement token-bucket rate limiter for upload and download with per-direction caps in `crates/adagio-core/src/bandwidth.rs`
- [X] T069 [P] [US4] Implement time-window bandwidth schedule evaluation (apply correct cap based on current time and day-of-week) in `crates/adagio-core/src/bandwidth.rs`
- [X] T070 [US4] Implement metered-connection detection and non-essential sync suspension in `crates/adagio-core/src/cycle/mod.rs`
- [X] T071 [P] [US4] Implement battery-level detection (via platform API) and sync suspension below configured threshold in `crates/adagio-core/src/cycle/mod.rs`
- [X] T072 [US4] Add bandwidth limits settings UI (upload KB/s, download KB/s, schedule editor) in `crates/adagio-desktop/src-ui/routes/Settings.svelte`
- [X] T113 [P] [US4] Add `max_upload_concurrency: u8` (default 3) and `max_download_concurrency: u8` (default 3) to `SyncPair` config in `crates/adagio-core/src/config.rs`; replace the hardcoded `Semaphore` permit counts in T046's `Propagator` with values read from config at cycle start; add "Max concurrent transfers" inputs (upload / download) alongside the bandwidth settings in `crates/adagio-desktop/src-ui/routes/Settings.svelte`

**Checkpoint**: User Story 4 fully functional and independently testable

---

## Phase 7: User Story 5 — Error Visibility and Recovery (Priority: P2)

**Goal**: No sync failure is silent. Transient errors retry with backoff; permanent
errors surface to the user. Failed items never block others.

**Independent Test**: Induce transient 5xx errors → verify retry with backoff and
eventual success. Induce permanent error on one item → verify other items complete.

### Tests for User Story 5 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T073 [P] [US5] Write unit tests for exponential backoff timing (1 s start, 2× doubling, 5 min cap, jitter) in `crates/adagio-core/src/error.rs`
- [X] T074 [P] [US5] Write integration tests for transient retry, per-item isolation, and credential expiry in `tests/integration/error_recovery.rs`

### Implementation for User Story 5

- [X] T075 [US5] Implement exponential backoff retry (1 s start, doubling, 5 min cap, jitter, max 5 attempts then park) in `crates/adagio-core/src/cycle/propagator.rs`
- [X] T076 [P] [US5] Implement per-item error isolation in `Propagator`: catch each item's error independently so other items continue in `crates/adagio-core/src/cycle/propagator.rs`
- [X] T077 [P] [US5] Implement HTTP 401 detection: pause account, emit re-auth signal on `tokio::sync::watch` channel in `crates/adagio-nextcloud/src/webdav.rs`
- [X] T078 [US5] Add disk-full error handling to the download path established in T045: catch `ErrorKind::StorageFull` (or OS-equivalent) around the temp-file write and final rename; on disk-full, delete the partial temp file and surface as `SyncError::Permanent("disk full")`; the destination file at the final path is left intact because T045 never writes there until the rename succeeds in `crates/adagio-core/src/transfer/download.rs`
- [X] T079 [P] [US5] Implement `in-progress write` detection: if file mtime/size change during checksumming, defer upload to next cycle in `crates/adagio-core/src/detection/local.rs`
- [X] T080 [P] [US5] Implement Tauri IPC command `get_error_items` returning path, operation, error category, cause for all parked items in `crates/adagio-desktop/src/commands/sync.rs`
- [X] T081 [P] [US5] Implement diagnostic bundle export: compress logs + redacted config + journal row-count stats into zip file in `crates/adagio-core/src/observability.rs`
- [X] T082 [US5] Add re-auth prompt UI (modal when account enters `AuthRequired` status) and errors list in `crates/adagio-desktop/src-ui/routes/Dashboard.svelte`

**Checkpoint**: User Story 5 fully functional and independently testable

---

## Phase 8: User Story 6 — Selective Sync (Priority: P3)

**Goal**: Users can exclude remote subdirectories from local materialization. Exclusions
take effect on the next cycle; re-inclusion triggers download.

**Independent Test**: Exclude a subdirectory before first sync → verify no local files
created for it. Re-include → verify files download on next cycle.

### Tests for User Story 6 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T083 [P] [US6] Write integration tests for exclude-before-sync, mid-run exclusion, and re-inclusion in `tests/integration/sync_cycle.rs`

### Implementation for User Story 6

- [X] T084 [US6] Implement selective sync exclusion list on `SyncPair` config: add/remove exclusion paths in `crates/adagio-core/src/config.rs`
- [X] T085 [P] [US6] Implement selective sync filtering in discovery phase: skip remote items whose paths are under an exclusion in `crates/adagio-core/src/detection/mod.rs`
- [X] T086 [P] [US6] Implement local removal of files when their path is newly added to the exclusion list (emit `DeleteLocal` ops in reconciler) in `crates/adagio-core/src/cycle/reconciler.rs`
- [X] T087 [P] [US6] Implement Tauri IPC command `list_remote_tree` returning all remote items including excluded ones (for remote browser) in `crates/adagio-desktop/src/commands/pair.rs`
- [X] T088 [US6] Implement selective sync UI: tree browser showing remote dirs with checkboxes, excluded dirs greyed-out but visible in `crates/adagio-desktop/src-ui/routes/Pairs.svelte`

**Checkpoint**: User Story 6 fully functional and independently testable

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, benchmarks, path safety, and log hygiene across all stories.

- [X] T089 [P] Add `///` doc comments to all public items in `crates/adagio-core/src/` (run `cargo doc --no-deps` and fix all warnings)
- [X] T090 [P] Add `///` doc comments to all public items in `crates/adagio-nextcloud/src/`
- [X] T091 Add criterion benchmark for `Reconciler` three-way diff hot path with 100k entries in `benches/reconciler.rs`
- [X] T092 [P] Add criterion benchmark for SHA-256 streaming throughput (1 MB, 10 MB, 100 MB) in `benches/checksum.rs`
- [X] T093 [P] Write Architecture Decision Records in `docs/adr/001-gui-framework.md`, `docs/adr/002-journal-storage.md`, `docs/adr/003-webdav-client.md`
- [X] T094 Implement path compatibility checker (reserved Windows names CON/PRN/AUX/NUL/COM1-9/LPT1-9, disallowed characters, trailing dot/space on Windows, path-too-long, case-only collision) in `crates/adagio-core/src/path_compat.rs`
- [X] T095 [P] Wire path compatibility check into `Propagator` before each local/remote operation; surface as permanent error when violated in `crates/adagio-core/src/cycle/propagator.rs`
- [X] T096 [P] Configure log rotation via `tracing-appender` (max 10 MB per file, 5 rotations) in `crates/adagio-core/src/observability.rs`
- [X] T115 [P] Add `benches/transfer_throughput.rs`: criterion benchmark that measures upload and download throughput (MB/s) against an in-process mock `RemoteClient` using a 50 MB payload; configure criterion to emit a CI-visible alert on any regression exceeding 10% vs the saved baseline in `benches/transfer_throughput.rs`
- [X] T116 Add CI memory and CPU gate steps to `.github/workflows/ci.yml` (Linux job): after integration tests build the release binary, measure idle RSS using `valgrind --tool=massif --pages-as-heap=yes` with a 30-second idle window; parse the peak heap value and fail the step if it exceeds 100 MB; add a `cargo criterion -- --color never 2>&1 | tee criterion_out.txt` step and grep for regression notices to fail the build (Constitution Principle V: <100 MB RSS, benchmarks required for hot paths)
- [X] T117 [P] Review `specs/001-nextcloud-file-sync/spec.md` Assumptions and FR descriptions for any divergence from the implemented behaviour; update to reflect actuals (Constitution Principle II: specs/ MUST remain in sync with implemented feature set); update `CLAUDE.md` spec reference if the spec filename or location has changed
- [X] T097 Run `quickstart.md` validation: execute every command in the guide end-to-end and fix any discrepancies

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 completion — BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Foundational completion — no dependencies on other stories
- **US2 (Phase 4)**: Depends on Foundational + US1 (account/pair infrastructure). Start after US1 milestone
- **US3 (Phase 5)**: Depends on US2 (conflict detection requires reconciler from US2)
- **US4 (Phase 6)**: Depends on US2 (chunked upload extends the upload path from US2). Can run in parallel with US3
- **US5 (Phase 7)**: Depends on US2 (error handling in propagator from US2). Can run in parallel with US3/US4
- **US6 (Phase 8)**: Depends on US2 (selective sync filters the discovery phase). Can run in parallel with US3/US4/US5
- **Polish (Phase 9)**: Depends on all desired user stories being complete

### User Story Dependencies

- **US1 (P1)**: Can start after Foundational — no story dependencies
- **US2 (P1)**: Requires US1 account/pair management infrastructure
- **US3 (P2)**: Requires US2 reconciler and propagator
- **US4 (P2)**: Requires US2 upload/download transfer path
- **US5 (P2)**: Requires US2 propagator
- **US6 (P3)**: Requires US2 detection and reconciler

### Within Each User Story

1. Test tasks MUST be written first and MUST FAIL before implementation begins
2. Models/types before services
3. Services before Tauri IPC commands
4. IPC commands before Svelte UI
5. Integration tests at the end of each story for acceptance verification

### Parallel Opportunities (within Phase 2)

```bash
# All trait definitions can run in parallel:
Task: T010 — Journal trait
Task: T011 — RemoteClient trait
Task: T012 — SyncEngine trait
Task: T013 — ChangeDetector trait
Task: T014 — TransferManager trait

# Contract tests in parallel after traits:
Task: T015 — Journal contract tests
Task: T016 — RemoteClient contract tests
```

### Parallel Opportunities (within Phase 4 — US2)

```bash
# Detection tasks in parallel:
Task: T039 — Local filesystem watcher
Task: T041 — Remote PROPFIND polling

# Transfer tasks in parallel:
Task: T044 — Single-PUT upload
Task: T045 — Streaming download
```

---

## Implementation Strategy

### MVP First (User Stories 1 + 2 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks all stories)
3. Complete Phase 3: User Story 1 (account + pair + first sync)
4. Complete Phase 4: User Story 2 (ongoing bidirectional sync)
5. **STOP and VALIDATE**: Test US1 + US2 independently → accounts configure, files sync
6. Demo / dogfood with a real Nextcloud server

### Incremental Delivery

1. Setup + Foundational → workspace compiles
2. US1 → account setup + first sync works
3. US2 → ongoing bidirectional sync works → **Minimal shippable sync client**
4. US3 → conflicts handled safely
5. US4 (parallel with US3) → large files + bandwidth control
6. US5 (parallel with US3/US4) → full error transparency
7. US6 → selective sync
8. Polish → production-ready

### Parallel Team Strategy (if staffed)

After Phase 2 completes:
- Developer A: US1 + US2 (P1 critical path)
- Developer B: US3 + US4 (conflict + transfer polish)
- Developer C: US5 + US6 (error handling + selective sync)

---

## Notes

- `[P]` tasks = different files, no incomplete dependencies — safe to parallelize
- `[USN]` label maps each task to a specific user story for traceability
- Each user story should be independently completable and testable
- Tests MUST fail before implementing — this is non-negotiable (Constitution Principle I)
- Commit after each completed task or logical group
- Stop at each phase checkpoint to validate the story independently
- Never write a task without a concrete file path — vague tasks cannot be executed
