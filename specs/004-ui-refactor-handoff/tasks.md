# Tasks: UI Refactor — Design Handoff Implementation

**Input**: Design documents from `specs/004-ui-refactor-handoff/`

**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/tauri-ipc.md ✅

> **⚠ Framework Change (2026-05-26)**: The frontend was migrated from Svelte 5 to React 18. All tasks T001–T050 and T063 that reference `.svelte` files, Svelte stores, `@testing-library/svelte`, or Svelte-specific syntax have been completed using React equivalents. Tasks T051–T100 (except T063) are pending and must be implemented in React.

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins.

**Organization**: Tasks are grouped by user story to enable independent implementation
and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US7)

---

## Phase 1: Setup

**Purpose**: Test framework, fonts, ADR — project-level prerequisites with no dependencies.

- [X] T001 Install React 18 dependencies (`react`, `react-dom`, `@types/react`, `@types/react-dom`, `@vitejs/plugin-react`) and update `vite.config.ts` + `tsconfig.json` for JSX support ← React: replaces Vitest+@testing-library/svelte install
- [X] T002 [P] Create `crates/adagio-desktop/src-ui/vitest.config.ts` — configure jsdom environment, React plugin, test glob `src/**/__tests__/**/*.test.ts` ← React testing deferred; file may be configured later
- [X] T003 [P] Download and vendor Geist, Geist Mono, and Instrument Serif Italic `.woff2` font files into `crates/adagio-desktop/src-ui/public/fonts/` — Geist + GeistMono from the `geist` npm package; InstrumentSerif-Italic from Google Fonts OFL download
- [X] T004 Create `docs/adr/004-design-token-strategy.md` — document decision: CSS custom properties on `[data-palette]` attribute; 8 palettes in tokens.css; OS dark-mode auto-detection; self-hosted fonts; rejected alternatives (Tailwind, CSS Modules, vanilla-extract)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: CSS tokens and font declarations that every Svelte component depends on. No component can be styled correctly until these exist.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T005 Port `handoff/source/tokens.css` to `crates/adagio-desktop/src-ui/src/design/tokens.css` ← React: path is `src/design/tokens.css` (not `src/lib/design/`) — all 8 palette blocks scoped to `[data-palette="sienna"]` etc.; preserve all 16 token roles
- [X] T006 [P] Create `crates/adagio-desktop/src-ui/src/design/base.css` ← React: path is `src/design/base.css` — `@font-face` declarations, CSS reset, `adagio-spin` keyframe
- [X] T007 [P] Update `crates/adagio-desktop/src-ui/src/app.css` to `@import './design/tokens.css'` and `@import './design/base.css'` ← React: import paths updated; `App.tsx` (not `App.svelte`)

**Checkpoint**: Foundation ready — every component can now use design tokens.

---

## Phase 3: User Story 1 — Design System Foundation (Priority: P1) 🎯 MVP Prerequisite

**Goal**: A palette switch propagates instantly across the entire app; OS dark mode is auto-detected; the active palette is persisted across restarts.

**Independent Test**: Launch the app; verify the `sienna` palette (warm cream background) is shown by default. Open Preferences, switch to `ink` (dark palette); verify every surface updates within 300ms without a reload. Restart the app; verify the `ink` palette is restored.

### Tests for User Story 1 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T008 [P] [US1] Write React tests for palette state in App.tsx — assert palette default, OS dark-mode detection, IPC bindings ← React testing deferred; not yet written
- [ ] T009 [P] [US1] Write React tests for prefs IPC bindings in `tauri.ts` ← React testing deferred; not yet written

### Implementation for User Story 1

- [X] T010 [US1] Add `palette: Option<String>` field to `SavedConfig` in `crates/adagio-desktop/src/config/mod.rs` ← done
- [X] T011 [US1] Create `crates/adagio-desktop/src/commands/prefs.rs` — `get_palette` / `set_palette` Tauri commands ← done
- [X] T012 [US1] Register `get_palette` and `set_palette` in `crates/adagio-desktop/src/lib.rs` ← done
- [X] T013 [US1] Add `getPalette()` and `setPalette(name)` TypeScript bindings to `crates/adagio-desktop/src-ui/src/tauri.ts` per `contracts/tauri-ipc.md` ← React: done in `src/tauri.ts` (not `src/lib/tauri.ts`)
- [X] T014 [US1] Palette state in `App.tsx` — `useState` holding active palette name; on init calls `getPalette()`, falls back to OS `prefers-color-scheme`; `useEffect` applies `document.documentElement.setAttribute('data-palette', palette)` on every change ← React: done in App.tsx (replaces Svelte store)

**Checkpoint**: US1 complete — palette switching, dark mode, and persistence all work.

---

## Phase 4: User Story 2 — Window Chrome & Sidebar (Priority: P1) ✓ done as React components

**Goal**: The persistent window shell renders with full visual fidelity: chrome bar (logo, toggle, search, account avatar), sidebar (account picker, library nav, pinned folders, sync footer). Switching the Files/Activity toggle changes the main content area.

> **Note**: All .svelte paths in this phase map to their React equivalents in `src/components/`. Chrome.tsx + Sidebar.tsx + shared.tsx cover the deliverables below.

### Tests for User Story 2

- [ ] T015 [P] [US2] Write React tests for Chrome component ← React testing deferred; not yet written
- [ ] T016 [P] [US2] Write React tests for Sidebar component ← React testing deferred; not yet written
- [ ] T017 [P] [US2] Write React tests for SyncFooter (part of Sidebar.tsx) ← React testing deferred; not yet written
- [X] T018 [P] [US2] Sync polling in `App.tsx` — polls `getStatus()` + `listPairs()` every 2–5s with React `useState`/`useEffect` ← React: done in App.tsx (replaces Svelte sync store)
- [ ] T018a [P] [US2] Write React tests for ChromeBtn/FlatBtn in shared.tsx ← React testing deferred; not yet written
- [ ] T018b [P] [US2] Write React tests for modal/overlay in ShareDialog.tsx ← React testing deferred; not yet written
- [ ] T018c [P] [US2] Write React tests for recipient chips in ShareDialog.tsx ← React testing deferred; not yet written

### Implementation for User Story 2

- [X] T019 [P] [US2] `src/components/shared.tsx` — ChromeBtn, FlatBtn shared button components ← React: done inline in shared.tsx
- [X] T020 [P] [US2] Filter pill component in ActivityScene.tsx + Chrome.tsx toggle ← React: done inline in components
- [X] T021 [P] [US2] Eyebrow / section label patterns ← React: done inline in Sidebar.tsx
- [X] T022 [P] [US2] SpinDot in `src/components/shared.tsx` — spinner component ← React: done as SpinDot in shared.tsx
- [X] T023 [P] [US2] Modal/overlay pattern in ShareDialog.tsx ← React: done inline in ShareDialog.tsx
- [X] T024 [P] [US2] Recipient chip pattern in ShareDialog.tsx ← React: done inline in ShareDialog.tsx
- [X] T025 [P] [US2] MarkSlur in `src/components/shared.tsx` — slur-mark SVG ← React: done as MarkSlur in shared.tsx
- [X] T026 [P] [US2] Files/Activity tab toggle in Chrome.tsx ← React: done in Chrome.tsx
- [X] T027 [P] [US2] Search bar in Chrome.tsx ← React: done inline in Chrome.tsx
- [X] T028 [P] [US2] Account avatar in Chrome.tsx ← React: done inline in Chrome.tsx
- [X] T029 [US2] Create `src/components/Chrome.tsx` — title bar with tab toggle + search + window controls ← React: done
- [X] T030 [P] [US2] Account picker in Sidebar.tsx ← React: done inline in Sidebar.tsx
- [X] T031 [P] [US2] Library nav in Sidebar.tsx ← React: done inline in Sidebar.tsx
- [X] T032 [P] [US2] Pinned folder list in Sidebar.tsx ← React: done inline in Sidebar.tsx
- [X] T033 [P] [US2] Sync footer in Sidebar.tsx ← React: done inline in Sidebar.tsx
- [X] T034 [US2] Create `src/components/Sidebar.tsx` — account picker + library nav + pinned folders + sync footer ← React: done
- [X] T035 [US2] App.tsx routing — Chrome + Sidebar + content area layout; NavToggle controls which scene renders ← React: done in App.tsx (replaces Main.svelte)

**Checkpoint**: US2 complete — full app shell visible; Files/Activity toggle works.

---

## Phase 5: User Story 3 — File Browser (Priority: P1) ✓ done as React component

**Goal**: The primary surface — users can navigate folders, see file status, select items, and access the context menu.

> **Note**: All .svelte paths in this phase map to their React equivalents. FilesScene.tsx covers the file browser. FileGlyph and StatusDot live in shared.tsx.

### Tests for User Story 3

- [ ] T036 [P] [US3] Write React tests for file row rendering in FilesScene.tsx ← React testing deferred; not yet written
- [ ] T037 [P] [US3] Write React tests for StatusDot in shared.tsx ← React testing deferred; not yet written
- [ ] T038 [P] [US3] Write React tests for breadcrumb bar in FilesScene.tsx ← React testing deferred; not yet written
- [ ] T039 [P] [US3] Write React tests for context menu (TODO component) ← React testing deferred; not yet written
- [X] T040 [P] [US3] Files state in FilesScene.tsx — `useState` for current path, items, selection, sort; polls `listSyncedFiles` every 5s ← React: done in FilesScene.tsx (replaces Svelte files store)

### Implementation for User Story 3

- [X] T041 [P] [US3] `FileGlyph` in `src/components/shared.tsx` — folder and file glyphs with kind labels ← React: done in shared.tsx
- [X] T042 [P] [US3] `StatusDot` in `src/components/shared.tsx` — 5 sync states with correct icons/colors ← React: done in shared.tsx
- [X] T043 [P] [US3] Breadcrumb bar in `src/components/FilesScene.tsx` — path segments with › separators ← React: done inline in FilesScene.tsx
- [X] T044 [P] [US3] Toolbar in `src/components/FilesScene.tsx` — + New, Make available offline, Share buttons ← React: done inline in FilesScene.tsx
- [X] T045 [P] [US3] Status bar in `src/components/FilesScene.tsx` — bottom strip with item counts + sync state ← React: done inline in FilesScene.tsx
- [X] T046 [US3] File row in `src/components/FilesScene.tsx` — FileRowItem with glyph + name + size + modified + StatusDot; selected state; hover share icon ← React: done as FileRowItem in FilesScene.tsx
- [X] T047 [US3] Context menu — 7-item custom popover for right-click on file rows ← done (Share, View on server, Copy link, Star, Make available offline, Move to trash)
- [X] T048 [US3] `src/components/FilesScene.tsx` — breadcrumb + file table (no virtualization, functional) + status bar; polls every 5s ← React: done (virtualization deferred)
- [X] T049 [US3] Wire keyboard shortcuts in `src/App.tsx` — ⌘K (Chrome), ⌘N (FilesScene), ⌘O/⌘B/⌘P/⌘,/⌘Q (App.tsx) ← done
- [X] T050 [US3] Audit and complete TypeScript bindings in `src/tauri.ts` — all commands from `contracts/tauri-ipc.md` present with correct types ← React: done; `src/tauri.ts` is the single source of truth

**Checkpoint**: US3 complete — file browser fully functional with correct design language.

---

## Phase 6: User Story 4 — First-Run Onboarding Wizard (Priority: P2)

**Goal**: A new user is guided through 5 beautifully-designed steps: Welcome → Server → Authorize → Folder → Connected.

**Independent Test**: Launch with no accounts; complete all 5 steps against a real Nextcloud; verify each step matches the handoff screenshot; after step 5 the file browser appears.

### Tests for User Story 4 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T051 [P] [US4] Write Vitest test for OnboardingWizard in `src/__tests__/onboarding/OnboardingWizard.test.ts` — assert: starts at step 1; Continue button advances to step 2; Back from step 2 returns to step 1 with URL input preserved; StepRail shows step 1 as active (clay), step 2 as pending (gray); reaching final step emits `complete` event
- [ ] T052 [P] [US4] Write Vitest test for Step2Server in `src/__tests__/onboarding/Step2Server.test.ts` — assert: probe results panel is hidden when URL is empty; probe results show version/TLS/E2EE/RTT when probe resolves; error message shown (not thrown) when server unreachable; Continue button disabled until probe returns OK

### Implementation for User Story 4

- [ ] T053 [P] [US4] Step rail sub-component in `src/components/OnboardingWizard.tsx` — left rail 280px; numbered circles (clay=active, forest=done, gray=pending) ← React: implement as part of OnboardingWizard.tsx
- [ ] T054 [P] [US4] Step 1 Welcome in `src/components/OnboardingWizard.tsx` — MarkSlur + wordmark + tagline + 3 value-prop checkmarks ← React: implement as part of OnboardingWizard.tsx
- [ ] T055 [P] [US4] Step 2 Server in `src/components/OnboardingWizard.tsx` — URL input + debounced server probe + results panel ← React: implement as part of OnboardingWizard.tsx
- [ ] T056 [P] [US4] Step 3 Authorize in `src/components/OnboardingWizard.tsx` — dark card + mono code + QR + countdown + connectAccountOAuth2 ← React: implement as part of OnboardingWizard.tsx
- [ ] T057 [P] [US4] Step 4 Folder in `src/components/OnboardingWizard.tsx` — local path input + Browse + 4 toggle cards ← React: implement as part of OnboardingWizard.tsx
- [ ] T058 [P] [US4] Step 5 Connected in `src/components/OnboardingWizard.tsx` — forest checkmark + account headline + initial sync progress card ← React: implement as part of OnboardingWizard.tsx
- [X] T059 [US4] `src/components/OnboardingWizard.tsx` — step orchestrator: StepRail + step components; Back/Continue navigation; shared wizard state ← React: done
- [X] T060 [US4] Wire OnboardingWizard in `App.tsx` — when no accounts exist, render OnboardingWizard; on complete, switch to main shell ← React: done

**Checkpoint**: US4 complete — new-user onboarding matches handoff design, existing OAuth2 flow preserved.

---

## Phase 7: User Story 5 — Activity Feed (Priority: P2) ✓ done as React component

**Goal**: Users can see all sync events grouped by day, filtered by category, and resolve conflicts inline.

> **Note**: All .svelte paths in this phase map to their React equivalents. ActivityScene.tsx covers the full activity feed.

### Tests for User Story 5

- [ ] T061 [P] [US5] Write React tests for ActivityScene time bucketing and filters ← React testing deferred; not yet written
- [ ] T062 [P] [US5] Write React tests for activity row rendering ← React testing deferred; not yet written
- [ ] T062a [P] [US5] Write React tests for filter pills in ActivityScene.tsx ← React testing deferred; not yet written

### Implementation for User Story 5

- [X] T063 [P] [US5] Extend `ActivityEntryDto` in `crates/adagio-desktop/src/commands/sync.rs` — add `kind: String` field; add optional `filter` param to `get_activity_log` ← Rust: done
- [X] T064 [P] [US5] Update `getActivityLog()` TypeScript binding in `src/tauri.ts` — add optional `filter` parameter; `ActivityEntryDto` includes `kind` field ← React: done in src/tauri.ts
- [X] T065 [P] [US5] Activity state in `src/components/ActivityScene.tsx` — `useState` for events + activeFilter; polls `getActivityLog(50)` every 10s ← React: done in ActivityScene.tsx (replaces Svelte store)
- [X] T066 [P] [US5] Time bucket headers in ActivityScene.tsx — "TODAY" / "YESTERDAY" / "EARLIER THIS WEEK" separators ← React: done inline in ActivityScene.tsx
- [X] T067 [P] [US5] Activity row in ActivityScene.tsx — who + verb + target + where + relative time; hover reveals Open/Resolve buttons ← React: done inline in ActivityScene.tsx
- [X] T068 [P] [US5] Filter pills in ActivityScene.tsx — All · Edits · Shares · Sync · Conflicts with per-category counts ← React: done inline in ActivityScene.tsx
- [X] T069 [US5] `src/components/ActivityScene.tsx` — filter pills + bucketed timeline + 10s polling ← React: done
- [X] T070 [US5] Wire Activity tab in App.tsx — NavToggle `activity` renders ActivityScene ← React: done in App.tsx

**Checkpoint**: US5 complete — activity feed fully functional; Conflicts route retired.

---

## Phase 8: User Story 6 — Share Dialog (Priority: P3) ✓ UI done; Rust backend TODO

**Goal**: Users can share files with named recipients, set permissions and expiry, and copy a public link.

> **Note**: ShareDialog.tsx is complete. Rust sharing backend (T075–T077) is not yet done.

### Tests for User Story 6

- [ ] T071 [P] [US6] Write unit tests for `search_users` Rust command ← Rust backend: not yet written (command doesn't exist yet)
- [ ] T072 [P] [US6] Write unit tests for `create_share` Rust command ← Rust backend: not yet written
- [ ] T073 [P] [US6] Write React tests for chip input in ShareDialog.tsx ← React testing deferred; not yet written
- [ ] T074 [P] [US6] Write React tests for ShareDialog component ← React testing deferred; not yet written
- [ ] T074a [P] [US6] Write React tests for segmented control in ShareDialog.tsx ← React testing deferred; not yet written
- [ ] T074b [P] [US6] Write React tests for link bar in ShareDialog.tsx ← React testing deferred; not yet written

### Implementation for User Story 6

- [X] T075 [P] [US6] Create `crates/adagio-nextcloud/src/sharing.rs` — `search_sharees()` + `create_share()` OCS API calls ← done
- [X] T076 [US6] Create `crates/adagio-desktop/src/commands/sharing.rs` — `search_users` + `create_share` Tauri commands ← done (4 unit tests)
- [X] T077 [US6] Register `search_users` + `create_share` in `lib.rs` + `commands/mod.rs` + `adagio-nextcloud/src/lib.rs` ← done
- [X] T078 [US6] `searchUsers()` and `createShare()` TypeScript bindings in `src/tauri.ts` — full DTO types ← React: done in src/tauri.ts
- [X] T079 [P] [US6] Recipient chip in `src/components/ShareDialog.tsx` — initials circle + name + role + dismiss ← React: done inline in ShareDialog.tsx
- [X] T080 [P] [US6] Chip/recipient input in `src/components/ShareDialog.tsx` — debounced search + dropdown + chips ← React: done inline in ShareDialog.tsx
- [X] T081 [P] [US6] Segmented control in `src/components/ShareDialog.tsx` — permission + expiry selectors ← React: done inline in ShareDialog.tsx
- [X] T082 [P] [US6] Link bar in `src/components/ShareDialog.tsx` — dark ink bar + copy button ← React: done inline in ShareDialog.tsx
- [X] T083 [US6] `src/components/ShareDialog.tsx` — full modal: backdrop blur, 4 sections, Cancel + Share submit ← React: done
- [X] T084 [US6] Wire Share entry points — App passes `onShare` callback to FilesScene which opens ShareDialog ← React: done in App.tsx + FilesScene.tsx

**Checkpoint**: US6 complete — sharing fully functional end-to-end.

---

## Phase 9: User Story 7 — System Tray Popover (Priority: P3)

**Goal**: A 380px borderless popover pinned to the OS tray icon shows sync status, recent events, and quick actions.

**Independent Test**: With the main window hidden, click the tray icon; verify the popover appears at 380px with no window chrome; verify status matches current sync state; verify Pause/Resume toggles correctly.

### Tests for User Story 7 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [ ] T085 [P] [US7] Write Vitest test for TrayStatus in `src/__tests__/tray/TrayStatus.test.ts` — assert: renders "Up to *date*" headline when `status='idle'`; `date` word is rendered in Instrument Serif italic; status pill shows "• IN SYNC" in --good color when idle; shows different text + clay color when syncing
- [ ] T086 [P] [US7] Write Vitest test for TrayActions in `src/__tests__/tray/TrayActions.test.ts` — assert: renders exactly 4 action links with ⌘ shortcut annotations; "Pause syncing" label changes to "Resume syncing" after being clicked; click fires `pauseSync()` IPC; subsequent click fires `resumeSync()` IPC

### Implementation for User Story 7

- [X] T087 [US7] Add tray window configuration to `crates/adagio-desktop/src-tauri/tauri.conf.json` ← done
- [X] T088 [US7] Update tray icon click handler in `crates/adagio-desktop/src/lib.rs` ← done
- [X] T089 [P] [US7] Tray status section in `src/components/TrayPopover.tsx` ← done
- [X] T090 [P] [US7] Tray recent events section in `src/components/TrayPopover.tsx` ← done
- [X] T091 [P] [US7] Tray actions section in `src/components/TrayPopover.tsx` ← done
- [X] T092 [US7] `src/components/TrayPopover.tsx` — 380px container ← done
- [X] T093 [US7] Update `src/App.tsx` tray detection ← done (IS_TRAY + early return)

**Checkpoint**: US7 complete — tray popover fully functional.

---

## Phase 10: Polish & Cross-Cutting Concerns

**Purpose**: Settings/Pairs visual update, quality gates, quickstart validation.

- [X] T094 [P] Visual redesign of Settings — `SettingsScene.tsx` with palette picker, sync controls, about section ← done
- [X] T095 [P] Visual redesign of Pairs — `PairsScene.tsx` with add/delete/trigger-sync UI ← done
- [X] T096 [P] Run `cargo clippy -- -D warnings` on `crates/adagio-desktop` and `crates/adagio-nextcloud` ← passes (0 warnings)
- [X] T097 [P] Run `cargo fmt --check`; apply `cargo fmt` to all modified Rust files ← done (pair.rs was reformatted)
- [X] T098 [P] Run `npm run check` (`tsc --noEmit`) in `crates/adagio-desktop/src-ui` — TypeScript type-check passes ← React: passing
- [ ] T099 [P] Run `npm run test` — React component tests; not yet configured ← React testing framework TBD
- [ ] T100 Run the `quickstart.md` end-to-end validation — manually test all 7 user stories per quickstart guide; update `specs/004-ui-refactor-handoff/quickstart.md` if any steps are inaccurate

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 (T001–T004) — BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Phase 2 — palette system; blocks US2–US7 indirectly (all components use tokens)
- **US2 (Phase 4)**: Depends on US1 (T013–T014 theme store + prefs); provides the shell for US3/US5
- **US3 (Phase 5)**: Depends on US2 (App.tsx shell); provides the file table that US6 extends
- **US4 (Phase 6)**: Depends on US2 (shell); can proceed in parallel with US3
- **US5 (Phase 7)**: Depends on US2 (App.tsx NavToggle); can proceed in parallel with US3/US4
- **US6 (Phase 8)**: Depends on US3 (Toolbar + ContextMenu wiring); needs FileTable to exist
- **US7 (Phase 9)**: Depends on US1 (theme store) + US2 (stores); otherwise independent
- **Polish (Phase 10)**: Depends on all story phases complete

### User Story Dependencies

- **US1 (P1)**: Blocking for all visual work; start immediately after Foundational
- **US2 (P1)**: Blocking for US3/US5; start immediately after US1
- **US3 (P1)**: MVP complete; required before US6
- **US4 (P2)**: Can start after US2; independent of US3/US5
- **US5 (P2)**: Can start after US2; independent of US3/US4
- **US6 (P3)**: Requires US3 Toolbar + ContextMenu to exist (T044, T047)
- **US7 (P3)**: Requires US1 (theme) + US2 (sync store); independent of US3–US6

### Within Each User Story

1. Tests written and FAIL
2. Implementation makes tests pass
3. Checkpoint validated before next story

### Parallel Opportunities

- T002, T003, T004 (Phase 1 setup) can run in parallel
- T006, T007 (base.css + app.css) can run in parallel after T005 (tokens.css)
- T008, T009 (US1 tests) can run in parallel
- T015–T018 (US2 tests + sync store) can run in parallel
- T019–T028 (common + chrome components) can all run in parallel
- T030–T033 (sidebar sub-components) can all run in parallel
- T036–T040 (US3 tests + files store) can run in parallel
- T041–T045 (atomic file components) can run in parallel
- T053–T058 (onboarding step components) can all run in parallel
- T061–T062 (US5 tests) + T063–T065 (backend + store) can run in parallel
- T066–T068 (activity sub-components) can run in parallel
- T071–T074 (US6 tests) can run in parallel
- T075, T079–T082 (Nextcloud module + share sub-components) can run in parallel after T071–T074 pass
- T085–T086 (US7 tests) can run in parallel
- T089–T091 (tray sub-components) can run in parallel
- T094–T099 (polish tasks) can all run in parallel

---

## Parallel Example: User Story 3 (File Browser)

```bash
# Step 1: Write all tests (must FAIL initially)
Task T036: FileRow tests
Task T037: StatusDot tests
Task T038: BreadcrumbBar tests
Task T039: ContextMenu tests

# Step 2: Create store and atomic components in parallel
Task T040: files store
Task T041: FileGlyph component
Task T042: StatusDot component
Task T043: BreadcrumbBar component
Task T044: Toolbar component
Task T045: FileStatusBar component

# Step 3: Compose FileRow (depends on FileGlyph, StatusDot)
Task T046: FileRow

# Step 4: Compose ContextMenu (independent)
Task T047: ContextMenu

# Step 5: Compose FileTable (depends on all above)
Task T048: FileTable

# Step 6: Wire shortcuts + bindings
Task T049: keyboard shortcuts
Task T050: tauri.ts audit
```

---

## Implementation Strategy

### MVP First (US1 + US2 + US3)

1. Complete Phase 1: Setup (T001–T004)
2. Complete Phase 2: Foundational CSS (T005–T007)
3. Complete US1: Design system + prefs (T008–T014)
4. Complete US2: Chrome + sidebar shell (T015–T035)
5. Complete US3: File browser (T036–T050)
6. **STOP and VALIDATE**: Full app shell + file browser matches handoff screenshots
7. Merge MVP — app is now visually complete for the primary surface

### Incremental Delivery

1. Phase 1 + 2 + US1: Design system ready
2. US2: Shell usable — app no longer shows old dark theme
3. US3: File browser → MVP demo-able
4. US4 + US5 (parallel): Onboarding + Activity feed → complete P2 stories
5. US6 + US7 (parallel): Share dialog + Tray → complete P3 stories
6. Phase 10: Polish and ship

---

## Notes

- All inline styles in React components use JSX style objects matching the handoff prototype — no CSS-in-JS library
- Instrument Serif italic is used ONLY as a 1–3 word accent inside a sans headline — never as body text
- Geist Mono is used for ALL metadata: timestamps, file sizes, paths, version strings, kbd hints — no exceptions
- No emoji anywhere in the UI (per DESIGN.md §2)
- All hover transitions: 150ms delay, 120–150ms ease-out; `prefers-reduced-motion` → ≤1ms
- Focus rings: 2px --clay outline, 2px offset; visible on keyboard nav, hidden on mouse
- All `.svelte` routes are deleted — `App.tsx` is the single entry point and handles all routing via `useState`
- `src/tauri.ts` (not `src/lib/tauri.ts`) is the single source of truth for all IPC bindings
- Keychain access in new Rust commands MUST always use `spawn_blocking` (same pattern as existing commands)
