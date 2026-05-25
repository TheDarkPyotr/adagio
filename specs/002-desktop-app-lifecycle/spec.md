# Feature Specification: Desktop App Lifecycle

**Feature Branch**: `002-desktop-app-lifecycle`

**Created**: 2026-05-24

**Status**: Draft

**Input**: User description: "desktop app — persistent config, running sync engine, and file browser"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Configuration Survives Restart (Priority: P1)

A user configures their Nextcloud account and chooses a local folder to sync. They close the app and reopen it later. All their settings — account details, sync folder, selective paths, scan interval — are exactly as they left them. The app is ready to sync without any reconfiguration.

**Why this priority**: Without persistence, the app is unusable in practice. Every launch requires starting from scratch. This is the minimum bar for the app to be a real tool rather than a demo.

**Independent Test**: Configure an account and one sync pair, quit the app, relaunch it, and verify the account and pair are immediately available with all original settings intact.

**Acceptance Scenarios**:

1. **Given** I have configured an account and a sync pair, **When** I close and reopen the app, **Then** my account and sync folder settings are restored automatically with no prompts.
2. **Given** I have configured multiple sync pairs with different selective paths, **When** I restart the app, **Then** all pairs are present with their selective paths and exclude patterns unchanged.
3. **Given** the app is launched for the first time on a clean install, **When** no saved configuration exists, **Then** the app opens on the onboarding screen with no pre-filled data.
4. **Given** the app crashes while running, **When** I reopen it, **Then** all configuration that existed before the crash is intact.

---

### User Story 2 - Sync Starts Automatically on Launch (Priority: P2)

A user opens the app in the morning. Without clicking anything, their sync folder begins checking for changes within a few seconds. New files added to the remote server overnight appear in the local folder automatically. Changes made locally while the app was closed are uploaded. The user sees the sync activity happening in the status bar.

**Why this priority**: Depends on US1 (needs saved config to know what to sync). Automatic startup sync is the core value proposition of a sync client — the user should not have to manually trigger it.

**Independent Test**: With a saved sync pair and at least one pending remote change, relaunch the app and observe that the change is downloaded within 10 seconds without any manual action.

**Acceptance Scenarios**:

1. **Given** a sync pair is configured and the remote server has new files, **When** I open the app, **Then** sync begins within 5 seconds and the new files are downloaded to the local folder.
2. **Given** a sync pair is configured and the remote server is unreachable at launch, **When** the app opens, **Then** it shows an offline indicator and retries silently in the background; no data is lost.
3. **Given** the app is running and I add a new file to the local sync folder, **When** the next scan interval elapses, **Then** the file is uploaded to the remote server.
4. **Given** the main window is closed, **When** sync is active, **Then** sync continues running in the background and the system tray icon remains present.
5. **Given** multiple sync pairs are configured, **When** the app launches, **Then** each pair starts its own independent sync cycle.

---

### User Story 3 - File Browser Shows Sync Status (Priority: P3)

A user wants to know the state of their sync folder. They open the file browser panel and see a list of all files and folders in their sync directory. Each item has a small status badge: a green checkmark for synced files, a clock for pending, an arrow for in-progress transfers, a warning icon for conflicts or errors. They can immediately see which files need attention without opening a file manager.

**Why this priority**: Depends on US2 (sync must run to have status to show). Improves visibility but the app syncs correctly without it — lower urgency than the engine running.

**Independent Test**: With a sync pair and at least one completed sync cycle, open the file browser and verify that each file in the local folder is listed with an accurate status badge reflecting its journal state.

**Acceptance Scenarios**:

1. **Given** a sync cycle has completed, **When** I view the file browser, **Then** every file in my sync folder is listed with a status of "synced", "pending", "error", or "conflict".
2. **Given** a file is actively being uploaded or downloaded, **When** I view the file browser, **Then** that file shows a transfer-in-progress indicator with directional arrow.
3. **Given** a file failed to sync due to a server error or conflict, **When** I view the file browser, **Then** that file shows an error badge and I can read a brief reason for the failure.
4. **Given** a sync operation completes while I am viewing the file browser, **When** the operation finishes, **Then** the affected file's status badge updates automatically without a manual page refresh.
5. **Given** I want to sync immediately rather than wait for the next interval, **When** I click the manual sync button, **Then** a sync cycle starts within 1 second and the status indicators update as it progresses.

---

### Edge Cases

- What happens when a saved sync pair references a local folder that has been deleted or moved?
- How does the app behave when account credentials have been removed from the OS keychain since last launch?
- How does the file browser handle a sync folder containing thousands of files without becoming unresponsive?
- What happens if two sync pairs share overlapping local folder paths?
- How does the app recover if the on-disk config file is corrupted or written partially due to a crash mid-save?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST save account configuration (server address, username, credential reference) to persistent local storage whenever an account is added, edited, or removed.
- **FR-002**: The app MUST save sync pair configuration (local folder path, remote folder path, scan interval, selective paths, exclude patterns) to persistent local storage whenever a pair is created, modified, or deleted.
- **FR-003**: The app MUST restore all saved accounts and sync pairs automatically on every launch, before any sync activity begins.
- **FR-004**: On startup, the app MUST start a sync cycle for every pair that has the "scan on startup" setting enabled, within 5 seconds of the configuration being loaded.
- **FR-005**: The sync engine MUST execute complete sync cycles (detect local changes, detect remote changes, reconcile differences, propagate operations) on the configured schedule for each pair.
- **FR-006**: The app MUST continue running sync cycles in the background after the main window is closed, remaining accessible via the system tray icon.
- **FR-007**: The file browser MUST list all files and folders within the selected sync pair's local folder.
- **FR-008**: Each item in the file browser MUST display its current sync status drawn from the app's sync history: synced, pending upload, pending download, uploading, downloading, conflict, or error.
- **FR-009**: The file browser MUST update status indicators automatically when a sync operation affecting a displayed file completes, without requiring a manual refresh.
- **FR-010**: Users MUST be able to trigger an immediate sync cycle from the dashboard or file browser.
- **FR-011**: The app MUST surface a clear error message when a saved sync pair's local folder no longer exists, and allow the user to either choose a new folder or remove the pair.
- **FR-012**: The app MUST detect when a saved account's credentials are absent from the keychain and prompt re-authentication before attempting to sync that pair.

### Key Entities

- **Saved Configuration**: On-disk representation of all accounts and sync pairs; the authoritative source of truth for app state across restarts. Contains everything needed to resume syncing except credentials (which live in the keychain).
- **Sync Pair Status**: Per-pair runtime state (idle, syncing, paused, error) used to drive dashboard indicators and tray icon state.
- **File Status Entry**: Per-file sync state (path, status, size, last modified, error message if any) derived from the app's sync history; drives the file browser display.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of configured accounts and sync pairs are available and operational within 3 seconds of app launch on all supported platforms.
- **SC-002**: For pairs with "scan on startup" enabled, the first sync cycle begins within 5 seconds of launch; no manual action required.
- **SC-003**: File browser displays current sync status for all files; status updates within 2 seconds of any sync operation completing.
- **SC-004**: Zero configuration steps required after restarting the app — users resume their prior session without reconfiguring accounts or folders.
- **SC-005**: Sync continues uninterrupted after the main window is closed; no file is dropped or skipped due to the window being hidden.
- **SC-006**: The app recovers gracefully from a corrupt configuration file: it starts in a clean state, informs the user, and does not crash.

## Assumptions

- The sync engine (core logic, reconciler, propagator) and Nextcloud client are already fully implemented and tested as part of feature 001-nextcloud-file-sync.
- Account credentials (passwords, tokens) are never written to the on-disk configuration file; only a credential reference is stored, and the actual secret remains in the OS keychain exclusively.
- The system tray icon already exists in the desktop shell; this feature makes it functional for background sync but does not redesign its appearance.
- The app targets macOS, Linux, and Windows — the same three platforms covered by the existing CI pipeline.
- "Persistent local storage" means a file in the user's app data directory (platform-appropriate location); the specific format is a planning decision.
- The file browser shows the contents of one sync pair at a time; switching between pairs is via a pair selector, not a tabbed interface.
- Files are listed flat (top-level only) by default; folders can be expanded to show their contents (tree view), but deep recursive listing on open is not required for initial scope.
