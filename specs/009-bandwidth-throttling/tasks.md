# Tasks: Bandwidth Throttling

**Input**: Design documents from `specs/009-bandwidth-throttling/`

**Prerequisites**: plan.md ✅ | spec.md ✅ | research.md ✅ | data-model.md ✅ | contracts/bandwidth-ipc.md ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins.

**Format**: `[ID] [P?] [Story?] Description`
- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[US#]**: User story this task belongs to

---

## Phase 1: Setup

**Purpose**: ADR documenting the chunk-level throttling decision.

- [X] T001 Create `docs/adr/013-chunk-level-throttling.md` — document the decision to call `TokenBucket::acquire(chunk_size)` inside the transfer loop (64 KB chunks) rather than pre-flight `acquire(file_size)`; explain that pre-flight causes bursts for large files that violate the "any 10-second window" requirement

**Checkpoint**: ADR written. Phases 2+ can proceed.

---

## Phase 2: Foundational — ThroughputMeter + Account type changes

**Purpose**: New `ThroughputMeter` struct and per-account bandwidth limit fields. Every
user story phase depends on these types existing.

**⚠️ CRITICAL**: No story implementation can compile until `ThroughputMeter` and the
`Account` fields are in place.

### Tests (write first — must FAIL)

- [X] T002 [P] Write failing Rust unit tests `throughput_meter_empty_returns_zero`, `throughput_meter_records_bytes`, `throughput_meter_evicts_old_entries` in `crates/adagio-core/src/bandwidth.rs` test module — assert new meter returns 0 rate; after recording 100 000 bytes, rate is > 0; entries older than window are evicted
- [X] T003 [P] Write failing Rust unit test `account_default_limits_are_zero` in `crates/adagio-core/src/types.rs` test module — assert `Account { upload_limit_kbps: 0, download_limit_kbps: 0, .. }` compiles and defaults are 0

### Implementation

- [X] T004 Add `ThroughputMeter` struct to `crates/adagio-core/src/bandwidth.rs` — fields: `window: VecDeque<(Instant, u64)>`, `window_secs: u64`; methods: `new() -> Self`, `record(bytes: u64)`, `rate_bytes_per_sec() -> u64`; `///` doc comments on all public items
- [X] T005 [P] Add `upload_limit_kbps: u64` and `download_limit_kbps: u64` to `adagio_core::types::Account` in `crates/adagio-core/src/types.rs` with `#[serde(default)]`
- [X] T006 [P] Add `upload_limit_kbps: u64` and `download_limit_kbps: u64` to `SavedAccount` in `crates/adagio-desktop/src/config/mod.rs` with `#[serde(default)]`; update `restore_accounts()` to copy these fields into `Account`

**Checkpoint**: `cargo test --lib -p adagio-core` — T002–T003 pass. `cargo build` clean.

---

## Phase 3: User Stories 1 & 2 — Upload and Download Throttling (Priority: P1 + P2) 🎯 MVP

**Goal**: `upload_single` and `download_file` accept an optional `TokenBucket` and
call `acquire(chunk_size)` for each 64 KB chunk. The `Propagator` constructs buckets
from account limits and passes them to the transfer functions.

**Independent Test**: Set a 200 Kbps limit, upload a 2 MB file against a mock client.
Assert elapsed time ≥ 8 seconds (2 000 000 bytes ÷ 25 000 bytes/s = 80 s — use small
test payload, e.g., 400 KB → 16 s at 25 KB/s).

### Tests for US1 + US2 (write first — must FAIL)

- [X] T007 [P] [US1] Write failing Rust unit test `upload_single_with_throttle_slows_transfer` in `crates/adagio-core/src/transfer/upload.rs` test module — mock client; set 50 KB/s limit; upload 100 KB; assert elapsed ≥ 1.8 s (10% tolerance)
- [X] T008 [P] [US2] Write failing Rust unit test `download_file_with_throttle_slows_transfer` in `crates/adagio-core/src/transfer/download.rs` test module — mock client; set 50 KB/s limit; download 100 KB; assert elapsed ≥ 1.8 s
- [X] T009 [P] [US1] Write failing Rust unit test `upload_single_no_throttle_is_fast` in `crates/adagio-core/src/transfer/upload.rs` — same upload with `throttle = None`; assert elapsed < 200 ms (mock is instant)
- [X] T010 [P] [US1] Write failing Rust unit test `propagator_records_upload_bytes_in_meter` in `crates/adagio-core/src/cycle/propagator.rs` — execute Upload op with non-zero throttle; assert `upload_meter.rate_bytes_per_sec()` > 0 after completion

### Implementation for US1 + US2

- [X] T011 [US1] Modify `upload_single` in `crates/adagio-core/src/transfer/upload.rs` — add `throttle: Option<Arc<TokenBucket>>` parameter; refactor from whole-file-read to 64 KB chunk loop; call `throttle.acquire(chunk_len).await` before writing each chunk; maintain existing `TransferProgress` emission; `///` doc comment update
- [X] T012 [P] [US2] Modify `download_file` in `crates/adagio-core/src/transfer/download.rs` — add `throttle: Option<Arc<TokenBucket>>` parameter; call `throttle.acquire(chunk_len).await` at each write boundary; `///` doc comment update — depends on T011 (same pattern, different file)
- [X] T013 [P] [US1] Update all callers of `upload_single` in `crates/adagio-core/src/transfer/engine.rs` to pass `None` for the new throttle parameter — depends on T011
- [X] T014 [P] [US1] Update caller of `upload_single` in `crates/adagio-core/src/conflict.rs` (the `PreserveBoth` / `Ask+Both` arm) to pass `None` — depends on T011
- [X] T015 [P] [US2] Update all callers of `download_file` to pass `None` in `crates/adagio-core/src/transfer/engine.rs` and `crates/adagio-core/src/conflict.rs` — depends on T012
- [X] T016 [US1] Add `upload_throttle: Option<Arc<TokenBucket>>`, `download_throttle: Option<Arc<TokenBucket>>`, `upload_meter: Arc<std::sync::Mutex<ThroughputMeter>>`, `download_meter: Arc<std::sync::Mutex<ThroughputMeter>>` fields to `Propagator` in `crates/adagio-core/src/cycle/propagator.rs`; add `with_bandwidth(upload_kbps: u64, download_kbps: u64) -> Self` constructor; pass throttles to `upload_single` / `download_file`; call `meter.record(bytes)` after each transfer — depends on T011, T012
- [X] T017 [US1] Update `DefaultSyncEngine` in `crates/adagio-core/src/cycle/mod.rs` — add `Arc<RwLock<HashMap<AccountId, (u64, u64)>>>` bandwidth limits map; `SyncCycle::run()` reads account limits and passes them to `Propagator::with_bandwidth()`; add `update_bandwidth_limits(account_id, upload_kbps, download_kbps)` method — depends on T016

**Checkpoint**: `cargo test --lib -p adagio-core` — T007–T010 pass. Upload and download respect configured limits.

---

## Phase 4: User Stories 3 & 4 — IPC + Daemon Support (Priority: P3 + P4)

**Goal**: Three new `DaemonRequest` variants allow the desktop and CLI to read and write
bandwidth limits. The daemon persists changes to `config.json` and applies them immediately.

**Independent Test**: Call `SetBandwidthLimits` via IPC. Run a sync. Verify upload is
throttled. Call `ClearBandwidthLimits`. Verify upload is full-speed again.

### Tests for US3 + US4 (write first — must FAIL)

- [X] T018 [P] [US3] Write failing Rust unit test `dispatcher_get_bandwidth_status_returns_limits` in `crates/adagio-daemon/src/dispatcher.rs` test module — create mock state with account having upload_limit_kbps=500; dispatch `GetBandwidthStatus`; assert response JSON contains `"upload_limit_kbps":500`
- [X] T019 [P] [US4] Write failing Rust unit test `dispatcher_set_bandwidth_limits_updates_account` in `crates/adagio-daemon/src/dispatcher.rs` — dispatch `SetBandwidthLimits { upload_kbps:200, download_kbps:0 }`; assert `AccountManager` returns account with updated limits
- [X] T020 [P] [US4] Write failing Rust unit test `dispatcher_clear_bandwidth_limits_zeros_both` in `crates/adagio-daemon/src/dispatcher.rs` — set limits, then dispatch `ClearBandwidthLimits`; assert both are 0

### Implementation for US3 + US4

- [X] T021 Add `GetBandwidthStatus { account_id: Option<String> }`, `SetBandwidthLimits { account_id: Option<String>, upload_kbps: u64, download_kbps: u64 }`, `ClearBandwidthLimits { account_id: Option<String> }` to `DaemonRequest` in `crates/adagio-ipc/src/types.rs`
- [X] T022 Handle all three new variants in `dispatch()` in `crates/adagio-daemon/src/dispatcher.rs`:
  - `GetBandwidthStatus` → look up account limits + read meters from DaemonProcess; return JSON object matching `BandwidthStatus` shape
  - `SetBandwidthLimits` → update `AccountManager` + call `engine.update_bandwidth_limits()`; persist config via `save_config()`
  - `ClearBandwidthLimits` → equivalent to `SetBandwidthLimits { upload_kbps: 0, download_kbps: 0 }`
  — depends on T021, T017
- [X] T023 [P] Add `upload_meters` and `download_meters` (`HashMap<AccountId, Arc<Mutex<ThroughputMeter>>>`) to `DaemonProcess` in `crates/adagio-daemon/src/dispatcher.rs` so `GetBandwidthStatus` can read live rates — depends on T022

**Checkpoint**: `GetBandwidthStatus` / `SetBandwidthLimits` / `ClearBandwidthLimits` all work via IPC.

---

## Phase 5: User Story 4 — CLI Bandwidth Subcommand (Priority: P4)

**Goal**: `adagio bandwidth status`, `adagio bandwidth set`, and `adagio bandwidth clear`
work from the terminal.

**Independent Test**: `adagio bandwidth set --upload-kbps 100` exits 0; `adagio bandwidth
status --json` reports the limit; `adagio bandwidth clear` removes it.

### Tests for US4 CLI (write first — must FAIL)

- [X] T024 [P] [US4] Write failing Rust unit tests `cli_parses_bandwidth_set_both_flags` and `cli_parses_bandwidth_clear` in `crates/adagio-cli/src/cli.rs` test module — assert correct variant and field values

### Implementation for US4 CLI

- [X] T025 [US4] Add `Commands::Bandwidth { command: BandwidthCommand }` and `BandwidthCommand { Status, Set { upload_kbps: Option<u64>, download_kbps: Option<u64> }, Clear }` to `crates/adagio-cli/src/cli.rs` — `///` doc comments; at least one of `upload_kbps`/`download_kbps` required for `Set` (validated in handler)
- [X] T026 [P] [US4] Create `crates/adagio-cli/src/handlers/bandwidth.rs` — implement `run_bandwidth_status(client, json)`, `run_bandwidth_set(client, upload_kbps, download_kbps, json)`, `run_bandwidth_clear(client, json)`; human output: table of direction / limit / current-speed; JSON output: raw value from daemon
- [X] T027 [P] [US4] Add `Commands::Bandwidth { command }` arm to `crates/adagio-cli/src/run.rs`; add `pub mod bandwidth` to `crates/adagio-cli/src/handlers/mod.rs`

**Checkpoint**: All three `adagio bandwidth` subcommands work end-to-end with running daemon.

---

## Phase 6: User Stories 3 & 5 — Desktop Settings Panel + Live Throughput (Priority: P3 + P5)

**Goal**: Settings → Sync → Bandwidth shows upload/download limit inputs and live speed.
Users can set and clear limits. The speed display updates every 2 seconds while visible.

**Independent Test**: Enter 300 in upload field, save, trigger a sync, verify upload is
throttled. Open Settings during active sync and verify live speed updates.

### Tests for US3 + US5 (write first — must FAIL)

- [X] T028 [P] [US3] Write failing React test `SettingsScene renders bandwidth section in Sync tab` in `crates/adagio-desktop/src-ui/src/__tests__/SettingsScene.test.tsx` — switch to Sync section; assert upload/download limit inputs exist
- [X] T029 [P] [US3] Write failing React test `bandwidth save calls setBandwidthLimits with correct values` — enter 300 in upload field; click Save; assert `invoke('set_bandwidth_limits', { uploadKbps: 300, downloadKbps: 0 })` called
- [X] T030 [P] [US3] Write failing React test `bandwidth invalid input shows validation error` — enter "abc"; assert error state visible and save button disabled

### Implementation for US3 + US5

- [X] T031 [US3] Add `getBandwidthStatus`, `setBandwidthLimits(uploadKbps: number, downloadKbps: number)`, `clearBandwidthLimits` to `crates/adagio-desktop/src-ui/src/tauri.ts`; add `BandwidthStatusDto` interface
- [X] T032 [US3] Add Tauri command handlers `get_bandwidth_status`, `set_bandwidth_limits`, `clear_bandwidth_limits` in `crates/adagio-desktop/src/commands/` (new file `bandwidth.rs` or append to `sync.rs`) — forward to `DaemonRequest::GetBandwidthStatus` / `SetBandwidthLimits` / `ClearBandwidthLimits`; register in `lib.rs` invoke_handler — depends on T021
- [X] T033 [US3] Add "Bandwidth" sub-section to the Sync tab in `crates/adagio-desktop/src-ui/src/components/SettingsScene.tsx`:
  - Upload limit input (number, Kbps; blank = 0 = unlimited)
  - Download limit input (number, Kbps; blank = 0 = unlimited)
  - Save button → calls `setBandwidthLimits`
  - Validation: non-numeric or negative → red border + error message
  - `useEffect` loads current limits from `getBandwidthStatus` when section opens
- [X] T034 [P] [US5] Add live speed display to the Bandwidth section in `crates/adagio-desktop/src-ui/src/components/SettingsScene.tsx` — `setInterval` polling `getBandwidthStatus` every 2 s while Bandwidth tab is visible; display "Current upload: X KB/s" and "Current download: X KB/s"
- [X] T035 [P] [US5] Add mock entries for `get_bandwidth_status`, `set_bandwidth_limits`, `clear_bandwidth_limits` to `crates/adagio-desktop/src-ui/src/__tests__/mocks/tauri.ts`

**Checkpoint**: Bandwidth Settings panel renders; save persists; live speed updates.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T036 `cargo clippy -- -D warnings` — fix all warnings across all affected crates
- [X] T037 [P] `cargo fmt --all --check` — run `cargo fmt --all` to fix formatting drift
- [X] T038 `cargo test --lib -p adagio-core` — all tests pass (133+ existing + new bandwidth tests)
- [X] T039 [P] `cargo test --lib -p adagio-ipc` — all pass
- [X] T040 [P] `cargo test -p adagio-daemon` — all pass
- [X] T041 [P] `cargo test --lib -p adagio-desktop` — all pass
- [X] T042 [P] `cd crates/adagio-desktop/src-ui && npm run test` — all pass (95+ existing + new bandwidth UI tests)
- [X] T043 Manual `quickstart.md` validation — step through all 5 user stories per `specs/009-bandwidth-throttling/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all story phases
- **Phase 3 (US1+US2)**: Depends on Phase 2 — MVP; implement first
- **Phase 4 (IPC+Daemon)**: Depends on Phase 2 + Phase 3 (engine needs bandwidth limits map)
- **Phase 5 (CLI)**: Depends on Phase 4
- **Phase 6 (UI)**: Depends on Phase 4
- **Phase 7 (Polish)**: Depends on all prior phases

### Within Phase 3

- T007–T010 (tests) are [P] — write simultaneously before any implementation
- T011 before T012 (same pattern; write upload first, copy pattern for download)
- T013, T014, T015 are [P] after T011/T012 (different caller files)
- T016 after T011, T012 (propagator uses both)
- T017 after T016

### Parallel Opportunities

Phase 2: T002, T003, T005, T006 all touch different files — [P].
Phase 3 tests: T007–T010 all different test modules — [P].
Phase 3 impl: T013, T014, T015 are [P] (caller updates in different files).
Phases 5 and 6 can proceed in parallel once Phase 4 is complete.

---

## Parallel Example: Phase 3 Test Sprint

```
# Write all failing tests simultaneously before any implementation:
T007: upload_single_with_throttle_slows_transfer  → transfer/upload.rs
T008: download_file_with_throttle_slows_transfer  → transfer/download.rs
T009: upload_single_no_throttle_is_fast           → transfer/upload.rs
T010: propagator_records_upload_bytes_in_meter    → cycle/propagator.rs

# Then implement:
T011 (upload.rs) → T013, T014 in parallel
T012 (download.rs) → T015 in parallel
T016 (propagator.rs, after T011+T012)
T017 (cycle/mod.rs, after T016)
```

---

## Implementation Strategy

### MVP (US1 + US2 only — upload and download throttling)

1. Phase 1 → Phase 2 → Phase 3
2. **STOP and VALIDATE**: `cargo test --lib -p adagio-core` passes; manual timing test confirms throttling works
3. `adagio bandwidth set` (once Phase 4+5 are done) enables full end-to-end

### Full Delivery

1. MVP above
2. Phase 4 (IPC + daemon) — enables all configuration paths
3. Phase 5 (CLI) — headless users can configure limits
4. Phase 6 (Settings UI + live display) — GUI users can configure limits and monitor
5. Phase 7 (polish + quickstart)

---

## Notes

- [P] tasks touch different files — launch together for speed
- T011 is the keystone of this feature — everything else depends on the new `upload_single` signature
- When T011 lands, T013/T014/T015 (caller updates) can all be done in parallel — they each touch a different file
- T016 (Propagator) is the second keystone — it wires the TokenBucket into the sync loop
- `BandwidthSchedule` (time-based scheduling) already exists in `bandwidth.rs` but is NOT used in this feature — do not remove it, just don't hook it up
- Total: **43 tasks** across 7 phases
