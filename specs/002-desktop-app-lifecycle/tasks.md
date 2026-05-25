# Tasks: Desktop App Lifecycle

**Input**: Design documents from `specs/002-desktop-app-lifecycle/`

**Branch**: `002-desktop-app-lifecycle` | **Spec**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins. The Red-Green-Refactor cycle is strictly enforced.

**Organization**: Tasks are grouped by user story to enable independent implementation and
testing of each story. Phases 3–5 can proceed in priority order (US1 → US2 → US3).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on other in-progress tasks)
- **[Story]**: Which user story this task belongs to ([US1], [US2], [US3])
- Exact file paths are included in every task description

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify and establish the dependency and directory foundations that all phases require.

- [X] T001 Verify tokio-util is in `crates/adagio-core/Cargo.toml` (needed for `CancellationToken`); add it if absent
- [X] T002 [P] Create `crates/adagio-desktop/src/config/` directory with a stub `mod.rs` (empty module, no logic yet)

---

## Phase 2: Foundational (Blocking Prerequisites — adagio-core)

**Purpose**: Extend the core library with full-pair storage and the per-pair runner loop. These
are pure Rust changes with no Tauri dependency and MUST be complete before any user story work.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

### Tests (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, run `cargo test -p adagio-core`, ensure they FAIL before implementing.**

- [X] T003 Write failing unit tests for `SyncPairManager` full-pair methods (`register_full_pair`, `get_pair`, `all_pairs`) in the `#[cfg(test)]` block of `crates/adagio-core/src/config.rs`
- [X] T004 [P] Write failing unit tests for `PairRunner` (start, stop via cancellation token, and trigger via channel) in the `#[cfg(test)]` block of `crates/adagio-core/src/cycle/runner.rs`
- [X] T005 [P] Write failing unit tests for `DefaultSyncEngine::start_pair`, `stop_pair`, and `trigger_pair` in the `#[cfg(test)]` block of `crates/adagio-core/src/cycle/mod.rs`

### Implementation

- [X] T006 Implement `register_full_pair(pair: SyncPair)`, `get_pair(id: &PairId) -> Option<&SyncPair>`, and `all_pairs() -> Vec<&SyncPair>` on `SyncPairManager` in `crates/adagio-core/src/config.rs`; retain deprecated `register_pair` for existing callers (T003 tests must pass green)
- [X] T007 Implement `PairRunner` struct and background sync loop (`tokio::select!` over interval tick, trigger `mpsc::Receiver`, and `CancellationToken`) in `crates/adagio-core/src/cycle/runner.rs` (T004 tests must pass green)
- [X] T008 Add `runners: Arc<RwLock<HashMap<PairId, PairRunner>>>` field to `DefaultSyncEngine`; implement `start_pair(pair, client, journal)`, `stop_pair(pair_id)`, and `trigger_pair(pair_id)` in `crates/adagio-core/src/cycle/mod.rs` (T005 tests must pass green)

**Checkpoint**: `cargo test -p adagio-core` passes; `DefaultSyncEngine::start_pair()` spawns a runner that executes `SyncCycle::run` on trigger.

---

## Phase 3: User Story 1 — Configuration Survives Restart (Priority: P1) 🎯 MVP

**Goal**: Accounts and sync pairs are saved to `config.json` on every mutation and automatically
restored on every app launch. No reconfiguration required after restart.

**Independent Test**: Configure an account and one sync pair, quit the app, relaunch it, and
verify that both are present with all original settings intact (check `config.json` on disk).

### Tests for User Story 1 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, run `cargo test -p adagio-desktop --lib`, ensure they FAIL.**

- [X] T009 [US1] Write failing unit tests for `SavedConfig` load/save round-trip (empty config, single account, multiple pairs, version field) in the `#[cfg(test)]` block of `crates/adagio-desktop/src/config/mod.rs`
- [X] T010 [P] [US1] Write failing unit tests for `AppState::new(config_dir)` loading saved accounts and pairs from a pre-written `config.json` fixture in the `#[cfg(test)]` block of `crates/adagio-desktop/src/state.rs`
- [X] T011 [P] [US1] Write failing unit tests for `add_account` and `remove_account` Tauri commands verifying that `config.json` is updated after each call in the `#[cfg(test)]` block of `crates/adagio-desktop/src/commands/account.rs`
- [X] T012 [P] [US1] Write failing unit tests for `create_pair` and `delete_pair` Tauri commands verifying that `config.json` is updated after each call in the `#[cfg(test)]` block of `crates/adagio-desktop/src/commands/pair.rs`

### Implementation for User Story 1

- [X] T013 [US1] Implement `SavedConfig`, `SavedAccount`, `SavedPair` structs (with `serde::{Serialize, Deserialize}`) and `load_config(path) -> SavedConfig` / `save_config(path, &SavedConfig) -> Result` functions (write-temp-then-rename for atomicity) in `crates/adagio-desktop/src/config/mod.rs` (T009 tests must pass green)
- [X] T014 [US1] Update `AppState::new` to accept `config_dir: PathBuf`, call `load_config`, populate `accounts` and `pairs` from the loaded config, and store `config_path` in `crates/adagio-desktop/src/state.rs` (T010 tests must pass green)
- [X] T015 [US1] Update `add_account` and `remove_account` commands to call `save_config` after mutating `AppState` in `crates/adagio-desktop/src/commands/account.rs` (T011 tests must pass green)
- [X] T016 [US1] Update `create_pair` and `delete_pair` commands to call `save_config` after mutating `AppState` in `crates/adagio-desktop/src/commands/pair.rs` (T012 tests must pass green)
- [X] T017 [US1] Update the Tauri `setup` hook in `crates/adagio-desktop/src/lib.rs` to pass `app.path().app_config_dir()?` to `AppState::new`

**Checkpoint**: Restart the app — all accounts and pairs are restored from disk. `config.json` contents match the UI state. No credentials appear in the file.

---

## Phase 4: User Story 2 — Sync Starts Automatically on Launch (Priority: P2)

**Goal**: On every launch, the app retrieves credentials from the OS keychain, constructs a
`NextcloudClient` per account, and starts a `PairRunner` for each configured pair. The first
sync cycle fires within 5 seconds for pairs with `scan_on_startup = true`. Background sync
continues after the main window is closed.

**Independent Test**: With a saved sync pair pointing to a live Nextcloud instance and a new
remote file present, relaunch the app and verify the file downloads within 10 seconds without
any manual action.

### Tests for User Story 2 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementing.**

- [X] T018 [US2] Write a failing integration test for the full startup sequence using `MockRemoteClient` in `tests/integration/desktop_lifecycle.rs`: verifies `start_engine_for_all_pairs` spawns one runner per configured pair and that a trigger fires a sync cycle
- [X] T019 [P] [US2] Write a failing unit test for `trigger_sync` delegating to `engine.trigger_pair()` (not the old stub) in the `#[cfg(test)]` block of `crates/adagio-desktop/src/commands/sync.rs`

### Implementation for User Story 2

- [X] T020 [US2] Implement `lifecycle::start_engine_for_all_pairs(state, app_handle)` in `crates/adagio-desktop/src/lifecycle.rs`: for each loaded pair, retrieve credentials via `spawn_blocking` + keychain, construct `NextcloudClient`, call `engine.start_pair(pair, client, journal)`; skip pair with a warning if credentials are missing (T018 test must pass green)
- [X] T021 [US2] Update the Tauri `setup` hook in `crates/adagio-desktop/src/lib.rs` to call `lifecycle::start_engine_for_all_pairs` after `AppState` is managed (T018 test must pass green)
- [X] T022 [US2] Replace the stub `trigger_sync` implementation with a call to `engine.trigger_pair(pair_id)` in `crates/adagio-desktop/src/commands/sync.rs`; return `Ok(())` immediately (T019 test must pass green)
- [X] T023 [US2] Verify window-close event handler in `crates/adagio-desktop/src/lib.rs` hides the window rather than exiting the process, so background sync continues with tray icon active

**Checkpoint**: App startup triggers a sync cycle for all configured pairs within 5 s. Closing the window does not stop sync. `trigger_sync` IPC command starts an immediate cycle.

---

## Phase 5: User Story 3 — File Browser Shows Sync Status (Priority: P3)

**Goal**: A `list_synced_files` IPC command lists the contents of a sync pair's local folder,
enriched with per-file sync status from the journal. The Dashboard shows these entries with
status badges and a manual sync button.

**Independent Test**: With a completed sync cycle, invoke `list_synced_files` for the pair and
verify that each file in the local folder appears in the response with an accurate `sync_status`
field; files not in the journal must return `"unknown"`.

### Tests for User Story 3 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementing.**

- [X] T024 [US3] Write failing unit tests for `list_synced_files` Tauri command in the `#[cfg(test)]` block of `crates/adagio-desktop/src/commands/pair.rs`: verify `FileStatusDto` fields for synced files, unknown files, and the sort order (dirs first, then files, each alphabetically)

### Implementation for User Story 3

- [X] T025 [US3] Implement `FileStatusDto` struct and `list_synced_files(pair_id, relative_path?)` Tauri command in `crates/adagio-desktop/src/commands/pair.rs`: read local directory, enrich each entry with journal status, return sorted `FileStatusDto[]` (T024 tests must pass green)
- [X] T026 [US3] Add file browser panel to `crates/adagio-desktop/src-ui/src/routes/Dashboard.svelte`: invoke `list_synced_files` on pair selection, render entries in a table with name, size, modified date, and `sync_status` badge icon
- [X] T027 [US3] Add manual sync button to the file browser panel in `crates/adagio-desktop/src-ui/src/routes/Dashboard.svelte` that calls `trigger_sync` for the selected pair and re-fetches the file list on completion
- [X] T028 [US3] Implement automatic status refresh in `crates/adagio-desktop/src-ui/src/routes/Dashboard.svelte`: poll `list_synced_files` (or listen for a sync-complete Tauri event) so badges update within 2 seconds of a sync operation finishing without a manual page refresh

**Checkpoint**: File browser lists all files with accurate status badges. Manual sync button triggers a cycle and updates badges. Directories appear before files in alphabetical order.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Observability, code quality, and validation across all stories.

- [X] T029 [P] Add `tracing::info!` / `tracing::warn!` structured log statements to the `PairRunner` sync loop (pair_id, cycle outcome) in `crates/adagio-core/src/cycle/runner.rs`
- [X] T030 [P] Add structured logging for config load and save operations (path, account count, pair count) in `crates/adagio-desktop/src/config/mod.rs`
- [X] T031 [P] Add structured logging for startup sequence (credentials found/missing per pair) in `crates/adagio-desktop/src/lifecycle.rs`
- [X] T032 Run `cargo clippy -- -D warnings` across all crates and fix any new warnings introduced by this feature
- [X] T033 [P] Run `cargo fmt --check` and fix any formatting issues introduced by this feature
- [X] T034 Run through `specs/002-desktop-app-lifecycle/quickstart.md` validation scenarios end-to-end and confirm all steps produce the expected outcomes

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 completion — **BLOCKS all user stories**
- **US1 (Phase 3)**: Depends on Phase 2 completion
- **US2 (Phase 4)**: Depends on Phase 3 completion (requires `AppState` with config and engine runners)
- **US3 (Phase 5)**: Depends on Phase 4 completion (requires sync cycles to have run so journal has entries)
- **Polish (Phase 6)**: Depends on all user stories being complete

### User Story Dependencies

- **US1 (Config persistence, P1)**: Can start once Foundational is done
- **US2 (Auto-sync, P2)**: Requires US1 (needs loaded config to know what pairs to start)
- **US3 (File browser, P3)**: Requires US2 (journal entries only exist after sync cycles run)

### Within Each User Story

- Test tasks MUST be written and MUST FAIL before any implementation task in that story begins
- Foundational types and methods before services and commands
- Commands before frontend (Svelte) changes
- Core implementation before observability/logging additions

### Parallel Opportunities

- T001 and T002 (Phase 1 setup) can run in parallel
- T003, T004, T005 (Phase 2 tests) can all be written in parallel (different files)
- T006, T007 can be worked in parallel once their respective tests exist
- T009, T010, T011, T012 (US1 tests) can be written in parallel
- T013–T016 can be worked in parallel once their respective tests exist
- T018 and T019 (US2 tests) can be written in parallel
- T029, T030, T031 (logging additions) can all run in parallel
- T032 and T033 can run in parallel

---

## Parallel Example: Phase 2 (Foundational)

```bash
# Launch all three foundational test tasks together (write failing tests):
Task T003: unit tests for SyncPairManager full-pair methods in adagio-core/src/config.rs
Task T004: unit tests for PairRunner in adagio-core/src/cycle/runner.rs
Task T005: unit tests for DefaultSyncEngine::start_pair/stop_pair/trigger_pair in adagio-core/src/cycle/mod.rs

# Once tests exist and fail, implement in parallel:
Task T006: implement SyncPairManager methods (adagio-core/src/config.rs)
Task T007: implement PairRunner (adagio-core/src/cycle/runner.rs)
# T008 depends on T006 and T007 so runs after both
```

## Parallel Example: User Story 1

```bash
# Launch all US1 test tasks together:
Task T009: SavedConfig round-trip tests (adagio-desktop/src/config/mod.rs)
Task T010: AppState::new with config_dir tests (adagio-desktop/src/state.rs)
Task T011: add_account/remove_account persistence tests (adagio-desktop/src/commands/account.rs)
Task T012: create_pair/delete_pair persistence tests (adagio-desktop/src/commands/pair.rs)

# Once tests exist and fail, implement in parallel:
Task T013: SavedConfig structs + load/save (adagio-desktop/src/config/mod.rs)
Task T015: account command updates (adagio-desktop/src/commands/account.rs)
Task T016: pair command updates (adagio-desktop/src/commands/pair.rs)
# T014 depends on T013; T017 depends on T014
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001–T002)
2. Complete Phase 2: Foundational (T003–T008) — CRITICAL, blocks all stories
3. Complete Phase 3: User Story 1 (T009–T017)
4. **STOP and VALIDATE**: Configure account + pair, restart app, verify config restored
5. Demo: persistent config working, app ready for auto-sync feature

### Incremental Delivery

1. Setup + Foundational → Core library ready
2. US1 → Test config persistence independently → App survives restart (**MVP**)
3. US2 → Test auto-sync independently → App syncs on startup
4. US3 → Test file browser independently → Users see sync status
5. Polish → Full feature complete

### Metrics

| Item | Count |
|------|-------|
| Total tasks | 34 |
| Phase 1 (Setup) | 2 |
| Phase 2 (Foundational) | 6 |
| Phase 3 (US1) | 8 |
| Phase 4 (US2) | 6 |
| Phase 5 (US3) | 5 |
| Phase 6 (Polish) | 6 |
| Parallelizable [P] | 18 |

---

## Notes

- `[P]` tasks touch different files with no in-flight dependencies — safe to run concurrently
- `[Story]` label maps each task to a user story for traceability and independent delivery
- Credentials MUST NEVER appear in `config.json`, logs, or test fixtures — only the keychain lookup key
- `save_config` MUST use write-to-temp-then-rename to prevent corruption on crash (per ADR 004)
- All Rust public items MUST carry `///` doc comments (Constitution Principle II)
- No `println!` in production code paths — use `tracing::info!` / `tracing::warn!` / `tracing::error!`
- Commit after each completed task or logical group to keep history clean
- Stop at each phase checkpoint to validate independently before moving on
