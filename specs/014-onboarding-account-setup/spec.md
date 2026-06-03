# Feature Specification: Onboarding — Account Setup Wizard

**Feature Branch**: `014-onboarding-account-setup`

**Created**: 2026-06-01

**Status**: Draft

**Input**: User description: "Wire the existing 5-step OnboardingWizard UI (Welcome → Server → Authorize → Where to sync → Begin) to real Tauri IPC. Currently the component has hardcoded/mock data throughout: URL validation is fake, the auth code is static, the folder picker doesn't open a dialog, the toggle settings aren't persisted, and the final connected screen shows fake file counts. The spec should cover making all 5 steps functional: real URL validation + Nextcloud capability detection, OAuth2 device flow (show real code + QR, poll for token), folder picker dialog, persisting toggle preferences, and a real initial sync progress readout on the final screen."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - First-Time Server Connection (Priority: P1)

A new user opens Adagio for the first time with no accounts configured. The onboarding wizard is shown automatically. They type in their Nextcloud server address and, within a few seconds, the app confirms the server is reachable and displays its version and capabilities. The user clicks Continue and moves to the authorization step.

**Why this priority**: This is the entry gate to the entire application. Until the server URL is validated and the server is confirmed reachable, no other step can proceed. Every first-time user hits this path.

**Independent Test**: Launch the app with no saved accounts. Enter a valid Nextcloud server URL. Verify that: (a) the server is confirmed reachable with version information shown within 3 seconds; (b) entering an unreachable URL shows a clear error message; (c) the user cannot advance past this step until a valid, reachable server is confirmed.

**Acceptance Scenarios**:

1. **Given** the user is on the Server step, **When** they enter a valid Nextcloud URL and pause, **Then** the app shows the detected server version and available capabilities (e.g. E2EE support, auth method) within 3 seconds.
2. **Given** the user enters an unreachable or malformed URL, **When** they attempt to advance, **Then** a clear inline error is shown and the user cannot proceed.
3. **Given** the server is reachable, **When** the user clicks Continue, **Then** they advance to the Authorize step.

---

### User Story 2 - Browser Authorization (Priority: P1)

After confirming their server, the user is shown a one-time authorization code and a QR code on screen. Their default browser opens automatically to their Nextcloud server's login page. They authenticate in the browser and grant access to Adagio. The wizard detects this automatically and advances to the next step — the user does not need to click anything.

**Why this priority**: Authorization is the single irreplaceable step: without a valid credential, no sync can happen. It must work end-to-end with real data, and the automatic detection of completion is the key UX improvement over asking the user to copy tokens manually.

**Independent Test**: With a valid server configured, reach the Authorize step. Verify: (a) a unique one-time code appears on screen; (b) the QR code encodes the correct authorization URL for the server; (c) a countdown timer decrements in real time; (d) completing authorization in the browser causes the wizard to advance automatically within 5 seconds; (e) if the code expires before the user authorizes, an expiry error is shown with an option to regenerate the code.

**Acceptance Scenarios**:

1. **Given** the user reaches the Authorize step, **When** the step loads, **Then** a unique one-time code and a scannable QR code are displayed and the countdown timer starts.
2. **Given** the browser has opened the Nextcloud authorization page, **When** the user completes authentication and grants access, **Then** the wizard automatically advances without any click from the user, within 5 seconds of authorization being granted.
3. **Given** the authorization code has expired (countdown reaches zero), **When** the user has not yet authorized, **Then** the expired code is visually indicated and the user is offered a "Try again" action to generate a fresh code.
4. **Given** the user closes the browser before authorizing, **When** the countdown has not yet expired, **Then** the wizard remains on the Authorize step with the same code still valid.

---

### User Story 3 - Local Folder Selection and Sync Preferences (Priority: P2)

After authorizing, the user is asked where on their machine the synced files should live. A pre-filled default path is shown. The user can either accept it or click "Browse…" to open their operating system's native folder-picker dialog and select a different location. Below the folder field, four sync preference toggles are shown; the user can adjust them. These settings are saved and take effect immediately.

**Why this priority**: While the folder picker and preferences are not blocking for an MVP proof of concept, without persisted preferences the app starts with silent defaults the user never chose — leading to unexpected behavior (e.g. all files downloaded when on-demand was expected). This step protects the user from surprise.

**Independent Test**: Reach the "Where to sync" step. Click "Browse…" and verify the OS folder-picker dialog opens and the chosen path is reflected in the input. Toggle each preference on and off. Complete onboarding. Restart the app and verify the chosen folder and all four toggle states are exactly as set during onboarding.

**Acceptance Scenarios**:

1. **Given** the user reaches the "Where to sync" step, **When** the step loads, **Then** a default local folder path is pre-filled in the folder input.
2. **Given** the user clicks "Browse…", **When** the dialog opens, **Then** the native OS folder-picker appears and the user can navigate to and select any folder.
3. **Given** the user selects a folder via the picker, **When** the dialog closes, **Then** the chosen path is shown in the folder input.
4. **Given** the user adjusts any of the four sync preference toggles, **When** they complete onboarding, **Then** the exact toggle states chosen are persisted and restored identically after an app restart.
5. **Given** the user selects a folder that already contains files, **When** they proceed, **Then** a notice is shown that existing files in the folder will be treated as local files for the sync.

---

### User Story 4 - Real Initial Sync Progress on Completion Screen (Priority: P2)

After configuration is complete, the user sees the "Begin" screen. It shows the real file count and total storage size for their connected Nextcloud account, and a live progress bar reflecting the actual initial synchronization — not placeholder values. The user can click "Open Adagio" at any point, even before the initial sync finishes.

**Why this priority**: Showing fake numbers at the end of a real setup flow breaks trust. The user needs to see actual data to confirm that the connection succeeded and sync has genuinely started.

**Independent Test**: Complete the full onboarding flow with a real Nextcloud account. On the Begin screen verify: (a) the file count and storage size match the values shown in the Nextcloud web interface; (b) the progress bar advances as files are actually downloaded; (c) clicking "Open Adagio" navigates to the main app view.

**Acceptance Scenarios**:

1. **Given** the user reaches the Begin step, **When** the step loads, **Then** the actual file count and total storage size of the connected Nextcloud account are displayed.
2. **Given** the initial sync is in progress, **When** the user views the Begin step, **Then** the progress bar and transferred-file count update in real time.
3. **Given** the user clicks "Open Adagio", **When** the initial sync is still running, **Then** the wizard closes and the user is taken to the main app; the sync continues in the background.

---

### Edge Cases

- What happens when the user closes or quits the app mid-wizard (before completing authorization)?
- What happens if the Nextcloud server becomes unreachable after URL validation but before authorization completes?
- What happens when the selected local folder is on a drive with insufficient free space for the initial sync?
- What happens if the authorization browser window is closed before the code expires — can the user reopen it?
- What happens when the user enters a server URL that redirects to a different host (e.g. behind a reverse proxy)?
- What happens if the user's Nextcloud account has zero files (empty account)?
- What happens if the app is launched for the second time while the wizard is still showing (e.g. from the dock/taskbar)?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST detect whether the entered server URL points to a reachable Nextcloud instance and display the server version and key capabilities (authentication method, E2EE availability) without the user requesting it explicitly.
- **FR-002**: The system MUST prevent the user from advancing past the Server step until a reachable, compatible Nextcloud server has been confirmed.
- **FR-003**: The system MUST display a unique, human-readable one-time authorization code and a scannable QR code on the Authorize step; both must encode live, server-issued values — not static placeholders.
- **FR-004**: The system MUST open the user's default browser to the Nextcloud authorization page automatically when the Authorize step is displayed.
- **FR-005**: The system MUST display a countdown timer on the Authorize step that accurately reflects the time remaining before the authorization code expires.
- **FR-006**: The system MUST detect when the user has completed browser authorization and automatically advance the wizard without requiring any additional user action.
- **FR-007**: The system MUST show an expiry state and a "Try again" action when the authorization code expires before the user completes browser authorization.
- **FR-008**: When the user clicks "Browse…" on the folder step, the system MUST open the operating system's native folder-picker dialog and populate the folder input with the user's selection.
- **FR-009**: The system MUST persist the four sync preference toggle states (on-demand files, pinned folders, smart bandwidth, external edit detection) set during onboarding across app restarts.
- **FR-010**: The system MUST display the real file count and total storage size for the connected account on the Begin (completion) screen, retrieved from the server after successful authorization.
- **FR-011**: The system MUST show a live-updating progress indicator on the Begin screen that reflects the actual state of the initial synchronization.
- **FR-012**: If the user quits the app mid-onboarding before authorization completes, the system MUST show the onboarding wizard again on next launch (no partial account should be saved).

### Key Entities

- **Onboarding Session**: A short-lived record of the in-progress setup. Holds the pending server URL, issued authorization code, and step position. Discarded entirely on completion or cancellation — never persisted as a saved account unless authorization succeeds.
- **Server Profile**: Validated details of a Nextcloud server — URL, detected version, supported capabilities. Created during the Server step; becomes part of the saved account on completion.
- **Sync Preferences**: The four user-configurable settings that govern how files are handled locally. Set during onboarding; persisted with the account and applied from the first sync onward.
- **Initial Sync Progress**: A read-only snapshot of the background sync state — total files, files transferred so far, bytes transferred. Displayed on the Begin screen; updated in real time.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user with a reachable Nextcloud server can complete the full onboarding wizard (server entry → authorization → folder selection → preferences → completion) in under 3 minutes from first launch to seeing the Begin screen.
- **SC-002**: Server reachability feedback (confirmed or error) appears within 3 seconds of the user finishing URL entry, without requiring the user to press any button.
- **SC-003**: The wizard advances automatically within 5 seconds of the user granting authorization in the browser — zero manual clicks required from the user after browser auth.
- **SC-004**: 100% of sync preference toggle states chosen during onboarding are preserved exactly after an app restart, verifiable by comparing toggle states before and after restart.
- **SC-005**: The file count and storage total displayed on the Begin screen match the values visible in the Nextcloud web interface for the same account, within a 1% margin.
- **SC-006**: First-time users who abandon onboarding mid-flow and relaunch the app are returned to the wizard start — no orphaned or partially-configured accounts appear in the account list.

## Assumptions

- The user has an existing Nextcloud account on a server running Nextcloud 16 or later (OAuth2 support required).
- The onboarding wizard is displayed only when no accounts are configured; users with at least one account add new ones through the Settings screen, not this wizard.
- A default local folder path (e.g. `~/Adagio`) is pre-populated; the user may override it via the folder picker but is not required to.
- On-demand file syncing, pinned-folder pinning, and smart bandwidth throttling are enabled by default; external-edit detection is disabled by default. These defaults can be changed during onboarding.
- The QR code shown on the Authorize step encodes the Nextcloud authorization URL so the user can scan it from a phone if the browser is on a different device.
- The authorization code has a 5-minute validity window; the countdown timer reflects this.
- The initial sync starts automatically in the background as soon as authorization and folder selection are complete; the user does not need to trigger it.
- Only the four toggle preferences visible in the existing UI are in scope for this feature; additional preferences (e.g. selective folder sync) are out of scope and handled elsewhere.
- The folder picker opens the native OS dialog; no custom file-browser UI is needed.
- Re-entry into a partially completed wizard (e.g. after crash) restarts from step 1 — resuming mid-flow is out of scope.
