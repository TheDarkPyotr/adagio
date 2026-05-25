# Implementation Plan: Account Setup UI — Nextcloud OAuth2 Login Flow

**Branch**: `003-account-oauth2-setup` | **Date**: 2026-05-25 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/003-account-oauth2-setup/spec.md`

## Summary

Implement the full end-to-end Nextcloud OAuth2 account connection flow in the Adagio
desktop client. A user enters their Nextcloud server URL; the app opens their default
browser to the Nextcloud authorization page; after they authenticate, the app receives the
callback via a loopback HTTP listener, exchanges the code for tokens, fetches the user's
display name, stores the token exclusively in the OS keychain, and displays the connected
account. Account removal clears both the config file entry and the keychain credential.

Substantial groundwork already exists: `auth.rs` provides PKCE generation, authorization
URL building, token exchange, token refresh, and keychain helpers. The Tauri commands
`add_account`, `remove_account`, and `list_accounts` are implemented. The main missing
piece is the `connect_account_oauth2` command that integrates all pieces into a single
user-facing flow.

## Technical Context

**Language/Version**: Rust stable (edition 2021, MSRV 1.78)

**Primary Dependencies**:
- `tokio` (workspace) — async runtime + `TcpListener` for loopback callback listener
- `reqwest` (workspace) — HTTP client for server probe, token exchange, user info
- `keyring 3` with `sync-secret-service` (workspace) — OS keychain via `spawn_blocking`
- `tauri 2` + `tauri-plugin-shell 2` (adagio-desktop) — `ShellExt::open()` for browser
- `serde_json` (workspace) — `TokenPair` serialisation
- `tracing` (workspace) — structured logging throughout the auth flow

**Storage**:
- `config.json` — `SavedAccount` structs (no credentials)
- OS keychain — `TokenPair` JSON blob keyed by `("adagio", account_id)`

**Testing**: `cargo test`; unit tests in `adagio-nextcloud`; integration tests in
`tests/integration/` requiring a live Nextcloud instance (gated by env var)

**Target Platform**: Linux, macOS, Windows

**Project Type**: Desktop app (Tauri 2.x + Svelte frontend)

**Performance Goals**:
- Server URL validation completes within 5 s (SC-003)
- Browser open provides visible UI feedback within 100 ms of click
- Account stored within 2 s of browser auth completion (SC-005 proxy)

**Constraints**:
- Credentials MUST NOT appear in any file, log, or IPC return value (FR-005)
- `keyring::Entry` is not `Send + Sync`; must be created inside `spawn_blocking`
- Loopback listener must use an ephemeral port (RFC 8252); no hardcoded port

**Scale/Scope**: Single-user desktop client; ≤10 accounts typical

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Principle | Status |
|------|-----------|--------|
| Tests authored and FAIL before implementation begins | I. Test-First | ✅ Enforced by task ordering: test tasks precede impl tasks |
| All public Rust items have `///` doc comments | II. Documentation as Code | ✅ Existing auth.rs already compliant; new functions will follow suit |
| ADR recorded in `docs/adr/` for significant design decisions | II. Documentation as Code | ❌ ADR for OAuth2 flow design needed — added as task T002 |
| Structured logging added to all new sync/network operations | III. Observability | ❌ `auth.rs` currently has no `tracing::` calls — added as task T009 |
| No `println!` in production code paths | III. Observability | ✅ No bare println! in existing or planned code |
| New feature implemented as independent crate/module with no direct coupling to core | IV. Extensibility | ✅ Auth logic in `adagio-nextcloud`; Tauri commands in `adagio-desktop` |
| Cross-module calls go through defined trait/interface contracts | IV. Extensibility | ✅ Commands call `adagio_nextcloud::auth::*` as stable API |
| Idle memory budget <100 MB RSS confirmed or N/A for this feature | V. Performance-Oriented | N/A — no persistent background state added |
| UI actions provide feedback within 100 ms confirmed or N/A | V. Performance-Oriented | ✅ "waiting for browser" state shown within 100 ms; browser open itself is external |
| Benchmarks added for any hot-path changes | V. Performance-Oriented | N/A — auth is one-shot, not a hot path |
| `cargo clippy -- -D warnings` passes | Dev Workflow | ✅ Target; enforced in CI |
| `cargo fmt --check` passes | Dev Workflow | ✅ Target; enforced in CI |
| All `unsafe` blocks have `// SAFETY:` comments | Dev Workflow | N/A — no unsafe blocks planned |
| All three platform CI targets (Linux, macOS, Windows) pass | Technology | ✅ Target; keyring 3 supports all three |

## Project Structure

### Documentation (this feature)

```text
specs/003-account-oauth2-setup/
├── plan.md              # This file
├── research.md          # Phase 0 output — OAuth2 decisions
├── data-model.md        # Phase 1 output — entities
├── quickstart.md        # Phase 1 output — dev setup & edge case testing
├── contracts/
│   └── tauri-ipc.md     # Phase 1 output — IPC command contracts
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
crates/adagio-nextcloud/
├── src/
│   ├── auth.rs                   # PKCE, auth URL, token exchange/refresh, keychain
│   │                             # ADD: connect_oauth2_flow() — full flow orchestration
│   │                             # ADD: validate_server_url() — /status.php probe
│   │                             # ADD: fetch_user_info() — /ocs/v2.php/cloud/user
│   │                             # ADD: tracing instrumentation throughout
│   └── lib.rs

crates/adagio-desktop/
├── src/
│   ├── commands/
│   │   └── account.rs            # ADD: connect_account_oauth2 Tauri command
│   │                             # REPLACE: initiate_oauth2 stub with real implementation
│   └── lib.rs
└── src-ui/src/
    ├── routes/
    │   ├── Onboarding.svelte     # UPDATE: wire OAuth2 flow, add "waiting for browser" state
    │   └── Settings.svelte       # UPDATE: add "Add Account" button + OAuth2 flow panel
    └── lib/
        └── tauri.ts              # ADD: connectAccountOAuth2() binding

tests/integration/
└── oauth2_flow.rs                # ADD: integration tests (env-gated, live Nextcloud)

docs/adr/
└── 003-oauth2-flow-design.md     # ADD: ADR for auth flow decisions
```

**Structure Decision**: The feature spans `adagio-nextcloud` (auth logic, no UI coupling)
and `adagio-desktop` (Tauri commands + Svelte UI). No new crates are needed. The loopback
listener lives inside the `connect_account_oauth2` command in `adagio-desktop` since it
requires `tauri::AppHandle` for the browser-open call and tokio for `TcpListener`.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| ADR missing at plan time | ADR written as first task (T002) | Could not write it before research was complete |
| Structured logging missing in existing auth.rs | Existing stub code lacked tracing; added as T009 | Skipping would violate Constitution §III |
