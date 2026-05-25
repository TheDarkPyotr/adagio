# IPC Contracts: Account Setup — Nextcloud OAuth2 Login Flow

**Feature**: 003-account-oauth2-setup
**Date**: 2026-05-25

These are the Tauri IPC command contracts exposed from the Rust backend to the Svelte
frontend. All commands are invoked via `invoke(commandName, args)`.

---

## `connect_account_oauth2`

Runs the complete OAuth2 flow end-to-end:
1. Validates the server URL (`/status.php` probe).
2. Spins up a loopback HTTP listener on an ephemeral port.
3. Opens the Nextcloud authorization URL in the system browser.
4. Waits (up to 5 minutes) for the callback to arrive.
5. Exchanges the authorization code for tokens.
6. Fetches the user's display name and username from OCS.
7. Checks for duplicate accounts.
8. Stores the token in the OS keychain.
9. Persists `Account` metadata to `config.json`.
10. Returns the created `AccountDto`.

**Replaces** the existing `initiate_oauth2` command (which was a stub).

**Input**:
```typescript
interface ConnectAccountOAuth2Request {
  server_url: string;   // e.g. "https://cloud.example.com"
}
```

**Output** (success):
```typescript
interface AccountDto {
  id: string;
  display_name: string;
  server_url: string;
  username: string;
}
```

**Errors** (string messages returned via `Err(String)`):

| Error condition | Message prefix |
|----------------|----------------|
| Server URL unreachable | `"Server unreachable: ..."` |
| Not a Nextcloud instance | `"Not a Nextcloud server: ..."` |
| Browser auth cancelled by user | `"Authorization cancelled"` |
| Callback timed out (5 min) | `"Authorization timed out"` |
| State mismatch (CSRF) | `"Invalid authorization response"` |
| Token exchange failed | `"Token exchange failed: ..."` |
| Duplicate account | `"Account already connected for this server and username"` |
| Keychain error | `"Credential storage failed: ..."` |

**Notes**:
- The command does NOT accept a `client_id` parameter; the bundled constants are used.
- The command blocks until the full flow completes or an error occurs; the frontend
  must show a "waiting for browser" state for the duration.
- `redirect_uri` is constructed internally using the ephemeral port.

---

## `add_account` *(existing, unchanged)*

Adds an account using a pre-obtained secret (app-password or previously-issued token).
Used by the app-password auth path. No changes for this feature.

**Input**:
```typescript
interface AddAccountRequest {
  server_url: string;
  username: string;
  display_name: string;
  secret: string;   // stored in keychain; never returned
}
```

**Output**: `AccountDto`

---

## `remove_account` *(existing, unchanged)*

Removes an account, its sync pairs, and its keychain credentials.

**Input**: `{ account_id: string }`

**Output**: `void`

---

## `list_accounts` *(existing, unchanged)*

Returns all saved accounts (no credentials).

**Input**: *(none)*

**Output**: `AccountDto[]`

---

## Frontend State Machine

The Onboarding and Settings flows share the same OAuth2 state machine:

```
idle
  │  user clicks "Connect with browser"
  ▼
validating_server          ← show spinner: "Checking server…"
  │  /status.php OK
  ▼
waiting_for_browser        ← show: "Complete login in your browser…" + Cancel button
  │  callback received
  ▼
exchanging_tokens          ← show spinner: "Completing sign-in…"
  │  success
  ▼
connected                  ← show: account display_name + server_url

(any error → idle with error message displayed)
```

The frontend invokes `connect_account_oauth2` once; all state transitions above happen
inside the single command on the Rust side. The frontend transitions UI solely based on
the pending `invoke` promise:
- While awaiting → `waiting_for_browser` state
- On resolve → `connected` state
- On reject → `idle` + display `error`
