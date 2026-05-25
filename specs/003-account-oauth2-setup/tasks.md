# Tasks: Account Setup UI — Nextcloud OAuth2 Login Flow

**Input**: Design documents from `specs/003-account-oauth2-setup/`

**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/tauri-ipc.md ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins.

**Organization**: Tasks are grouped by user story to enable independent implementation
and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)

---

## Phase 1: Setup

**Purpose**: Documentation gate required by Constitution (§II) before any implementation.

- [X] T001 Create ADR at `docs/adr/003-oauth2-flow-design.md` documenting: auth code flow with PKCE params (forward-compat), loopback redirect via raw tokio TcpListener, bundled client credentials, refresh-on-401 strategy, OCS user info endpoint, status.php server probe

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared auth infrastructure used by all three user stories. Must be complete before US1–US3 can be implemented.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T002 Add `tracing::` instrumentation (INFO/DEBUG/ERROR levels) to all existing functions in `crates/adagio-nextcloud/src/auth.rs`: `exchange_code`, `refresh_access_token`, `store_credentials`, `retrieve_credentials`, `delete_credentials`
- [X] T003 [P] Implement `pub async fn validate_server_url(server_url: &str) -> Result<(), SyncError>` in `crates/adagio-nextcloud/src/auth.rs` — GET `{server_url}/status.php` with 5 s timeout; return `Err` if non-200 or JSON lacks `"installed": true`
- [X] T004 [P] Implement `pub async fn fetch_user_info(server_url: &str, access_token: &str) -> Result<(String, String), SyncError>` in `crates/adagio-nextcloud/src/auth.rs` — GET `/ocs/v2.php/cloud/user?format=json` with `Authorization: Bearer` and `OCS-APIREQUEST: true` headers; return `(username, display_name)` from `ocs.data.id` and `ocs.data.display-name`

**Checkpoint**: Foundation ready — user story implementation can now begin.

---

## Phase 3: User Story 1 — First-Time Account Connection (Priority: P1) 🎯 MVP

**Goal**: A new user can enter their Nextcloud server URL, complete browser OAuth2 auth,
and see their account listed in the app with credentials stored only in the OS keychain.

**Independent Test**: Launch app with no saved accounts. Click "Connect with browser", enter
a valid Nextcloud URL, complete auth. Verify: account appears with display name and server
URL; `config.json` contains no token; OS keychain entry exists for the account UUID.

### Tests for User Story 1 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T005 [P] [US1] Write unit tests for `validate_server_url` in `crates/adagio-nextcloud/src/auth.rs` — test: valid NC server returns Ok; non-200 returns Err "Server unreachable"; missing `installed` field returns Err "Not a Nextcloud server"; timeout returns Err; use `mockito` or hand-rolled mock
- [X] T006 [P] [US1] Write unit tests for `fetch_user_info` in `crates/adagio-nextcloud/src/auth.rs` — test: valid OCS response extracts id + display-name; missing fields return Err; 401 returns Err; use mock HTTP server
- [X] T007 [P] [US1] Write unit tests for the loopback callback listener in `crates/adagio-desktop/src/oauth2_callback.rs` — test: `spawn_callback_listener()` returns a port > 0; sending a GET to `http://127.0.0.1:{port}/callback?code=abc&state=xyz` resolves the future with `(code="abc", state="xyz")`; state mismatch returns Err; timeout (use short duration) returns Err
- [X] T008 [US1] Write unit/integration test for `connect_account_oauth2` command in `crates/adagio-desktop/src/commands/account.rs` — test: invalid server URL returns "Server unreachable" error; duplicate account returns "Account already connected" error (can mock HTTP layer; full flow test gated on `ADAGIO_TEST_NC_URL` env var)

### Implementation for User Story 1

- [X] T009 [US1] Implement `pub async fn spawn_callback_listener(expected_state: &str) -> Result<(u16, impl Future<Output = Result<String, SyncError>>), SyncError>` in `crates/adagio-desktop/src/oauth2_callback.rs` — binds `TcpListener` on `127.0.0.1:0`, extracts port, returns `(port, callback_future)` where the future accepts one connection, parses `?code=&state=` from the request line, validates state, sends HTML success response, and resolves with the `code`; wrap with `tokio::time::timeout` (5 min)
- [X] T010 [US1] Implement `connect_account_oauth2` Tauri command in `crates/adagio-desktop/src/commands/account.rs` — full flow: (1) `validate_server_url`, (2) `generate_pkce_pair` + `generate_state`, (3) `spawn_callback_listener`, (4) build auth URL with ephemeral `redirect_uri`, (5) `ShellExt::open` browser, (6) await callback future → `code`, (7) `exchange_code`, (8) `fetch_user_info` → `(username, display_name)`, (9) check duplicate accounts, (10) `store_credentials` via `spawn_blocking`, (11) persist `Account` + `SavedAccount` to `AppState` + `config.json`; return `AccountDto`
- [X] T011 [US1] Register `connect_account_oauth2` command in `crates/adagio-desktop/src/lib.rs` — add to `tauri::Builder` `.invoke_handler` alongside existing commands; also declare `oauth2_callback` module in `src/`
- [X] T012 [US1] Add `connectAccountOAuth2(serverUrl: string): Promise<AccountDto>` binding in `crates/adagio-desktop/src-ui/src/lib/tauri.ts` — `invoke("connect_account_oauth2", { serverUrl })`
- [X] T013 [US1] Update `crates/adagio-desktop/src-ui/src/routes/Onboarding.svelte` — replace placeholder OAuth2 path: (a) add `"oauth2"` auth mode that calls `connectAccountOAuth2(serverUrl)` instead of `addAccount`; (b) while `invoke` is pending show "Waiting for browser — complete login then return here" with a Cancel note; (c) on resolve show the returned account; (d) error state displays the error string from Rust; no manual username/display-name fields needed for OAuth2 path

**Checkpoint**: US1 fully functional — new user can connect an account via browser OAuth2.

---

## Phase 4: User Story 2 — Adding a Second Account (Priority: P2)

**Goal**: A user with one account already saved can add a second account from a different
Nextcloud server; both accounts coexist independently.

**Independent Test**: With one account saved, open account settings, click "Add Account",
complete OAuth2 for a second server URL (or same server, different user). Verify: both
accounts listed; credentials stored under separate keychain entries.

### Tests for User Story 2 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T014 [P] [US2] Write unit test for duplicate detection in `crates/adagio-desktop/src/commands/account.rs` — test: adding an account with the same normalised server_url + username as an existing account returns `Err("Account already connected for this server and username")`; different username on same server succeeds; same username on different server succeeds
- [X] T015 [P] [US2] Write unit test verifying `list_accounts` returns both accounts after two successful connections — can use in-memory `AppState` with two pre-populated accounts

### Implementation for User Story 2

- [X] T016 [US2] Add duplicate detection to `connect_account_oauth2` in `crates/adagio-desktop/src/commands/account.rs` — after `fetch_user_info` resolves, iterate `state.accounts.list()`, normalise URLs (trim trailing slash, lowercase scheme+host), and return `Err("Account already connected for this server and username")` if a match is found (T016 depends on T010)
- [X] T017 [US2] Add "Accounts" section to `crates/adagio-desktop/src-ui/src/routes/Settings.svelte` — on `onMount` call `listAccounts()` and display each account as a row (display name + server URL); add "Add Account" button that opens the same OAuth2 flow state machine as Onboarding (reuse or extract an `OAuth2Flow` component); show loading/error states

**Checkpoint**: US1 + US2 both functional — multiple accounts can coexist.

---

## Phase 5: User Story 3 — Account Removal (Priority: P3)

**Goal**: A user can remove an account; the action is confirmed before executing; removal
clears the account, all its sync pairs, and the OS keychain credential.

**Independent Test**: With one account saved, trigger "Remove Account". Cancel → account
still present. Confirm → account gone from list; no pairs referencing it; no keychain entry.

### Tests for User Story 3 (MANDATORY — Constitution Principle I: Test-First)

> **REQUIRED: Write these tests FIRST, ensure they FAIL before implementation**

- [X] T018 [P] [US3] Write unit test for `remove_account` in `crates/adagio-desktop/src/commands/account.rs` — test: account removed from `AppState`; associated pair entries removed from config; `delete_credentials` called (stub keychain for test); returns `Ok(())`; removing a non-existent ID returns `Ok(())` (idempotent)
- [X] T019 [P] [US3] Write Svelte component test or manual test spec in `specs/003-account-oauth2-setup/checklists/removal-ui.md` — checklist: confirmation dialog appears before delete; clicking Cancel keeps account; clicking Confirm triggers `removeAccount`; success removes row from list; error shows error message

### Implementation for User Story 3

- [X] T020 [US3] Review and verify `remove_account` cascade in `crates/adagio-desktop/src/commands/account.rs` — confirm: `state.accounts.remove(&id)` removes from in-memory state; `cfg.accounts.retain` removes from config; `cfg.pairs.retain` removes associated pairs; `delete_credentials` called via `spawn_blocking`; if already correct, add `tracing::info!` log lines and the test from T018 covers it
- [X] T021 [US3] Add per-account "Remove" button with confirmation dialog in `crates/adagio-desktop/src-ui/src/routes/Settings.svelte` — when Remove clicked, show inline confirmation ("Remove {display_name}? This will also delete all sync pairs for this account.") with Confirm/Cancel; on Confirm call `removeAccount(account.id)`; on success remove row from local list; on error show error message inline

**Checkpoint**: Full account lifecycle implemented — add, list, and remove all functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Constitution compliance, error-message quality, and security verification.

- [X] T022 [P] Add error-message timing test (SC-003): write a test in `crates/adagio-nextcloud/src/auth.rs` that calls `validate_server_url` with an unroutable address and asserts the error is returned within 6 s (5 s timeout + 1 s margin)
- [X] T023 [P] Verify no credentials on disk (SC-002): add a test in `crates/adagio-desktop/src/commands/account.rs` that after `connect_account_oauth2` succeeds, reads `config.json` and asserts it contains no `token`, `password`, or `secret` keys; run `grep -i "token\|password\|secret" config.json` in quickstart verification
- [X] T024 [P] Add `///` doc comments to all new public items: `spawn_callback_listener` in `oauth2_callback.rs`, `validate_server_url` and `fetch_user_info` in `auth.rs`, `connect_account_oauth2` in `account.rs`
- [X] T025 Run `cargo clippy -- -D warnings` and fix all warnings across `adagio-nextcloud` and `adagio-desktop` crates
- [X] T026 [P] Run `cargo fmt --check`; apply `cargo fmt` to fix any formatting issues in all modified files
- [X] T027 Run the quickstart.md end-to-end validation (server probe, browser flow, keychain check, edge cases table) and update quickstart.md if any steps are inaccurate

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup (T001) — BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Phase 2 (T002–T004)
- **User Story 2 (Phase 4)**: Depends on US1 implementation (T010, T016 builds on it)
- **User Story 3 (Phase 5)**: Depends on Phase 2 (T002); `remove_account` already exists
- **Polish (Phase 6)**: Depends on all story phases complete

### User Story Dependencies

- **US1 (P1)**: Can start after Phase 2; no dependency on US2/US3
- **US2 (P2)**: Duplicate detection (T016) must be added after `connect_account_oauth2` exists (T010); Settings UI (T017) is independent of US1 UI
- **US3 (P3)**: `remove_account` already exists; needs UI + test coverage; independent of US1/US2 tests

### Within Each User Story

1. Tests written and FAIL
2. Implementation makes tests pass
3. Polish and logging verified
4. Checkpoint validated before next story

### Parallel Opportunities

- T003 and T004 (foundational) can run in parallel (different functions)
- T005, T006, T007 (US1 tests) can run in parallel (different files)
- T012 and T013 (US1 frontend) can run in parallel after T010 is complete
- T014 and T015 (US2 tests) can run in parallel
- T018 and T019 (US3 tests) can run in parallel
- T022, T023, T024, T026 (polish) can run in parallel

---

## Parallel Example: User Story 1

```bash
# Step 1: Write all tests in parallel (all MUST FAIL initially)
Task T005: unit tests for validate_server_url
Task T006: unit tests for fetch_user_info
Task T007: unit tests for spawn_callback_listener

# Step 2: Implement foundational functions (T003, T004)
Task T003: validate_server_url implementation
Task T004: fetch_user_info implementation

# Step 3: Implement the callback listener (T009)
# Step 4: Implement the Tauri command (T010) — depends on T009
# Step 5: Register + wire frontend (T011, T012, T013 — T012/T013 parallel)
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001 — ADR)
2. Complete Phase 2: Foundational (T002–T004)
3. Write US1 tests (T005–T008) — ensure they FAIL
4. Implement US1 (T009–T013) — make tests pass
5. **STOP and VALIDATE**: Full OAuth2 flow works end-to-end
6. Verify: `config.json` has no tokens; keychain has the credential; account listed

### Incremental Delivery

1. Phase 1 + 2: Foundation ready
2. Phase 3 (US1): First account connection via browser → MVP demo-able
3. Phase 4 (US2): Duplicate detection + Settings "Add Account" button
4. Phase 5 (US3): Account removal with confirmation dialog
5. Phase 6: Polish — clippy, docs, security verification

---

## Notes

- `remove_account` command and `add_account` command already exist — verify correctness, add tests, add tracing
- The loopback listener lives in `adagio-desktop` (not `adagio-nextcloud`) because it needs `tauri::AppHandle` for `ShellExt::open`; keep the HTTP parsing logic minimal
- Keychain access MUST always be wrapped in `spawn_blocking`; never create `keyring::Entry` on an async thread
- `config.json` MUST NOT contain any token, password, or secret — verified by T023
- `client_id` and `client_secret` constants live in `crates/adagio-nextcloud/src/auth.rs`; production values are injected at release time via CI env vars
