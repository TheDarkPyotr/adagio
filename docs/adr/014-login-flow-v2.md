# ADR 014: Use Nextcloud Login Flow v2 for Onboarding Wizard

**Date**: 2026-06-01
**Status**: Accepted
**Feature**: `014-onboarding-account-setup`

## Context

The onboarding wizard (step 3 — Authorize) needs to display a one-time code and QR code
to the user while waiting for them to complete authentication in their browser.

The existing `connect_account_oauth2` command uses OAuth2 PKCE with a loopback callback
listener. In this flow the client generates PKCE parameters, opens the browser to the
authorization URL, and waits for the redirect callback. This works well but provides no
short, human-readable code to display — the authorization URL is 100+ characters and
unsuitable for typing or compact QR display.

## Decision

Use **Nextcloud Login Flow v2** (`POST /index.php/login/v2`) exclusively for the onboarding
wizard. This flow is retained alongside the existing PKCE implementation, which remains
available for non-wizard use cases (CLI, headless, re-auth).

The Login Flow v2 protocol:
1. Client POSTs to `/index.php/login/v2` and immediately receives:
   - `login` — a short URL the browser must visit (encodes a per-session token)
   - `poll.endpoint` + `poll.token` — used to poll for completed credentials
2. Client opens `login` URL in the default browser; user authenticates on Nextcloud.
3. Client polls `poll.endpoint` with `poll.token` (POST, form body) every 2 s.
4. On success, Nextcloud returns `{ server, loginName, appPassword }`.

The `login` URL path contains a short alphanumeric session token. We extract the last
8 characters to display as "XXXX · XXXX" (formatted). The full `login` URL is encoded
as a QR code (SVG, generated server-side with the `qrcode` crate).

## Rationale

| Criterion | PKCE loopback | Login Flow v2 |
|-----------|--------------|---------------|
| Displayable code | ❌ (no short code) | ✅ (last 8 chars of token) |
| QR code (compact) | ❌ (long auth URL) | ✅ (short login URL) |
| Works without OAuth2 app config | ❌ (requires OAuth2 app registered) | ✅ (available on all NC ≥ 16) |
| Security model | PKCE + CSRF state | Server-issued token, no secret on client |
| Standard | RFC 7636 | Nextcloud proprietary (stable since NC 16) |
| Cross-device support | ❌ | ✅ (QR scannable from phone) |

Login Flow v2 wins on all UI-relevant criteria and is universally available on supported
Nextcloud versions.

## Consequences

- A new `login_flow` module is added to `crates/adagio-nextcloud`.
- The existing `connect_account_oauth2` / PKCE / `oauth2_callback` code is **unchanged**;
  the wizard uses a separate code path.
- Credentials returned by Login Flow v2 are stored identically to PKCE credentials
  (app-password format stored in OS keychain under the account's keychain key).
- Login Flow v2 sessions expire after 20 minutes on Nextcloud's side. The UI enforces a
  5-minute countdown (stricter than the server's limit) to prompt the user to retry early.
- No migration is needed — both flows coexist.

## Alternatives Considered

**Keep PKCE loopback (current)**: No short code to display; QR would encode a 100+ char
URL, reducing scannability. Rejected.

**OAuth2 Device Authorization Grant (RFC 8628)**: Provides a standard device code flow.
Nextcloud does not implement RFC 8628. Rejected.

**App-password manual entry**: User generates an app-password in NC settings and pastes it.
Poor UX for an onboarding flow; also requires the user to navigate NC settings. Rejected.
