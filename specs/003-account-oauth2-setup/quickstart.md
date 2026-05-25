# Dev Quickstart: Account Setup UI — Nextcloud OAuth2 Login Flow

**Feature**: 003-account-oauth2-setup
**Date**: 2026-05-25

---

## Prerequisites

1. A running Nextcloud instance (NC16+) reachable from your dev machine.
   - Docker quickstart: `docker run -d -p 8080:80 nextcloud:latest`
   - Access at `http://localhost:8080` (admin / admin on first launch)

2. Register the Adagio OAuth2 client in Nextcloud:
   - Log in as admin → **Settings → Security → OAuth 2.0 clients → Add client**
   - Name: `Adagio Desktop (dev)`
   - Redirect URI: `http://127.0.0.1` (Nextcloud accepts any port on loopback)
   - Note the generated `Client ID` and `Client secret`

3. Set the bundled constants in `crates/adagio-nextcloud/src/auth.rs` for local testing:
   ```rust
   pub const OAUTH2_CLIENT_ID: &str = "<your-client-id>";
   pub const OAUTH2_CLIENT_SECRET: &str = "<your-client-secret>";
   ```

4. Build the project:
   ```sh
   cargo build -p adagio-desktop
   ```

---

## Running the OAuth2 Flow

```sh
cargo tauri dev
```

1. The app opens. If no accounts are configured, the Onboarding view appears with the OAuth2 path selected by default.
2. Enter `http://localhost:8080` in the server URL field.
3. Click **"Connect with browser"** — the system browser opens the Nextcloud login page.
4. Log in with your Nextcloud credentials and click **Allow**.
5. The browser redirects to `http://127.0.0.1:<port>/callback?code=...`.
6. The app detects the callback, closes the listener, and exchanges the code for tokens.
7. The app fetches the user display name from `/ocs/v2.php/cloud/user`.
8. The account appears in the account list with the display name and server URL.

---

## Verifying Credential Storage

After a successful connection, verify no token is on disk:

```sh
# config.json must NOT contain any token or password
cat ~/.config/adagio/config.json | grep -i "token\|password\|secret"
# Expected: no output

# On Linux (libsecret): confirm token is in the keychain
secret-tool lookup service adagio account <account-uuid>
# Expected: the JSON token blob
```

---

## Edge Case Testing

| Scenario | How to trigger | Expected result |
|----------|---------------|-----------------|
| Unreachable server | Enter `https://192.0.2.1` (unroutable) | Error within 5 s: "Server unreachable" |
| Not a Nextcloud server | Enter `https://example.com` | Error: "Not a Nextcloud server" |
| User closes browser without authorising | Open browser, close tab, wait | After 5 min: "Authorization timed out" |
| Same account added twice | Complete OAuth2 flow twice for same server+user | Error: "Account already connected…" |
| Keychain locked | Lock keychain before clicking Connect | Error: "Credential storage failed" |

---

## Running Tests

```sh
# Unit tests (no Nextcloud required)
cargo test -p adagio-nextcloud -- auth::

# Integration tests (requires a running Nextcloud + env vars)
ADAGIO_TEST_NC_URL=http://localhost:8080 \
ADAGIO_TEST_NC_USER=admin \
ADAGIO_TEST_NC_PASS=admin \
cargo test -p adagio-desktop -- oauth2_flow
```
