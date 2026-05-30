# Research: UI Refactor — Design Handoff Implementation

**Feature**: 004-ui-refactor-handoff
**Date**: 2026-05-25

---

## Decision 0: Frontend Framework — Svelte 5 → React 18 (Supersedes Decisions 3, 5, 9)

**Decision**: Switch from Svelte 5 to React 18 (`@vitejs/plugin-react`) as the frontend framework.

**Rationale**: The design handoff prototype (`handoff/source/desktop-app.jsx`, `desktop-scenes.jsx`) is written in React JSX with all styles as inline objects. Every translation pass from JSX inline styles to Svelte CSS classes loses fidelity (flex values, letter-spacing, font-weight, padding). Switching to React allows the prototype's component structure to be adapted directly — mock data is replaced with live Tauri IPC calls, but the style values remain verbatim. The two failed attempts at Svelte translation confirmed this.

**Impact on other decisions**:
- Decision 3 (Dark Mode): Svelte 5 `$state` store → `useState`/`useEffect` in `App.tsx`; `document.documentElement.setAttribute('data-palette', ...)` approach unchanged
- Decision 5 (Testing): `@testing-library/svelte` → `@testing-library/react` (or Vitest + jsdom); deferred
- Decision 9 (Svelte 5 Runes): superseded entirely — React hooks used instead

**Migration scope**:
- Deleted: all 80+ `.svelte` files, `src/lib/stores/`, `src/routes/`, `src/App.svelte`, `src/main.ts`
- Created: `src/main.tsx`, `src/App.tsx`, `src/tauri.ts`, `src/design/`, `src/components/`
- Unchanged: `tauri.conf.json`, all Rust backend code, `public/fonts/`, `vite.config.ts` (updated plugin only)

**Alternatives considered**:
- **Continue translating to Svelte**: Multiple failed attempts demonstrated that every translation pass loses pixel-level fidelity. The prototype's inline-style architecture is incompatible with Svelte's scoped CSS model.
- **Lit/Web Components**: Would require rewriting the prototype from scratch; no advantage over React given the prototype is JSX.

---

## Decision 1: CSS Token Delivery

**Decision**: CSS custom properties on the `[data-palette]` attribute of the `<html>` element, loaded from a global `tokens.css` stylesheet ported directly from `handoff/source/tokens.css`.

**Rationale**: CSS custom properties are the only CSS mechanism that can be overridden at runtime without a reload. Placing palette blocks on `[data-palette="sienna"]` / `[data-palette="ink"]` etc. means a single attribute change on `<html>` switches the entire app simultaneously. No JS manipulation of individual values is needed. Tauri's CSP (`style-src 'self' 'unsafe-inline'`) already permits this pattern.

**Alternatives considered**:
- **CSS Modules per-component**: Cannot cascade palette tokens across the component tree without prop drilling.
- **Tailwind**: Explicitly rejected by the design handoff ("anything but Tailwind").
- **vanilla-extract**: Adds a build-time dependency and complexity that the handoff's bespoke token set doesn't need.
- **JS-in-CSS (styled-components style)**: Would fight Svelte's scoped style system.

---

## Decision 2: Font Strategy

**Decision**: Self-host all three typefaces (`Geist`, `Geist Mono`, `Instrument Serif`) as `.woff2` files in `crates/adagio-desktop/src-ui/public/fonts/`. Declare them via `@font-face` in the global `tokens.css`.

**Rationale**: A desktop Tauri app should not require internet access to render correctly. Google Fonts CDN and Vercel's Geist CDN are both blocked by Tauri's default CSP (`default-src 'self'`). Self-hosting is required.

**Sources**:
- `Geist` + `Geist Mono`: Available from the `geist` npm package (MIT license, created by Vercel). Extract `.woff2` at build time or vendor them directly.
- `Instrument Serif`: Available from Google Fonts under the OFL license. Download the italic variant (the only weight used).

**Alternatives considered**:
- **CDN loading**: Blocked by CSP. Would require relaxing `font-src` in `tauri.conf.json` to include `https://fonts.gstatic.com` — an unnecessary network dependency.
- **System fonts**: No system font provides the design's specific aesthetic requirements (tight-tracked Geist, monospaced metadata, serif italic accent).

---

## Decision 3: Dark Mode / Palette Switching

**Decision**: *(Updated by Decision 0)* React `useState`/`useEffect` in `App.tsx` holds the active palette name. App.tsx applies it as a `data-palette` attribute on `<html>`. The value is persisted to `config.json` via a new `set_palette` Tauri command and restored on launch via `get_palette`. OS dark-mode preference (via `window.matchMedia('prefers-color-scheme: dark')`) is checked on startup; if the user has never set a preference, dark mode maps to the `ink` palette.

**Rationale**: Keeping palette state in React `useState` at the App.tsx level ensures any component can receive it via props. The `document.documentElement.setAttribute('data-palette', ...)` approach is identical regardless of framework. Persisting via `config.json` is consistent with how other app preferences are stored.

**Alternatives considered**:
- **localStorage**: Simpler but not shared with the Rust config layer; would diverge from the existing preference model.
- **Tauri's store plugin**: An additional dependency for a single string value; overkill.

---

## Decision 4: System Tray Popover Window

**Decision**: Register the tray icon via Tauri 2's existing `tray-icon` feature (already enabled in `Cargo.toml`). Clicking the tray icon creates/toggles a second `WebviewWindow` with `decorations: false`, `width: 380`, `resizable: false`, positioned near the tray icon coordinates. The tray window serves `/?tray=1` and App.tsx checks `new URLSearchParams(location.search).has('tray')` to render `TrayPopover.tsx` instead of the main shell.

**Rationale**: Avoids a second Vite entry point (simpler build configuration). The single-page App.tsx already handles conditional scene rendering; adding one more condition is minimal. Tauri 2's `WebviewWindowBuilder::new()` with `position()` set to the tray icon coordinates produces the correct popover behavior.

**Alternatives considered**:
- **Separate Vite entry (multi-page app)**: More correct architecturally, but requires splitting the Vite config and complicating the build. Given the tray UI is small, the route-within-same-SPA approach is pragmatic.
- **OS native menu (using `tauri-plugin-menu`)**: Cannot be styled with design tokens. The handoff explicitly shows a designed HTML popover, not an OS menu.
- **Embed tray UI inside main window as a floating panel**: Not possible when the main window is hidden/minimized; breaks the "tray = always accessible" contract.

---

## Decision 5: Component Testing Framework

**Decision**: *(Updated by Decision 0)* Testing framework selection deferred. Options: `@testing-library/react` + Vitest + jsdom, or Vitest with React support. Will be configured once the React component set stabilizes. `npm run check` (`tsc --noEmit`) passes and is the current quality gate.

**Rationale**: The switch to React invalidates the original `@testing-library/svelte` choice. `@testing-library/react` is the idiomatic equivalent and integrates with the existing Vite/Vitest setup. Deferring avoids configuring a test framework mid-implementation when component APIs may still shift.

**Note on test-first for UI components**: Constitution Principle I requires tests to FAIL before implementation. This principle is deferred for React components until the testing framework is configured. TypeScript strict mode (`tsc --noEmit`) provides a partial substitute as a compile-time correctness gate.

**Alternatives considered**:
- **Playwright (e2e)**: Complementary, not a replacement. Playwright e2e tests require a running Tauri app and are expensive; unit tests with @testing-library are cheaper for the 30+ components in this feature.
- **Storybook**: Excellent for visual iteration but adds significant dev dependency weight. Not needed given the handoff already provides the design spec.

---

## Decision 6: Keyboard Shortcuts

**Decision**: Register app-level keyboard shortcuts via `document.addEventListener('keydown', ...)` in `App.tsx` (not in each scene component). The handler is a single switch statement on `event.key` + modifier flags. Tray-window shortcuts (⌘O, ⌘B, ⌘P, ⌘,, ⌘Q) are also handled in `TrayPopover.tsx`.

**Rationale**: The `tauri-plugin-globalShortcut` (not currently installed) intercepts shortcuts even when the app is not focused — not needed here. All shortcuts in the spec (`⌘K`, `⌘N`, `⌘O`, `⌘B`, `⌘P`, `⌘,`, `⌘Q`) are in-app shortcuts that should only fire when Adagio has focus. DOM event listeners are sufficient and require no new Tauri plugin.

**Alternatives considered**:
- **tauri-plugin-globalShortcut**: Overkill; intercepts at OS level even when app not focused.
- **Per-component shortcut handlers**: Fragile — multiple components can compete for the same keydown event.

---

## Decision 7: Context Menu

**Decision**: Custom React component (inline in `FilesScene.tsx`, to be extracted as `ContextMenu.tsx`) that renders an absolutely-positioned panel. Triggered by the `contextmenu` DOM event on file rows. Position is clamped to the viewport boundary. Styled with design tokens (paper bg, hairline border, sm shadow, 6px radius).

**Rationale**: A native OS context menu (via `tauri-plugin-menu`) cannot use CSS custom properties or match the handoff's visual design. Custom HTML menus are standard in Electron/Tauri apps.

**Alternatives considered**:
- **tauri-plugin-menu**: Cannot be styled. Rejected.
- **Right-click via CSS :focus-within**: Insufficient for programmatic positioning and touch events.

---

## Decision 8: Gap Analysis — New vs Existing Tauri Commands

The following commands **already exist** and cover UI needs for the refactor:

| UI Need | Existing Command |
|---------|-----------------|
| File browser list | `list_synced_files` |
| Sync status for sidebar footer | `get_status` (SyncStatusDto) |
| Activity feed data | `get_activity_log` (Vec<ActivityEntryDto>) |
| Conflict list | `list_conflicts` |
| Pause/resume sync | `pause_sync` / `resume_sync` |
| Account picker | `list_accounts` |
| Pinned folders (pairs) | `list_pairs` |
| Trigger sync button | `trigger_sync` |

The following commands are **new and must be added**:

| UI Need | New Command |
|---------|-------------|
| Share dialog: search recipients | `search_users(account_id, query)` → calls Nextcloud sharees OCS API |
| Share dialog: create share | `create_share(request)` → calls Nextcloud share OCS API |
| Preference: palette persistence | `get_palette()` / `set_palette(name)` → config.json |
| Activity feed: filter by kind | `get_activity_log` extended with optional `filter` parameter |

**Note**: `get_activity_log` already exists but returns `ActivityEntryDto` — this DTO needs to be verified and potentially extended with a `kind` field (`edit | share | sync | conflict`) to support the activity feed filter pills. If the existing DTO already captures this (given `list_conflicts` and sync events are tracked), the extension may be minimal.

---

## Decision 9: Svelte 5 Runes vs Legacy Syntax *(Superseded by Decision 0)*

**Decision**: *(Superseded)* This decision is no longer applicable. The frontend framework was switched to React 18 (see Decision 0). React hooks (`useState`, `useEffect`, `useCallback`, `useMemo`) replace all Svelte 5 rune patterns (`$state`, `$derived`, `$effect`, `$props`).

**Original rationale**: Svelte 5 runes provide better TypeScript integration, more explicit reactivity, and are the forward-compatible API.

**Alternatives considered**:
- **Keep Svelte 4 legacy syntax**: Would work (Svelte 5 is backward-compatible) but misses the opportunity to clean up reactive declarations during the visual refactor.

---

## Decision 10: Activity Feed DTO Extension

**Decision**: Extend `ActivityEntryDto` in `crates/adagio-desktop/src/commands/sync.rs` with a `kind: String` field (`"edit" | "share" | "sync" | "conflict"`) derived from the existing event data. The filter parameter to `get_activity_log` accepts an optional `kind` string. No new OCS API call is needed — the activity log is populated by the existing sync engine events.

**Rationale**: The existing `get_activity_log` already reads from the SQLite journal. The `kind` classification can be derived from existing fields (e.g., `SyncOp` type → `"sync"`, error status → `"conflict"`, etc.). A new Nextcloud Activity OCS API call would duplicate data already tracked locally.

**Alternatives considered**:
- **Call Nextcloud's `/ocs/v2.php/apps/activity/api/v2/activity`**: Would show server-side activity including edits by other users. Valuable for a future feature but out of scope for this refactor — the spec's activity feed shows local sync events.
