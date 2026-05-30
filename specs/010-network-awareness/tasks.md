# Tasks: Network Awareness

**Input**: Design documents from `specs/010-network-awareness/`

**Prerequisites**: plan.md ✅ | spec.md ✅ | research.md ✅ | data-model.md ✅ | contracts/network-ipc.md ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins.

**Format**: `[ID] [P?] [Story?] Description`
- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[US#]**: User story this task belongs to

---

## Phase 1: Setup

**Purpose**: ADR, module skeleton, and new cargo dependencies.

- [X] T001 Create `docs/adr/014-network-awareness-detection.md` — document decision to use `zbus` (Linux D-Bus), `battery` crate (cross-platform power), `wifi_scan` crate (SSID), and 3-second polling fallback for macOS/Windows; note supersession of `is_metered_connection()` and `battery_level_percent()` stubs
- [X] T002 Create `crates/adagio-core/src/network/` directory with empty module files: `mod.rs`, `detector.rs`, `monitor.rs`, `linux.rs`, `macos.rs`, `windows.rs`, `fallback.rs` — each file just declares the module; add `pub mod network;` to `crates/adagio-core/src/lib.rs`
- [X] T003 [P] Add new dependencies to `crates/adagio-core/Cargo.toml`: `zbus = { version = "4", features = ["tokio"], optional = true }` under `[target.'cfg(target_os = "linux")'.dependencies]`; `battery = "0.7"` and `wifi_scan = "0.4"` under `[dependencies]`

**Checkpoint**: Module tree compiles empty. Phases 2+ can proceed.

---

## Phase 2: Foundational — Types, Traits, Config

**Purpose**: `NetworkPolicy`, `NetworkAction`, `NetworkState` types and `NetworkDetector`
trait. Every user story phase depends on these. Config persistence and daemon wiring also
live here so US1 can integrate immediately.

**⚠️ CRITICAL**: No story implementation can compile until these types exist.

### Tests (write first — must FAIL)

- [X] T004 [P] Write failing unit tests `policy_default_is_all_allow`, `effective_action_blocked_ssid_always_wins`, `effective_action_pause_beats_throttle`, `effective_action_allow_when_no_conditions_match` in `crates/adagio-core/src/network/mod.rs` test module — assert default policy has on_metered=Allow, on_battery=Allow, empty blocked_ssids; assert composition rules
- [X] T005 [P] Write failing unit test `mock_detector_reports_injected_state` in `crates/adagio-core/src/network/detector.rs` test module — create MockNetworkDetector with metered=true, assert is_metered() returns true

### Implementation

- [X] T006 Implement `NetworkAction` enum (Allow/Throttle/Pause, serde lowercase), `NetworkPolicy` struct with `#[serde(default)]` on all fields, `NetworkState` struct, and `derive_effective_action(policy, state) -> EffectiveAction` fn in `crates/adagio-core/src/network/mod.rs`
- [X] T007 [P] Implement `NetworkDetector` trait (`is_metered`, `is_on_battery`, `current_ssid`) + `MockNetworkDetector { metered, on_battery, ssid }` with injectable state in `crates/adagio-core/src/network/detector.rs`
- [X] T008 [P] Add `network_policy: NetworkPolicy` field with `#[serde(default)]` to `SavedConfig` in `crates/adagio-desktop/src/config/mod.rs`; update `restore_accounts()` / `restore()` call sites in `crates/adagio-daemon/src/main.rs` to load and hold `NetworkPolicy`
- [X] T009 Create `crates/adagio-daemon/src/network_monitor.rs` — `spawn_network_monitor(engine: Arc<DefaultSyncEngine>, policy: Arc<RwLock<NetworkPolicy>>, token: CancellationToken)` that starts a `tokio::time::interval(3s)` poll loop using the platform detector; wire into `TaskTracker` in `crates/adagio-daemon/src/main.rs` alongside the existing server task

**Checkpoint**: `cargo build` clean. Policy types, trait, mock, and config field all compile.

---

## Phase 3: User Story 1 — Metered Connection Policy (Priority: P1) 🎯 MVP

**Goal**: Detect metered network connections on all three platforms and enforce
`on_metered: pause | throttle | allow` policy via the running `NetworkMonitor` task.

**Independent Test**: Set `on_metered: pause`, simulate metered=true via MockNetworkDetector,
assert `engine.pause()` is called within one poll tick.

### Tests for US1 (write first — must FAIL)

- [X] T010 [P] [US1] Write failing unit tests `monitor_pauses_engine_on_metered_pause_policy` and `monitor_throttles_engine_on_metered_throttle_policy` in `crates/adagio-core/src/network/monitor.rs` test module — use MockNetworkDetector + MockSyncEngine; assert engine state after one poll
- [X] T011 [P] [US1] Write failing unit test `monitor_deduplicates_state_calls` in `crates/adagio-core/src/network/monitor.rs` — assert engine.pause() is called once, not on every tick, when state is unchanged
- [X] T012 [P] [US1] Write failing unit test `ipc_get_network_status_includes_metered_and_policy` in `crates/adagio-daemon/src/dispatcher.rs` test module — dispatch GetNetworkStatus, assert response JSON contains `metered`, `effective_action`, `policy.on_metered`
- [X] T013 [P] [US1] Write failing CLI parse tests `cli_parses_network_status` and `cli_parses_network_set_on_metered_pause` in `crates/adagio-cli/src/handlers/network.rs` test module

### Implementation for US1

- [X] T014 [US1] Implement Linux metered detector in `crates/adagio-core/src/network/linux.rs` — query `org.freedesktop.NetworkManager` active device `Metered` property via `zbus`; return true for values 2 (yes) and 3 (yes-guessed); catch all `zbus::Error` and return false (graceful degradation)
- [X] T015 [P] [US1] Implement macOS metered stub (always returns false — no public API) in `crates/adagio-core/src/network/macos.rs` with `#[cfg(target_os = "macos")]`
- [X] T016 [P] [US1] Implement Windows metered detector in `crates/adagio-core/src/network/windows.rs` — `INetworkListManager` + `INetworkConnectionCost::GetCost()`; check for `NLM_CONNECTION_COST_FIXED | VARIABLE | ROAMING` flags via `windows-rs`; return false on any error
- [X] T017 [P] [US1] Implement safe fallback detector (always returns false/None) in `crates/adagio-core/src/network/fallback.rs`
- [X] T018 [US1] Implement `NetworkMonitor::new()` + `run()` loop in `crates/adagio-core/src/network/monitor.rs` — holds `Arc<dyn NetworkDetector>`, `Arc<RwLock<NetworkPolicy>>`, `Arc<DefaultSyncEngine>`, cached `EffectiveAction`; on effective action change: `Pause` → `engine.pause()`, `Throttle(kbps)` → `update_bandwidth_limits` for all accounts then resume if paused, `Allow` → clear throttle + `engine.resume()`; skip `resume()` if user manually paused (track via a `bool` flag)
- [X] T019 [US1] Add `GetNetworkStatus` and `SetNetworkPolicy { on_metered, on_battery, throttle_kbps }` (all fields `Option<…>`) variants to `DaemonRequest` + matching responses in `crates/adagio-ipc/src/types.rs`
- [X] T020 [US1] Handle `GetNetworkStatus` and `SetNetworkPolicy` in `dispatch()` in `crates/adagio-daemon/src/dispatcher.rs`; `SetNetworkPolicy` persists to `config.json` and updates the shared `Arc<RwLock<NetworkPolicy>>`; `GetNetworkStatus` reads live `NetworkState` from monitor + stored policy and returns JSON matching `NetworkStatusDto`
- [X] T021 [P] [US1] Add `DaemonProcess.network_policy: Arc<RwLock<NetworkPolicy>>` and `DaemonProcess.network_state: Arc<Mutex<NetworkState>>` fields to `crates/adagio-daemon/src/dispatcher.rs`; initialize in `crates/adagio-daemon/src/main.rs`
- [X] T022 [US1] Add `Commands::Network { command: NetworkCommand }` + `NetworkCommand { Status, Set { on_metered, on_battery, throttle_kbps }, BlockSsid { ssid }, UnblockSsid { ssid }, ListBlocked }` to `crates/adagio-cli/src/cli.rs` with `///` doc comments and `#[arg(long)]` / `#[arg(value_parser)]` annotations
- [X] T023 [P] [US1] Create `crates/adagio-cli/src/handlers/network.rs` — `run_network()` dispatcher + `run_network_status()` + `run_network_set()` (human table output + JSON); add `pub mod network` to `crates/adagio-cli/src/handlers/mod.rs`; wire `Commands::Network` arm in `crates/adagio-cli/src/run.rs`

**Checkpoint**: `cargo test -p adagio-core -p adagio-daemon -p adagio-cli` — US1 tests pass. `adagio network status` and `adagio network set --on-metered pause` work end-to-end.

---

## Phase 4: User Story 2 — Battery/Power Policy (Priority: P2)

**Goal**: Detect battery vs AC state on all three platforms and enforce `on_battery` policy.

**Independent Test**: Set `on_battery: pause`, simulate on_battery=true via MockNetworkDetector,
assert engine pauses; simulate AC restored, assert engine resumes.

### Tests for US2 (write first — must FAIL)

- [X] T024 [P] [US2] Write failing unit tests `monitor_pauses_engine_on_battery_pause_policy` and `monitor_throttles_engine_on_battery_throttle_policy` in `crates/adagio-core/src/network/monitor.rs` test module
- [X] T025 [P] [US2] Write failing unit test `monitor_resumes_when_ac_restored` in monitor.rs — simulate battery=true then false, assert resume() called

### Implementation for US2

- [X] T026 [US2] Implement battery detection in `crates/adagio-core/src/network/linux.rs` — query UPower D-Bus `/org/freedesktop/UPower/devices/DisplayDevice` `State` property; State=2 (discharging) → on_battery=true; fallback to existing sysfs `/sys/class/power_supply/*/status` if D-Bus fails
- [X] T027 [P] [US2] Implement battery detection in `crates/adagio-core/src/network/macos.rs` using `battery` crate — `Manager::new()?.batteries()?.next()` → check `State::Discharging`; return false on any error
- [X] T028 [P] [US2] Implement battery detection in `crates/adagio-core/src/network/windows.rs` using `battery` crate — same pattern as macOS; return false on error
- [X] T029 [US2] Extend `NetworkMonitor::run()` in `crates/adagio-core/src/network/monitor.rs` to incorporate battery state into `derive_effective_action` call (already wired in data model — both metered and battery feed into composition); add `is_on_battery()` call to the poll loop
- [X] T030 [P] [US2] Remove `SyncConfig.pause_on_metered` and `SyncConfig.min_battery_percent` fields and `should_suspend()` method from `crates/adagio-core/src/cycle/mod.rs` — these are now superseded by `NetworkMonitor`; update all tests referencing them

**Checkpoint**: Battery state transitions trigger pause/throttle/allow correctly.

---

## Phase 5: User Story 3 — SSID Block List (Priority: P3)

**Goal**: Detect the current Wi-Fi SSID and pause sync whenever the device connects to a
blocked network name. SSID block list persists in config.

**Independent Test**: Add "TestNet" to block list, simulate ssid="TestNet" via MockNetworkDetector,
assert engine.pause() called. Simulate ssid="Other", assert engine.resume() called.

### Tests for US3 (write first — must FAIL)

- [X] T031 [P] [US3] Write failing unit tests `monitor_pauses_on_blocked_ssid_connection` and `monitor_resumes_on_blocked_ssid_disconnect` in `crates/adagio-core/src/network/monitor.rs` test module
- [X] T032 [P] [US3] Write failing CLI parse tests `cli_parses_network_block_ssid`, `cli_parses_network_unblock_ssid`, `cli_parses_network_list_blocked` in `crates/adagio-cli/src/handlers/network.rs` test module

### Implementation for US3

- [X] T033 [US3] Implement SSID detection in `crates/adagio-core/src/network/linux.rs` — query NM `ActiveConnection` → `SpecificObject` (access point path) → `org.freedesktop.NetworkManager.AccessPoint` `Ssid` byte array → UTF-8; return None on any error
- [X] T034 [P] [US3] Implement SSID detection in `crates/adagio-core/src/network/macos.rs` using `wifi_scan` — `Station::associated()` → `ssid`; return None on Location Services denial or any error
- [X] T035 [P] [US3] Implement SSID detection in `crates/adagio-core/src/network/windows.rs` using `wifi_scan` — `Station::associated()` → `ssid`; return None on error
- [X] T036 [US3] Extend `NetworkMonitor::run()` poll loop in `crates/adagio-core/src/network/monitor.rs` to call `current_ssid()` and check against `policy.blocked_ssids` before calling `derive_effective_action` — blocked SSID short-circuits to `Pause` regardless of other conditions
- [X] T037 [US3] Add `ManageBlockedSsid { action: String, ssid: Option<String> }` variant to `DaemonRequest` in `crates/adagio-ipc/src/types.rs` (action: `"add"` | `"remove"` | `"list"`)
- [X] T038 [US3] Handle `ManageBlockedSsid` in `dispatch()` in `crates/adagio-daemon/src/dispatcher.rs` — add/remove entries in `network_policy.blocked_ssids`, deduplicate, persist to `config.json`, update shared policy
- [X] T039 [P] [US3] Implement `run_network_block_ssid`, `run_network_unblock_ssid`, `run_network_list_blocked` in `crates/adagio-cli/src/handlers/network.rs`; human output prints SSID list one per line; JSON returns array

**Checkpoint**: Block/unblock persists across daemon restarts; sync pauses within 5s of connecting to blocked SSID.

---

## Phase 6: User Story 4 — Live Status Display (Priority: P4)

**Goal**: `adagio network status` shows a complete snapshot: SSID, metered flag, power
source, effective action + reason. CLI table and JSON mode both complete.

**Independent Test**: Run `adagio network status --json` against a running daemon; assert
all required fields are present and correctly typed.

### Tests for US4 (write first — must FAIL)

- [X] T040 [P] [US4] Write failing unit test `get_network_status_response_contains_all_required_fields` in `crates/adagio-daemon/src/dispatcher.rs` test module — dispatch `GetNetworkStatus`, assert response contains `metered`, `on_battery`, `ssid`, `effective_action`, `throttle_kbps`, `reason`, `policy.*`

### Implementation for US4

- [X] T041 [US4] Implement complete `GetNetworkStatus` response body in `crates/adagio-daemon/src/dispatcher.rs` — read `NetworkState` from `DaemonProcess.network_state` mutex + policy; return full `NetworkStatusDto` JSON object
- [X] T042 [P] [US4] Implement human-readable table output for `run_network_status` in `crates/adagio-cli/src/handlers/network.rs` — 3-column table: CONDITION / STATE / POLICY with rows for Metered, Battery, SSID; summary line "Effective action: …"

**Checkpoint**: `adagio network status` and `adagio network status --json` both return complete, accurate snapshots.

---

## Phase 7: User Story 5 — Settings Panel (Priority: P5)

**Goal**: Settings → Sync → Network in the desktop app: policy dropdowns, throttle input,
SSID block list manager, live 3-second status refresh.

**Independent Test**: Navigate to Settings → Sync → Network; verify dropdowns, inputs, and
live status panel render; save a policy; verify `adagio network status` reflects the change.

### Tests for US5 (write first — must FAIL)

- [X] T043 [P] [US5] Write failing React test `SettingsScene renders network section with metered and battery selectors` in `crates/adagio-desktop/src-ui/src/__tests__/SettingsScene.test.tsx` — navigate to Sync tab, assert Network heading and dropdowns exist
- [X] T044 [P] [US5] Write failing React test `network save calls setNetworkPolicy with correct values` in `SettingsScene.test.tsx` — select "Pause" for metered, enter 300 for throttle, click Save; assert `invoke('set_network_policy', { onMetered: 'pause', throttleKbps: 300 })` called
- [X] T045 [P] [US5] Write failing React test `network block ssid adds to list` in `SettingsScene.test.tsx` — enter "TestNet", click Add; assert `invoke('add_blocked_ssid', { ssid: 'TestNet' })` called

### Implementation for US5

- [X] T046 [US5] Add `NetworkStatusDto`, `NetworkPolicyDto` interfaces + `getNetworkStatus()`, `setNetworkPolicy()`, `addBlockedSsid()`, `removeBlockedSsid()`, `listBlockedSsids()` bindings to `crates/adagio-desktop/src-ui/src/tauri.ts`
- [X] T047 [US5] Create `crates/adagio-desktop/src/commands/network.rs` — 5 Tauri command handlers (`get_network_status`, `set_network_policy`, `add_blocked_ssid`, `remove_blocked_ssid`, `list_blocked_ssids`), each forwarding to the corresponding `DaemonRequest`
- [X] T048 [P] [US5] Add `pub mod network` to `crates/adagio-desktop/src/commands/mod.rs`; register all 5 new commands in the `invoke_handler!` macro in `crates/adagio-desktop/src/lib.rs`
- [X] T049 [US5] Add Network subsection to the Sync tab in `crates/adagio-desktop/src-ui/src/components/SettingsScene.tsx`:
  - On-metered `<select>` (Allow / Throttle / Pause)
  - On-battery `<select>` (Allow / Throttle / Pause)
  - Throttle Kbps `<input type="number">` (shown only when either select is Throttle)
  - SSID block list with text input + Add button + removable chips
  - Save button → calls `setNetworkPolicy`
  - Live status panel (metered, battery, SSID, effective action + reason) refreshed every 3s via `setInterval` while Network section is visible
- [X] T050 [P] [US5] Add mock entries for all 5 new network commands to `crates/adagio-desktop/src-ui/src/__tests__/mocks/tauri.ts` (return sensible defaults)

**Checkpoint**: Settings panel renders; save persists; live status updates every 3s.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T051 `cargo clippy --workspace -- -D warnings` — fix all warnings in network module and new files
- [X] T052 [P] `cargo fmt --all -- --check` — run `cargo fmt --all` to fix any drift
- [X] T053 `cargo test --workspace` — all tests pass (139+ existing core + new network tests)
- [X] T054 [P] `npm run test` — all UI tests pass (99+ existing + new network panel tests)
- [X] T055 [P] Verify all new public Rust items in `crates/adagio-core/src/network/` have `///` doc comments
- [X] T056 Run manual validation per `specs/010-network-awareness/quickstart.md` — step through US1–US5 validation scenarios

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all story phases
- **Phase 3 (US1)**: Depends on Phase 2 — MVP; implement first
- **Phase 4 (US2)**: Depends on Phase 2 + Phase 3 (monitor struct already exists)
- **Phase 5 (US3)**: Depends on Phase 2 + Phase 3 (monitor + IPC patterns established)
- **Phase 6 (US4)**: Depends on Phase 3–5 (status must include all three condition types)
- **Phase 7 (US5)**: Depends on Phase 3–6 (all IPC variants must exist)
- **Phase 8 (Polish)**: Depends on all prior phases

### Within Phase 3 (US1)

- T010–T013 (tests) are [P] — write before any implementation
- T014, T015, T016, T017 are [P] — platform detectors touch different files
- T018 after T014–T017 (monitor needs detectors)
- T019 before T020, T021 (dispatcher needs IPC types)
- T022, T023 [P] after T019 (CLI needs IPC types)

### Parallel Opportunities

**Phase 2**: T004, T005, T007, T008 all different files — [P].
**Phase 3 tests**: T010–T013 all different modules — [P].
**Phase 3 platform detectors**: T014–T017 all different `cfg` files — [P].
**Phase 4**: T024, T025 tests [P]; T027, T028 battery impls [P].
**Phase 5**: T031, T032 tests [P]; T034, T035 SSID impls [P].
**Phase 7**: T043–T045 React tests [P]; T048, T050 wiring [P].

---

## Implementation Strategy

### MVP (Phase 1–3, US1 only)

1. Complete Phase 1 (Setup) + Phase 2 (Foundational)
2. Complete Phase 3 (US1): metered detection + pause/throttle policy
3. **STOP AND VALIDATE**: `adagio network set --on-metered pause` + `adagio network status`
4. At this point, the feature is useful for the most common case (mobile hotspot protection)

### Incremental Delivery

1. Setup + Foundational → types compile, config persists
2. US1 → metered protection works on all platforms
3. US2 → battery drain protection works
4. US3 → SSID block list works
5. US4 → status display complete
6. US5 → full UI controls available
7. Each story adds protection without breaking previous stories

---

## Notes

- `MockNetworkDetector` in `detector.rs` is the key test accelerator — inject any state without real platform APIs
- The 3-second poll interval meets the 5-second reaction requirement on all platforms
- On Linux, prefer zbus signal subscription in production but fall back to polling if signal subscription fails (network restarts can cause D-Bus disconnects)
- `cargo test --workspace` will run all network tests headlessly; no real NM/UPower/battery required
- Total: **56 tasks** across 8 phases
