# Tasks: Onboarding — Account Setup Wizard

**Input**: Design documents from `specs/014-onboarding-account-setup/`

**Prerequisites**: plan.md ✅ | spec.md ✅ | research.md ✅ | data-model.md ✅ | contracts/onboarding-ipc.md ✅ | quickstart.md ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins. The Red-Green-Refactor cycle is strictly enforced.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US4)
- Exact file paths are included in every description

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Dependency additions, module scaffolding, ADR, and plugin registration — no logic yet.

- [X] T001 Record architecture decision in `docs/adr/014-login-flow-v2.md`: document the choice of Nextcloud Login Flow v2 over PKCE for the onboarding wizard, alternatives considered, and migration notes
- [X] T002 Add `tauri-plugin-dialog = "2"` and `qrcode = "0.14"` to `crates/adagio-desktop/Cargo.toml`; add `tauri-plugin-dialog` to `[build-dependencies]` in `tauri.conf.json` allowlist
- [X] T003 [P] Scaffold the Login Flow v2 module: add `pub mod login_flow;` to `crates/adagio-nextcloud/src/lib.rs`; create `crates/adagio-nextcloud/src/login_flow.rs` with empty public type stubs (`LoginFlowInitResponse`, `LoginCredentials`) and function stubs (`begin_login_flow`, `poll_login_flow`)
- [X] T004 [P] Scaffold the auth_flow module: create `crates/adagio-desktop/src/auth_flow/mod.rs` with `AuthFlowState` struct stub (fields: `poll_token: String`, `poll_endpoint: String`, `server_url: String`, `task_handle: JoinHandle<()>`, `expires_at: std::time::Instant`)
- [X] T005 Create `crates/adagio-desktop/src/commands/onboarding.rs` with five stub `#[tauri::command]` functions (`probe_server`, `begin_auth_flow`, `pick_folder`, `get_account_remote_stats`, `complete_onboarding`); register `mod onboarding;` in `crates/adagio-desktop/src/commands/mod.rs`; add all five commands to the `tauri::Builder` handler list in `crates/adagio-desktop/src/lib.rs`; register `.plugin(tauri_plugin_dialog::init())` in `lib.rs`; add `auth_flow` module (`mod auth_flow;`) to `lib.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Login Flow v2 HTTP protocol + config extension — foundational pieces that all user story phases depend on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T006 [P] Write failing unit tests for Login Flow v2 protocol in `crates/adagio-nextcloud/src/login_flow.rs` (`#[cfg(test)]` block): test `LoginFlowInitResponse` JSON deserialisation from a sample `/login/v2` response; test `poll_login_flow` parsing of a 200 credential response; test `poll_login_flow` treating HTTP 404 as "not yet complete" (returns `Ok(None)`); tests must compile and FAIL before T008
- [X] T007 [P] Write failing unit tests for `OnboardingPrefs` config extension in `crates/adagio-desktop/src/config/mod.rs`: test serde round-trip of `SavedConfig` with `onboarding_prefs` field populated; test that deserialising a JSON string *without* `onboarding_prefs` field yields `OnboardingPrefs::default()` (backward compatibility); tests must FAIL before T009
- [X] T008 Implement Login Flow v2 HTTP protocol in `crates/adagio-nextcloud/src/login_flow.rs`: define `LoginFlowInitResponse { login: String, poll: PollDetails }` and `LoginCredentials { server: String, login_name: String, app_password: String }`; implement `begin_login_flow(server_url: &str) -> Result<LoginFlowInitResponse, SyncError>` (POST `/index.php/login/v2`); implement `poll_login_flow(endpoint: &str, token: &str) -> Result<Option<LoginCredentials>, SyncError>` (POST endpoint, returns `None` on 404); add `/// ` doc comments to all public items; make T006 tests pass
- [X] T009 [P] Extend `SavedConfig` in `crates/adagio-desktop/src/config/mod.rs`: add `#[serde(default)] pub onboarding_prefs: OnboardingPrefs`; define `OnboardingPrefs { vfs_enabled: bool, pin_pinned_folders: bool, smart_bandwidth: bool, watch_external_edits: bool }` with `#[derive(Serialize, Deserialize, Clone, Default)]` and field defaults via `#[serde(default = "…")]`; make T007 tests pass
- [X] T010 [P] Extend `AppState` in `crates/adagio-desktop/src/state.rs`: add `pub auth_flow: Mutex<Option<crate::auth_flow::AuthFlowState>>` field; update `AppState::new()` / builder to initialise it as `Mutex::new(None)`; complete the `AuthFlowState` struct in `crates/adagio-desktop/src/auth_flow/mod.rs` with all fields from the data model

**Checkpoint**: Foundation ready — Login Flow v2 can be called, config can persist prefs, AppState holds auth session.

---

## Phase 3: User Story 1 — First-Time Server Connection (Priority: P1) 🎯 MVP

**Goal**: Step 2 (Server) shows real server version and capabilities from a live probe; Continue is blocked until a valid, reachable Nextcloud is confirmed.

**Independent Test**: Launch the app with `DEV_FORCE_ONBOARD = true`. Enter `http://localhost:8080` (Docker NC). Verify the version badge shows the real NC version within 3 s. Enter `http://192.0.2.1` (unroutable). Verify inline error appears and Continue is disabled.

### Tests for User Story 1 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T011 [P] [US1] Write failing unit tests for `probe_server` in `crates/adagio-desktop/src/commands/onboarding.rs` (`#[cfg(test)]`): assert a mocked 200 `/status.php` response with `versionstring: "28.0.1"` produces `ServerProbeDto { reachable: true, version: "28.0.1", version_ok: true }`; assert connection refused produces `reachable: false` with a non-empty `error`; assert `version_ok: false` when major version < 16; tests must FAIL before T013
- [X] T012 [P] [US1] Write failing frontend tests for step 2 in `crates/adagio-desktop/src-ui/src/__tests__/OnboardingWizard.test.tsx`: mock `probeServer` returning `{ reachable: true, version: "28.0.1", version_ok: true, e2ee_available: true, … }`; assert version string appears in step 2 after input; mock `probeServer` returning `{ reachable: false, error: "Server unreachable" }`; assert Continue button is disabled and error text is shown; tests must FAIL before T015

### Implementation for User Story 1

- [X] T013 [US1] Implement `probe_server` command in `crates/adagio-desktop/src/commands/onboarding.rs`: `GET {server_url}/status.php` with 5 s timeout; parse `versionstring` and `maintenance` from JSON; derive `version_ok` (major ≥ 16), `e2ee_available` (major ≥ 20), `tls_valid` (no TLS error); record `latency_ms`; emit `tracing::info!` with server_url and version; add `///` doc comments; return `ServerProbeDto`; make T011 tests pass
- [X] T014 [P] [US1] Add `ServerProbeDto` interface and `probeServer(serverUrl: string): Promise<ServerProbeDto>` binding to `crates/adagio-desktop/src-ui/src/tauri.ts`
- [X] T015 [US1] Wire step 2 (Server) in `crates/adagio-desktop/src-ui/src/components/OnboardingWizard.tsx`: replace the hardcoded `url` default with component state driven by the input; call `probeServer(url)` on a 600 ms debounce (use `useEffect` + `setTimeout`); store `ServerProbeDto | null` in state; render real version and capabilities (auth flow, TLS, E2EE, latency) in the detected panel; disable Continue and show `probe.error` text when `!probe?.reachable || !probe?.version_ok`; make T012 tests pass

**Checkpoint**: Step 2 is fully functional. Probe the live Docker NC instance and confirm real version badge.

---

## Phase 4: User Story 2 — Browser Authorization (Priority: P1)

**Goal**: Step 3 (Authorize) shows a real one-time code and real QR code; browser opens automatically; wizard auto-advances when the user completes auth; expiry state + "Try again" works.

**Independent Test**: With a valid server URL from step 2, reach step 3. Verify a real code (not "F4PS · 9TRX") is shown. Scan the QR code with a phone — it should open the NC login page. Complete login in the browser. Verify the wizard advances to step 4 automatically (no click). Reload the step without completing auth; let the 5-min countdown expire — verify the expiry UI appears.

### Tests for User Story 2 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T016 [P] [US2] Write failing unit tests for `begin_auth_flow` command in `crates/adagio-desktop/src/commands/onboarding.rs`: mock `login_flow::begin_login_flow` returning a `LoginFlowInitResponse`; assert `display_code` is formatted "XXXX · XXXX"; assert `qr_svg` is a non-empty string starting with `<svg`; assert `AuthFlowState` is stored in `AppState.auth_flow`; tests must FAIL before T020
- [X] T017 [P] [US2] Write failing unit tests for the auth_flow polling task in `crates/adagio-desktop/src/auth_flow/mod.rs`: test that `run_poll_task` emits the `adagio://auth-flow-complete` Tauri event payload when `poll_login_flow` returns `Ok(Some(credentials))`; test that it emits `adagio://auth-flow-expired` when the deadline elapses; tests must FAIL before T019
- [X] T018 [P] [US2] Write failing frontend tests for step 3 in `crates/adagio-desktop/src-ui/src/__tests__/OnboardingWizard.test.tsx`: mock `beginAuthFlow` returning `{ display_code: "AB12 · CD34", login_url: "https://…", qr_svg: "<svg>…</svg>", expires_at: … }`; assert `"AB12 · CD34"` appears in step 3; assert the SVG container is rendered; mock `listenAuthFlowComplete` firing immediately; assert wizard advances to step 4; mock `listenAuthFlowExpired` firing; assert expiry state shown; tests must FAIL before T022

### Implementation for User Story 2

- [X] T019 [US2] Implement `run_poll_task` in `crates/adagio-desktop/src/auth_flow/mod.rs`: `tokio::spawn` loop that calls `login_flow::poll_login_flow` every 2 s; on `Ok(Some(creds))` — create account via daemon (reuse `connect_account_oauth2` credential storage logic), emit `adagio://auth-flow-complete` with `AccountDto`, clear `AppState.auth_flow`; on deadline exceeded — emit `adagio://auth-flow-expired`, clear `AppState.auth_flow`; add `tracing::info!` at start, completion, and expiry; add `///` doc comments; make T017 tests pass
- [X] T020 [US2] Implement `begin_auth_flow` command in `crates/adagio-desktop/src/commands/onboarding.rs`: call `login_flow::begin_login_flow(&server_url)`; extract last 8 chars of login token and format as "XXXX · XXXX" for `display_code`; generate `qr_svg` using `qrcode::QrCode::new(login_url)` rendered as inline SVG string; abort any existing `AuthFlowState.task_handle`; store new `AuthFlowState` in `AppState.auth_flow`; open browser via `tauri_plugin_shell`; spawn `run_poll_task`; return `AuthFlowInitDto`; make T016 tests pass
- [X] T021 [P] [US2] Add `AuthFlowInitDto` interface and `beginAuthFlow()`, `listenAuthFlowComplete()`, `listenAuthFlowExpired()` bindings to `crates/adagio-desktop/src-ui/src/tauri.ts`
- [X] T022 [US2] Wire step 3 (Authorize) in `crates/adagio-desktop/src-ui/src/components/OnboardingWizard.tsx`: on step 3 mount call `beginAuthFlow(url)`; store `AuthFlowInitDto` in state; replace static code with `dto.display_code`; replace `FakeQR` with `<div dangerouslySetInnerHTML={{ __html: dto.qr_svg }}/>` in fixed-size container; replace static countdown with a `useEffect` timer that counts down from `dto.expires_at - Date.now()/1000`; subscribe to `listenAuthFlowComplete` → store `accountId` in wizard state and advance to step 4; subscribe to `listenAuthFlowExpired` → set `expired: true` state, swap Continue for "Try again" button that re-calls `beginAuthFlow`; unsubscribe both listeners on unmount; make T018 tests pass

**Checkpoint**: Steps 2 and 3 are fully functional. Auth flow completes end-to-end with real NC instance.

---

## Phase 5: User Story 3 — Local Folder Selection & Sync Preferences (Priority: P2)

**Goal**: Step 4 (Where to sync) opens a real OS folder-picker dialog; all 4 toggle states survive navigation; `complete_onboarding` persists everything and creates the first sync pair.

**Independent Test**: Reach step 4. Click "Browse…" — OS dialog opens. Select a folder — path reflected in input. Toggle all 4 switches. Click Continue (to step 5) then Back (to step 4) — assert toggle states unchanged. Complete onboarding. Restart the app. Assert toggle states match what was set.

### Tests for User Story 3 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T023 [P] [US3] Write failing unit tests for `pick_folder` command in `crates/adagio-desktop/src/commands/onboarding.rs`: test that a `None` dialog result returns `Ok(None)`; test that a `Some(PathBuf)` result returns `Ok(Some(String))`; tests must FAIL before T026
- [X] T024 [P] [US3] Write failing integration tests for `complete_onboarding` command in `crates/adagio-desktop/tests/integration/desktop_lifecycle.rs`: assert `DaemonRequest::CreatePair` is sent with `vfs_enabled = true` when prefs has `vfs_enabled: true`; assert `scan_interval_secs = 30` when `watch_external_edits: true`; assert `DaemonRequest::SetNetworkPolicy` is sent when `smart_bandwidth: true`; assert `config.json` contains `onboarding_prefs` after the call; tests must FAIL before T027
- [X] T025 [P] [US3] Write failing frontend tests for step 4 in `crates/adagio-desktop/src-ui/src/__tests__/OnboardingWizard.test.tsx`: mock `pickFolder` returning `"/home/user/Adagio"`; assert path shown in folder input after click; assert On-demand and Smart bandwidth toggles start ON; assert Watch external edits starts OFF; navigate Back then Continue; assert all toggle states unchanged; tests must FAIL before T029

### Implementation for User Story 3

- [X] T026 [US3] Implement `pick_folder` command in `crates/adagio-desktop/src/commands/onboarding.rs` using `tauri_plugin_dialog::DialogExt::dialog().file().blocking_pick_folder()`; convert `FilePath` to `String` via `to_string_lossy`; add `///` doc comment; make T023 tests pass
- [X] T027 [US3] Implement `complete_onboarding` command in `crates/adagio-desktop/src/commands/onboarding.rs`: derive `scan_interval_secs` (30 if `watch_external_edits`, else 7200); send `DaemonRequest::CreatePair` with `account_id`, `local_root`, `vfs_enabled`, `scan_interval_secs`; if `smart_bandwidth` send `DaemonRequest::SetNetworkPolicy { on_battery: "throttle", throttle_kbps: 500 }`; write `onboarding_prefs` to `config.json` via `save_config`; emit `tracing::info!` with pair_id; add `///` doc comments; return `PairDto`; make T024 tests pass
- [X] T028 [P] [US3] Add `OnboardingPrefsInput` interface and `pickFolder()`, `completeOnboarding(accountId, prefs)` bindings to `crates/adagio-desktop/src-ui/src/tauri.ts`
- [X] T029 [US3] Wire step 4 (Where to sync) in `crates/adagio-desktop/src-ui/src/components/OnboardingWizard.tsx`: lift toggle state (`vfsEnabled`, `pinPinned`, `smartBandwidth`, `watchExternal`) and `folder` to top-level wizard state so they survive step navigation; set correct defaults (on-demand ON, pin ON, bandwidth ON, watch OFF); wire "Browse…" button to `pickFolder()` and update `folder` state on success; on "Open Adagio" click in step 5 call `completeOnboarding(accountId, { local_folder: folder, … })`; update `onComplete` prop signature to `onComplete(accountId: string, pairId: string)`; make T025 tests pass

**Checkpoint**: Full onboarding flow creates a real sync pair. Restart and confirm prefs persisted.

---

## Phase 6: User Story 4 — Real Initial Sync Progress (Priority: P2)

**Goal**: Step 5 (Begin) shows the real remote file count and storage size; the progress bar reflects actual sync state; clicking "Open Adagio" transitions to the main app.

**Independent Test**: Complete full onboarding. On step 5, compare file count and storage total shown against Nextcloud web UI (Settings → Personal info). Confirm progress bar advances as files download. Click "Open Adagio" and confirm main app view appears with the new pair visible.

### Tests for User Story 4 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T030 [P] [US4] Write failing unit tests for `get_account_remote_stats` command in `crates/adagio-desktop/src/commands/onboarding.rs`: mock OCS `/cloud/users/{username}` returning `{ data: { quota: { total: 107374182400, used: 5368709120 } } }`; assert `RemoteStatsDto { total_bytes: 107374182400, used_bytes: 5368709120, file_count: None }`; test returns `Err` when credentials not found; tests must FAIL before T032
- [X] T031 [P] [US4] Write failing frontend tests for step 5 in `crates/adagio-desktop/src-ui/src/__tests__/OnboardingWizard.test.tsx`: mock `getAccountRemoteStats` returning `{ total_bytes: 93_000_000_000, used_bytes: 5_000_000_000, file_count: 3247 }`; assert formatted bytes displayed ("86.7 GB" or similar); assert `getStatus` poll shows sync progress values; assert clicking "Open Adagio" calls `onComplete`; tests must FAIL before T034

### Implementation for User Story 4

- [X] T032 [US4] Implement `get_account_remote_stats` command in `crates/adagio-desktop/src/commands/onboarding.rs`: look up account in config; retrieve credentials from keychain; call `GET {server_url}/ocs/v2.php/cloud/users/{username}` with Bearer token and `Accept: application/json`; parse `data.quota.total` and `data.quota.used`; return `RemoteStatsDto { total_bytes, used_bytes, file_count: None }`; emit `tracing::debug!`; add `///` doc comments; make T030 tests pass
- [X] T033 [P] [US4] Add `RemoteStatsDto` interface and `getAccountRemoteStats(accountId: string)` binding to `crates/adagio-desktop/src-ui/src/tauri.ts`
- [X] T034 [US4] Wire step 5 (Begin) in `crates/adagio-desktop/src-ui/src/components/OnboardingWizard.tsx`: on step 5 mount call `getAccountRemoteStats(accountId)`; display `total_bytes` formatted (bytes → human-readable GB/MB); show spinner for `file_count` while `null`; replace static progress bar with values from `getStatus()` (reuse `syncStatus` prop or poll internally every 2 s); replace static "3,247 files · 86.7 GB" placeholder entirely; make T031 tests pass
- [X] T035 [US4] Update `crates/adagio-desktop/src-ui/src/App.tsx`: change `OnboardingWizard` `onComplete` handler signature to `(accountId: string, pairId: string) => void`; in `handleAddAccountComplete` and the first-run `onComplete`, call `listAccounts()` and `listPairs()` to populate state; ensure the new pair is reflected in the sidebar immediately after onboarding

**Checkpoint**: Full 5-step wizard is functional end-to-end with real data at every step.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Doc comments, observability, edge cases, and quality gates.

- [X] T036 [P] Add `///` doc comments to all public Rust items not yet documented: `LoginFlowInitResponse`, `LoginCredentials`, `begin_login_flow`, `poll_login_flow` in `crates/adagio-nextcloud/src/login_flow.rs`; `AuthFlowState`, `run_poll_task` in `crates/adagio-desktop/src/auth_flow/mod.rs`; all five commands in `crates/adagio-desktop/src/commands/onboarding.rs`
- [X] T037 [P] Verify and complete structured logging: confirm `tracing::info!` at probe_server call site (server_url, version, latency_ms), at begin_auth_flow (server_url, display_code), at poll success/expiry (server_url, outcome), at complete_onboarding (account_id, pair_id, prefs summary); confirm no `println!` in any new file
- [X] T038 Handle wizard edge cases in `crates/adagio-desktop/src-ui/src/components/OnboardingWizard.tsx`: if `beginAuthFlow` returns an error (server went offline after step 2), show error inline on step 3 with a "Go back" link; if `complete_onboarding` returns an error (folder not writable), show error on step 5 with "Choose different folder" action; if `getAccountRemoteStats` fails (network lost), show "—" instead of spinner indefinitely; if account has zero files, show "No files yet" instead of "0 files"
- [ ] T039 [P] Run end-to-end validation per `specs/014-onboarding-account-setup/quickstart.md`: confirm each acceptance criterion from the spec passes against the Docker NC instance; document any deviations
- [X] T040 Run `cargo clippy -- -D warnings` and `cargo fmt --check` across all modified crates (`adagio-desktop`, `adagio-nextcloud`); fix all warnings; confirm `cargo test -p adagio-desktop onboarding` and `npm run test:run` (from `src-ui/`) both pass

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 completion — **BLOCKS all user stories**
- **US1 (Phase 3)**: Depends on Phase 2 — `probe_server` needs no Login Flow v2
- **US2 (Phase 4)**: Depends on Phase 2 (needs Login Flow v2) + Phase 3 (needs wired URL from step 2)
- **US3 (Phase 5)**: Depends on Phase 4 (needs `accountId` from auth completion)
- **US4 (Phase 6)**: Depends on Phase 5 (`complete_onboarding` must run before step 5 stats)
- **Polish (Phase 7)**: Depends on Phases 3–6 complete

### User Story Dependencies

- **US1 (P1)**: Can start after Phase 2 — no dependency on other user stories
- **US2 (P1)**: Depends on US1 (wizard step 2 must provide validated server URL to step 3)
- **US3 (P2)**: Depends on US2 (needs `accountId` from auth-flow-complete event)
- **US4 (P2)**: Depends on US3 (`complete_onboarding` must succeed to produce a pair and accountId for stats)

### Within Each User Story

- Test tasks MUST be written and FAIL before implementation tasks begin
- Rust type definitions before command implementations
- TypeScript bindings can be written in parallel with Rust implementation
- Frontend component wiring depends on TypeScript bindings

### Parallel Opportunities

- T003, T004 can run in parallel (different crates/modules)
- T006, T007 can run in parallel (different files)
- T008, T009, T010 can run in parallel (different files, T008 depends on T006 test writing, T009 on T007)
- Within each user story, test tasks [P] can be written together; bindings task [P] can be done alongside Rust implementation

---

## Parallel Example: User Story 2

```
# Write all US2 tests together (before any implementation):
T016: begin_auth_flow command unit tests
T017: auth_flow polling task unit tests
T018: frontend step 3 component tests

# Then implement in dependency order:
T019: auth_flow polling task (no external deps)
T020: begin_auth_flow command (depends on T019 + T008 login_flow)
T021: TypeScript bindings (parallel with T020)
T022: Wire OnboardingWizard step 3 (depends on T020 + T021)
```

---

## Implementation Strategy

### MVP First (Steps 2 + 3 only — Users can connect an account)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational
3. Complete Phase 3: US1 (real server probe)
4. Complete Phase 4: US2 (real auth flow)
5. **STOP and VALIDATE**: A user can enter a URL, see real capabilities, authenticate, and get an account created
6. Steps 4 + 5 remain mocked — the app falls through to the main view on auth completion

### Incremental Delivery

1. Setup + Foundational → scaffold ready
2. + US1 → step 2 shows real server info
3. + US2 → auth flow works end-to-end (MVP!)
4. + US3 → folder picker + preferences persisted
5. + US4 → Begin screen shows real data
6. + Polish → production-ready

---

## Notes

- `[P]` tasks touch different files and have no incomplete task dependencies — safe to run concurrently
- `[Story]` label maps each task to a specific user story for independent traceability
- Each user story produces a self-contained, independently testable increment
- `DEV_FORCE_ONBOARD = true` in `App.tsx` (line ~28) enables wizard testing without removing real accounts — revert before committing
- The existing `connect_account_oauth2` (PKCE flow) is preserved unchanged; the wizard uses the new Login Flow v2 path exclusively
