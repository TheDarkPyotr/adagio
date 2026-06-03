# Feature Specification: UI Refactor — Design Handoff Implementation

**Feature Branch**: `004-ui-refactor-handoff`

**Created**: 2026-05-25

**Status**: In Progress

**Input**: User description: "Full refactor of the UI based on the sketch provided in the handoff folder — integrate full UI support as in the sketch, with all features the UI exposes."

---

## Overview

Adagio's current UI is a functional but unstyled scaffold. This feature replaces it entirely with the visual identity and interaction model defined in the design handoff (`handoff/`). The result is a quiet, editorial desktop client that keeps sync state permanently visible without ever interrupting the user with dialogs or badges.

The handoff defines five screens: an onboarding wizard, a file browser, an activity feed, a share dialog, and a system-tray popover. It also defines a complete design system (tokens, typography, color palettes, component vocabulary). This spec covers all five screens and the underlying design system as independently deliverable user stories.

---

## User Scenarios & Testing

### User Story 1 — Design System Foundation (Priority: P1)

A developer opening the application for the first time after the refactor sees a consistent visual language across every surface: warm cream backgrounds, tight-tracked Geist typography, Geist Mono for all metadata, and the Adagio slur-mark in the top-left. The app feels editorial and calm — nothing fights for attention.

**Why this priority**: All other user stories depend on the design system. Without tokens, typography, and the component vocabulary in place, no screen can be built correctly. This is the foundational layer.

**Independent Test**: Open the app. Verify: (a) the window chrome appears in the correct background color with the slur-mark logo and wordmark; (b) all text uses the correct typefaces at the correct weights; (c) the eight color palettes are selectable and immediately update the entire UI; (d) switching to dark mode inverts the palette correctly while keeping all component shapes intact.

**Acceptance Scenarios**:

1. **Given** the app is launched, **When** the main window renders, **Then** the background color matches the canonical sienna palette `--cream` token, and text uses Geist at the specified weights and tracking values.
2. **Given** the user opens Preferences and selects a different palette, **When** the palette changes, **Then** every surface — chrome, sidebar, file table, modals — updates simultaneously without a reload.
3. **Given** the OS dark-mode preference is active, **When** the app launches, **Then** the `ink` palette is applied automatically, inventing the polarity of cream/ink tokens while preserving all other design decisions.
4. **Given** the user has enabled "Reduce Motion" in OS accessibility settings, **When** any transition would normally play, **Then** all transitions complete in ≤1ms.

---

### User Story 2 — Window Chrome & Sidebar Navigation (Priority: P1)

A returning user opens Adagio and immediately sees the chrome bar with the Files/Activity toggle, a ⌘K search bar, notification bell, settings gear, and account avatar. The sidebar shows the account they are connected to, a library navigation section (All files, Favorites, Recent, Shared, Tagged), and a list of pinned folders with per-folder sync indicators. A footer strip in the sidebar shows live sync progress whenever files are actively syncing.

**Why this priority**: The chrome and sidebar are the persistent shell that frames every screen. They must exist before any content screen can be built or tested.

**Independent Test**: Launch the app with one account and at least one pinned folder configured. Verify: chrome renders with all five right-side controls; the Files/Activity toggle switches the main panel; the sidebar shows the account, library section, pinned folders with correct status dots, and the sync footer animates when syncing is active.

**Acceptance Scenarios**:

1. **Given** one account is configured, **When** the main window opens, **Then** the sidebar top shows the account initial tile (30×30), account name, and host in monospace.
2. **Given** the sidebar is visible, **When** a pinned folder is actively syncing, **Then** its dot is clay-colored with a spinner; when in sync the dot is green; when an error exists the dot is danger-colored.
3. **Given** files are being synced, **When** the sidebar footer is visible, **Then** it shows a spinner, "Syncing N files", the total size, an ETA, and a 3px progress bar in the clay accent color.
4. **Given** the user clicks the Files or Activity pill in the chrome toggle, **When** the toggle changes, **Then** the main content area switches instantly without a page reload.
5. **Given** the user presses ⌘K, **When** the shortcut fires, **Then** the search bar in the chrome receives focus.

---

### User Story 3 — File Browser (Priority: P1)

A user navigating their Nextcloud files sees a breadcrumb trail at the top showing their current path, a toolbar with "+ New", "Make available offline", and "Share" buttons, and a sortable table listing files and folders. Each row shows a file-type glyph, name, size, item count (for folders), last-modified date, and a per-file sync status indicator. A status bar at the bottom of the window shows aggregate counts and the current sync state.

**Why this priority**: The file browser is the primary surface users spend most of their time in. It is the core value proposition of the application.

**Independent Test**: Launch the app with a configured sync pair containing mixed files and folders in different sync states. Navigate into a subfolder. Verify: breadcrumb reflects the path; file table shows correct glyphs, sizes, dates, and status icons; the status bar updates to reflect the selected item count; double-clicking a folder navigates into it.

**Acceptance Scenarios**:

1. **Given** the user is at the root of a sync pair, **When** the file browser renders, **Then** the breadcrumb shows `server › folder › **currentFolder**` with the last segment in bold.
2. **Given** a file is in "cloud-only" state, **When** it appears in the file table, **Then** its status column shows a cloud-outline icon; when the user clicks "Make available offline", the status transitions to the syncing indicator and then to the pinned indicator once complete.
3. **Given** a file has a conflict, **When** it appears in the table, **Then** its status shows a danger-colored warning triangle; no modal or toast appears — the indicator is the sole notification.
4. **Given** the user hovers over a file row, **When** hover activates after 150ms, **Then** a share icon appears at the right edge of the row.
5. **Given** the user right-clicks a file or folder, **When** the context menu appears, **Then** it contains: Share, Make available offline, Pin, Copy link, Reveal in file manager, View on server, Move to trash.
6. **Given** the user single-clicks a row, **When** the click registers, **Then** the row is selected (paper-2 background, 2px clay left edge) without opening the item.
7. **Given** the user double-clicks a folder, **When** the double-click registers, **Then** the browser navigates into that folder and the breadcrumb updates.
8. **Given** the user presses ⌘N, **When** the shortcut fires, **Then** a new-folder creation interaction begins inline.

---

### User Story 4 — First-Run Onboarding Wizard (Priority: P2)

A new user who has never connected Adagio to a Nextcloud server is guided through five steps: a welcome screen, a server URL entry screen with automatic detection, a browser-based OAuth2 authorization screen, a local folder and preferences setup screen, and a confirmation screen showing the account is connected and the initial sync has begun.

**Why this priority**: Onboarding is the critical first impression. An existing onboarding flow already works functionally; this redesigns it to match the handoff's 5-step wizard with the new visual identity.

**Independent Test**: Launch the app with no accounts configured. Complete all five steps using a real Nextcloud instance. Verify: each step matches the handoff screenshot; the back/continue navigation works; the step indicators on the left rail update correctly; the app lands in the file browser after step 5.

**Acceptance Scenarios**:

1. **Given** no accounts are configured, **When** the app launches, **Then** the onboarding wizard appears showing Step 1 (Welcome) with the slur-mark logo, tagline, and three value-proposition checkmarks (no telemetry, files in filesystem, AGPL-3.0).
2. **Given** the user is on Step 2 (Server), **When** they type a valid Nextcloud URL and pause, **Then** the server is probed and auto-detection results appear: Nextcloud version with green dot, auth flow type, TLS status, E2EE availability, and round-trip time.
3. **Given** the user is on Step 3 (Authorize), **When** the step renders, **Then** a dark card displays a one-time code in monospaced large type, a QR code, a 5-minute countdown, and "Waiting for confirmation at \<host\>…" with a spinner.
4. **Given** the user is on Step 4 (Folder), **When** the step renders, **Then** four toggle cards appear: On-demand sync (default on), Pin pinned folders (default on), Smart bandwidth (default on), Watch external edits (default off); the user can browse for the local folder path.
5. **Given** the user is on Step 5 (Connected), **When** the step renders, **Then** a forest-green checkmark, "Connected to \<host\>" eyebrow, and a headline confirm success; an initial sync progress card shows "bringing lightest 200 files first".
6. **Given** the user is on any step except Step 1, **When** they click Back, **Then** they return to the previous step with their inputs preserved.
7. **Given** the user is on the last step, **When** they click the final continue button, **Then** the onboarding wizard closes and the main file browser appears.

---

### User Story 5 — Activity Feed (Priority: P2)

A user switching to the Activity tab sees a chronological list of sync events grouped by time period (Today, Yesterday, Earlier this week). Each event shows who did what to which file, in which folder, and how long ago. A filter bar at the top lets them narrow to Edits, Shares, Sync, or Conflicts. Hovering over a conflict event reveals a "Resolve" button.

**Why this priority**: The activity feed is the primary way users understand what has happened to their files. It replaces the current Conflicts screen with a richer, filter-driven view.

**Independent Test**: With at least one of each event type (edit, share, sync, conflict) in the activity log, open the Activity tab. Verify: all events appear in the correct time buckets; the filter pills update the visible set with per-category counts; hovering a conflict row shows "Open" and "Resolve" buttons.

**Acceptance Scenarios**:

1. **Given** the user clicks the Activity pill in the chrome toggle, **When** the activity feed renders, **Then** events are grouped under "Today", "Yesterday", and "Earlier this week" time-bucket headers.
2. **Given** the activity feed is open, **When** the user clicks the "Conflicts" filter pill, **Then** only conflict events are shown, and the pill shows the conflict count.
3. **Given** a conflict event is displayed, **When** the user hovers over it for 150ms, **Then** "Open" and "Resolve" action buttons appear at the right edge of the row.
4. **Given** the activity feed is open, **When** a new sync event arrives in real time, **Then** it appears at the top of the "Today" bucket without requiring a manual refresh.
5. **Given** there are no events matching the selected filter, **When** the filter is applied, **Then** a short empty-state message appears ("Nothing to show here — all quiet.").

---

### User Story 6 — Share Dialog (Priority: P3)

A user who wants to share a file or folder clicks the Share button on a file row or in the toolbar. A modal appears with four sections: a chip input to add recipients by name or email, segmented controls for permission level and expiry, a dark link bar with a generated share URL and copy button, and an optional note textarea. On submission, the share is created in Nextcloud and the modal closes.

**Why this priority**: Sharing is a secondary action that requires a fully-built file browser first (US3). It is a modal overlay and can be developed and tested independently once the chrome and file browser exist.

**Independent Test**: With a file selected, click Share. Fill in a recipient, set permission to "View", expiry to "7 days", copy the link, add a note, and click "Share with 1 person". Verify: modal opens and closes correctly; the recipient chip renders correctly; the share link is generated; the activity feed gains a "shared" event entry.

**Acceptance Scenarios**:

1. **Given** the user clicks Share on a file, **When** the modal opens, **Then** it shows a backdrop blur, the file name as the modal title, file metadata (size, path, last-edited) below the title, and the four content sections.
2. **Given** the user types a name or email in the "With" field, **When** a match is found from the Nextcloud user directory, **Then** suggestions appear and selecting one creates a chip with the user's initials, name, and role label.
3. **Given** recipient chips are present, **When** the user clicks × on a chip, **Then** that recipient is removed from the share.
4. **Given** the "Or, a link" section is visible, **When** the user clicks the copy button, **Then** the share URL is copied to the clipboard and the button briefly confirms the copy.
5. **Given** the user checks "Password protect", **When** the checkbox is ticked, **Then** a password input field appears below the link bar.
6. **Given** the target folder is end-to-end encrypted, **When** the share modal opens, **Then** the footer shows a lock icon with "End-to-end encrypted folder".
7. **Given** the user clicks Cancel, **When** the modal closes, **Then** no share is created and no network requests are sent.

---

### User Story 7 — System Tray / Menu Bar Popover (Priority: P3)

A user who wants a quick status check without switching to the main window clicks the Adagio icon in the OS system tray or menu bar. A 380px popover appears showing the current sync status ("Up to *date*" in serif italic when idle), the last-sync timestamp and file count in monospace, the three most recent activity events, and quick-action links for opening the folder, opening in browser, pausing sync, and opening Preferences.

**Why this priority**: The tray is a secondary access point. It requires the design system and the sync state model to exist (US1, US2), but is otherwise independent of the file browser and activity feed.

**Independent Test**: With the app running and one sync pair active, click the tray icon. Verify: the popover renders at 380px width with no window chrome; "IN SYNC" or the current sync state shows in the header; the recent events section lists the last three events; each quick-action link fires the correct command.

**Acceptance Scenarios**:

1. **Given** the sync engine is idle, **When** the tray popover opens, **Then** the header shows "Up to *date*" (serif italic on "date") and the status pill shows "• IN SYNC" in green monospace.
2. **Given** sync is in progress, **When** the tray popover opens, **Then** the headline changes to reflect the active state (e.g., "Syncing…") and the status pill shows a clay spinner.
3. **Given** the recent-events section is visible, **When** 3 events are listed, **Then** each shows a 24px icon, the file or folder name, and a relative timestamp (e.g., "4 min ago").
4. **Given** the user clicks "Open Adagio folder" (⌘O), **When** the action fires, **Then** the OS file manager opens at the synced folder root and the popover closes.
5. **Given** the user clicks "Pause syncing" (⌘P), **When** the action fires, **Then** the sync engine pauses, the tray icon updates to reflect the paused state, and the menu item changes to "Resume syncing".
6. **Given** the user clicks "Quit Adagio" in the popover footer, **When** the action fires, **Then** the application exits cleanly after any in-progress sync operations complete or are safely interrupted.

---

### Edge Cases

- What happens when the Nextcloud server becomes unreachable mid-session? (Expected: sidebar footer shows error state, per-folder dots turn danger-colored; no modal interruption.)
- What happens when a file is modified locally while upload is in progress? (Expected: row status transitions to syncing; upload restarts from the changed version.)
- What happens when the user has no files in a folder? (Expected: file browser shows an appropriate empty state without hiding the toolbar.)
- What happens when a sync conflict cannot be auto-resolved? (Expected: row shows conflict indicator; activity feed gains a conflict event; no automatic modal.)
- What happens when the user switches palette while a file is syncing? (Expected: palette changes immediately, sync continues uninterrupted.)
- What happens when the system tray popover is opened on a monitor with limited vertical space? (Expected: popover height caps and scrolls the recent-events section rather than overflowing off-screen.)
- What happens when the Nextcloud user directory returns no matches for a share recipient search? (Expected: "No results" label in the suggestion dropdown; user can still add by exact email.)

---

## Requirements

### Functional Requirements

- **FR-001**: The application MUST render using a design token system that maps palette roles (background, text, accent, status) to CSS custom properties, with a sienna palette as the default.
- **FR-002**: The application MUST support eight selectable color palettes, each overriding the full set of design tokens without requiring a reload.
- **FR-003**: The application MUST support a dark mode that is automatically applied when the OS dark-mode preference is active and can be manually overridden in Preferences.
- **FR-004**: All typographic rules from the design handoff MUST be applied: Geist for UI/body, Geist Mono for all metadata (timestamps, file sizes, paths), Instrument Serif italic for accent phrases only.
- **FR-005**: The application MUST display the slur-mark logo and "adagio" wordmark in the window chrome top-left corner.
- **FR-006**: The window chrome MUST contain a Files/Activity toggle pill, a ⌘K search bar, a notification bell, a settings gear, and an account avatar.
- **FR-007**: The sidebar MUST show an account picker tile (initial + name + host), a Library section (All files, Favorites, Recent, Shared, Tagged), a Pinned Folders section with per-folder sync status dots, and a sync-progress footer when syncing is active.
- **FR-008**: The file browser MUST display files and folders in a table with columns: Name, Size, Items, Modified, Status — each sortable by the user.
- **FR-009**: Each file row MUST display a file-type glyph appropriate to the file kind (folder, PDF, Markdown, Figma, ZIP, SVG, audio, plain text, and a generic fallback).
- **FR-010**: Each file row MUST display a status indicator reflecting one of five states: in-sync (green check), syncing (clay spinner), cloud-only (cloud outline), pinned-offline (dark check), conflict (danger triangle).
- **FR-011**: The file browser MUST show a breadcrumb trail reflecting the current navigation path, with the last segment bold.
- **FR-012**: The file browser MUST show a status bar at the bottom with item count, selection count, local storage used, and cloud storage total; plus sync state summary (ETA, last full sync time) on the right.
- **FR-013**: Right-clicking a file or folder MUST present a context menu with: Share, Make available offline, Pin, Copy link, Reveal in file manager, View on server, Move to trash.
- **FR-014**: The onboarding wizard MUST present exactly five sequential steps: Welcome, Server URL + detection, Browser authorization (one-time code + QR), Local folder + toggle preferences, Connected + initial sync progress.
- **FR-015**: The onboarding Server step MUST auto-detect the Nextcloud version, auth flow, TLS status, E2EE availability, and round-trip time once a valid URL is entered.
- **FR-016**: The activity feed MUST display events grouped into time buckets (Today, Yesterday, Earlier this week) with filter pills (All, Edits, Shares, Sync, Conflicts) showing per-category counts.
- **FR-017**: Each activity event MUST display a 32px action-icon circle, an actor name, a verb, a target filename in italic, an optional "with whom" note, a folder path, and a relative timestamp.
- **FR-018**: Hovering over a conflict activity event for 150ms MUST reveal "Open" and "Resolve" action buttons at the row's right edge.
- **FR-019**: The Share dialog MUST support adding recipients via a chip-input with live search against the Nextcloud user directory.
- **FR-020**: The Share dialog MUST provide segmented controls for permission level (View / Comment / Edit) and link expiry (24h / 7 days / 30 days / No limit).
- **FR-021**: The Share dialog MUST generate and display a shareable link with a one-click copy button, and offer optional toggles for password protection, hiding downloads, and open-notification.
- **FR-022**: The system tray popover MUST display the current sync status headline, last-sync metadata in monospace, the three most recent activity events, and quick-action links with keyboard shortcut annotations.
- **FR-023**: Pause/resume syncing MUST be accessible from both the system tray popover (⌘P) and the Preferences screen, and the tray icon MUST reflect the paused state visually.
- **FR-024**: All keyboard shortcuts MUST be functional: ⌘K (search), ⌘N (new folder), ⌘O (open sync folder), ⌘B (open server in browser), ⌘P (pause/resume sync), ⌘, (Preferences), ⌘Q (quit).
- **FR-025**: Focus rings for keyboard navigation MUST be 2px clay-color with 2px offset, visible only during keyboard navigation and hidden on mouse interaction.
- **FR-026**: All hover transitions MUST have a 150ms delay and complete in 120–150ms ease-out; prefers-reduced-motion MUST reduce all transitions to ≤1ms.
- **FR-027**: Sync state MUST be surfaced through: per-row status indicators, sidebar footer, status bar bottom — never through toasts, modals, or badge counters.
- **FR-028**: Conflict state MUST be surfaced through: per-row danger triangle, activity feed entry — never through a blocking modal.

### Key Entities

- **Design Token**: A named CSS custom property (e.g., `--cream`, `--clay`) that maps a semantic role to a palette-specific value. Tokens are overridden per palette by replacing the root custom-property values.
- **Palette**: A named set of design tokens (e.g., "sienna", "ink") covering all 16 semantic color roles. Eight palettes ship; `sienna` is the default; `ink` is the dark mode.
- **File Node**: An item in the file browser — either a file or folder — with a path, kind, size, modification time, sync status, optional share count, and optional item count.
- **Activity Event**: A record of something that happened to a file or folder: who acted, what verb, on what target, in which folder, with whom (optional), at what time. Rendered in the activity feed.
- **Share**: A permission grant giving one or more named users (or any bearer of a link) access to a file or folder with a specified capability level and optional expiry.
- **Account**: A Nextcloud connection — host, display name, user identity, initial character, and color assignment for the avatar tile.
- **Sync State**: The current per-item and aggregate sync health: one of `ok`, `sync`, `cloud`, `pin`, `conflict`. Drives every status indicator in the UI.

---

## Success Criteria

### Measurable Outcomes

- **SC-001**: A new user can complete the entire onboarding wizard (Steps 1–5) and reach the main file browser in under 3 minutes on a stable connection.
- **SC-002**: The file browser renders up to 500 files without visible lag — the list appears fully populated within 1 second of navigating to any folder.
- **SC-003**: A user can share a file with a named recipient and copy the share link in under 30 seconds from first clicking Share.
- **SC-004**: All five sync states (in-sync, syncing, cloud-only, pinned-offline, conflict) are visually distinguishable from one another without requiring any tooltip or label to differentiate them.
- **SC-005**: The application responds to palette changes (including dark mode toggle) without requiring a restart — the change takes effect in under 300ms.
- **SC-006**: All 7 keyboard shortcuts (⌘K, ⌘N, ⌘O, ⌘B, ⌘P, ⌘,, ⌘Q) are functional and discoverable via visible annotations in the tray popover menu.
- **SC-007**: No sync event — including conflicts — triggers a modal dialog or toast notification. All sync state is permanently visible in the chrome, sidebar, or file table.
- **SC-008**: The activity feed filter pills update the visible set within 100ms of being clicked, with no perceptible reflow.

---

## Assumptions

- The existing Tauri + Svelte stack **was replaced** with React 18 (`@vitejs/plugin-react`); the refactor uses React TSX components with inline JSX styles, matching the handoff prototype directly.
- The existing Rust sync engine and Tauri IPC commands (`list_synced_files`, `trigger_sync`, `connect_account_oauth2`, `list_accounts`, `remove_account`, `get_error_items`, etc.) remain unchanged; this feature is purely a frontend redesign.
- Geist and Instrument Serif fonts MUST be self-hosted as `.woff2` files in `public/fonts/`; CDN font loading is blocked by the application's `default-src 'self'` Content Security Policy. Font licensing is handled separately from this feature.
- The system tray popover is implemented as a borderless Tauri window pinned to the tray icon, not a native OS menu.
- "On-demand sync" (cloud-only files that download on open) is surfaced in the UI but its backend implementation may require a separate feature.
- The Nextcloud user-directory search for share recipients is accessed via an existing or new Tauri IPC command — the exact command contract is defined during planning.
- Dark mode detection uses the OS preference signal available to Tauri; manual override is stored in the user config alongside the selected palette name.
- Right-click context menu actions that are not yet backed by backend commands (e.g., "View on server", "Move to trash") will be wired to stubs that show an appropriate "coming soon" state rather than failing silently.
- The prototype source in `handoff/source/` is the **direct implementation reference** — the React JSX components in the prototype are adapted directly, with mock data replaced by live Tauri IPC calls.
