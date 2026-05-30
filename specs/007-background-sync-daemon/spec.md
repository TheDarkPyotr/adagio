# Feature Specification: Background Sync Daemon

**Feature Branch**: `007-background-sync-daemon`

**Created**: 2026-05-29

**Status**: Draft

**Input**: User description: "Daemon Extraction — extract the sync engine into a standalone background process that runs independently of the GUI, communicates over local IPC, and survives GUI close and GUI crash."

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Sync Continues After the Window Is Closed (Priority: P1)

A user is working with Adagio open, syncing files. They close the main window to
declutter their desktop. Under the current design, sync stops. With this feature,
the sync process keeps running silently in the background — the user's files stay
up to date even though no window is visible. When they reopen the app, everything
looks exactly as if they had left it open.

**Why this priority**: This is the single most disruptive limitation of the
current design. Every other feature (auto-start on login, crash recovery, future
CLI) depends on the sync engine being decoupled from the window. Without this
story, the product cannot be used as a "set and forget" sync client.

**Independent Test**: Close the main window while a sync pair is configured.
Wait 60 seconds. Open the app again. Verify the activity log shows sync events
that occurred while the window was closed, and that remote changes made during
that period have been downloaded locally.

**Acceptance Scenarios**:

1. **Given** the desktop app is open and at least one sync pair is active,
   **When** the user closes the main window (via the window's close button or
   Cmd/Alt+W), **Then** the sync engine continues running without interruption,
   the tray icon remains visible and reflects current sync state.

2. **Given** the main window has been closed and sync is running in the
   background, **When** the user clicks the tray icon or reopens the app,
   **Then** the window reappears and immediately shows the current sync status,
   activity log, and any pending conflicts — with no loading delay beyond 1
   second for the connection to be re-established.

3. **Given** sync is running in the background, **When** a new conflict is
   detected, **Then** the tray icon reflects the conflict (e.g., badge count
   updates), even though the main window is closed.

4. **Given** the user chose "Quit Adagio" from the tray menu, **When** that
   action completes, **Then** both the background process and the tray icon
   stop, and no further syncing occurs.

---

### User Story 2 — Sync Starts Automatically at Login (Priority: P2)

A user installs Adagio, completes onboarding, and restarts their computer. They
expect their Nextcloud files to start syncing in the background without having to
open any window. The sync process should start at login, quietly, and be ready
before the user opens the app.

**Why this priority**: "Set and forget" is the core promise of a desktop sync
client. Auto-start at login is how users expect sync clients (Dropbox, iCloud,
OneDrive) to behave. Without it, the user must manually open the app after every
reboot — a significant friction point.

**Independent Test**: Configure at least one sync pair, then reboot the machine
(or simulate a login). Without opening the app, verify that after a short delay
(≤ 5 seconds) the sync process is running, and that any remote changes made
before the reboot are downloaded and reflected on disk.

**Acceptance Scenarios**:

1. **Given** the user has installed Adagio and configured at least one sync pair,
   **When** the user logs in to their OS session, **Then** the sync background
   process starts automatically within 5 seconds and begins syncing all
   configured pairs, with no window or splash screen appearing.

2. **Given** the auto-start mechanism is registered at login, **When** the user
   opens the desktop app for the first time after login, **Then** the app
   connects to the already-running background process and reflects the current
   state (including any sync activity that occurred before the app was opened).

3. **Given** the user explicitly disables "Start at login" in Settings,
   **When** the user logs in, **Then** the background process does NOT start
   automatically, and the app behaves as it does today (sync only while the
   app window is open).

---

### User Story 3 — App Opens Instantly by Connecting to a Running Process (Priority: P3)

A user clicks the Adagio tray icon or dock icon. If the background sync process is
already running, the window should appear immediately with live data — no
"connecting…" spinner, no re-initialization. The window is just a view into a
process that was already active.

**Why this priority**: First-open latency is a quality-of-life issue that affects
daily users. The architectural change creates the risk of a slow "connect and
load" startup. This story ensures that doesn't happen.

**Independent Test**: Start the background process separately. Then open the app.
Measure time from click to the point where sync status, pair list, and activity
log are visible. Verify it is under 1 second.

**Acceptance Scenarios**:

1. **Given** the background sync process is already running (started at login or
   by a previous app session), **When** the user opens the app window, **Then**
   the window appears with current sync status, pair list, conflict count, and
   the last 20 activity entries all visible within 1 second.

2. **Given** the background sync process is running, **When** a second instance
   of the desktop app is launched (e.g., user double-clicks the dock icon),
   **Then** only one window appears (the existing session is brought to focus),
   and no second background process is started.

3. **Given** the background process is running and the app window is open,
   **When** sync status changes (e.g., a transfer completes or a conflict is
   detected), **Then** the window updates in real time without any user action
   required.

---

### User Story 4 — User Can Start and Stop Background Sync from Settings (Priority: P4)

A user wants to temporarily stop all sync activity — perhaps while on a metered
connection or preparing for a presentation. They open Settings, stop the background
process with one click, and know that nothing will sync until they restart it.

**Why this priority**: Control over the background process is a trust feature.
Users need to be confident they can stop sync at any time. This story also
covers the path to restart after a crash (manual recovery).

**Independent Test**: Open Settings, click "Stop background sync". Verify the
background process is no longer running (no sync activity occurs). Then click
"Start background sync" and verify sync resumes.

**Acceptance Scenarios**:

1. **Given** the background process is running, **When** the user opens Settings
   and clicks "Stop background sync", **Then** all active sync transfers finish
   gracefully (or are paused), the process stops, and the UI shows a clear
   "Background sync is stopped" state with a "Start" button.

2. **Given** background sync is stopped, **When** the user clicks "Start
   background sync", **Then** the background process starts within 3 seconds,
   sync resumes for all configured pairs, and the UI returns to its normal
   state.

3. **Given** background sync is stopped and the user closes the app window,
   **When** the user reopens the app later, **Then** the app correctly shows
   "Background sync is stopped" (the stopped state is remembered, not auto-
   restarted by the act of opening the window).

---

### User Story 5 — The App Recovers Automatically If the Background Process Crashes (Priority: P5)

The background sync process crashes unexpectedly (e.g., due to an OS update,
resource exhaustion, or a bug). The desktop app, which is open and connected,
detects this and attempts to automatically restart the process. The user sees a
brief "Reconnecting…" indicator but does not need to take any action in the
normal case.

**Why this priority**: Crash recovery is a reliability baseline. A background
process that the user can never see is one that must recover without user action.
Failing silently after a crash would make the product feel broken.

**Independent Test**: Kill the background process externally. Observe the app
window. Verify the "Reconnecting…" state appears, the process is restarted
automatically, and normal sync operation resumes within 10 seconds — without
the user clicking anything.

**Acceptance Scenarios**:

1. **Given** the app window is open and connected to the background process,
   **When** the background process terminates unexpectedly, **Then** the app
   detects the disconnection within 3 seconds and displays a "Reconnecting…"
   indicator that does not block the user from viewing current pair/conflict
   data.

2. **Given** the app has detected the background process is down, **When** the
   automatic restart attempt succeeds, **Then** the "Reconnecting…" indicator
   disappears, sync resumes, and the user does not need to take any action.

3. **Given** the automatic restart has failed 3 times in a row, **When** the
   fourth attempt would have occurred, **Then** the app stops retrying, shows a
   clear error message ("Background sync could not be restarted"), and presents
   a "Restart sync" button for the user to try manually.

4. **Given** the background process crashed while a sync transfer was in
   progress, **When** the process is restarted, **Then** in-progress transfers
   resume or restart cleanly — no corrupt partial files are left behind, and
   the journal reflects the correct state.

---

### Edge Cases

- **What happens if the background process is already running when the app tries
  to start a second one?** The second start attempt is rejected silently — only
  one instance of the background process may run per user session at a time. The
  app connects to the existing instance instead.

- **What happens if the background process cannot start at login because
  credentials are not yet available (e.g., the OS keychain is locked)?** The
  process starts but enters a "waiting for credentials" state; sync does not
  begin until the user unlocks their session. The app shows an appropriate
  prompt when opened.

- **What happens if the app is open and the user disconnects from the network
  entirely?** The background process continues running; sync operations pause
  and retry on reconnection. The app reflects the offline state.

- **What if the process takes longer than 5 seconds to start at login on a
  slow machine?** The auto-start mechanism makes one attempt; if the process
  hasn't responded within 10 seconds, it is considered failed and the tray icon
  reflects "not running". The user can start it manually from the app.

- **What happens to in-flight transfers when the user clicks "Stop background
  sync"?** Active transfers are allowed to complete for up to 30 seconds. After
  that, they are paused and will resume from their last checkpoint when sync
  is restarted. The user sees a "Finishing current transfer…" message during
  this window.

- **What if the machine has no tray/menu-bar support (some Linux environments)?**
  The background process runs normally. The app window is the only control
  surface. The process can also be stopped via a menu item inside the app.

- **What happens to existing users who upgrade from the embedded-engine version?**
  On first launch after upgrade, the app migrates automatically: starts the
  background process, transfers ownership of the engine state (journal path,
  config), and opens the window as normal. No user action required.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The sync engine MUST run in a separate background process,
  independent of the desktop GUI process, so that sync continues when the GUI
  window is closed or the GUI process is restarted.

- **FR-002**: The background process MUST start automatically when the user logs
  in to their OS session, on all three supported platforms (Linux, macOS,
  Windows), without any window or notification appearing.

- **FR-003**: The background process MUST expose a local communication channel
  that the desktop app and future command-line tools can connect to, limited to
  connections from the same OS user account.

- **FR-004**: All commands currently available in the desktop app (view pairs,
  trigger sync, resolve conflicts, add/remove accounts, view activity, pause/
  resume) MUST continue to work exactly as they do today, routed through the
  communication channel to the background process.

- **FR-005**: The background process MUST push real-time notifications to
  connected clients (desktop app, future CLI) for: sync status changes, conflict
  detected, conflict resolved, transfer progress updates.

- **FR-006**: Only one instance of the background process per user session MAY
  run at any time. A second start attempt MUST connect to the existing instance
  instead of starting a new one.

- **FR-007**: The background process MUST shut down cleanly when: the user
  selects "Quit Adagio" from the tray, the OS initiates user logout, or the user
  stops it from Settings. In-progress transfers MUST be allowed up to 30 seconds
  to reach a safe checkpoint before the process exits.

- **FR-008**: The desktop app MUST detect when the background process has stopped
  unexpectedly within 3 seconds and MUST attempt to restart it automatically,
  up to 3 consecutive times before presenting a manual recovery option.

- **FR-009**: The background process MUST register itself for OS-level auto-start
  during the existing onboarding / first-launch flow, and MUST deregister if the
  user disables "Start at login" in Settings.

- **FR-010**: The desktop app MUST connect to an already-running background
  process within 1 second of the window being opened, and MUST display current
  sync state without requiring the user to wait for a full re-initialization.

- **FR-011**: The upgrade path from the current embedded-engine design MUST be
  handled automatically on first launch of the new version, with no manual
  migration steps required from the user.

- **FR-012**: The background process MUST NOT hold the SQLite journal open in
  a way that would cause data corruption if the process is killed via SIGKILL or
  the equivalent on other platforms. The journal MUST be in a consistent,
  recoverable state after any unclean exit.

- **FR-013**: The "Start at login" preference and the "running / stopped" state
  of the background process MUST be exposed in the Settings view of the desktop
  app, with clear controls for each.

### Key Entities

- **Background Process**: The long-running background sync service. Attributes:
  running/stopped state, PID, uptime, version, number of connected clients. One
  instance per user session.

- **IPC Channel**: The local communication endpoint through which the desktop app
  and future tools send commands to the background process and receive push
  notifications. Attributes: socket/pipe path (platform-specific), connection
  state (connected/disconnected/reconnecting), latency.

- **Client Connection**: A single connected session from the desktop app or a
  future CLI tool to the background process. Attributes: connected-at timestamp,
  subscribed event types.

- **Auto-Start Registration**: The OS-level entry that causes the background
  process to launch at user login. Attributes: enabled/disabled, platform
  mechanism (systemd unit, LaunchAgent plist, registry/Task Scheduler entry).

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Files synced while the GUI window is closed reach the correct local
  state within the same time window as they would have when the window was open —
  no increase in sync latency attributable to the window being closed.

- **SC-002**: On a reference machine, the background process starts within 5
  seconds of user login with no user interaction. Verified by measuring time from
  login event to first successful sync heartbeat in the activity log.

- **SC-003**: When the user opens the desktop app while the background process
  is already running, the window displays current sync status, pair list, and
  activity within 1 second of appearing. No loading indicator should remain
  visible beyond that point.

- **SC-004**: Unplanned background process termination is detected and the
  process is automatically restarted with no user action, 90% of the time within
  10 seconds of termination. The remaining 10% covers cases where the OS itself
  prevents restart (low memory, permission change).

- **SC-005**: Zero data-loss incidents attributable to the process separation.
  After any unclean exit of the background process, the local file state and the
  sync journal are consistent: no partial files, no orphaned journal entries.

- **SC-006**: 100% of functional capabilities available before this change
  (pair management, conflict resolution, activity log, account management,
  pause/resume) remain accessible and functionally identical after the change,
  as measured by passing the existing automated test suite without modification.

- **SC-007**: Upgrading from the previous version to this version completes in
  under 60 seconds with no user action beyond approving the upgrade, on a
  machine with an existing configuration.

---

## Assumptions

- **Preserved data paths**: The SQLite journal location, config file path, and
  OS keychain entries are unchanged by this feature. The background process reads
  from and writes to the same locations as the current embedded engine.

- **Single user account**: This feature targets a per-user background process.
  Shared-machine or multi-user-session scenarios (e.g., Fast User Switching) are
  not addressed in this iteration.

- **Tray icon persists**: The OS tray/menu-bar icon is managed by the desktop app
  process, not the background process. The background process notifies the app
  via the IPC channel when state changes that should update the tray icon.

- **Platform auto-start mechanisms**: Linux uses the XDG autostart spec
  (`~/.config/autostart/`) or, where available, a systemd user service.
  macOS uses a LaunchAgent plist in `~/Library/LaunchAgents/`. Windows uses the
  HKCU Run registry key. These mechanisms are assumed to be available on all
  supported OS versions.

- **No user-visible process management UI beyond Settings**: Users control the
  background process exclusively through the desktop app's Settings panel and the
  tray icon. There is no separate daemon management window.

- **Existing sync behaviour is preserved**: The change in process architecture
  does not alter sync logic, conflict detection, retry behaviour, or any other
  runtime behaviour of the engine. `adagio-core` is unchanged.

- **Connection security is OS-enforced**: The IPC channel is secured by OS-level
  file permissions (Unix socket ownership mode 0600, or named pipe DACL on
  Windows). No application-layer authentication token is required.

- **Keychain availability at background process start**: On macOS and Windows,
  the OS keychain is assumed to be unlocked by the time the background process
  starts after login. On Linux with a locked keyring (e.g., KDE Wallet not yet
  unlocked), sync is deferred until credentials become available, not silently
  dropped.
