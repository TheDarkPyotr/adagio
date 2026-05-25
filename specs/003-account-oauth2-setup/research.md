# Research: Account Setup UI — Nextcloud OAuth2 Login Flow

**Feature**: 003-account-oauth2-setup
**Date**: 2026-05-25

---

## Decision 1: OAuth2 Flow Variant

**Decision**: Standard authorization code flow with PKCE parameters included.

**Rationale**: Nextcloud's `oauth2` app (all supported versions, NC16+) does not currently
validate PKCE code challenges — it ignores `code_challenge` and `code_challenge_method`.
However, the PKCE parameters cause no errors and are included for forward-compatibility: once
Nextcloud adds PKCE server-side support the client will already comply. A static
`client_secret` is still required for the token exchange (confidential client model).

The authorization and token endpoints are:
- Auth:  `{server_url}/index.php/apps/oauth2/authorize`
- Token: `{server_url}/index.php/apps/oauth2/api/v1/token`

Token response shape:
```json
{"access_token": "...", "token_type": "Bearer", "expires_in": 3600, "refresh_token": "..."}
```

**Alternatives considered**:
- Plain auth code flow without PKCE — simpler but discards future compatibility
- App-password flow — already supported by the existing `add_account` command; not the
  browser-based flow required by FR-003

---

## Decision 2: Loopback Callback Listener

**Decision**: Raw `tokio::net::TcpListener` bound to `127.0.0.1:0` (ephemeral port) with
manual HTTP parsing. No additional crate required.

**Rationale**: The project already depends on `tokio`; the OAuth2 callback is a single
GET request containing `code` and `state` query parameters. A full HTTP framework (`axum`,
`tiny_http`) would be a new dependency for a one-shot, <10-line task. The listener is spun
up in an async task, parks on `listener.accept()`, reads the first request line, extracts the
query string, and sends a minimal HTML success response before dropping the socket.

Loopback flow:
1. Bind `TcpListener::bind("127.0.0.1:0")` → get assigned `port`.
2. Set `redirect_uri = "http://127.0.0.1:{port}/callback"` in the auth URL.
3. Open browser, then `listener.accept()` (with 5-minute timeout).
4. Extract `?code=...&state=...`, validate `state`, close the socket.
5. Exchange `code` for tokens via `exchange_code()`.

**Alternatives considered**:
- `tiny_http` — synchronous, requires `spawn_blocking`; adds a dep for one-shot use
- Hardcoded port 9876 — port collision risk; violates RFC 8252 best practices

---

## Decision 3: Client Credentials Distribution

**Decision**: Ship a bundled `client_id` + `client_secret` compiled into the binary as
constants. Users do not register an OAuth2 client themselves.

**Rationale**: Every Nextcloud instance requires an OAuth2 client to be registered by an
admin at `/settings/admin/security`. For Adagio's target audience (self-hosters), requiring
them to register a client before onboarding is a prohibitive UX barrier. The bundled
credentials are registered once in the official documentation; the secret is not a
high-value credential (it identifies the *app*, not the *user*). This is the same approach
used by the official Nextcloud desktop client.

For the MVP the constants are:
```rust
pub const OAUTH2_CLIENT_ID: &str = "adagio-desktop";
pub const OAUTH2_CLIENT_SECRET: &str = "adagio-desktop-secret";
// NOTE: production values are set at release time via CI env vars
```

**Alternatives considered**:
- User-entered client credentials — too much friction for non-technical users
- Dynamic registration (RFC 7591) — Nextcloud does not support it

---

## Decision 4: Token Storage Format

**Decision**: Store a JSON blob `{"access_token":"...","refresh_token":"..."}` in the OS
keychain under service `"adagio"`, entry key = `account_id` (UUID string).

**Rationale**: `auth.rs` already implements `TokenPair` + `store_credentials` /
`retrieve_credentials` using this exact shape. Bundling both tokens in one keychain entry
minimises the number of keychain prompts and keeps the retrieval code simple.
`keyring::Entry::new("adagio", account_id)` is the established convention.

**Alternatives considered**:
- Two separate keychain entries per account — doubles keychain operations; no benefit
- Store `expires_at` in the blob — added complexity; refresh-on-401 strategy makes it
  unnecessary (see Decision 5)

---

## Decision 5: Token Refresh Strategy

**Decision**: Refresh on 401. Attempt the API call; if the server returns HTTP 401, call
`refresh_access_token()` and retry once. Do not proactively check `expires_in`.

**Rationale**: Nextcloud access tokens expire in 3600 s (hard-coded server-side). Proactive
refresh requires persisting `expires_at` and a background timer, which adds state management
complexity. The 401-retry loop is simpler and works correctly even when the system clock
drifts or the token is revoked. The retry adds at most one extra round trip per hour.

**Alternatives considered**:
- Proactive refresh (check `expires_at` before each request) — requires storing a
  timestamp, background task, and clock handling; over-engineered for a 1-hour TTL
- No refresh (user re-authenticates on expiry) — unacceptable UX

---

## Decision 6: User Info Endpoint

**Decision**: After a successful token exchange, fetch the authenticated user's info from
`GET /ocs/v2.php/cloud/user?format=json` with `Authorization: Bearer <access_token>` and
`OCS-APIREQUEST: true` headers.

Response shape (relevant fields):
```json
{
  "ocs": {
    "data": {
      "id": "alice",
      "display-name": "Alice Müller",
      "email": "alice@example.com"
    }
  }
}
```

`data.id` becomes `Account.username`; `data.display-name` becomes `Account.display_name`.

**Alternatives considered**:
- `/ocs/v1.php/cloud/user` — same data but v2 returns cleaner HTTP status codes
- Let users type their display name — worse UX; spec assumption §Assumptions item 6 rules it out

---

## Decision 7: Server URL Validation

**Decision**: Probe `{server_url}/status.php` with a GET request (5-second timeout). A
valid Nextcloud server returns HTTP 200 with JSON containing `"installed": true`.

```json
{"installed":true,"maintenance":false,"needsDbUpgrade":false,"version":"28.0.1",...}
```

If the request times out, returns non-200, or the JSON lacks `"installed": true`, surface
FR-007's actionable error to the UI before opening the browser.

**Alternatives considered**:
- HEAD `/index.php/apps/oauth2/authorize` — redirects to login page; harder to parse
- Skip validation, rely on browser error — violates FR-002 and SC-003 (5 s feedback)

---

## Decision 8: Duplicate Account Detection

**Decision**: Before saving a new account, iterate `AccountManager::list()` and reject if
any existing account has the same (normalised) `server_url` **and** `username` combination.
Return a user-visible error: "Account already connected for this server and username."

URL normalisation: trim trailing slashes, lowercase scheme+host.

**Alternatives considered**:
- Detect by `keychain_service_key` collision — less readable; UUID keys don't carry
  semantic meaning
- Allow duplicates, detect at sync time — violates FR-010
