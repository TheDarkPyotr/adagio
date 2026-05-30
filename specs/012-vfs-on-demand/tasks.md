# Tasks: VFS On-demand Files

**Input**: Design documents from `specs/012-vfs-on-demand/`

**Prerequisites**: plan.md ✅ | spec.md ✅ | research.md ✅ | data-model.md ✅ | contracts/vfs-ipc.md ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins.

**Format**: `[ID] [P?] [Story?] Description`
- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[US#]**: User story this task belongs to

---

## Phase 1: Setup

**Purpose**: New crate scaffold, ADR, database migration, config fields.

- [X] T001 Create `docs/adr/016-vfs-platform-drivers.md` — document decision to use `fuse3` (Linux), `objc2-file-provider` (macOS), `wincs` (Windows); in-process FUSE handler; SQLite LRU eviction; `VfsProvider` trait in new `adagio-vfs` crate
- [X] T002 Create `crates/adagio-vfs/` crate scaffold: `Cargo.toml` with platform-gated deps (`fuse3` on Linux, `objc2-file-provider` on macOS, `wincs` on Windows), `src/lib.rs` exporting `VfsProvider` trait, `VfsError`, `VfsState`, `VfsCacheEntry`, `VfsMountHandle`; add `adagio-vfs` to workspace `Cargo.toml`
- [X] T003 [P] Create `crates/adagio-core/migrations/003_vfs_tables.sql` with `vfs_cache_metadata` (pair_id, path, remote_size, remote_etag, remote_mtime, state, cached_at, last_accessed_at, cache_bytes) and `vfs_pinned_paths` (pair_id, path, pinned_at) tables plus indices; run migration in `SqliteJournal::open()`
- [X] T004 [P] Add three VFS config fields with `#[serde(default)]` to `SyncPair` in `crates/adagio-core/src/types.rs`: `vfs_enabled: bool` (false), `vfs_cache_max_bytes: u64` (20 GiB), `vfs_eviction_threshold_bytes: u64` (5 GiB); mirror on `SavedPair` in `crates/adagio-desktop/src/config/mod.rs`; update `restore_pairs()` in `crates/adagio-daemon/src/main.rs`

**Checkpoint**: `cargo build --workspace` clean. New crate compiles with empty impls.

---

## Phase 2: Foundational — VfsProvider trait + Journal extension

**Purpose**: The `VfsProvider` trait, `VfsJournal` query helpers, and `MockVfsProvider` that all user story phases depend on.

### Tests (write first — must FAIL)

- [X] T005 [P] Write failing unit tests `vfs_state_transitions_are_valid`, `vfs_cache_entry_default_is_cloud_only` in `crates/adagio-vfs/src/lib.rs` test module — assert valid and invalid state transitions per spec FR-1
- [X] T006 [P] Write failing unit tests `vfs_journal_upsert_returns_entry`, `vfs_journal_lru_query_orders_by_last_accessed` in `crates/adagio-core/src/vfs/journal.rs` test module — use in-memory SQLite

### Implementation

- [X] T007 Implement `VfsProvider` trait in `crates/adagio-vfs/src/lib.rs`:
  - `is_supported() -> bool`
  - `async fn mount(mount_point, pair, client: Arc<dyn RemoteClient>, journal: Arc<dyn Journal>) -> Result<VfsMountHandle, VfsError>`
  - `async fn unmount(pair_id) -> Result<(), VfsError>`
  - `async fn update_placeholders(entries: &[VfsCacheEntry]) -> Result<(), VfsError>`
  - `async fn set_locally_available(path) -> Result<(), VfsError>`
  - `async fn set_pinned(path) -> Result<(), VfsError>`
  - `async fn set_cloud_only(path) -> Result<(), VfsError>`
- [X] T008 [P] Implement `MockVfsProvider` in `crates/adagio-vfs/src/mock.rs` — records all calls, returns configurable results; used in all unit tests
- [X] T009 [P] Implement `FallbackVfsProvider` in `crates/adagio-vfs/src/fallback.rs` — `is_supported()` returns false; all operations return `VfsError::NotSupported`
- [X] T010 Create `crates/adagio-core/src/vfs/mod.rs` + `crates/adagio-core/src/vfs/journal.rs` with `VfsJournal` extension trait on `SqliteJournal`:
  - `async fn upsert_vfs_entry(entry: &VfsCacheEntry) -> Result<(), JournalError>`
  - `async fn all_vfs_entries(pair_id: &PairId) -> Result<Vec<VfsCacheEntry>, JournalError>`
  - `async fn get_vfs_entry(pair_id, path) -> Result<Option<VfsCacheEntry>, JournalError>`
  - `async fn vfs_lru_candidates(pair_id, limit: usize) -> Result<Vec<VfsCacheEntry>, JournalError>` — `ORDER BY last_accessed_at ASC WHERE state = 'locally_available'`
  - `async fn is_path_pinned(pair_id, path) -> Result<bool, JournalError>` — checks `vfs_pinned_paths` with directory prefix matching
- [X] T011 [P] Add `pub mod vfs` to `crates/adagio-core/src/lib.rs`; add `adagio-vfs` dependency to `crates/adagio-core/Cargo.toml`

**Checkpoint**: `cargo test --lib -p adagio-core` T005–T006 pass. `cargo build` clean.

---

## Phase 3: User Story 1 — Browse All Files Without Downloading (Priority: P1) 🎯 MVP

**Goal**: When a VFS-mode pair is enabled, all remote files appear as cloud-only placeholders in the local folder immediately, with metadata populated from the remote snapshot.

**Independent Test**: Given a remote with 100 files, after `VfsPairRunner::run_metadata_sync()`, `vfs_cache_metadata` has 100 `cloud_only` entries; `cached_bytes = 0` for all.

### Tests for US1 (write first — must FAIL)

- [X] T012 [P] [US1] Write failing unit test `metadata_sync_populates_cloud_only_entries` in `crates/adagio-core/src/vfs/mod.rs` test module — mock remote with 100 files; after `run_metadata_sync()`, assert 100 cloud_only journal entries; total `cache_bytes = 0`
- [X] T013 [P] [US1] Write failing unit test `metadata_sync_detects_remote_deletions` — remote deletes 3 files after initial sync; next `run_metadata_sync()` removes their journal entries and placeholder files
- [X] T014 [P] [US1] Write failing unit test `vfs_stats_reports_cloud_only_count` in `crates/adagio-daemon/src/dispatcher.rs` test module — dispatch `GetVfsStats`; assert `cloud_only_count` matches journal

### Implementation for US1

- [X] T015 [US1] Implement `VfsPairRunner` in `crates/adagio-core/src/vfs/mod.rs`:
  - `async fn run_metadata_sync(pair, client, journal, provider)` — fetches remote snapshot, reconciles with `vfs_cache_metadata` (upserts new/modified, deletes removed), calls `provider.update_placeholders()` for new cloud-only entries
  - Wrap in a `tokio::task::spawn` loop (same interval logic as `PairRunner`)
  - No content downloads — metadata only
- [X] T016 [US1] Add `DaemonRequest::GetVfsStats { pair_id: String }` to `crates/adagio-ipc/src/types.rs`; add handler in `crates/adagio-daemon/src/dispatcher.rs` that reads `VfsStats` from `VfsJournal`
- [X] T017 [P] [US1] Add `get_vfs_stats` Tauri command in `crates/adagio-desktop/src/commands/vfs.rs`; register in `crates/adagio-desktop/src/lib.rs`; add `VfsStatsDto` + `getVfsStats` binding in `crates/adagio-desktop/src-ui/src/tauri.ts`
- [X] T018 [P] [US1] Add VFS mode toggle and cache-usage display to `PairRow` in `crates/adagio-desktop/src-ui/src/components/PairsScene.tsx` — shows "VFS (on-demand)" vs "Copy sync" label; shows cached bytes and cloud-only count when VFS is enabled
- [X] T019 [P] [US1] Add `adagio vfs status [--pair-id ID]` CLI subcommand in `crates/adagio-cli/src/handlers/vfs.rs`; wire into `Commands::Vfs` in `crates/adagio-cli/src/cli.rs` and `crates/adagio-cli/src/run.rs`

**Checkpoint**: `cargo test -p adagio-core` T012–T013 pass. Metadata sync populates journal; stats visible via IPC and CLI.

---

## Phase 4: User Story 2 — Transparent On-demand Download (Priority: P2)

**Goal**: Opening a cloud-only file triggers a download through the FUSE handler, serving bytes to the application and caching content locally.

**Independent Test**: FUSE `read()` on a cloud-only file triggers one download call; after read, file state = `locally_available`; `cache_bytes = file_size`.

### Tests for US2 (write first — must FAIL)

- [ ] T020 [P] [US2] Write failing unit test `on_demand_read_transitions_to_locally_available` in `crates/adagio-vfs/src/linux.rs` test module (Linux only) — mock `RemoteClient`; after `read()` on cloud-only file, assert state = `locally_available` and `cache_bytes = file_size`
- [ ] T021 [P] [US2] Write failing unit test `on_demand_read_failure_keeps_cloud_only` — `RemoteClient` returns error; assert state stays `cloud_only`; `FUSE read()` returns `libc::EIO`
- [ ] T022 [P] [US2] Write failing unit test `provider_set_locally_available_updates_journal` — after `MockVfsProvider::set_locally_available(path)`, assert journal entry `state = locally_available` and `cached_at` is non-null

### Implementation for US2

- [ ] T023 [US2] Implement Linux FUSE3 driver in `crates/adagio-vfs/src/linux.rs`:
  - Struct `LinuxVfsProvider` implementing `VfsProvider`
  - Implement `fuse3::Filesystem` trait with: `lookup`, `getattr`, `readdir`/`readdirplus`, `open`, `read`
  - `read()` handler: check local cache → if miss, download via `client.download(path, Some(ByteRange { start: offset, end: offset + size }))` → write to disk cache → call `journal.upsert_vfs_entry(state: locally_available, cached_at: now, last_accessed_at: now)` → return bytes
  - `is_supported()` returns `cfg!(target_os = "linux") && /dev/fuse exists`
- [ ] T024 [P] [US2] Implement local disk cache in `crates/adagio-vfs/src/cache.rs`: `CacheStore { root_dir: PathBuf }` with `read(path, offset, size) -> Option<Bytes>` and `write(path, offset, bytes) -> Result<u64>` — stores content at `root_dir/{pair_id}/{path}`
- [ ] T025 [P] [US2] Update `VfsPairRunner` to mount the VFS provider when pair starts and unmount on stop in `crates/adagio-core/src/vfs/mod.rs`; pass `Arc<dyn RemoteClient>` and `Arc<dyn Journal>` to `provider.mount()`

**Checkpoint**: `cargo test -p adagio-vfs` T020–T022 pass (Linux). Opening a cloud-only file via FUSE triggers download; state transitions correctly.

---

## Phase 5: User Story 3 — Pin Files for Offline Access (Priority: P3)

**Goal**: User can pin a file or folder; pinned content downloads proactively and remains accessible without network.

**Independent Test**: After `SetVfsPin(path, true)`, file downloads; after taking network offline, file is still readable.

### Tests for US3 (write first — must FAIL)

- [ ] T026 [P] [US3] Write failing unit test `pin_path_downloads_content_immediately` in `crates/adagio-core/src/vfs/mod.rs` — call `pin_path(path)`; assert mock client received download call; state = `pinned`
- [ ] T027 [P] [US3] Write failing unit test `pin_directory_covers_all_children` — pin `Documents/`; assert all files under `Documents/` are queued for download
- [ ] T028 [P] [US3] Write failing unit test `dispatcher_set_vfs_pin_true_inserts_pinned_paths_row` in dispatcher tests

### Implementation for US3

- [ ] T029 [US3] Implement `pin_path(pair_id, path, client, journal, provider)` in `crates/adagio-core/src/vfs/mod.rs`:
  - Inserts into `vfs_pinned_paths`
  - Queries all journal entries under the path (via LIKE `path || '/%'`)
  - Enqueues each cloud-only file for download in background Tokio task
  - Calls `provider.set_pinned(path)` for each completed download
- [ ] T030 [US3] Add `DaemonRequest::SetVfsPin { pair_id, path, pinned: bool }` to `crates/adagio-ipc/src/types.rs`; handler in dispatcher calls `pin_path` or `unpin_path` accordingly; persist pin rows; call `save_config` if needed
- [ ] T031 [P] [US3] Add `set_vfs_pin` Tauri command in `crates/adagio-desktop/src/commands/vfs.rs`; add `setVfsPin` binding in `crates/adagio-desktop/src-ui/src/tauri.ts`
- [ ] T032 [P] [US3] Add `adagio vfs pin <path>` and `adagio vfs unpin <path>` CLI subcommands in `crates/adagio-cli/src/handlers/vfs.rs`

**Checkpoint**: Pin triggers download; pinned files survive daemon restart; `vfs_pinned_paths` table populated.

---

## Phase 6: User Story 4 — Evict Cached Files (Priority: P4)

**Goal**: User can evict locally-available files to reclaim disk space; automatic LRU eviction triggers on low disk.

**Independent Test**: After `evict_file(path)`, local content removed; `cache_bytes = 0`; state = `cloud_only`; file entry still visible.

### Tests for US4 (write first — must FAIL)

- [ ] T033 [P] [US4] Write failing unit test `evict_file_removes_content_updates_state` in `crates/adagio-core/src/vfs/eviction.rs` — file in locally_available; after `evict(path)`, assert `cache_bytes = 0`, `state = cloud_only`, entry still exists in journal
- [ ] T034 [P] [US4] Write failing unit test `evict_pinned_file_returns_error` — pinned file; evict returns `VfsError::PathIsPinned`
- [ ] T035 [P] [US4] Write failing unit test `auto_eviction_uses_lru_order` — 5 locally_available files with different last_accessed_at; trigger eviction needing N bytes; assert least-recently-accessed files are evicted first

### Implementation for US4

- [ ] T036 [US4] Implement `crates/adagio-core/src/vfs/eviction.rs`:
  - `async fn evict_file(pair_id, path, journal, provider, cache)` — checks not pinned; deletes from `CacheStore`; sets state = `cloud_only`, `cached_at = null`, `cache_bytes = 0`; calls `provider.set_cloud_only(path)`
  - `async fn run_auto_eviction(pair_id, journal, provider, cache, threshold_bytes)` — checks free disk; runs LRU query; evicts until free disk >= threshold or no more evictable files
- [ ] T037 [US4] Add auto-eviction check to `VfsPairRunner` post-metadata-sync step in `crates/adagio-core/src/vfs/mod.rs`; call `run_auto_eviction` if free disk < threshold
- [ ] T038 [US4] Add `DaemonRequest::EvictVfsFile { pair_id, path }` to `crates/adagio-ipc/src/types.rs`; handler in dispatcher calls `evict_file`; returns error if pinned
- [ ] T039 [P] [US4] Add `evict_vfs_file` Tauri command; add `evictVfsFile` binding in `tauri.ts`; add evict button in `PairsScene.tsx` (shown on hover over locally-available file entries)
- [ ] T040 [P] [US4] Add `adagio vfs evict <path>` and `adagio vfs evict --all` CLI subcommands in `crates/adagio-cli/src/handlers/vfs.rs`

**Checkpoint**: Manual and auto eviction work; pinned files protected; LRU order verified.

---

## Phase 7: User Story 5 — Coexistence with Copy Sync (Priority: P5)

**Goal**: VFS pairs and copy-sync pairs run independently on the same daemon without interference. Switching a pair from copy sync to VFS is non-destructive.

**Independent Test**: Running both a copy-sync pair cycle and a VFS metadata sync simultaneously produces no errors; each pair's journal entries are isolated.

### Tests for US5 (write first — must FAIL)

- [ ] T041 [P] [US5] Write failing integration test `copy_sync_and_vfs_pairs_run_concurrently_without_interference` in `crates/adagio-core/src/vfs/mod.rs` — two pairs with same account; one copy-sync, one VFS; run both cycles; assert each has correct journal entries with no cross-contamination
- [ ] T042 [P] [US5] Write failing unit test `switch_copy_to_vfs_converts_synced_files_to_locally_available` — pair has 50 synced copy-sync files; `convert_to_vfs(pair)` runs; assert all 50 files now have state = `locally_available` in `vfs_cache_metadata`; original files untouched on disk

### Implementation for US5

- [ ] T043 [US5] Implement `convert_pair_to_vfs(pair_id, journal)` in `crates/adagio-core/src/vfs/mod.rs` — reads all `journal_entries` for the pair (status=Synced); inserts corresponding `vfs_cache_metadata` rows with `state = locally_available`, `cached_at = now`, `cache_bytes = file_size`; sets `pair.vfs_enabled = true`
- [ ] T044 [P] [US5] Update `DefaultSyncEngine::start_pair()` in `crates/adagio-core/src/cycle/mod.rs` to check `pair.vfs_enabled`; if true, spawn `VfsPairRunner` instead of copy-sync `PairRunner`
- [ ] T045 [P] [US5] Update `CreatePair` handler in `crates/adagio-daemon/src/dispatcher.rs` to pass `vfs_enabled`, `vfs_cache_max_bytes`, `vfs_eviction_threshold_bytes` from request params; update `DaemonRequest::CreatePair` in IPC types

**Checkpoint**: Both pair types coexist; switching from copy to VFS converts without data loss; `cargo test --workspace` all pass.

---

## Phase 8: macOS + Windows Platform Drivers (Parallel with Phase 7)

**Purpose**: Platform-specific `VfsProvider` implementations for macOS FileProvider and Windows CfApi.

- [ ] T046 [P] Implement `MacosVfsProvider` in `crates/adagio-vfs/src/macos.rs` using `objc2-file-provider`:
  - `is_supported()` — returns `cfg!(target_os = "macos")` && macOS version >= 12
  - `mount()` — registers `NSFileProviderDomain`; starts the FileProvider extension session
  - `update_placeholders()` — calls `providePlaceholder(at:)` for each cloud-only entry
  - `set_locally_available()` / `set_pinned()` / `set_cloud_only()` — updates file provider state
  - Hydration callback (`fetchContents(for:)`) delegates to `fetch_content()` using `Arc<dyn RemoteClient>`
- [ ] T047 [P] Implement `WindowsVfsProvider` in `crates/adagio-vfs/src/windows.rs` using `wincs`:
  - `is_supported()` — returns `cfg!(windows)` && Windows 10 1709+
  - `mount()` — registers sync root via `CfRegisterSyncRoot()`; starts callback loop
  - `update_placeholders()` — calls `CfCreatePlaceholders()` in batches
  - `CF_CALLBACK_TYPE_FETCH_DATA` handler — downloads content via `Arc<dyn RemoteClient>`; calls `CfExecute(TRANSFER_DATA)`
  - `CfSetPinState()` for pin/unpin operations
- [ ] T048 [P] Add platform selector in `crates/adagio-vfs/src/lib.rs`:
  ```rust
  pub fn create_platform_provider() -> Arc<dyn VfsProvider> {
      #[cfg(target_os = "linux")]   { Arc::new(LinuxVfsProvider::new()) }
      #[cfg(target_os = "macos")]   { Arc::new(MacosVfsProvider::new()) }
      #[cfg(windows)]               { Arc::new(WindowsVfsProvider::new()) }
      #[cfg(not(any(...)))]         { Arc::new(FallbackVfsProvider) }
  }
  ```

---

## Phase 9: Polish & Cross-Cutting Concerns

- [ ] T049 `cargo clippy --workspace -- -D warnings` — fix all warnings
- [ ] T050 [P] `cargo fmt --all -- --check`
- [ ] T051 `cargo test --workspace` — all tests pass including VFS unit tests
- [ ] T052 [P] `cd crates/adagio-desktop/src-ui && npm test` — 102+ UI tests pass
- [ ] T053 [P] Add `vfs_enabled` and `cache_max_bytes` to `CreatePair` UI form in `crates/adagio-desktop/src-ui/src/components/PairsScene.tsx` — show VFS option only when platform supports it (`getVfsSupported()` Tauri command)
- [ ] T054 [P] Run manual validation per `specs/012-vfs-on-demand/quickstart.md` — US1 metadata scan, US2 on-demand open, US3 pin + offline, US4 evict + re-open

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all story phases
- **Phase 3 (US1)**: Depends on Phase 2 — MVP; implement first; Linux FUSE not required
- **Phase 4 (US2)**: Depends on Phase 3 (FUSE handler needs journal entries from US1)
- **Phase 5 (US3)**: Depends on Phase 3 (pin needs cloud-only entries)
- **Phase 6 (US4)**: Depends on Phase 3 + Phase 4 (eviction needs locally_available state)
- **Phase 7 (US5)**: Depends on Phase 3 (pair routing needs VfsPairRunner)
- **Phase 8 (Platform)**: Parallel with Phase 7; depends on Phase 2 (VfsProvider trait)
- **Phase 9 (Polish)**: Depends on all prior phases

### Parallel Opportunities

**Phase 1**: T003, T004 touch different files — [P].  
**Phase 2**: T005, T006 tests; T007-T009 impls all different files — [P].  
**Phase 3**: T012–T014 tests all [P]; T017–T019 different files [P].  
**Phase 4**: T020–T022 tests [P]; T024–T025 different files [P].  
**Phase 8**: T046, T047 different platform files — [P].

---

## Implementation Strategy

### MVP (Phases 1–3, US1 only — no FUSE required)

1. Phase 1: Crate scaffold + DB migration + config fields
2. Phase 2: VfsProvider trait + VfsJournal + MockVfsProvider
3. Phase 3: VfsPairRunner metadata sync + GetVfsStats IPC + CLI `adagio vfs status`

**MVP delivers**: daemon populates VFS journal from remote; stats visible via CLI and IPC. No OS-level virtual folder yet — that's US2 (FUSE). This proves the core data model and sync pipeline before touching OS-specific APIs.

### Incremental Delivery

1. Phases 1–3 → metadata pipeline works (testable on all platforms)
2. Phase 4 → FUSE/on-demand works on Linux
3. Phases 5–6 → pin and eviction work
4. Phase 7 → coexistence proven
5. Phase 8 → macOS/Windows platform drivers

---

## Notes

- `VfsProvider::is_supported()` = false on unsupported platforms → VFS toggle hidden in UI; no crash
- All FUSE filesystem operations run as Tokio tasks; `read()` awaits download inline (blocks kernel request, which is correct FUSE behavior)
- The `vfs_cache_metadata` table is separate from `journal_entries` to avoid bloating the existing schema
- Total: **54 tasks** across 9 phases
