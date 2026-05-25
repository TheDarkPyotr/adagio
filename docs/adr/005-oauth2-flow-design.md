# ADR-005: OAuth2 Account Connection Flow Design

**Status**: Accepted
**Date**: 2026-05-25
**Feature**: 003-account-oauth2-setup

---

## Context

Adagio needs to connect to Nextcloud servers on behalf of users. The spec requires a
browser-based OAuth2 flow (FR-003/FR-004) so users authenticate with their existing
Nextcloud credentials without exposing a password to the app. Three design decisions
required explicit trade-off analysis.

---

## Decision 1: Auth Code Flow with PKCE Parameters (Forward-Compatible)

**Chosen**: Standard authorization code flow. PKCE parameters (`code_challenge`,
`code_challenge_method=S256`, `code_verifier`) are included in every request.

**Rationale**: Nextcloud's `oauth2` app (all supported versions through NC28) does not
currently validate PKCE code challenges — it ignores the parameters. However, including
them causes no errors and ensures the client will comply automatically once Nextcloud
adds server-side PKCE support. A static `client_secret` is still required for the token
exchange because Nextcloud uses the confidential-client model.

**Rejected alternatives**:
- Plain auth code flow without PKCE: simpler, but discards forward-compatibility.
- OAuth 2.0 Device Authorization Grant (RFC 8628): not supported by Nextcloud.

---

## Decision 2: Loopback Callback Listener via Raw `tokio::net::TcpListener`

**Chosen**: Bind `TcpListener` on `127.0.0.1:0` (ephemeral port), parse the single
incoming HTTP GET request manually, extract `?code=&state=` from the request line, send
a minimal HTML success response, and resolve the future.

**Rationale**: The project already depends on `tokio`. The callback is a single one-shot
GET request; a full HTTP framework (`axum`, `hyper`, `tiny_http`) would be a new
dependency for <10 lines of actual parsing logic. Using an ephemeral port (`:0`) eliminates
port-collision risk and complies with RFC 8252 §7.3.

**Rejected alternatives**:
- `tiny_http` crate: synchronous, requires `spawn_blocking`, adds a dependency.
- Hardcoded port 9876: collision risk; RFC 8252 non-compliant.
- Custom URI scheme (`adagio://callback`): requires OS-level registration on all three
  platforms; significantly more complex deployment.

---

## Decision 3: Bundled Client Credentials

**Chosen**: Ship a single `client_id` + `client_secret` pair compiled into the binary
as constants. Production values are injected at release time via CI environment variables.

**Rationale**: Every Nextcloud instance requires an OAuth2 client registered by an admin.
Requiring each self-hosted user to register a client before onboarding is a prohibitive
UX barrier. Bundling credentials (the same approach as the official Nextcloud desktop
client) keeps onboarding to "enter your server URL → browser opens". The secret identifies
the application, not the user; a leaked secret cannot be used to access user data.

**Rejected alternatives**:
- User-entered client credentials: too much friction for non-technical users.
- Dynamic client registration (RFC 7591): not supported by Nextcloud.
- Per-instance credentials stored in config: requires a registration step before first use.

---

## Consequences

- Adagio is compatible with all Nextcloud servers that have the `oauth2` app enabled
  (NC16+), which is the default since NC21.
- When Nextcloud adds server-side PKCE validation, no client changes are needed.
- The bundled `client_secret` must be registered in the Nextcloud admin panel for each
  self-hosted server where Adagio is used. Documentation and installer scripts will
  automate this where possible.
- The loopback callback approach means the redirect URI must match `http://127.0.0.1`
  (any port) in the Nextcloud OAuth2 client registration. Nextcloud 16+ accepts wildcard
  loopback registration when the redirect URI host is `127.0.0.1` or `localhost`.
