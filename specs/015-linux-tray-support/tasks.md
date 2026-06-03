---
description: "Task list for Linux System Tray Support (015)"
---

# Tasks: Linux System Tray Support

**Input**: Design documents from `specs/015-linux-tray-support/`

**Prerequisites**: plan.md ✅ · spec.md ✅ · research.md ✅ · data-model.md ✅ · contracts/tray-window.md ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY. Tests MUST be written first and MUST FAIL before implementation begins.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: User story label (US1–US5 per spec.md)

---

## Phase 1: Setup

**Purpose**: Create scaffolding and eliminate developer confusion from stale files before any implementation work.

- [x] T001 Delete stale `crates/adagio-desktop/src-tauri/tauri.conf.json` — this file is NOT compiled by Tauri 2 (`src-tauri/src/` is empty); the active config is `crates/adagio-desktop/tauri.conf.json` (adjacent to Cargo.toml); the stale copy has a partial tray window definition that would confuse implementers targeting the wrong file
- [x] T002 [P] Create ADR skeleton at `docs/adr/018-linux-tray-window.md` with sections: Status, Context, Decision, Consequences (content filled in Phase 8)

---

## Phase 2: Foundational — Tests First (Red Phase)

**Purpose**: Write ALL failing tests before any implementation. Constitution Principle I is NON-NEGOTIABLE.

**⚠️ CRITICAL**: Every test here MUST fail before proceeding to Phase 3.

- [x] T003 Write failing Rust test `compute_tray_position_top_panel` in `crates/adagio-desktop/src/lifecycle.rs` — asserts popover Y > click Y when click.y < screen_h/2
- [x] T004 [P] Write failing Rust test `compute_tray_position_bottom_panel` in `crates/adagio-desktop/src/lifecycle.rs` — asserts popover Y < click Y when click.y > screen_h/2
- [x] T005 [P] Write failing Rust test `compute_tray_position_wayland_fallback` in `crates/adagio-desktop/src/lifecycle.rs` — asserts fallback to (screen_w - 380 - 16, 48) when click = (0.0, 0.0)
- [x] T006 [P] Write failing Rust test `get_platform_is_valid_string` in `crates/adagio-desktop/src/commands/mod.rs` — asserts `["linux", "macos", "windows"].contains(&get_platform())` (platform-neutral; must NOT use `assert_eq!(p, "linux")` which would fail macOS/Windows CI)
- [x] T007 [P] Write failing frontend test `renders Ctrl+ modifier on Linux` in `crates/adagio-desktop/src-ui/src/__tests__/TrayPopover.test.tsx` — mocks `invoke('get_platform')` → `"linux"`, asserts kbd text contains `"Ctrl+"`

**Checkpoint**: Run `cargo test -p adagio-desktop` and `npm test` — T003–T006 and T007 MUST fail with "function not found" or compilation errors.

---

## Phase 3: User Story 1 — Tray Icon Visible in Top Bar (P1) 🎯 MVP

**Goal**: The adagio monochrome icon appears in the Linux system notification area when Adagio starts. App does not crash if no tray host is available.

**Independent Test**: Launch `cargo tauri dev` on Ubuntu 22.04 with AppIndicator extension; confirm icon appears in top bar within 3 seconds. Or run `busctl --user monitor` and confirm a StatusNotifierItem is registered.

### Implementation for User Story 1

- [x] T008 [US1] Add `"tray"` webview window entry to `app.windows` array in `crates/adagio-desktop/tauri.conf.json` — set label (`"tray"`), url (`"index.html?tray"`), width (380), height (520), visible (false), decorations (false), alwaysOnTop (true), resizable (false), skipTaskbar (true) — do NOT add `shadow` or `focus` fields; these are not valid Tauri 2 cross-platform window properties
- [x] T009 [P] [US1] Change `app.trayIcon.iconPath` from `"icons/icon.png"` to `"icons/png-mono/adagio-icon-mono-24.png"` in `crates/adagio-desktop/tauri.conf.json`
- [x] T010 [P] [US1] Add `bundle.deb.depends` array with `["libayatana-appindicator3-1 | libappindicator3-1"]` and `bundle.rpm.depends` with `["libayatana-appindicator3"]` in `crates/adagio-desktop/tauri.conf.json`
- [x] T011 [US1] Add `tracing::warn!("no tray icon registered — tray host may be unavailable on this desktop environment")` guard for `app.tray_by_id("main").is_none()` in `crates/adagio-desktop/src/lib.rs` setup function (after existing tray lookup block)

**Checkpoint**: `cargo build -p adagio-desktop` must succeed. On a Linux desktop with AppIndicator, the icon should now appear in the notification area.

---

## Phase 4: User Story 2 — Tray Popover Opens on Left-Click (P1)

**Goal**: Left-clicking the tray icon opens the 380 px popover anchored correctly below (top panel) or above (bottom panel) the icon. Clicking outside dismisses it. Works on both X11 and Wayland (with fallback).

**Independent Test**: Click tray icon — popover appears with correct position. Click outside — popover hides. Click icon again — popover reappears. Verify on KDE (bottom panel) and GNOME (top panel).

### Implementation for User Story 2

- [x] T012 [US2] Implement `pub fn compute_popover_position(click: (f64, f64), win_size: (u32, u32), screen_w: u32, screen_h: u32) -> (i32, i32)` in `crates/adagio-desktop/src/lifecycle.rs` — top-panel path (y = click.y + 8), bottom-panel path (y = click.y - win_h - 8), Wayland fallback when click == (0.0, 0.0) → (screen_w - 380 - 16, 48); add `/// doc comment`
- [x] T013 [US2] Replace inline position math in tray click handler (`lib.rs` lines 126–133) with call to `lifecycle::compute_popover_position`; use `app_handle.primary_monitor().ok().flatten()` for screen dimensions; add `tracing::debug!` log on Wayland fallback; add `let Some(tray_win) = ... else { tracing::warn!(...); return; }` guard in `crates/adagio-desktop/src/lib.rs`
- [x] T014 [US2] Verify `WindowEvent::Focused(false)` handler (line 149 of `lib.rs`) already covers the `"tray"` label — confirm the window label check is present and add `tracing::debug!("tray window lost focus, hiding")` log line in `crates/adagio-desktop/src/lib.rs`

**Checkpoint**: Run `cargo test -p adagio-desktop` — T003, T004, T005 now PASS. Manual: click tray icon, popover appears; click outside, popover hides.

---

## Phase 5: User Story 3 — Quick Actions Work from Tray (P2)

**Goal**: All four action rows function correctly. Keyboard shortcut labels show `Ctrl+` on Linux instead of `⌘`.

**Independent Test**: With popover open, click each action and verify the expected side effect (file manager, browser, pause toggle, main window focus, quit).

### Implementation for User Story 3

- [x] T015 [US3] Implement `get_platform()` Tauri command in `crates/adagio-desktop/src/commands/mod.rs` — `#[tauri::command] pub fn get_platform() -> &'static str` with `#[cfg(target_os)]` branches for linux/macos/windows; add `/// doc comment`
- [x] T016 [US3] Register `commands::get_platform` in the `invoke_handler!` macro in `crates/adagio-desktop/src/lib.rs` (append to existing list)
- [x] T017 [P] [US3] Add `export const getPlatform = (): Promise<'linux' | 'macos' | 'windows'> => invoke('get_platform');` to `crates/adagio-desktop/src-ui/src/tauri.ts`
- [x] T018 [US3] Update `TrayPopover.tsx` to import `getPlatform`, add `const [platform, setPlatform] = useState<string>('macos')` + `useEffect` to call `getPlatform()` on mount, replace hardcoded `'⌘O'`/`'⌘B'`/`'⌘P'`/`'⌘,'` with template literal using `mod = platform === 'macos' ? '⌘' : 'Ctrl+'` in `crates/adagio-desktop/src-ui/src/components/TrayPopover.tsx`

**Checkpoint**: Run `cargo test -p adagio-desktop` — T006 now PASSES. Run `npm test` — T007 now PASSES. Manual: on Linux, action rows show `Ctrl+O`, `Ctrl+B`, etc.

---

## Phase 6: User Story 4 — Consistent Visual Design (P2)

**Goal**: Tray popover is visually faithful to `handoff/tray.html` on Linux. CSS variables, fonts, and spacing are correct. Empty state and long-name truncation work.

**Independent Test**: Open `handoff/tray.html` in a browser alongside the running popover at `http://localhost:5173/?tray` — layout, typography, and status colors must match visually.

### Implementation for User Story 4

- [x] T019 [US4] Check `crates/adagio-desktop/src-ui/src/app.css` for external font imports (Google Fonts `@import` or `@font-face` with remote URLs); if found, ensure fonts are bundled locally or the tray window's CSP is updated to `"font-src 'self' https://fonts.gstatic.com; style-src 'self' 'unsafe-inline'"` — Tauri's default CSP blocks external font loads in the tray webview
- [x] T020 [P] [US4] Verify `crates/adagio-desktop/src-ui/src/app.css` declares all required CSS custom properties (`--cream`, `--clay`, `--ink`, `--forest`, `--good`, `--warn`, `--danger`, `--hairline`, `--body`, `--mono`, `--serif`, `--r-*`, `--shadow-*`) at `:root` so they are available in the tray webview context
- [x] T021 [P] [US4] Add a layout-sanity test in `crates/adagio-desktop/src-ui/src/__tests__/TrayPopover.test.tsx` asserting: (1) the popover root element has inline `width: 380`, and (2) the "Recent" section heading text is rendered when `getActivityLog` returns entries

**Checkpoint**: `npm test` passes. Opening `?tray` in the dev server at 100% zoom renders a layout matching `handoff/tray.html`.

---

## Phase 7: User Story 5 — Documentation and Daemon Persistence (P3)

**Goal**: Users can follow the quickstart guide on Ubuntu 22.04 without hitting undocumented blockers. The difference between daemon auto-start (existing) and GUI auto-start (deferred v2) is clearly explained.

**Independent Test**: Follow `quickstart.md` from scratch on Ubuntu 22.04; confirm tray icon appears without additional research.

### Implementation for User Story 5

- [x] T022 [US5] Update `specs/015-linux-tray-support/quickstart.md` to clarify that "Start at login" in Preferences controls **daemon** auto-start only; add a note that manual GUI launch is required in v1; add GNOME AppIndicator installation steps if not already present
- [x] T023 [P] [US5] Add a "Linux Tray Requirements" section to the project `README.md` (or create one if absent) covering: (1) AppIndicator extension for GNOME, (2) `libayatana-appindicator3-1` runtime dependency, (3) KDE/XFCE native support without extras

**Checkpoint**: A new user on Ubuntu 22.04 can follow the documentation without running into undocumented blockers.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: ADR content, lint, format, final CI gates.

- [x] T024 Fill `docs/adr/018-linux-tray-window.md` with complete ADR content: Status (Accepted), Context (missing tray window, icon asset, position logic gap, Wayland caveat), Decision (separate window + 24 px mono icon + panel-side heuristic), Consequences (libayatana dep, GNOME extension requirement, Wayland fallback logged at WARN, desktop-app autostart deferred to v2)
- [x] T025 [P] Run `cargo clippy -- -D warnings` and fix all new warnings from T011–T016 in `crates/adagio-desktop/`
- [x] T026 [P] Run `cargo fmt --check` and apply `cargo fmt` if needed in `crates/adagio-desktop/`
- [x] T027 [P] Run `npm run typecheck` (or `tsc --noEmit`) and fix any TypeScript errors in `crates/adagio-desktop/src-ui/`
- [x] T028 Run `cargo test -p adagio-desktop` — all position tests (T003–T005) and platform test (T006) MUST pass
- [x] T029 [P] Run `npm test` in `crates/adagio-desktop/src-ui/` — all TrayPopover tests including T007 (Ctrl+ label) and T021 (layout sanity) MUST pass

**Checkpoint**: All CI gates pass. Feature complete.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately; T001 and T002 are parallel
- **Foundational (Phase 2)**: Depends on Phase 1 — BLOCKS all user stories (write tests first!)
- **US1 (Phase 3)**: Depends on Phase 2 — config changes only; T009 and T010 parallel with T008
- **US2 (Phase 4)**: Depends on Phase 3 (tray window must exist before click handler is meaningful); T012 is a pure function independent of T013/T014
- **US3 (Phase 5)**: Depends on Phase 2; T017 parallel with T015
- **US4 (Phase 6)**: Depends on Phase 2 only — visual verification is independent of US2/US3
- **US5 (Phase 7)**: Documentation only — parallel with US3/US4
- **Polish (Phase 8)**: Depends on all user story phases; T025/T026/T027 are parallel

### User Story Dependencies

- **US1 (P1)**: Depends on Phase 1 only
- **US2 (P1)**: Depends on US1 (tray window must exist)
- **US3 (P2)**: Depends on US2; T017 can start after Phase 2
- **US4 (P2)**: Depends on Phase 2 only
- **US5 (P3)**: No code dependency

### Parallel Opportunities Within Phases

- **Phase 1**: T001 and T002 are independent
- **Phase 3**: T009 and T010 run in parallel with T008
- **Phase 5**: T017 (TypeScript binding) runs in parallel with T015 (Rust command)
- **Phase 6**: T020 and T021 run in parallel with T019
- **Phase 7**: T022 and T023 are independent
- **Phase 8**: T025, T026, T027 run in parallel

---

## Parallel Example: User Story 2

```bash
# T012 (pure function) can start as soon as tests are written:
Task T012: "Implement compute_popover_position() in lifecycle.rs"

# After T012 completes:
Task T013: "Update tray click handler in lib.rs"
Task T014: "Verify focus-lost dismiss in lib.rs"
```

---

## Implementation Strategy

### MVP First (US1 + US2 Only — visible, clickable tray)

1. Complete Phase 1: Setup (T001–T002)
2. Complete Phase 2: Write all failing tests (T003–T007)
3. Complete Phase 3: US1 — icon visible (T008–T011)
4. Complete Phase 4: US2 — popover opens (T012–T014)
5. **STOP and VALIDATE**: Tray icon appears on Linux; click opens popover; click outside closes it
6. Demo/ship as functional baseline

### Incremental Delivery

1. Phases 1–4 → Tray visible and clickable (MVP)
2. Phase 5 → Correct keyboard shortcut labels
3. Phase 6 → Visual fidelity verified
4. Phase 7 → Users can follow docs without confusion
5. Phase 8 → All quality gates pass → merge to main

---

## Notes

- T001 (delete stale `src-tauri/tauri.conf.json`) must be done first to prevent the wrong file being edited
- T008 (add tray window to `tauri.conf.json`) is the single highest-impact implementation task — nothing else works without it
- T009 and T010 are config-only and can be committed separately from T008
- `TrayPopover.tsx` is already feature-complete for content (status, recent, actions, footer) — Phase 5 adds only the platform modifier label
- `compute_popover_position` in T012 is a pure function — write it test-first; no Tauri runtime needed
- T014 (focus-loss dismiss) is already coded in lib.rs; T014 is verification + a log line, not new logic
- `[P]` tasks = different files, no blocking dependency on each other
- Commit after T008–T011 (config changes); after T012–T014 (Rust position logic); after T015–T018 (TypeScript)
