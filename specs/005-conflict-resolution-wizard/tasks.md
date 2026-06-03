# Tasks: Conflict Resolution Wizard

**Input**: Design documents from `specs/005-conflict-resolution-wizard/`

**Prerequisites**: plan.md ✅ | spec.md ✅ | research.md ✅ | data-model.md ✅ | contracts/tauri-ipc.md ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before implementation begins.

**Format**: `[ID] [P?] [Story?] Description`
- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[US#]**: User story this task belongs to

---

## Phase 1: Setup

**Purpose**: ADR documenting the key architectural decision made during planning.

- [ ] T001 Create `docs/adr/006-conflict-resolution-execution.md` — document the decision to execute file I/O directly in the Tauri command (not via propagator queue), with rationale and alternatives considered per research.md Decision 1

---

## Phase 2: Foundational — Data Model & DB Migration

**Purpose**: Type-system and storage changes that all user story phases depend on. No user story work can begin until these are complete.

**⚠️ CRITICAL**: These changes touch `types.rs` and the journal schema. All user story code assumes these are in place.

### Tests (write first — must FAIL)

- [ ] T002 [P] Write failing Rust unit test `conflict_kind_serializes_snake_case` — assert `ConflictKind::ContentModified` serialises to `"content_modified"` etc.; assert all three variants exist in `crates/adagio-core/src/types.rs` test module
- [ ] T003 [P] Write failing Rust unit test `conflict_record_has_is_dir_defaults_false` — assert `ConflictRecord { is_dir: false, conflict_kind: ConflictKind::ContentModified, ..}` compiles and round-trips through serde with correct defaults in `crates/adagio-core/src/types.rs` test module
- [ ] T004 [P] Write failing Rust unit test `conflict_side_has_both_variant` — assert `ConflictSide::Both` exists and is distinct from `Local` / `Remote` in `crates/adagio-core/src/types.rs` test module
- [ ] T005 [P] Write failing Rust unit test `conflict_dto_includes_is_dir_and_kind` — assert `ConflictDto` serialised JSON contains `"is_dir"` and `"conflict_kind"` keys in `crates/adagio-desktop/src/commands/conflicts.rs` test module

### Implementation

- [ ] T006 Add `ConflictKind` enum (`ContentModified`, `RenamedBothSides`, `DeletedWithContent`) with `#[serde(rename_all = "snake_case")]` and `///` doc comments to `crates/adagio-core/src/types.rs`
- [ ] T007 Add `is_dir: bool` and `conflict_kind: ConflictKind` fields to `ConflictRecord` in `crates/adagio-core/src/types.rs`; add `#[serde(default)]` on `is_dir` and `conflict_kind` for backward compat
- [ ] T008 Add `Both` variant to `ConflictSide` enum in `crates/adagio-core/src/types.rs`
- [ ] T009 [P] Update `ConflictDto` in `crates/adagio-desktop/src/commands/conflicts.rs` — add `is_dir: bool` and `conflict_kind: String` fields and update `From<ConflictRecord>` mapping
- [ ] T010 [P] Add SQLite migration in `crates/adagio-desktop/src/migrations/` — `ALTER TABLE conflicts ADD COLUMN is_dir INTEGER NOT NULL DEFAULT 0; ALTER TABLE conflicts ADD COLUMN conflict_kind TEXT NOT NULL DEFAULT 'content_modified'`

**Checkpoint**: `cargo test` passes with new type-system additions. All user story phases can now begin.

---

## Phase 3: User Story 1 — Resolve a Conflict When Detected (Priority: P1) 🎯 MVP

**Goal**: User can see a pending conflict, view local vs server metadata, and resolve it with one click. All three resolution sides work correctly. Conflict badge appears in Chrome when a conflict is detected.

**Independent Test**: Simulate a file conflict, trigger sync, verify the badge appears in Chrome, open the wizard, verify local/server metadata is shown, click each resolution button and confirm the correct file outcome.

### Tests for User Story 1 (write first — must FAIL)

- [ ] T011 [P] [US1] Write failing Rust unit test `resolver_ask_both_runs_preserve_both_logic` in `crates/adagio-core/src/conflict.rs` — assert that `ConflictSide::Both` in `Ask` policy renames local, downloads remote, returns `BothKept` resolution
- [ ] T012 [P] [US1] Write failing Rust unit test `conflict_copy_path_for_directory_no_extension` in `crates/adagio-core/src/conflict.rs` — assert `conflict_copy_path("photos/", ...)` produces `"photos (conflicted copy from …)"` with no trailing dot/extension
- [ ] T013 [P] [US1] Write failing Rust unit test `resolver_folder_rename_keep_local_overwrites_server_name` in `crates/adagio-core/src/conflict.rs`
- [ ] T014 [P] [US1] Write failing Rust unit test `resolver_folder_rename_keep_both_creates_sibling_folder` in `crates/adagio-core/src/conflict.rs`
- [ ] T015 [P] [US1] Write failing Rust unit test `resolve_conflict_local_uploads_local_file` in `crates/adagio-desktop/src/commands/conflicts.rs` — mock journal + client; assert upload is called with correct paths
- [ ] T016 [P] [US1] Write failing Rust unit test `resolve_conflict_remote_downloads_remote_file` in `crates/adagio-desktop/src/commands/conflicts.rs`
- [ ] T017 [P] [US1] Write failing Rust unit test `resolve_conflict_both_renames_local_downloads_remote_uploads_copy` in `crates/adagio-desktop/src/commands/conflicts.rs`
- [ ] T018 [P] [US1] Write failing Rust unit test `resolve_conflict_unknown_id_returns_error` in `crates/adagio-desktop/src/commands/conflicts.rs`
- [ ] T019 [P] [US1] Write failing Rust unit test `resolve_conflict_invalid_side_returns_error` in `crates/adagio-desktop/src/commands/conflicts.rs`
- [ ] T020 [P] [US1] Write failing React test `ConflictWizard renders file name and local vs server metadata` in `crates/adagio-desktop/src-ui/src/__tests__/ConflictWizard.test.tsx`
- [ ] T021 [P] [US1] Write failing React test `ConflictWizard calls resolveConflict("local") when Keep Local Version is clicked` in `crates/adagio-desktop/src-ui/src/__tests__/ConflictWizard.test.tsx`
- [ ] T022 [P] [US1] Write failing React test `ConflictWizard calls resolveConflict("remote") when Keep Server Version is clicked` in `crates/adagio-desktop/src-ui/src/__tests__/ConflictWizard.test.tsx`
- [ ] T023 [P] [US1] Write failing React test `ConflictWizard calls resolveConflict("both") when Keep Both is clicked` in `crates/adagio-desktop/src-ui/src/__tests__/ConflictWizard.test.tsx`
- [ ] T024 [P] [US1] Write failing React test `ConflictWizard shows loading spinner while resolution is in-flight` in `crates/adagio-desktop/src-ui/src/__tests__/ConflictWizard.test.tsx`
- [ ] T025 [P] [US1] Write failing React test `ConflictWizard shows conflict_kind label for folder conflicts` in `crates/adagio-desktop/src-ui/src/__tests__/ConflictWizard.test.tsx`
- [ ] T026 [P] [US1] Write failing React test `ConflictWizard shows impact warning with file list for DeletedWithContent kind` in `crates/adagio-desktop/src-ui/src/__tests__/ConflictWizard.test.tsx`
- [ ] T027 [P] [US1] Write failing React test `Chrome renders conflict badge with count when pendingConflicts > 0` in `crates/adagio-desktop/src-ui/src/__tests__/Chrome.test.tsx`
- [ ] T028 [P] [US1] Write failing React test `Chrome does not render conflict badge when pendingConflicts is 0` in `crates/adagio-desktop/src-ui/src/__tests__/Chrome.test.tsx`

### Implementation for User Story 1

- [ ] T029 [US1] Update `conflict::resolve()` in `crates/adagio-core/src/conflict.rs` — add `ConflictSide::Both` arm to the `Ask` policy branch, running the same logic as `PreserveBoth`; add `///` doc comments
- [ ] T030 [US1] Add folder conflict helpers to `crates/adagio-core/src/conflict.rs`:  `resolve_folder_rename()` (returns which path wins or creates sibling) and `resolve_delete_vs_content()` (returns list of affected paths for impact warning); add `///` doc comments
- [ ] T031 [US1] Rewrite `resolve_conflict` command in `crates/adagio-desktop/src/commands/conflicts.rs` — fetch `ConflictRecord` + `SyncPair` + `Account` from state; retrieve credentials via `spawn_blocking` (keychain pattern from `lifecycle.rs:38–68`); build `NextcloudClient`; dispatch to file/folder resolver; persist `ConflictResolution`; emit `adagio://conflict-resolved` event; add `#[instrument]` with `path` + `side` fields
- [ ] T032 [US1] Add `emit_conflict_detected(handle, count)` helper to `crates/adagio-desktop/src/commands/conflicts.rs` and wire it into the propagator callback in `crates/adagio-desktop/src/lib.rs` after `upsert_conflict` succeeds
- [ ] T033 [P] [US1] Update `crates/adagio-desktop/src-ui/src/tauri.ts` — update `resolveConflict` signature to accept `"local" | "remote" | "both"`; add `listenConflictDetected(cb)` and `listenConflictResolved(cb)` event listener bindings
- [ ] T034 [P] [US1] Create `crates/adagio-desktop/src-ui/src/components/ConflictWizard.tsx` — modal overlay showing: file/folder name + path, local vs server size + mtime metadata comparison, `conflict_kind` label for folder conflicts, impact file list when `kind === "deleted_with_content"`, three resolution buttons ("Keep Local Version" / "Keep Server Version" / "Keep Both"), loading spinner while `resolveConflict` is in-flight
- [ ] T035 [US1] Update `crates/adagio-desktop/src-ui/src/App.tsx` — add `pendingConflicts: number` state initialised from `list_conflicts` on mount; subscribe to `adagio://conflict-detected` and `adagio://conflict-resolved` events to update count; add `wizardOpen: boolean` state; render `<ConflictWizard>` overlay when `wizardOpen` is true
- [ ] T036 [P] [US1] Update `crates/adagio-desktop/src-ui/src/components/Chrome.tsx` — add `pendingConflicts?: number` and `onOpenConflicts?: () => void` props; when `pendingConflicts > 0` render a numbered badge on the bell button (or dedicated conflict icon); clicking fires `onOpenConflicts()`

**Checkpoint**: Single conflict fully detectable and resolvable end-to-end. Badge appears when a conflict exists, disappears after resolution. All three sides (local/remote/both) produce correct file outcomes.

---

## Phase 4: User Story 2 — Work Through Multiple Queued Conflicts (Priority: P2)

**Goal**: When multiple conflicts exist, the wizard presents them sequentially with a progress counter; resolving one automatically advances to the next.

**Independent Test**: Create three simultaneous conflicts, open the wizard — verify "1 of 3" header, resolve first → auto-advances to "2 of 3", resolve second → "3 of 3", resolve third → wizard closes and badge clears.

### Tests for User Story 2 (write first — must FAIL)

- [ ] T037 [P] [US2] Write failing React test `ConflictWizard shows "Conflict N of M" step counter` in `crates/adagio-desktop/src-ui/src/__tests__/ConflictWizard.test.tsx`
- [ ] T038 [P] [US2] Write failing React test `ConflictWizard auto-advances to next conflict after successful resolution` in `crates/adagio-desktop/src-ui/src/__tests__/ConflictWizard.test.tsx`
- [ ] T039 [P] [US2] Write failing React test `ConflictWizard closes and calls onClose when last conflict is resolved` in `crates/adagio-desktop/src-ui/src/__tests__/ConflictWizard.test.tsx`

### Implementation for User Story 2

- [ ] T040 [US2] Update `crates/adagio-desktop/src-ui/src/components/ConflictWizard.tsx` — accept `conflicts: ConflictDto[]` prop (full pending queue); add internal `currentIndex` state; render "Conflict {n} of {total}" counter in wizard header
- [ ] T041 [US2] Update `crates/adagio-desktop/src-ui/src/components/ConflictWizard.tsx` — after a successful `resolveConflict` call, increment `currentIndex`; when `currentIndex >= conflicts.length`, call `onClose()` to close the wizard
- [ ] T042 [US2] Update `crates/adagio-desktop/src-ui/src/App.tsx` — on wizard open, call `list_conflicts(pairId)` and filter `resolution === null` to build the full pending queue; pass array as `conflicts` prop to `ConflictWizard`

**Checkpoint**: Multiple pending conflicts can be resolved sequentially without re-opening the wizard. Counter shows accurate progress.

---

## Phase 5: User Story 3 — Dismiss and Resolve Conflicts Later (Priority: P3)

**Goal**: User can dismiss the wizard without resolving; the badge persists; reopening the wizard shows the same pending conflicts.

**Independent Test**: Open wizard with one pending conflict, click dismiss — verify wizard closes but badge still shows count "1". Click badge again — wizard reopens with the same conflict.

### Tests for User Story 3 (write first — must FAIL)

- [ ] T043 [P] [US3] Write failing React test `ConflictWizard dismiss button closes wizard without calling resolveConflict` in `crates/adagio-desktop/src-ui/src/__tests__/ConflictWizard.test.tsx`
- [ ] T044 [P] [US3] Write failing React test `App does not reset pendingConflicts count when ConflictWizard is closed via dismiss` in `crates/adagio-desktop/src-ui/src/__tests__/App.test.tsx`

### Implementation for User Story 3

- [ ] T045 [US3] Add dismiss button (X icon or "Resolve later" link) to `ConflictWizard.tsx` header — calls `onClose()` without calling `resolveConflict`; does not advance `currentIndex`
- [ ] T046 [US3] Ensure `App.tsx` does NOT reset `pendingConflicts` when the wizard closes — only `adagio://conflict-resolved` events (from actual resolutions) decrement the count; wizard close from dismiss is purely a UI state change

**Checkpoint**: All three user stories independently functional. Badge, queue, and dismiss all behave correctly together.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T047 `cargo clippy -- -D warnings` — fix all warnings across `adagio-core` and `adagio-desktop`; zero suppressions allowed
- [ ] T048 [P] `cargo fmt --check` — run `cargo fmt` to fix any formatting drift introduced during implementation
- [ ] T049 [P] `npm run check` — `tsc --noEmit` must pass with zero type errors across all new and modified `.tsx`/`.ts` files
- [ ] T050 `cargo test` — all Rust tests pass (36+ existing + ~10 new conflict tests)
- [ ] T051 `npm run test` — all React component tests pass (76+ existing + ~11 new ConflictWizard + Chrome tests)
- [ ] T052 Run `specs/005-conflict-resolution-wizard/quickstart.md` end-to-end validation — manually verify all three conflict types (file, folder rename, delete-vs-content) and all three resolutions (local/server/both); confirm no timestamp-suffix duplicates are created; update `quickstart.md` if any steps are inaccurate

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — can start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user story phases
- **Phase 3 (US1)**: Depends on Phase 2 — core story; must complete before US2/US3 phases (they extend the wizard)
- **Phase 4 (US2)**: Depends on Phase 3 (wizard component must exist to add queue to it)
- **Phase 5 (US3)**: Depends on Phase 3 (dismiss button added to existing wizard)
- **Phase 6 (Polish)**: Depends on all prior phases

### Within Phase 3 (US1)

- All test tasks (T011–T028) are [P] and can be written simultaneously before any implementation
- T029 before T030 (both in `conflict.rs`; T030 adds helpers T031 calls)
- T031 after T029–T030 (calls the helpers)
- T032 after T031 (emits events from the completed command)
- T033, T034, T036 are [P] with each other (different files: `tauri.ts`, `ConflictWizard.tsx`, `Chrome.tsx`)
- T035 (`App.tsx`) after T034 (`ConflictWizard.tsx` must exist to import)

### Parallel Opportunities

Within Phase 2: T002–T005 (all test files, different locations)
Within Phase 3 tests: T011–T028 (all test files, different locations)
Within Phase 3 implementation: T033, T034, T036 (tauri.ts / ConflictWizard.tsx / Chrome.tsx)
Phase 6: T047–T049 (quality gates touch different toolchains)

---

## Parallel Example: Phase 3 Test Writing Sprint

```
# Launch all Phase 3 test tasks together — they touch different files and all fail:
T011: resolver_ask_both — crates/adagio-core/src/conflict.rs
T012: conflict_copy_path_for_directory — crates/adagio-core/src/conflict.rs
T013: resolver_folder_rename_keep_local — crates/adagio-core/src/conflict.rs
T014: resolver_folder_rename_keep_both — crates/adagio-core/src/conflict.rs
T015–T019: resolve_conflict_* — crates/adagio-desktop/src/commands/conflicts.rs
T020–T026: ConflictWizard.* — src/__tests__/ConflictWizard.test.tsx
T027–T028: Chrome badge — src/__tests__/Chrome.test.tsx
```

---

## Implementation Strategy

### MVP (User Story 1 only)

1. Phase 1 → Phase 2 → Phase 3
2. **Stop and validate**: single conflict, badge, all three resolutions working
3. `cargo test` + `npm run test` + `quickstart.md` file-conflict scenarios only

### Full Delivery (all three stories)

1. MVP above
2. Phase 4 (US2): queue + auto-advance
3. Phase 5 (US3): dismiss + badge persistence
4. Phase 6: polish + full quickstart validation including folder conflicts

---

## Notes

- [P] tasks = different files, no pending dependencies — launch together for speed
- All test tasks MUST be written and confirmed failing before their paired implementation begins
- `conflict_copy_path()` already handles the `(conflicted copy from …)` naming for files — the directory variant (T012/T030) extends it for paths without extensions
- The `resolve_conflict` rewrite (T031) uses the exact keychain + `NextcloudClient` construction pattern from `crates/adagio-desktop/src/lifecycle.rs:38–75`
- `ConflictWizard` is a modal overlay — same architectural pattern as `ShareDialog` (backdrop click does NOT dismiss; use the explicit dismiss button)
- Badge persistence after dismiss (T046) requires App.tsx to treat wizard `onClose` and the `adagio://conflict-resolved` event as independent state transitions
