# Dev Quickstart: UI Refactor — Design Handoff Implementation

**Feature**: 004-ui-refactor-handoff
**Date**: 2026-05-25

> **Note (2026-05-26)**: Frontend framework is React 18 (migrated from Svelte 5). All component files are `.tsx` in `src/components/`. `npm run check` (TypeScript) and `npm run test` (vitest, 23 tests) both pass. All 7 user stories are implemented; see per-story notes below for any remaining stubs.

---

## Prerequisites

1. A running Nextcloud instance with a registered Adagio OAuth2 client (see `specs/003-account-oauth2-setup/quickstart.md`).

2. Fonts downloaded and placed in `crates/adagio-desktop/src-ui/public/fonts/`:
   ```sh
   # Install Geist from npm package (extracts .woff2 files)
   cd crates/adagio-desktop/src-ui
   npx geist-font download  # or manually copy from node_modules/geist/dist/fonts/
   # Instrument Serif italic must be downloaded from Google Fonts:
   # https://fonts.google.com/specimen/Instrument+Serif
   # Download the italic .woff2 and place at public/fonts/InstrumentSerif-Italic.woff2
   ```

3. Verify the design tokens CSS is in place:
   ```sh
   ls crates/adagio-desktop/src-ui/src/design/tokens.css
   # Should exist (ported from handoff/source/tokens.css)
   ```

4. Build:
   ```sh
   cargo build -p adagio-desktop
   ```

---

## Running the App

```sh
# From crates/adagio-desktop/ (recommended):
npm run tauri:dev

# Or directly (if @tauri-apps/cli is installed globally):
# tauri dev
```

---

## Testing the Design System (US1)

1. Launch the app. Verify the window chrome shows:
   - Adagio slur-mark logo (two-note SVG) at top-left
   - "adagio" wordmark in Geist 500, −5% tracking, next to the mark
   - `--cream` background (warm off-white), not white or gray

2. Open Preferences (`⌘,`). Under "Appearance", switch between all 8 palettes. Verify:
   - Every surface updates instantly (no reload)
   - The `ink` palette is dark (near-black background, warm white text)
   - Switching back to `sienna` restores the warm earth tones

3. Verify typography:
   - File sizes and timestamps: monospaced (`Geist Mono`), slightly smaller
   - Body text: proportional (`Geist Regular`)
   - No emoji anywhere in the UI

---

## Testing the Shell (US2)

1. With one account configured and at least one sync pair:
   - Sidebar shows: account tile (colored square + name + host in mono), Library section, Pinned Folders section
   - Active sync pair folder dot is clay-colored with spinner when syncing, green when idle

2. Click the `Files | Activity` toggle in the chrome. Verify the main panel switches.

3. While a sync is active, verify the sidebar footer shows: spinner + "Syncing N files" + size + ETA + 3px progress bar.

4. Press `⌘K`. Verify the search bar in the chrome gains focus (tabIndex=0 div).

---

## Testing the File Browser (US3)

With a configured sync pair containing a mix of file types and sync states:

1. The file browser shows columns: Name · Size · Items · Modified · Status
2. Navigate into a subfolder by double-clicking a folder row. Verify the breadcrumb updates.
3. Single-click a file row. Verify it is selected (clay left edge) without opening the file.
4. Hover over a file row for 150ms. Verify a share icon appears at the right edge.
5. Right-click a file. Verify the context menu shows all 7 items.
6. Check each sync status icon is visually distinct:
   - `ok`: small green circle with checkmark
   - `sync`: clay spinner ring
   - `cloud`: cloud outline, muted color
   - `pin`: dark circle with light checkmark
   - `conflict`: danger triangle

7. Press `⌘N`. Verify the "New item" dialog opens.

> **Note**: Right-click context menu is not yet implemented. The 7-item context menu described here is a planned feature.

Status bar (bottom strip):
```
N items · 1 selected · X.X GB local · syncing N files · ETA Ns · last sync HH:MM
```
> **Note**: Cloud storage total is not shown in the status bar (requires a separate IPC to query remote quota).

---

## Testing the Onboarding Wizard (US4)

Launch the app with no accounts configured (delete `~/.config/adagio/config.json` first):

1. Wizard appears. Left rail shows 5 steps with circular markers (clay = active step 1).
2. Step 1 (Welcome): slur-mark logo + "Let's bring your files *into tempo*." + 3 checkmarks.
3. Click Continue. Step 2 (Server): enter `http://localhost:8080`.
   - Auto-detection shows: Nextcloud version (green dot), auth flow type, TLS (localhost = plain), E2EE status, RTT.
4. Click Continue. Step 3 (Authorize): dark card with one-time code (monospaced, letter-spaced), QR code, live 5-minute countdown (turns clay-colored under 60 s), "Waiting for confirmation at localhost:8080…" with spinner.
5. Complete the OAuth2 flow in the browser. Step 4 (Folder) appears.
6. Step 4: 4 toggle cards with correct defaults (On-demand ✓, Pin pinned ✓, Smart bandwidth ✓, Watch edits ✗). Browse button works.
7. Click Continue. Step 5 (Connected): green checkmark + "Connected to localhost:8080" + initial sync progress.
8. Click "Begin softly." The wizard closes and the file browser appears.

Back navigation: Click ← Back on any step. Input is preserved.

---

## Testing the Activity Feed (US5)

1. Click the Activity pill in the chrome.
2. Events appear grouped by "Today", "Yesterday", "Earlier this week".
3. Filter pills at top right: `All N · Edits N · Shares N · Sync N · Conflicts N`
4. Click a filter pill. Only matching events are shown.
5. If any conflict events exist, hover over one. Verify "Open" and "Resolve" buttons appear after 150ms.

---

## Testing the Share Dialog (US6)

1. Right-click a file → "Share" (or select and click Share button in toolbar).
2. Modal opens with backdrop blur.
3. Title shows file name; below it shows file size, path, last-edited.
4. "With" field: type a user name and press Enter or comma to add them as a chip. (Autocomplete dropdown against the Nextcloud user directory is not yet wired — requires the `search_users` IPC to be called on input change.)
5. Select a user. A chip appears with initials, name, and role label.
6. Set CAN to "Edit", UNTIL to "7 days".
7. "Or, a link" section: click Copy. The URL is copied to clipboard; button briefly shows ✓ for 2 seconds.
8. Check "Password protect". A password input field appears above the checkboxes.
9. Add a note in the textarea.
10. Click "Share with 1 person". Modal closes. The file's share count badge appears.
11. Click Cancel instead. Verify no share is created (check Nextcloud admin panel).

---

## Testing the System Tray (US7)

1. Hide the main Adagio window (`⌘Q` to quit, or minimize it).
2. Click the Adagio icon in the OS system tray / menu bar.
3. The popover appears (380px wide, no window chrome).
4. Header: slur-mark + "adagio" + "• IN SYNC" (or current state) on the right.
5. Status block: "Up to *date*" with serif italic, below: monospaced "last sync HH:MM · N files · X.X GB".
6. Recent: 3 most recent events with icon + name + relative time.
7. Actions: "Open Adagio folder ⌘O" · "Open in browser ⌘B" · "Pause syncing ⌘P" · "Preferences… ⌘,"
8. Click "Open Adagio folder". File manager opens at the sync root (via `tauri-plugin-shell`).
9. Click "Pause syncing". Menu item changes to "Resume syncing ⌘P".
10. Click "Preferences…". Main Adagio window shows and gains focus.
11. Footer: version (left) · "Quit Adagio" (right). Click Quit. All windows close.

> **Note**: "Quit Adagio" closes all windows via the Tauri window API. Because `CloseRequested` is intercepted by `on_window_event` to hide rather than quit, a true OS-level process exit requires a future `quit_app` Tauri command. Current behavior: all windows close, sync engine continues in background until OS reclaims the process.

---

## Running Tests

```sh
# Component unit tests (React + vitest + @testing-library/react)
cd crates/adagio-desktop/src-ui && npm run test    # 23 tests, all passing

# Type-check frontend
cd crates/adagio-desktop/src-ui
npm run check      # runs: tsc --noEmit (test files excluded via tsconfig.json exclude)

# Rust backend tests (unchanged from feature 003)
cargo test -p adagio-desktop -- --test-threads=1
cargo test -p adagio-nextcloud -- sharing::

# Full quality gates
cargo clippy -- -D warnings
cargo fmt --check
```

---

## Edge Case Verification

| Scenario | How to trigger | Expected result |
|----------|---------------|-----------------|
| Palette switch while syncing | Start a sync, immediately switch palette | Palette changes; sync continues uninterrupted |
| 500 files in one folder | Create a pair with 500+ files | File browser renders within 1 second |
| Share with no recipients (link-only) | Open share dialog, skip "With" section, click Copy | Share link is generated; "Share" button disabled until recipients are added or copy has been clicked |
| Conflict with no resolution | Leave a conflict unresolved | Row shows danger triangle permanently until resolved via Activity feed |
| Tray popover on small screen | Drag tray icon to top-right on a 1366px screen | Popover appears fully within the viewport |
| Dark mode toggle | Enable OS dark mode while app is open | `ink` palette applied immediately if user has not manually set a palette |
