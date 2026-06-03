# Feature Specification: Linux System Tray Support

**Feature Branch**: `015-linux-tray-support`

**Created**: 2026-06-03

**Status**: Draft

**Input**: User description: "I want to make the tray feature, visually described in handbook folder, fully supported, functionally and visually via the UI. The tray must be supported on linux OSes, visible in the top bar and aligned with the daemon and desktop features. Must be consistent and reliable."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Tray Icon Visible in Top Bar (Priority: P1)

After launching Adagio on any supported Linux desktop environment (GNOME with AppIndicator extension, KDE Plasma, XFCE), the adagio icon appears in the system notification area / top-bar tray. The icon uses the correct monochrome asset and updates to reflect sync state (idle, syncing, paused, error).

**Why this priority**: Without a visible tray icon, the user has no ambient indicator that adagio is running. All other tray interactions depend on this.

**Independent Test**: Install Adagio on a fresh Ubuntu 22.04 + GNOME desktop and confirm the icon appears in the top bar without any manual workaround.

**Acceptance Scenarios**:

1. **Given** Adagio is running and the desktop has a compatible tray host, **When** the user looks at the notification area, **Then** the adagio monochrome icon is visible and reacts to the system theme (light/dark).
2. **Given** sync is actively running, **When** the user looks at the tray icon, **Then** the icon or badge communicates a non-idle state (e.g., animated or badge indicator).
3. **Given** the system does not have a tray host (vanilla GNOME without extension), **When** Adagio starts, **Then** the app still launches successfully and logs a clear warning rather than crashing.

---

### User Story 2 — Tray Popover Opens on Left-Click (Priority: P1)

Clicking the tray icon once opens the 380 px-wide tray popover window, anchored below the icon. The popover matches the visual design from `handoff/tray.html`: adagio logo + status badge, status headline, last-sync metadata, three recent-activity rows, four quick-action buttons, and a footer with version and Quit.

**Why this priority**: The popover is the primary surface for ambient status and quick actions. It must work reliably on both X11 and Wayland.

**Independent Test**: Click the tray icon; confirm popover appears positioned flush with the top bar, the headline matches the current daemon sync state, and clicking outside dismisses it.

**Acceptance Scenarios**:

1. **Given** the popover is hidden, **When** the user left-clicks the tray icon, **Then** the popover slides into view anchored to the icon position with correct top-bar offset.
2. **Given** the popover is visible, **When** the user clicks outside it or clicks the icon again, **Then** the popover is hidden.
3. **Given** the daemon is connected and syncing, **When** the popover opens, **Then** the headline reads "Syncing N files" with the correct count, and the status dot is amber.
4. **Given** the daemon is idle (all files in sync), **When** the popover opens, **Then** the headline reads "Up to date" and the status dot is green.
5. **Given** Adagio is running under Wayland (e.g., GNOME 45+), **When** the tray icon is clicked, **Then** the popover opens reliably without requiring X11 fallback.

---

### User Story 3 — Quick Actions Work from Tray (Priority: P2)

Each of the four quick-action rows in the popover performs its action and closes the popover (or performs the action in-place for toggle operations):

- **Open Adagio folder** → opens the local sync root in the system file manager
- **Open in browser** → opens the Nextcloud server URL in the default browser
- **Pause / Resume syncing** → toggles pause state via the daemon; label updates immediately
- **Preferences…** → raises the main Adagio window

**Why this priority**: These are the day-to-day interactions users expect from a system-tray app. Pause/resume is especially critical to avoid unintentional uploads on metered connections.

**Independent Test**: With the popover open, click each action row and verify the expected outcome without the app needing a restart.

**Acceptance Scenarios**:

1. **Given** a configured sync pair, **When** "Open Adagio folder" is clicked, **Then** the file manager opens at the configured local root.
2. **Given** a configured account, **When** "Open in browser" is clicked, **Then** the default browser opens the server URL.
3. **Given** sync is running, **When** "Pause syncing" is clicked, **Then** the daemon pauses, the action label updates to "Resume syncing", and the status badge updates.
4. **Given** the main window is hidden, **When** "Preferences…" is clicked, **Then** the main window becomes visible and is brought to the foreground.
5. **Given** "Quit Adagio" is clicked in the footer, **Then** both the tray window and the main window close cleanly and the daemon is instructed to stop.

---

### User Story 4 — Consistent Visual Design (Priority: P2)

The tray popover renders faithfully to the design in `handoff/tray.html` across supported Linux desktops, including both light and dark system themes. Typography, spacing, icon glyphs, and status colors match the design spec.

**Why this priority**: Visual consistency builds trust. A sloppy-looking popover undermines the quality signal the rest of the app sets.

**Independent Test**: Open `handoff/tray.html` in a browser side-by-side with the running popover; confirm the layouts are visually equivalent (no missing icons, no layout overflow, correct color palette).

**Acceptance Scenarios**:

1. **Given** the system is in light mode, **When** the popover is open, **Then** the popover uses the `sienna` theme palette (cream background, ink text, clay accent).
2. **Given** the system is in dark mode, **When** the popover is open, **Then** the popover switches to the `ink` theme palette automatically.
3. **Given** no recent activity exists, **When** the popover is open, **Then** the Recent section shows "No recent activity." rather than being empty or broken.
4. **Given** a file name is very long, **When** it appears in the Recent section, **Then** it is truncated with ellipsis and does not overflow the 380 px width.

---

### User Story 5 — Documentation and Daemon Persistence (Priority: P3)

*(Scope note: desktop-app GUI autostart is deferred to v2 per `research.md` Finding 7. US5 in v1 covers documentation clarity and the existing daemon auto-start behavior.)*

Users enabling "Start at login" in Preferences have the Adagio **daemon** configured to start automatically via an XDG autostart entry. When the user manually opens the Adagio desktop app after login, the tray icon appears and the daemon is already running. Quickstart documentation clearly explains this behavior and the GNOME AppIndicator requirement.

**Why this priority**: Accurate documentation prevents user confusion. The daemon auto-start is a pre-existing feature; this story ensures it is clearly communicated in context of the tray.

**Independent Test**: Follow the quickstart.md guide on Ubuntu 22.04; complete setup without hitting an undocumented blocker.

**Acceptance Scenarios**:

1. **Given** the quickstart.md documentation, **When** a new user on Ubuntu 22.04 follows it, **Then** they can install the AppIndicator extension, install `libayatana-appindicator3-1`, and see the tray icon without additional research.
2. **Given** "Start at login" is enabled in Preferences, **When** the user reboots, **Then** the daemon starts automatically; opening the Adagio app manually shows a tray icon that immediately transitions to "IN SYNC" (daemon was already running).

---

### Edge Cases

- What happens when the tray host is unavailable (pure Wayland compositor without xdg-foreign protocol, or no StatusNotifierItem support)? App must start without crashing; show a warning in logs.
- What if the "tray" webview window fails to position correctly because the icon position cannot be determined (some Wayland compositors do not expose icon geometry)? Fall back to a fixed position (top-right corner of the primary monitor).
- What if the user has multiple monitors with different DPI settings? The popover must appear on the correct monitor and scale appropriately.
- What if the daemon is not running when the popover is opened? The popover should show a "Connecting…" or "Unreachable" state rather than blank/error UI.
- What if the user's desktop does not support `iconAsTemplate` (Linux does not)? The icon must still be visible — use a static PNG fallback.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST register a tray icon via the platform's native indicator protocol (StatusNotifierItem/AppIndicator on Linux) when the desktop environment supports it.
- **FR-002**: The tray icon MUST use a 24 px monochrome PNG asset (`icons/png-mono/adagio-icon-mono-24.png`). The OS scales from this source for 16/22 px display sizes; no additional size variants are needed in the Tauri configuration.
- **FR-003**: The tray icon MUST NOT crash or prevent app startup when no tray host is available; it MUST log a warning and continue without a tray icon.
- **FR-004**: Left-clicking the tray icon MUST toggle the tray popover window (show if hidden, hide if visible).
- **FR-005**: The tray popover window MUST be positioned directly below the tray icon on the top bar, accounting for Linux panel height and multi-monitor setups.
- **FR-006**: The tray popover MUST auto-dismiss when it loses focus (user clicks elsewhere on the desktop).
- **FR-007**: The tray popover MUST display the current sync status headline and status dot color matching the daemon state (idle → green "IN SYNC", syncing → amber "SYNCING N FILES", paused → muted "PAUSED", error/unreachable → red).
- **FR-008**: The tray popover MUST display the last-sync timestamp and total file count/size from the daemon.
- **FR-009**: The tray popover MUST display up to 3 recent activity entries from the daemon activity log.
- **FR-010**: The "Open Adagio folder" action MUST open the configured local sync root in the system default file manager.
- **FR-011**: The "Open in browser" action MUST open the configured Nextcloud server URL in the system default browser.
- **FR-012**: The "Pause/Resume syncing" action MUST invoke the daemon's pause/resume command and update the UI label immediately.
- **FR-013**: The "Preferences…" action MUST bring the main Adagio window to the foreground (show and focus).
- **FR-014**: The "Quit Adagio" footer action MUST terminate the application cleanly (close all windows, stop daemon).
- **FR-015**: *(Deferred — v2)* The "Start at login" preference SHOULD eventually create/remove an XDG autostart entry for the Adagio desktop app (`~/.config/autostart/ai.neuralagent.adagio.desktop`) on Linux. This is distinct from the pre-existing `adagio-daemon.desktop` managed by the daemon. In v1, the daemon's "Start at login" feature (already implemented) is the only autostart mechanism; the GUI launches manually. See `research.md` Finding 7 for rationale.
- **FR-016**: The tray popover MUST respect the user's active color palette. The palette is user-selected (not auto-detected from the OS dark-mode setting in v1); the `ink` palette provides a dark-mode appearance. Automatic system-theme detection is deferred to v2.
- **FR-017**: The tray window MUST be declared as a separate labeled webview window (`"tray"`) in the Tauri configuration, distinct from the main window.
- **FR-018**: The popover MUST work correctly under both X11 and Wayland session types; if exact position cannot be determined on Wayland, fall back to a sensible default position.

### Key Entities

- **TrayWindow**: The secondary Tauri webview window (label `"tray"`) — frameless, always-visible on top, auto-hidden on blur. Contains the `TrayPopover` React component.
- **TrayIcon**: The OS-level notification-area icon registered at app startup. On Linux: StatusNotifierItem / AppIndicator. Holds a reference to the `TrayWindow` for click handling.
- **SyncStatusDto**: Daemon data contract — `status`, `active_file_count`, `total_bytes`, `last_sync_at`. Already defined; tray consumes it.
- **ActivityEntryDto**: Daemon data contract — `id`, `kind`, `target`, `at`. Already defined; tray shows the 3 most recent.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The tray icon appears in the top bar on Ubuntu 22.04 LTS (GNOME + AppIndicator extension), KDE Plasma 5/6, and XFCE within 3 seconds of Adagio launch.
- **SC-002**: The tray popover opens and closes within 100 ms of a click event (perceived as instant; aligns with constitution Principle V).
- **SC-003**: The popover content (status headline, recent rows) reflects the actual daemon state within 5 seconds of the daemon state changing.
- **SC-004**: Zero crashes or error dialogs attributable to tray initialization on all three supported desktop environments.
- **SC-005**: The tray popover renders pixel-equivalent to `handoff/tray.html` at 1× DPI (verified by screenshot comparison) on at least two Linux DEs.
- **SC-006**: *(Deferred — v2)* After enabling desktop-app "Start at login" (requires FR-015 implementation), the tray icon should be visible within 15 seconds of desktop session start without user action. Not validated in v1.

## Assumptions

- Target Linux distributions: Ubuntu 22.04 LTS, Ubuntu 24.04 LTS, Fedora 40, and Arch Linux (rolling). Other distributions are best-effort.
- On GNOME, users are expected to install the `gnome-shell-extension-appindicator` (Ubuntu) or `gnome-shell-extension-ayatana-compatibility` extension; the app MUST NOT require this for crash-free startup, but the tray icon will only be visible if it is installed.
- `libappindicator3` or `libayatana-appindicator3` must be installed on the target system; the app's `.deb` / `.rpm` packages should declare these as dependencies.
- The existing `TrayPopover.tsx` React component and its tests are correct and complete; this feature focuses on the Tauri/Rust integration layer, window configuration, icon assets, and Linux-specific positioning/focus behavior.
- Keyboard shortcut labels in the tray UI display `⌘` symbols; on Linux these should display as `Ctrl` to match platform conventions.
- Wayland support targets compositors that implement the `xdg_foreign` and `layer_shell` protocols (GNOME ≥ 45 with Mutter, KDE Plasma 6). Older Wayland compositors without these protocols will fall back to X11 XWayland if available.
- The daemon is already implemented and exposes `get_status`, `get_activity_log`, `pause_sync`, `resume_sync`, `list_pairs`, and `list_accounts` IPC commands.
- `iconAsTemplate: true` in `tauri.conf.json` is a macOS-only flag and has no effect on Linux; this must not prevent the icon from appearing.
