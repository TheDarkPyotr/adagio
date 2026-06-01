# Implementation Plan: Onboarding — Account Setup Wizard

**Branch**: `014-onboarding-account-setup` | **Date**: 2026-06-01 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/014-onboarding-account-setup/spec.md`

## Summary

Wire the existing 5-step `OnboardingWizard` UI to live backend services. Each step currently uses
hardcoded or static mock data; this plan replaces every mock with a real IPC call or event listener.
The key architectural addition is a **Nextcloud Login Flow v2** backend module that returns a
displayable code + QR URL immediately and resolves asynchronously via a Tauri event when the user
completes browser auth. All four sync-preference toggles are persisted to `config.json` and applied
to the first pair created at wizard completion.

## Technical Context

**Language/Version**: Rust 1.96 (stable, workspace edition 2021) + TypeScript 5 / React 18

**Primary Dependencies**:
- Tauri 2.x — desktop shell + IPC
- `tokio` — async runtime (already in workspace)
- `reqwest` — HTTP for server probe and Login Flow polling (already in workspace)
- `tauri-plugin-dialog` 2.x — **new**: native OS folder-picker dialog
- `qrcode` crate — **new**: QR matrix generation (SVG/text, no image deps needed)
- `serde` / `serde_json` — (already in workspace)
- `tracing` — structured logging (already in workspace)

**Storage**: SQLite (`sqlx`) for pair + sync state; `config.json` for account/preference metadata; OS keychain for credentials

**Testing**: `cargo test` (unit + integration); `vitest` (frontend component tests)

**Target Platform**: Linux, macOS, Windows (all three CI targets required)

**Project Type**: Desktop app (Tauri 2 shell over Rust backend)

**Performance Goals**:
- Server probe feedback visible within 3 s of URL entry
- Auth flow code displayed within 2 s of reaching step 3
- Wizard auto-advances within 5 s of browser auth completion
- All UI transitions respond within 100 ms (Constitution § V)

**Constraints**: Credentials MUST NOT appear in `config.json`, logs, or on-screen text after auth.
Onboarding state is ephemeral — no partial account is persisted until auth succeeds.

**Scale/Scope**: Single account being added; no concurrent onboarding sessions.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Principle | Status |
|------|-----------|--------|
| Tests authored and FAIL before implementation begins | I. Test-First | ✅ required — tests for each new command and UI step must be written first |
| All public Rust items have `///` doc comments | II. Documentation as Code | ✅ required for all new commands and types |
| ADR recorded in `docs/adr/` for significant design decisions | II. Documentation as Code | ✅ required — Login Flow v2 vs PKCE decision must be recorded |
| Structured logging added to all new sync/network operations | III. Observability | ✅ required — probe, auth flow start/poll/complete, pair creation |
| No `println!` in production code paths | III. Observability | ✅ enforced by existing CI clippy gate |
| New feature implemented as independent crate/module with no direct coupling to core | IV. Extensibility | ✅ new `commands/onboarding.rs` + `auth_flow/` module; no core changes |
| Cross-module calls go through defined trait/interface contracts | IV. Extensibility | ✅ new IPC commands are the public boundary |
| Idle memory budget <100 MB RSS confirmed or N/A | V. Performance-Oriented | N/A — onboarding is a one-time flow, not an idle service |
| UI actions provide feedback within 100 ms confirmed | V. Performance-Oriented | ✅ input debounce on URL field; all IPC responses show spinner within 100 ms |
| Benchmarks added for any hot-path changes | V. Performance-Oriented | N/A — no hot paths (QR generation is one-shot) |
| `cargo clippy -- -D warnings` passes | Dev Workflow | ✅ enforced by CI |
| `cargo fmt --check` passes | Dev Workflow | ✅ enforced by CI |
| All `unsafe` blocks have `// SAFETY:` comments | Dev Workflow | N/A — no unsafe needed |
| All three platform CI targets (Linux, macOS, Windows) pass | Technology | ✅ required; dialog plugin must be tested on all three |

## Project Structure

### Documentation (this feature)

```text
specs/014-onboarding-account-setup/
├── plan.md              ← this file
├── research.md          ← Phase 0: Login Flow v2 vs PKCE decision, QR gen, dialog plugin
├── data-model.md        ← Phase 1: ServerProbeDto, AuthFlowInitDto, OnboardingPrefsDto, RemoteStatsDto
├── contracts/
│   └── onboarding-ipc.md  ← Phase 1: all new Tauri IPC commands + events
└── quickstart.md        ← Phase 1: dev setup for testing the wizard end-to-end
```

### Source Code

```text
crates/adagio-desktop/
├── Cargo.toml                          ← add tauri-plugin-dialog, qrcode
├── src/
│   ├── commands/
│   │   ├── onboarding.rs               ← NEW: probe_server, begin_auth_flow,
│   │   │                                       pick_folder, get_account_remote_stats,
│   │   │                                       complete_onboarding
│   │   └── mod.rs                      ← register new commands
│   ├── auth_flow/
│   │   └── mod.rs                      ← NEW: Login Flow v2 polling task + event emission
│   └── lib.rs                          ← register tauri-plugin-dialog
└── src-ui/src/
    ├── tauri.ts                        ← add: probeServer, beginAuthFlow, pickFolder,
    │                                          getAccountRemoteStats, completeOnboarding,
    │                                          listenAuthFlowComplete, listenAuthFlowExpired
    └── components/
        └── OnboardingWizard.tsx        ← wire all 5 steps to real IPC

crates/adagio-nextcloud/src/
└── login_flow.rs                       ← NEW: Login Flow v2 HTTP protocol (POST /login/v2,
                                                poll endpoint, extract credentials)
```

## Phase 0: Research

See [research.md](research.md) for full findings. Summary of key decisions:

### Decision 1 — Authentication Protocol: Nextcloud Login Flow v2

**Decision**: Replace the existing PKCE callback flow in `connect_account_oauth2` with
Nextcloud Login Flow v2 for the onboarding wizard.

**Rationale**:
- Login Flow v2 (`POST /index.php/login/v2`) returns a short login URL and a poll token
  immediately, making it trivial to show a displayable code and QR to the user.
- It works on all Nextcloud ≥ 16 servers regardless of whether OAuth2 apps are enabled.
- The client polls an endpoint until the server delivers `{ server, loginName, appPassword }`;
  this polling naturally maps to a background Tokio task that emits a Tauri event on completion.
- The existing PKCE flow (loopback callback listener) remains intact for any future headless / CLI
  use cases; the wizard simply uses the new flow instead.

**Alternatives considered**:
- *Keep PKCE*: The loopback callback approach works but gives the client no short code to display.
  The QR would encode the full authorization URL (100+ chars), which is awkward to type.
- *OAuth2 Device Authorization Grant (RFC 8628)*: Nextcloud does not support RFC 8628; Login Flow v2
  is Nextcloud's equivalent.

**ADR**: `docs/adr/014-login-flow-v2.md`

### Decision 2 — QR Code Generation: `qrcode` crate, SVG output

**Decision**: Use the `qrcode` crate on the Rust side to generate an SVG string; pass it to the
frontend as a plain string and render with `dangerouslySetInnerHTML` inside a constrained container.

**Rationale**: Avoids pulling in image encoding (PNG/JPEG) or a JavaScript QR library. SVG scales
perfectly at any size and is safe to embed (the URL we encode is entirely under our control).

**Alternatives considered**:
- *JavaScript QR library (e.g. qrcode.js)*: Works but adds a frontend dependency and requires
  the login URL to be passed to JS first. Server-side generation keeps secrets off the frontend bundle.
- *PNG via `image` crate*: Heavier dependency tree; unnecessary for this use case.

### Decision 3 — Folder Picker: `tauri-plugin-dialog`

**Decision**: Use `tauri-plugin-dialog` 2.x `open()` with `directory: true`.

**Rationale**: Official Tauri 2 plugin, cross-platform, zero custom code. The frontend calls
`invoke('pick_folder')` which calls `tauri_plugin_dialog::DialogExt::dialog().file().pick_folder()`.

### Decision 4 — Sync Preferences Persistence

**Decision**: Add an `[onboarding_prefs]` section to `config.json` (`SavedConfig`). The four
toggles map as follows:

| Toggle | Config field | Applied at |
|--------|-------------|------------|
| On-demand files | `pair.vfs_enabled` | `create_pair` call in `complete_onboarding` |
| Pin pinned folders | `onboarding_prefs.pin_pinned_folders` | read by VFS runner at startup |
| Smart bandwidth | `onboarding_prefs.smart_bandwidth` → sets network policy `on_battery: throttle` | `complete_onboarding` calls `set_network_policy` |
| Watch external edits | `pair.scan_interval_secs` set to 30 s (enabled) or 7200 s (disabled) | `create_pair` call |

### Decision 5 — Remote Stats: Nextcloud OCS `quota` endpoint

**Decision**: Call `GET /ocs/v2.php/cloud/users/{username}` with Accept: `application/json`.
The response includes `quota.total` (bytes) and `quota.used` (bytes). File count is obtained from
`GET /remote.php/dav/files/{username}/` (PROPFIND depth 1, return `getlastmodified` + `getcontenttype`).
Since a full recursive count is expensive, use the sync engine's remote tree scan result
(`list_remote_tree` IPC) which is already performed during pair initialization.

**Rationale**: Avoids a separate expensive PROPFIND; the pair scan result is already available
after `complete_onboarding` triggers the first sync. If the scan hasn't finished, show a spinner
and update once available.

## Phase 1: Design & Contracts

See [data-model.md](data-model.md) and [contracts/onboarding-ipc.md](contracts/onboarding-ipc.md).

### New IPC Commands

| Command | Direction | Purpose |
|---------|-----------|---------|
| `probe_server` | UI → Backend | Validate URL; return version + capabilities |
| `begin_auth_flow` | UI → Backend | Start Login Flow v2; return code + QR SVG immediately |
| `pick_folder` | UI → Backend | Open OS folder-picker dialog; return chosen path or null |
| `get_account_remote_stats` | UI → Backend | Return file count + total bytes from remote |
| `complete_onboarding` | UI → Backend | Create pair + persist prefs; return pair ID |

### New Tauri Events (Backend → UI)

| Event | Payload | Purpose |
|-------|---------|---------|
| `adagio://auth-flow-complete` | `{ account: AccountDto }` | Auth succeeded; wizard auto-advances |
| `adagio://auth-flow-expired` | `{}` | Code expired before user authorized |

### Onboarding Wizard Step Wiring

| Step | Was | Now |
|------|-----|-----|
| 1 — Welcome | Static text | Static text (no change needed) |
| 2 — Server | Hardcoded URL + fake "Nextcloud 28.0.1" detected badge | `probe_server(url)` on 600 ms debounce; real version + capabilities shown |
| 3 — Authorize | Static code "F4PS · 9TRX", fake QR, fake countdown, no auto-advance | `begin_auth_flow(url)` → real code + real SVG QR; real countdown from `expires_at`; listen `adagio://auth-flow-complete` |
| 4 — Where to sync | Static path, fake Browse button, toggle state lost on navigation | `pick_folder()` on Browse click; toggle state lifted to wizard-level state; `get_account_remote_stats` queued |
| 5 — Begin | Fake counts "3,247 files · 86.7 GB", static progress bar | Real `RemoteStatsDto`; live progress from `getStatus()` polled every 2 s |

### Key Structural Changes

- `OnboardingWizard` receives `onComplete(accountId: string, pairId: string)` instead of bare `onComplete()`.
  `App.tsx` updated to create the first pair ID from the result before showing the main view.
- `complete_onboarding` is called when user clicks "Open Adagio" on step 5 (or immediately after step 4
  if the user doesn't wait for the Begin screen). It creates the pair synchronously and returns.
- The ephemeral auth-flow state (poll handle, token) is held in a `Mutex<Option<AuthFlowState>>`
  in `AppState`; it is cleared on `auth-flow-complete` or `auth-flow-expired`.

## Complexity Tracking

> No Constitution violations — no Complexity Tracking entries required.
