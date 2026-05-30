# Tasks: Background Sync Daemon

**Input**: Design documents from `specs/007-background-sync-daemon/`

**Prerequisites**: plan.md ✅ | spec.md ✅ | research.md ✅ | data-model.md ✅ | contracts/ipc-protocol.md ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins.

**Format**: `[ID] [P?] [Story?] Description`
- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[US#]**: User story this task belongs to

---

## Phase 1: Setup

**Purpose**: ADRs and workspace scaffolding. No story can begin without the crate skeletons.

- [x] T001 Create `docs/adr/009-ipc-transport.md` — document the NDJSON-over-Unix-socket decision, why hand-rolled tokio over jsonrpc-core/jsonrpsee, framing spec (newline delimiter), RPC vs. subscription connection types, and how Windows named pipes mirror Unix socket semantics
- [x] T002 [P] Create `docs/adr/010-detached-process-spawn.md` — document why `std::process::Command` is used instead of Tauri sidecar (sidecar auto-kills on parent exit per issue #3062), detached spawn flags per platform, health-check via connection attempt
- [x] T003 Add `adagio-ipc` and `adagio-daemon` to `Cargo.toml` workspace members; create `crates/adagio-ipc/Cargo.toml` (library, deps: tokio full, serde, serde_json, tokio-util) and `crates/adagio-ipc/src/lib.rs` (empty pub mods: types, transport, client)
- [x] T004 [P] Create `crates/adagio-daemon/Cargo.toml` (binary, deps: adagio-core, adagio-ipc, adagio-nextcloud, tokio, serde_json, tracing, tracing-subscriber) and `crates/adagio-daemon/src/main.rs` (stub that compiles: `fn main() {}`)

**Checkpoint**: `cargo check -p adagio-ipc -p adagio-daemon` passes with empty crates.

---

## Phase 2: Foundational — Shared Protocol Types

**Purpose**: `DaemonRequest`, `DaemonResponse`, `DaemonEvent` enums and the platform transport helpers. Every user story depends on these types existing.

**⚠️ CRITICAL**: No user story implementation can compile until these types are defined.

### Tests (write first — must FAIL)

- [x] T005 [P] Write failing Rust unit test `daemon_request_get_status_serialises_to_snake_case` in `crates/adagio-ipc/src/types.rs` test module — assert `serde_json::to_string(&DaemonRequest::GetStatus)` produces `{"method":"get_status","params":{}}`
- [x] T006 [P] Write failing Rust unit test `daemon_event_conflict_detected_deserialises` in `crates/adagio-ipc/src/types.rs` test module — assert `serde_json::from_str::<DaemonEvent>(r#"{"event":"conflict_detected","payload":{"pending_count":2}}"#)` succeeds and produces the expected variant
- [x] T007 [P] Write failing Rust unit test `daemon_socket_path_is_absolute` in `crates/adagio-ipc/src/transport.rs` test module — assert `daemon_socket_path()` returns an absolute `PathBuf`

### Implementation

- [x] T008 [P] Implement `DaemonRequest`, `DaemonResponse`, `DaemonEvent` enums in `crates/adagio-ipc/src/types.rs` — all 22 RPC method variants per `contracts/ipc-protocol.md`; `#[serde(rename_all = "snake_case")]` on all; `///` doc comments on every public item
- [x] T009 [P] Implement `daemon_socket_path() -> PathBuf`, `connect_rpc() -> Result<(BufReader<UnixStream>, BufWriter<UnixStream>)>`, and `connect_subscription()` in `crates/adagio-ipc/src/transport.rs` — Linux: `$XDG_RUNTIME_DIR/adagio/daemon.sock`; macOS: `~/Library/Application Support/adagio/daemon.sock`; Windows: `\\.\pipe\adagio-daemon-<sid>` via `tokio::net::windows::named_pipe`

**Checkpoint**: `cargo test -p adagio-ipc` — T005–T007 pass.

---

## Phase 3: User Story 1 — Sync Continues After the Window Is Closed (Priority: P1) 🎯 MVP

**Goal**: The sync engine runs in a standalone `adagio-daemon` process. Closing the
Tauri GUI window does not stop sync. The Tauri app connects to the running daemon on
startup and forwards all commands over IPC.

**Independent Test**: Close the app window. Use `pgrep adagio-daemon` to confirm daemon
is still running. Trigger a remote file change. Verify the file is downloaded
while the window is closed. Reopen the app — status and activity log reflect what
happened.

### Tests for User Story 1 (write first — must FAIL)

- [x] T010 [P] [US1] Write failing Rust unit test `dispatcher_routes_ping_to_pong` in `crates/adagio-daemon/src/dispatcher.rs` test module — call `dispatch(DaemonRequest::Ping, &mock_state)`, assert `DaemonResponse::Pong { version, uptime_secs }` is returned
- [x] T011 [P] [US1] Write failing Rust unit test `dispatcher_routes_get_status` in `crates/adagio-daemon/src/dispatcher.rs` test module — mock `DefaultSyncEngine` returning `EngineStatus::Idle`, assert `DaemonResponse::Status(...)` returned
- [x] T012 [P] [US1] Write failing Rust unit test `single_instance_exits_when_daemon_already_running` in `crates/adagio-daemon/src/main.rs` test module — bind socket to temp path, attempt second bind, assert error; with live daemon ping, assert second start exits cleanly
- [x] T013 [P] [US1] Write failing Rust unit test `graceful_shutdown_drains_tasks_within_30s` in `crates/adagio-daemon/src/shutdown.rs` test module — spawn 3 mock tasks that sleep 1 s each, call `graceful_shutdown(cancel, tracker, ...)`, assert all complete within 5 s
- [x] T014 [P] [US1] Write failing Rust unit test `app_state_has_daemon_client_not_engine` in `crates/adagio-desktop/src/state.rs` test module — assert `AppState` fields contain `DaemonClient` and NOT `DefaultSyncEngine`
- [x] T015 [P] [US1] Write failing Rust unit test `get_status_command_calls_daemon_request` in `crates/adagio-desktop/src/commands/sync.rs` test module — mock `DaemonClient`, call `get_status` Tauri command, assert `client.request(DaemonRequest::GetStatus)` was called
- [x] T016 [P] [US1] Write failing Rust unit test `resolve_conflict_command_calls_daemon_request` in `crates/adagio-desktop/src/commands/conflicts.rs` test module — mock client, call `resolve_conflict("id", "local")`, assert `DaemonRequest::ResolveConflict { id, side }` was forwarded
- [x] T017 [P] [US1] Write failing Rust unit test `conflict_detected_daemon_event_emits_tauri_event` in `crates/adagio-desktop/src/lib.rs` test module — mock daemon subscription emitting `DaemonEvent::ConflictDetected { pending_count: 3 }`, assert Tauri `adagio://conflict-detected` event is emitted with `{ pending_count: 3 }`

### Implementation for User Story 1

**adagio-daemon** (build the daemon binary):

- [x] T018 [US1] Implement `crates/adagio-daemon/src/events.rs` — `EventBroadcaster { tx: broadcast::Sender<DaemonEvent> }` with `emit_conflict_detected(pending_count)`, `emit_conflict_resolved(id, pending_count)`, `emit_sync_status_changed(status, pair_id)`, `emit_transfer_progress(...)`; `///` doc comments on all public items
- [x] T019 [US1] Implement `crates/adagio-daemon/src/dispatcher.rs` — `dispatch(req: DaemonRequest, state: &DaemonProcess) -> Result<DaemonResponse>` with one match arm per request variant; each arm calls the same engine/journal method the current Tauri command handler calls; `#[instrument]` tracing on the function; `///` doc comments — depends on T018
- [x] T020 [US1] Implement `crates/adagio-daemon/src/server.rs` — `accept_loop(listener, dispatcher, event_tx)` that spawns a task per accepted connection; `handle_rpc_connection(stream, dispatcher)` reads NDJSON lines, calls `dispatcher::dispatch`, writes response; `handle_subscription_connection(stream, event_rx)` forwards `DaemonEvent` to client as NDJSON — depends on T019
- [x] T021 [US1] Implement `crates/adagio-daemon/src/shutdown.rs` — `graceful_shutdown(cancel: CancellationToken, tracker: TaskTracker, journal: Arc<SqliteJournal>)`: cancel token, `timeout(30s, tracker.wait())`, then `journal.checkpoint().await` (new `checkpoint` method or raw SQL `PRAGMA wal_checkpoint(RESTART)`)
- [x] T022 [US1] Implement `crates/adagio-daemon/src/main.rs` — `#[tokio::main]`: (1) compute socket path via `transport::daemon_socket_path()`; (2) single-instance guard (try bind, on error ping existing instance, exit 0 if alive, delete stale socket and retry); (3) load config + accounts + journal + engine (same as current `lifecycle.rs`); (4) spawn `server::accept_loop`; (5) wait for `SIGTERM`/`SIGINT`/`stop_daemon` command; (6) call `shutdown::graceful_shutdown` — depends on T020, T021

**adagio-ipc DaemonClient**:

- [x] T023 [US1] Implement `DaemonClient` in `crates/adagio-ipc/src/client.rs`: `connect_or_start(daemon_binary_path: &Path) -> Result<Self>` — try connect with 300 ms timeout × 3; on failure spawn daemon via `std::process::Command` (detached stdio, `DETACHED_PROCESS` flag on Windows), retry connect for 5 s; `request(DaemonRequest) -> Result<DaemonResponse>` — write NDJSON, await response matched by `id`; `subscribe() -> broadcast::Receiver<DaemonEvent>` — background task drives subscription stream; `///` doc comments — depends on T008, T009

**adagio-desktop refactor**:

- [x] T024 [US1] Rewrite `crates/adagio-desktop/src/state.rs` — `AppState { daemon: Arc<DaemonClient>, config_path: PathBuf }`; `AppState::new(config_dir, daemon_binary_path)` calls `DaemonClient::connect_or_start`; remove `engine`, `journal`, `accounts`, `pairs` fields — depends on T023
- [x] T025 [US1] Rewrite `crates/adagio-desktop/src/lifecycle.rs` — `connect_or_start_daemon(config_dir: &Path) -> Result<DaemonClient>`: determines daemon binary path as sibling of current executable (`current_exe()` → replace binary name); calls `DaemonClient::connect_or_start`; returns connected client — depends on T023
- [x] T026 [US1] Update `crates/adagio-desktop/src/lib.rs` setup — replace `AppState::new(config_dir)?` with `connect_or_start_daemon(config_dir)` + `AppState { daemon, config_path }`; subscribe to daemon events and forward to Tauri (`adagio://conflict-detected`, `adagio://conflict-resolved`, `adagio://sync-status-changed`); forward `ConnectionState::Failed` to frontend as `adagio://daemon-unreachable` event; verify `CloseRequested` handler still hides window (daemon is separate, no change needed) — depends on T024, T025
- [x] T027 [P] [US1] Refactor `crates/adagio-desktop/src/commands/sync.rs` — all commands (`get_status`, `trigger_sync`, `pause_sync`, `resume_sync`, `get_activity_log`, `get_status`, `get_error_items`) become `state.daemon.request(DaemonRequest::Xyz { .. }).await?.try_into()?`; keep existing function signatures identical — depends on T024
- [x] T028 [P] [US1] Refactor `crates/adagio-desktop/src/commands/conflicts.rs` — `list_conflicts`, `resolve_conflict`, `dismiss_all_conflicts` forward to daemon; keep existing signatures — depends on T024
- [x] T029 [P] [US1] Refactor `crates/adagio-desktop/src/commands/pair.rs` — `create_pair`, `delete_pair`, `list_pairs`, `list_synced_files`, `get_exclude_patterns`, `list_remote_tree` forward to daemon — depends on T024
- [x] T030 [P] [US1] Refactor `crates/adagio-desktop/src/commands/account.rs` — `add_account`, `remove_account`, `list_accounts`, `connect_account_oauth2` forward to daemon; `commands/sharing.rs` `search_users`, `create_share` forward to daemon — depends on T024

**Checkpoint**: `cargo test --lib -p adagio-daemon` — T010–T013 pass. `cargo test --lib -p adagio-desktop` — T014–T017 pass + 37 existing tests still pass.

---

## Phase 4: User Story 2 — Sync Starts Automatically at Login (Priority: P2)

**Goal**: `adagio-daemon` registers itself for OS login auto-start during onboarding.
"Start at login" is toggleable from Settings with no window appearing at login.

**Independent Test**: Enable "Start at login" in Settings. Verify the auto-start entry
exists (`~/.config/autostart/adagio-daemon.desktop` / plist / registry key). Disable
it. Verify entry is removed.

### Tests for User Story 2 (write first — must FAIL)

- [x] T031 [P] [US2] Write failing Rust unit test `autostart_enable_creates_desktop_file` in `crates/adagio-daemon/src/autostart.rs` test module (Linux only, `#[cfg(target_os = "linux")]`) — call `set_start_at_login(true, Path::new("/usr/bin/adagio-daemon"))`, assert file created at `~/.config/autostart/adagio-daemon.desktop` with correct `Exec=` line
- [x] T032 [P] [US2] Write failing Rust unit test `autostart_disable_removes_desktop_file` — call enable then `set_start_at_login(false, ...)`, assert file removed
- [x] T033 [P] [US2] Write failing Rust unit test `set_start_at_login_command_calls_daemon` in `crates/adagio-desktop/src/commands/daemon.rs` test module — mock `DaemonClient`, call `set_start_at_login(true)` Tauri command, assert `DaemonRequest::SetStartAtLogin { enabled: true }` was sent

### Implementation for User Story 2

- [x] T034 [US2] Implement `crates/adagio-daemon/src/autostart.rs` — `set_start_at_login(enabled: bool, daemon_path: &Path) -> Result<()>` and `is_start_at_login_enabled() -> bool` with three `#[cfg]` branches: Linux writes/deletes XDG `.desktop` file; macOS writes/deletes LaunchAgent plist + shells out to `launchctl load/unload`; Windows writes/deletes HKCU Run value via `winreg` crate; `///` doc comments; wire `DaemonRequest::SetStartAtLogin` arm in `dispatcher.rs`
- [x] T035 [US2] Add `winreg = "0.52"` to `[target.'cfg(windows)'.dependencies]` in `crates/adagio-daemon/Cargo.toml`
- [x] T036 [US2] Create `crates/adagio-desktop/src/commands/daemon.rs` — Tauri commands: `start_daemon()`, `stop_daemon()`, `get_daemon_status() -> DaemonStatusDto`, `set_start_at_login(enabled: bool)`; register all four in `lib.rs` `invoke_handler!`; `DaemonStatusDto { running: bool, uptime_secs: Option<u64>, connection_state: String }` — depends on T024
- [x] T037 [P] [US2] Add "Background sync" section to `crates/adagio-desktop/src-ui/src/components/SettingsScene.tsx` — "Start at login" toggle (`onToggleStartAtLogin` prop → calls `set_start_at_login`); reads initial state from `get_daemon_status()`
- [x] T038 [P] [US2] Add `setStartAtLogin`, `getDaemonStatus`, `startDaemon`, `stopDaemon` to `crates/adagio-desktop/src-ui/src/tauri.ts` — type-safe wrappers matching current convention

**Checkpoint**: Settings shows toggle. Enable/disable creates/deletes platform entry.

---

## Phase 5: User Story 3 — App Opens Instantly by Connecting to Running Daemon (Priority: P3)

**Goal**: If the daemon is already running when the app opens, the app connects in
< 1 s without spawning a second daemon and without a loading spinner.

**Independent Test**: Start daemon standalone. Open app. Measure time to live data.
Open a second app instance — confirm only one daemon process.

### Tests for User Story 3 (write first — must FAIL)

- [x] T039 [P] [US3] Write failing Rust unit test `client_connect_skips_spawn_when_daemon_running` in `crates/adagio-ipc/src/client.rs` test module — start a mock server, call `DaemonClient::connect_or_start` pointing to a fake binary path, assert the binary was NOT spawned (child spawn count = 0)
- [x] T040 [P] [US3] Write failing Rust unit test `connect_returns_within_1s_when_daemon_running` in `crates/adagio-ipc/src/client.rs` test module — start mock server, time `connect_or_start`, assert elapsed < 1 s

### Implementation for User Story 3

- [x] T041 [US3] Harden `DaemonClient::connect_or_start` in `crates/adagio-ipc/src/client.rs` — first attempt: connect with 300 ms timeout; if succeeds, return immediately (no spawn); if fails, spawn + retry; ensure the "no spawn" fast path is tested and confirmed ≤ 300 ms — depends on T023
- [x] T042 [P] [US3] Add `adagio://daemon-connection-state` Tauri event emission in `crates/adagio-desktop/src/lib.rs` — emit `{ state: "connected" \| "reconnecting" \| "failed" }` whenever `ConnectionState` watch fires; frontend can show spinner or error without blocking the rest of the UI

**Checkpoint**: App connects to running daemon in < 1 s. No duplicate daemon process.

---

## Phase 6: User Story 4 — Start and Stop Background Sync from Settings (Priority: P4)

**Goal**: Settings panel exposes explicit stop/start controls for the daemon.
Stopped state persists across window close/open.

**Independent Test**: Click "Stop background sync". Confirm `pgrep adagio-daemon` empty.
Close and reopen app — still stopped. Click "Start". Confirm daemon running within 3 s.

### Tests for User Story 4 (write first — must FAIL)

- [x] T043 [P] [US4] Write failing Rust unit test `stop_daemon_command_forwards_stop_daemon_request` in `crates/adagio-desktop/src/commands/daemon.rs` — mock client, call `stop_daemon()`, assert `DaemonRequest::StopDaemon` sent
- [x] T044 [P] [US4] Write failing Rust unit test `start_daemon_command_spawns_and_connects` in `crates/adagio-desktop/src/commands/daemon.rs` — mock daemon binary (script that listens on socket), call `start_daemon()`, assert client is connected after call
- [x] T045 [P] [US4] Write failing React test `SettingsScene renders stop background sync button when daemon running` in `crates/adagio-desktop/src-ui/src/__tests__/SettingsScene.test.tsx` — mock `getDaemonStatus()` returning `{ running: true }`, assert "Stop background sync" button rendered
- [x] T046 [P] [US4] Write failing React test `SettingsScene renders start background sync button when daemon stopped` — mock `{ running: false }`, assert "Start background sync" button rendered

### Implementation for User Story 4

- [x] T047 [US4] Complete `stop_daemon()` Tauri command in `crates/adagio-desktop/src/commands/daemon.rs` — sends `DaemonRequest::StopDaemon`, transitions app `ConnectionState` to `Stopped` (distinct from `Failed` — user-initiated stop should NOT trigger auto-restart); `start_daemon()` calls `DaemonClient::connect_or_start`, transitions state back to `Connected` — depends on T036
- [x] T048 [P] [US4] Update `crates/adagio-desktop/src-ui/src/components/SettingsScene.tsx` — wire "Stop background sync" / "Start background sync" buttons to `stop_daemon()` / `start_daemon()` Tauri commands; show spinner while transitioning; show daemon uptime when running — depends on T037

**Checkpoint**: Stop/start cycle tested; stopped state does not auto-restart on window open.

---

## Phase 7: User Story 5 — App Recovers Automatically If Daemon Crashes (Priority: P5)

**Goal**: Unplanned daemon termination is detected within 3 s. The app auto-restarts the
daemon up to 3 times. On 3 consecutive failures, shows an error with a manual button.

**Independent Test**: Kill daemon with `kill -9`. Observe "Reconnecting…" indicator.
Confirm daemon is restarted within 10 s. Then prevent restarts and confirm error UI.

### Tests for User Story 5 (write first — must FAIL)

- [x] T049 [P] [US5] Write failing Rust unit test `client_detects_disconnect_within_3s` in `crates/adagio-ipc/src/client.rs` — start mock server, drop it, assert client `ConnectionState` transitions to `Reconnecting` within 3 s
- [x] T050 [P] [US5] Write failing Rust unit test `client_transitions_to_failed_after_3_consecutive_failures` in `crates/adagio-ipc/src/client.rs` — mock `connect_or_start` always returning error, drive reconnect loop, assert state is `Failed` after 3 attempts
- [x] T051 [P] [US5] Write failing Rust unit test `user_initiated_stop_does_not_trigger_auto_restart` in `crates/adagio-ipc/src/client.rs` — set `ConnectionState` to `Stopped` (user-initiated), then simulate disconnect; assert reconnect loop is NOT triggered
- [x] T052 [P] [US5] Write failing React test `App shows reconnecting indicator when connection state is reconnecting` in `crates/adagio-desktop/src-ui/src/__tests__/App.test.tsx` — mock `adagio://daemon-connection-state` event with `{ state: "reconnecting" }`, assert "Reconnecting…" element visible
- [x] T053 [P] [US5] Write failing React test `App shows error and restart button when connection state is failed` — mock `{ state: "failed" }`, assert error message + "Restart sync" button visible

### Implementation for User Story 5

- [x] T054 [US5] Implement reconnection loop in `DaemonClient` in `crates/adagio-ipc/src/client.rs` — background heartbeat pings every 5 s; on ping failure transitions to `Reconnecting { attempt: 1 }`; calls `connect_or_start` up to 3 times with 1 s between attempts; on success → `Connected`; after 3 failures → `Failed`; if current state is `Stopped` (user-initiated) skip reconnect loop entirely — depends on T023
- [x] T055 [US5] Propagate `ConnectionState` to frontend in `crates/adagio-desktop/src/lib.rs` — watch `DaemonClient::connection_state()`, emit `adagio://daemon-connection-state` Tauri event on each transition; `Stopped` → `{ state: "stopped" }`, `Reconnecting` → `{ state: "reconnecting", attempt: n }`, `Failed` → `{ state: "failed" }`, `Connected` → `{ state: "connected" }` — depends on T026
- [x] T056 [US5] Add reconnection UI to `crates/adagio-desktop/src-ui/src/App.tsx` — subscribe to `adagio://daemon-connection-state` event; show a dismissable banner when `reconnecting` (does not block UI); show a full overlay with "Restart sync" button when `failed`; "Restart sync" calls `startDaemon()` Tauri command — depends on T055
- [x] T057 [P] [US5] Add CSS / style for reconnection banner and error overlay in `crates/adagio-desktop/src-ui/src/app.css` — uses existing design tokens (var(--ink), var(--cream), var(--clay) for error)

**Checkpoint**: All US5 tests pass. kill -9 on daemon → "Reconnecting…" → auto-restart within 10 s.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [x] T058 `cargo clippy -- -D warnings` — fix all warnings across `adagio-ipc`, `adagio-daemon`, `adagio-core`, `adagio-desktop`; zero suppressions
- [x] T059 [P] `cargo fmt --all --check` — run `cargo fmt --all` to fix any formatting drift
- [x] T060 `cargo test --lib -p adagio-core` — all 133 existing core tests still pass (no regressions)
- [x] T061 [P] `cargo test --lib -p adagio-ipc` — all new IPC type + client tests pass
- [x] T062 [P] `cargo test --lib -p adagio-daemon` — all dispatcher + shutdown + autostart tests pass
- [x] T063 `cargo test --lib -p adagio-desktop` — all 37 existing tests + new command-forwarding tests pass
- [x] T064 [P] `cd crates/adagio-desktop/src-ui && npm run test` — all React tests pass (91 existing + new reconnection UI tests)
- [x] T065 Manual quickstart.md validation — step through all 5 user stories per `specs/007-background-sync-daemon/quickstart.md`; update quickstart if any step is inaccurate

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user story phases
- **Phase 3 (US1)**: Depends on Phase 2 — core extraction; MUST complete before US2, US4
- **Phase 4 (US2)**: Depends on Phase 3 (dispatcher and DaemonClient must exist)
- **Phase 5 (US3)**: Depends on Phase 2 (DaemonClient types) — can begin in parallel with Phase 3 after T023
- **Phase 6 (US4)**: Depends on Phase 3 (commands/daemon.rs uses AppState with DaemonClient)
- **Phase 7 (US5)**: Depends on Phase 5 (reconnect loop is an extension of DaemonClient)
- **Phase 8 (Polish)**: Depends on all prior phases

### Within Phase 3 (US1)

- All test tasks (T010–T017) are [P] — write simultaneously before any implementation
- T018 (events.rs) before T019 (dispatcher.rs depends on EventBroadcaster)
- T019 (dispatcher.rs) before T020 (server.rs calls dispatcher)
- T020, T021 before T022 (main.rs wires server + shutdown)
- T023 (DaemonClient) before T024 (AppState uses DaemonClient)
- T024 before T025, T026
- T027–T030 are [P] with each other (different command files, all depend on T024)

### Parallel Opportunities

Within Phase 2: T005–T007 (tests), T008–T009 (impl) — all touch different files.
Within Phase 3 tests: T010–T017 — all different files, all parallel.
Within Phase 3 impl daemon side: T018 → T019 → T020, then T021 independently.
Within Phase 3 impl desktop side: T023 → T024 → T025, T026; then T027–T030 all parallel.
Phases 4 and 5 can begin in parallel once Phase 3 is complete.

---

## Parallel Example: Phase 3 Test Sprint

```
# All US1 tests can be written simultaneously — different test modules:
T010: dispatcher_routes_ping_to_pong         → crates/adagio-daemon/src/dispatcher.rs
T011: dispatcher_routes_get_status           → crates/adagio-daemon/src/dispatcher.rs
T012: single_instance_exits_when_running     → crates/adagio-daemon/src/main.rs
T013: graceful_shutdown_drains_within_30s    → crates/adagio-daemon/src/shutdown.rs
T014: app_state_has_daemon_client_not_engine → crates/adagio-desktop/src/state.rs
T015: get_status_command_calls_daemon        → crates/adagio-desktop/src/commands/sync.rs
T016: resolve_conflict_command_calls_daemon  → crates/adagio-desktop/src/commands/conflicts.rs
T017: conflict_event_forwarded_to_tauri      → crates/adagio-desktop/src/lib.rs
```

---

## Implementation Strategy

### MVP (User Story 1 only — daemon running + desktop refactored)

1. Phase 1 → Phase 2 → Phase 3
2. **STOP and VALIDATE**: `pgrep adagio-daemon` alive after window close; all
   Tauri commands work through IPC; 37 existing desktop tests still pass
3. Manual quickstart US1 steps

### Full Delivery (all 5 user stories)

1. MVP above
2. Phase 4 (US2): auto-start + settings toggle
3. Phase 5 (US3): instant reconnect < 1 s verified
4. Phase 6 (US4): stop/start from Settings
5. Phase 7 (US5): crash recovery + reconnection UI
6. Phase 8: polish + full quickstart validation

---

## Notes

- [P] tasks touch different files — launch together for speed
- Constitution Principle I is non-negotiable: every test task MUST be written and
  confirmed failing before its paired implementation begins
- T027–T030 are the largest parallel batch — 4 command files, each ~100 lines of
  near-mechanical forwarding; ideal for parallel implementation
- `commands/prefs.rs` (`get_palette`, `set_palette`) is explicitly NOT refactored —
  it reads/writes local config and has no daemon dependency
- The Tauri `CloseRequested` handler (hide window, prevent close) requires NO change —
  the daemon is a separate process that was never tied to window lifecycle
- Total: **65 tasks** across 8 phases
