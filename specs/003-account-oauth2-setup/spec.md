# Feature Specification: Account Setup UI — Nextcloud OAuth2 Login Flow

**Feature Branch**: `003-account-oauth2-setup`

**Created**: 2026-05-25

**Status**: Draft

**Input**: User description: "Account setup UI — Nextcloud login/OAuth2 flow for adding a new account"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - First-Time Account Connection (Priority: P1)

A new user opens the app for the first time with no accounts configured. They are guided to
enter their Nextcloud server URL, which opens a browser window where they authenticate with
their Nextcloud credentials. After authorising the app, they are returned to the desktop app
with their account connected and ready to configure sync pairs.

**Why this priority**: Without at least one connected account the app is entirely non-functional.
This is the core onboarding path that every user must complete before doing anything else.

**Independent Test**: Launch the app with no saved accounts. Trigger the "Add Account" flow.
Enter a valid Nextcloud server URL. Complete authentication in the browser. Verify the account
appears in the app with display name and server URL shown correctly, and that credentials are
stored securely (not in any plain-text file).

**Acceptance Scenarios**:

1. **Given** no accounts are configured, **When** the user opens the app, **Then** an "Add Account" prompt is shown rather than an empty dashboard.
2. **Given** the user enters a valid Nextcloud server URL, **When** they submit the form, **Then** a browser window opens at the Nextcloud login/authorisation page.
3. **Given** the user successfully authenticates in the browser, **When** the authorisation is granted, **Then** the browser closes (or redirects) and the desktop app shows the newly added account.
4. **Given** a successful authorisation, **When** the account is saved, **Then** credentials are stored only in the OS keychain — no password or token appears in any file or log.
5. **Given** a successful authorisation, **When** the account is saved, **Then** the account's display name and server URL are visible in the account list.

---

### User Story 2 - Adding a Second Account (Priority: P2)

A user who already has one account connected wants to add a second Nextcloud account (e.g.
a personal and a work server). They open Settings, click "Add Account", and complete the same
OAuth2 flow for the second server. Both accounts then coexist and can each have independent
sync pairs.

**Why this priority**: Multi-account support is a common real-world requirement; the add-account
flow must work when accounts already exist, not only on first launch.

**Independent Test**: With one account already saved, trigger "Add Account" a second time with
a different server URL. After completing auth, verify both accounts are listed and their
credentials are stored independently under separate keychain keys.

**Acceptance Scenarios**:

1. **Given** one account already exists, **When** the user opens the account settings panel, **Then** an "Add Account" button is visible.
2. **Given** the user clicks "Add Account" and enters a different server URL, **When** they complete the browser auth flow, **Then** the new account is added without affecting the existing one.
3. **Given** two accounts are connected, **When** the user views the account list, **Then** both accounts are shown with their respective server URLs and display names.

---

### User Story 3 - Account Removal (Priority: P3)

A user wants to disconnect an account they no longer use. They select the account and choose
"Remove". The app confirms the action, removes the account and all its associated sync pairs
from the configuration, and clears the stored credentials from the OS keychain.

**Why this priority**: Users must be able to clean up accounts, both for privacy and to keep
the interface tidy. This is lower priority than adding accounts but required for a complete
account lifecycle.

**Independent Test**: With one account saved, trigger account removal. Verify the account no
longer appears in the app, its sync pairs are gone from the config file, and its credentials
have been removed from the OS keychain.

**Acceptance Scenarios**:

1. **Given** an account exists, **When** the user selects "Remove Account", **Then** a confirmation prompt is shown before any deletion occurs.
2. **Given** the user confirms removal, **When** the operation completes, **Then** the account no longer appears in the account list.
3. **Given** the user confirms removal, **When** the operation completes, **Then** all sync pairs associated with that account are also removed.
4. **Given** the user confirms removal, **When** the operation completes, **Then** the account's credentials are deleted from the OS keychain.
5. **Given** the user cancels the confirmation prompt, **When** the dialog is dismissed, **Then** the account and all its data remain unchanged.

---

### Edge Cases

- What happens when the user enters a server URL that is unreachable or returns an error?
- What happens when the user closes the browser before completing the OAuth2 authorisation?
- What happens when the authorisation flow times out (browser left open too long)?
- What happens when the Nextcloud server does not support OAuth2 PKCE?
- What happens when the user tries to add the same account (same server + username) twice?
- What happens when the OS keychain is locked or unavailable during credential storage?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Users MUST be able to initiate adding a new Nextcloud account by entering a server URL.
- **FR-002**: The system MUST validate that the entered URL points to a reachable Nextcloud instance before opening the browser auth flow.
- **FR-003**: The system MUST open the user's default browser to the Nextcloud authorisation page to complete authentication.
- **FR-004**: The system MUST receive the authorisation callback automatically without requiring the user to manually copy tokens.
- **FR-005**: Credentials MUST be stored exclusively in the OS keychain; they MUST NOT appear in any configuration file, log, or on-screen text.
- **FR-006**: The system MUST display a confirmation once an account is successfully connected, showing the account display name and server URL.
- **FR-007**: The system MUST show a clear, actionable error message when the server URL is invalid or the server cannot be reached.
- **FR-008**: The system MUST show a clear, actionable error message when the browser authorisation is cancelled or times out.
- **FR-009**: Users MUST be able to add multiple accounts from different Nextcloud servers.
- **FR-010**: The system MUST prevent adding a duplicate account (same server URL and username combination already exists).
- **FR-011**: Users MUST be able to remove an existing account after explicitly confirming the action.
- **FR-012**: Removing an account MUST also remove all sync pairs associated with that account.
- **FR-013**: Removing an account MUST delete the associated credentials from the OS keychain.
- **FR-014**: The account list MUST be persisted across app restarts and restored automatically on launch.

### Key Entities

- **Account**: A connected Nextcloud server. Attributes: display name, server URL, username, a keychain reference key (not the credential itself). One account may have many sync pairs.
- **Credential**: The authentication token stored in the OS keychain. Referenced by the account's keychain key; never stored in any config file.
- **Authorisation Session**: A short-lived in-progress login attempt. Holds the pending server URL and PKCE state needed to verify the callback. Discarded after success or timeout.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user with a working Nextcloud server can complete the full account connection flow in under 2 minutes from clicking "Add Account" to seeing the account listed.
- **SC-002**: 100% of successful authorisations result in credentials stored only in the OS keychain — zero tokens or passwords in config files or logs, verifiable by file inspection.
- **SC-003**: Error messages for unreachable servers or cancelled auth flows are shown within 5 seconds of the failure occurring.
- **SC-004**: The account list is fully restored after an app restart with no additional user action, verifiable by restarting the app and confirming the account list matches the pre-restart state.
- **SC-005**: Removing an account completes (account gone, pairs gone, keychain entry deleted) within 2 seconds of the user confirming.

## Assumptions

- The Nextcloud server supports OAuth2 with PKCE (available since Nextcloud 16); servers older than NC16 are out of scope.
- The desktop app runs on Linux, macOS, or Windows — all three platforms have an OS keychain available.
- The user has a default browser configured on their system.
- A local loopback HTTP listener (`http://localhost:<ephemeral-port>`) is acceptable as the OAuth2 redirect URI; no custom URI scheme registration is needed.
- The app shows a "waiting for browser" screen while the OAuth2 callback is pending.
- Account display names are retrieved from the Nextcloud server after successful auth (user profile endpoint); users do not type them manually.
- Sync pair setup is a separate step after account connection and is out of scope for this feature.
