# Tasks: Bulk Upload Driver

**Input**: Design documents from `specs/011-bulk-upload-driver/`

**Prerequisites**: plan.md ✅ | spec.md ✅ | research.md ✅ | data-model.md ✅ | contracts/bulk-upload-ipc.md ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins.

**Format**: `[ID] [P?] [Story?] Description`
- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[US#]**: User story this task belongs to

---

## Phase 1: Setup

**Purpose**: ADR and module skeleton.

- [X] T001 Create `docs/adr/015-bulk-upload-driver.md` — document decision to use `JoinSet`-based worker pool activating inside `SyncCycle::run()` between reconciliation and propagation; reuse `upload_chunked`/`upload_single`; no new IPC types; `upsert_batch` for journal atomicity
- [X] T002 [P] Add three bulk config fields with `#[serde(default)]` to `SyncPair` in `crates/adagio-core/src/types.rs`: `bulk_upload_workers: u8` (default 8), `bulk_upload_threshold_files: u32` (default 50), `bulk_upload_chunk_threshold_bytes: u64` (default 10_485_760); add `default_bulk_upload_workers()`, `default_bulk_upload_threshold_files()`, `default_bulk_upload_chunk_threshold_bytes()` helper fns
- [X] T003 [P] Add same three fields with `#[serde(default)]` to `SavedPair` in `crates/adagio-desktop/src/config/mod.rs`; update `restore_pairs()` in `crates/adagio-daemon/src/main.rs` to copy bulk fields from `SavedPair` to `SyncPair`

**Checkpoint**: `cargo build` clean. Bulk config fields exist on both structs.

---

## Phase 2: Foundational — BulkUploadDriver skeleton

**Purpose**: Core struct, `should_activate()` function, and `BulkUploadResult` type. All
user story phases depend on these. Test them independently before wiring into `SyncCycle`.

**⚠️ CRITICAL**: No story implementation can compile until these types exist.

### Tests (write first — must FAIL)

- [X] T004 [P] Write failing unit tests `should_activate_when_remote_empty_and_enough_files`, `should_not_activate_below_threshold`, `should_not_activate_when_remote_not_empty` in `crates/adagio-core/src/bulk_upload.rs` test module — assert activation condition logic with various remote/local counts against a `SyncPair` with default bulk config
- [X] T005 [P] Write failing unit test `bulk_upload_result_tracks_counts` — construct `BulkUploadResult`, assert uploaded/skipped/errors/bytes fields are summed correctly with `+=`

### Implementation

- [X] T006 Create `crates/adagio-core/src/bulk_upload.rs` with:
  - `BulkUploadResult { uploaded: u32, skipped: u32, errors: u32, bytes: u64 }` with `+=` impl
  - `BulkUploadDriver<'a>` struct holding `pair`, `client`, `journal`, `broadcaster: Option<EventBroadcaster>`
  - `BulkUploadDriver::should_activate(remote_count: usize, upload_count: usize, pair: &SyncPair) -> bool` — returns true when `upload_count >= pair.bulk_upload_threshold_files as usize && remote_count < (upload_count as f64 * 0.10).ceil() as usize`
  - `BulkUploadDriver::new(...)` constructor
  - All public items have `///` doc comments
- [X] T007 [P] Add `pub mod bulk_upload` to `crates/adagio-core/src/lib.rs`

**Checkpoint**: `cargo test --lib -p adagio-core` — T004, T005 pass. `cargo build` clean.

---

## Phase 3: User Story 1 — Parallel Uploads (Priority: P1) 🎯 MVP

**Goal**: `BulkUploadDriver::run()` uploads files in parallel using a `JoinSet` worker pool.
`upload_single` for files below threshold, `upload_chunked` for files at or above.

**Independent Test**: Give the driver 20 mock files; assert all 20 are uploaded, journal has
20 Synced entries, and the mock's upload call count equals 20.

### Tests for US1 (write first — must FAIL)

- [X] T008 [P] [US1] Write failing unit test `driver_uploads_all_files_in_parallel` in `crates/adagio-core/src/bulk_upload.rs` — 20 small files, `workers=4`; after `driver.run(ops, ...)`, assert `MockRemoteClient` received 20 upload calls and journal has 20 Synced entries
- [X] T009 [P] [US1] Write failing unit test `driver_routes_large_file_to_chunked` — 1 file above `bulk_upload_chunk_threshold_bytes`; assert `begin_chunked_upload` was called (not the single-PUT path)
- [X] T010 [P] [US1] Write failing unit test `driver_result_counts_match` — 5 files, all succeed; assert `result.uploaded == 5`, `result.errors == 0`, `result.bytes == total_size`

### Implementation for US1

- [X] T011 [US1] Implement `BulkUploadDriver::run()` in `crates/adagio-core/src/bulk_upload.rs`:
  - Extract `path` from each `SyncOp::Upload { path, local_checksum }`
  - Skip paths already Synced in journal (call `journal.all_entries` at start, build a `HashSet<RelativePath>` of Synced paths)
  - Skip paths with permanent errors (check journal for `status == Error && error_message.starts_with("permanent error:")`)
  - Spawn workers using `tokio::task::JoinSet` bounded by `Arc<Semaphore>` with `pair.bulk_upload_workers` permits
  - Each worker: stat local file for size; if size >= `pair.bulk_upload_chunk_threshold_bytes` call `upload_chunked(client, local, remote, &opts, tx)` else call `upload_single(client, local, remote, &Default::default(), tx, None)`; on success call `journal.upsert` with `status=Synced`; on error call `journal.upsert` with `status=Error`
  - Build `TransferOptions { chunked_threshold: pair.bulk_upload_chunk_threshold_bytes, chunk_size: 5 * 1024 * 1024, bandwidth_cap: None }`
  - Collect results from `JoinSet`, accumulate into `BulkUploadResult`
- [X] T012 [US1] Wire `BulkUploadDriver` into `SyncCycle::run()` in `crates/adagio-core/src/cycle/mod.rs`:
  - After `reconciler::reconcile(...)`, partition `plan.ops` into `upload_ops` (all `SyncOp::Upload`) and `other_ops`
  - Call `BulkUploadDriver::should_activate(remote_items.len(), upload_ops.len(), &self.pair)`
  - If true: create driver, call `driver.run(upload_ops, ...)`, add bulk result to `result`; then run standard `Propagator` on `other_ops` only
  - If false: re-insert `upload_ops` back into `plan.ops`, run standard Propagator on full `plan.ops` as before
  - Add `tracing::info!` log when bulk mode activates: `"bulk upload activated files={} workers={}"`, and on completion `"bulk upload completed uploaded={} errors={}"`

**Checkpoint**: `cargo test --lib -p adagio-core` — T008–T010 pass. Driver uploads all files, journal correct.

---

## Phase 4: User Story 2 — Resume After Interruption (Priority: P2)

**Goal**: Files already Synced in the journal are excluded from the upload queue on
subsequent runs. Interrupted bulk uploads do not re-upload completed files.

**Independent Test**: Pre-populate journal with N Synced entries for N files in the op
list; assert driver uploads 0 of those N files and call count on mock = 0 for them.

### Tests for US2 (write first — must FAIL)

- [X] T013 [P] [US2] Write failing unit test `driver_skips_already_synced_files` in `crates/adagio-core/src/bulk_upload.rs` — pre-insert 3 Synced entries in journal; provide 5 upload ops (3 already Synced, 2 new); assert mock received exactly 2 upload calls and `result.skipped == 3`
- [X] T014 [P] [US2] Write failing unit test `driver_skips_permanent_error_files` — pre-insert 2 Error entries (error_message starts with "permanent error:"); assert those 2 paths are not uploaded and `result.skipped == 2`

### Implementation for US2

- [X] T015 [US2] Refine skip logic in `BulkUploadDriver::run()` in `crates/adagio-core/src/bulk_upload.rs` — load journal entries ONCE at driver start; build two `HashSet<RelativePath>`: `synced_paths` (status=Synced) and `permanent_error_paths` (status=Error, error starts with "permanent error:"); skip any upload op whose path is in either set; increment `result.skipped` for each skipped file

**Checkpoint**: Interrupted bulk upload correctly resumes from only unfinished files.

---

## Phase 5: User Story 3 — Chunked Upload for Large Files (Priority: P3)

**Goal**: Files at or above `bulk_upload_chunk_threshold_bytes` use `upload_chunked`, which
internally handles chunk-level resume via `list_uploaded_chunks`.

**Independent Test**: Upload one file above threshold; assert `begin_chunked_upload` and
`finalize_chunked_upload` were both called (not single-PUT path). With pre-seeded partial
chunks, assert only remaining chunks are re-uploaded.

### Tests for US3 (write first — must FAIL)

- [X] T016 [P] [US3] Write failing unit test `driver_uses_chunked_for_large_file` in `crates/adagio-core/src/bulk_upload.rs` — 1 file of 15 MB, threshold=10 MB; assert `mock.begin_chunked_upload_called()` is true
- [X] T017 [P] [US3] Write failing unit test `driver_uses_single_for_small_file` — 1 file of 5 MB, threshold=10 MB; assert `mock.begin_chunked_upload_called()` is false and `mock.upload_call_count()` is 1

### Implementation for US3

- [X] T018 [US3] Confirm size-based routing in `BulkUploadDriver::run()` in `crates/adagio-core/src/bulk_upload.rs` is correct (T011 already implements this); add `local_path.metadata()?.len()` call to get file size before routing; add `tracing::debug!` log for each file indicating which path was chosen

**Checkpoint**: Large files use chunked; small files use single-PUT. Chunk resume works via existing `upload_chunked` logic.

---

## Phase 6: User Story 4 — Progress Events (Priority: P4)

**Goal**: TransferProgress events are forwarded to `EventBroadcaster` during bulk upload
so the desktop app and CLI show real-time file/byte counts.

**Independent Test**: Run driver with an `EventBroadcaster`; assert at least one
`TransferProgress` event is emitted per file during upload.

### Tests for US4 (write first — must FAIL)

- [X] T019 [P] [US4] Write failing unit test `driver_emits_progress_events` in `crates/adagio-core/src/bulk_upload.rs` — 3 files; collect TransferProgress events; assert at least 3 events were emitted with non-zero bytes_done values

### Implementation for US4

- [X] T020 [US4] Add progress forwarding to `BulkUploadDriver::run()` in `crates/adagio-core/src/bulk_upload.rs`:
  - Create `mpsc::channel::<TransferProgress>(64)` at the start of `run()`
  - Pass the sender to each worker's upload call (channel sender cloned per worker)
  - Spawn a forwarding task: `tokio::spawn(async move { while let Some(tp) = rx.recv().await { broadcaster.emit_transfer_progress(...); } })`
  - Forwarding task cancels when all senders drop (workers finish)
- [X] T021 [P] [US4] Update daemon's `DaemonProcess` in `crates/adagio-daemon/src/dispatcher.rs` to carry `EventBroadcaster`; confirm it's available to pass when constructing `BulkUploadDriver` from `SyncCycle` — check existing field and add if missing; update `SyncCycle` to accept optional `EventBroadcaster` for bulk driver

**Checkpoint**: Progress events appear in daemon logs and reach UI clients during bulk upload.

---

## Phase 7: User Story 5 — Automatic Activation (Priority: P5)

**Goal**: Bulk driver activates automatically on first sync of a near-empty remote without
user configuration. Standard cycle takes over after completion.

**Independent Test**: SyncCycle::run() with remote_items=0 and 60 Upload ops in plan.ops
with default pair config; assert `bulk_activated=true` and bulk driver is called instead of
standard propagator for Upload ops.

### Tests for US5 (write first — must FAIL)

- [X] T022 [P] [US5] Write failing integration test `sync_cycle_activates_bulk_when_remote_empty` in `crates/adagio-core/src/cycle/mod.rs` test module — mock client returns empty remote; local has 60 files; after `SyncCycle::run()`, assert all 60 files are uploaded and journal has 60 Synced entries
- [X] T023 [P] [US5] Write failing unit test `sync_cycle_does_not_activate_bulk_below_threshold` — local has 30 files (below default 50 threshold); remote empty; assert standard propagator path is taken (bulk NOT activated)

### Implementation for US5

- [X] T024 [US5] Verify activation wiring in `crates/adagio-core/src/cycle/mod.rs` is complete (T012 already implements this); add integration with `SyncCycle`'s existing `memory_sampler` pattern to pass `EventBroadcaster` optionally through to `BulkUploadDriver`; add `tracing::info!` "bulk upload handoff to standard cycle" log after bulk completes

**Checkpoint**: Full end-to-end: `cargo test --lib -p adagio-core` — all US5 tests pass. Bulk activates automatically.

---

## Phase 8: Daemon wiring + config persistence

**Purpose**: Persist bulk config fields through the daemon's config.json round-trip.

- [X] T025 Update `build_saved_config` in `crates/adagio-daemon/src/dispatcher.rs` to include `bulk_upload_workers`, `bulk_upload_threshold_files`, `bulk_upload_chunk_threshold_bytes` in the pair JSON object
- [X] T026 [P] Update `restore_pairs()` in `crates/adagio-daemon/src/main.rs` to read bulk fields from JSON (with fallback to defaults if absent) and set them on `SyncPair`
- [X] T027 [P] Update `CreatePair` handler in `crates/adagio-daemon/src/dispatcher.rs` to accept and persist optional `bulk_upload_workers`, `bulk_upload_threshold_files`, `bulk_upload_chunk_threshold_bytes` params; add the same three optional fields to `DaemonRequest::CreatePair` in `crates/adagio-ipc/src/types.rs`

---

## Phase 9: Polish & Cross-Cutting Concerns

- [X] T028 `cargo clippy --workspace -- -D warnings` — fix all warnings in bulk_upload.rs and modified files
- [X] T029 [P] `cargo fmt --all -- --check` — run `cargo fmt --all` to fix formatting
- [X] T030 `cargo test --workspace` — all tests pass (156+ existing + new bulk driver tests)
- [X] T031 [P] Verify all public items in `crates/adagio-core/src/bulk_upload.rs` have `///` doc comments
- [X] T032 [P] Run manual validation per `specs/011-bulk-upload-driver/quickstart.md` for US1 and US2

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all story phases
- **Phase 3 (US1)**: Depends on Phase 2 — MVP; implement first
- **Phase 4 (US2)**: Depends on Phase 3 (extends `run()` skip logic)
- **Phase 5 (US3)**: Depends on Phase 3 (routing already in `run()`, just verify + test)
- **Phase 6 (US4)**: Depends on Phase 3 (adds progress forwarding to existing `run()`)
- **Phase 7 (US5)**: Depends on Phase 3 (activation wiring already in `SyncCycle::run()`)
- **Phase 8 (Daemon)**: Depends on Phase 1 (config fields); can run in parallel with Phase 3+
- **Phase 9 (Polish)**: Depends on all prior phases

### Parallel Opportunities

**Phase 1**: T002, T003 touch different files — [P].  
**Phase 2 tests**: T004, T005 different modules — [P].  
**Phase 3 tests**: T008, T009, T010 different test functions — [P].  
**Phase 4 tests**: T013, T014 — [P].  
**Phase 5 tests**: T016, T017 — [P].  
**Phase 7 tests**: T022, T023 — [P].  
**Phase 8**: T025, T026, T027 different files — [P] after config fields exist.

---

## Implementation Strategy

### MVP (Phases 1–3, US1 only)

1. Complete Phase 1 (ADR + config fields)
2. Complete Phase 2 (BulkUploadDriver skeleton)
3. Complete Phase 3 (parallel upload run())
4. **STOP AND VALIDATE**: 20-file mock test passes, all journal entries Synced
5. At this point: parallel initial upload works end-to-end

### Incremental Delivery

1. Setup + Foundational → types and struct exist
2. US1 → parallel bulk upload works (MVP!)
3. US2 → resume after interruption works
4. US3 → large files use chunked protocol
5. US4 → progress visible in UI
6. US5 → automatic activation verified

---

## Notes

- `BulkUploadDriver::run()` is built incrementally: US1 adds the core loop, US2 adds skip logic, US3 adds routing (already in US1), US4 adds progress forwarding
- `upload_chunked` already handles chunk-level resume internally — no extra code needed for US3 beyond correct routing
- The `MockRemoteClient` in existing tests tracks call counts and can be used directly for resume verification
- Total: **32 tasks** across 9 phases
