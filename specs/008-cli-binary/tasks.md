# Tasks: CLI Binary

**Input**: Design documents from `specs/008-cli-binary/`

**Prerequisites**: plan.md ✅ | spec.md ✅ | research.md ✅ | data-model.md ✅ | contracts/cli-commands.md ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins.

**Format**: `[ID] [P?] [Story?] Description`
- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[US#]**: User story this task belongs to

---

## Phase 1: Setup

**Purpose**: ADRs, workspace changes, new crate skeleton, `platform_config_dir()` migration.
All subsequent phases depend on this.

- [x] T001 Create `docs/adr/011-no-table-crate.md` — document decision to use hand-rolled `format!` column alignment instead of `tabled`/`comfy-table`; state rationale (data already as `serde_json::Value`, 3-column tables, zero binary overhead)
- [x] T002 [P] Create `docs/adr/012-platform-config-dir-migration.md` — document decision to move `platform_config_dir()` from `adagio-daemon/src/main.rs` to `adagio-ipc/src/transport.rs`; state why both CLI and daemon need it
- [x] T003 Add `clap = { version = "4.5", features = ["derive"] }` to `[workspace.dependencies]` in `Cargo.toml`
- [x] T004 Add `adagio-cli` to `[workspace.members]` in `Cargo.toml`; create `crates/adagio-cli/Cargo.toml` (bin, deps: adagio-ipc, clap, tokio full, serde_json, anyhow) and stub `crates/adagio-cli/src/main.rs` that prints "TODO" and compiles
- [x] T005 Migrate `platform_config_dir()` from `crates/adagio-daemon/src/main.rs` into `crates/adagio-ipc/src/transport.rs` as a `pub fn`; update `crates/adagio-daemon/src/main.rs` to call `adagio_ipc::transport::platform_config_dir()` instead — verify `cargo build -p adagio-daemon` still passes

**Checkpoint**: `cargo check -p adagio-cli -p adagio-daemon` passes; `cargo test --lib -p adagio-ipc` still green.

---

## Phase 2: Foundational — Core CLI Infrastructure

**Purpose**: clap derive types, error mapping, output helpers, and the dispatch skeleton.
Every user story phase depends on these.

**⚠️ CRITICAL**: No story implementation can compile until `cli.rs` + `error.rs` + `output.rs` exist.

### Tests (write first — must FAIL)

- [x] T006 [P] Write failing Rust unit tests `cli_parses_status_command`, `cli_global_json_flag`, `cli_json_after_subcommand` in `crates/adagio-cli/src/cli.rs` test module — assert `Cli::parse_from(["adagio","status"])` returns `Commands::Status`; assert `--json` before and after subcommand both set `json=true`
- [x] T007 [P] Write failing Rust unit tests `exit_code_daemon_err_is_2` and `exit_code_unreachable_is_3` in `crates/adagio-cli/src/error.rs` test module — assert `CliError::DaemonError("x".into()).exit_code() == 2` and `CliError::Unreachable("x".into()).exit_code() == 3`
- [x] T008 [P] Write failing Rust unit test `print_json_produces_valid_json` in `crates/adagio-cli/src/output.rs` test module — assert that `print_json(serde_json::json!({"key":"val"}))` writes only parseable JSON to stdout with no ANSI escape codes

### Implementation

- [x] T009 Implement `Cli`, `Commands`, `PairsCommand`, `AccountsCommand`, `ConflictsCommand`, `DaemonCommand` in `crates/adagio-cli/src/cli.rs` — use `#[derive(Parser, Subcommand, Args)]`; `--json` with `#[arg(long, global=true)]`; all variants per data-model.md; `///` doc comments on all public items
- [x] T010 [P] Implement `CliError { DaemonError(String), Unreachable(String) }` + `fn exit_code(&self) -> i32` (2/3) in `crates/adagio-cli/src/error.rs`; `impl From<anyhow::Error> for CliError`
- [x] T011 [P] Implement `fn print_json(v: &serde_json::Value)` (writes `serde_json::to_string_pretty` to stdout) and `fn print_table_row(cols: &[&str], widths: &[usize])` + `fn print_table_header(cols: &[&str], widths: &[usize])` in `crates/adagio-cli/src/output.rs`
- [x] T012 Implement `run.rs` dispatch skeleton — `pub async fn run(cli: Cli, client: Arc<DaemonClient>) -> Result<(), CliError>` with a `match cli.command` that calls a placeholder for each variant (returns `Ok(())` for now); create empty handler files `crates/adagio-cli/src/handlers/{status,sync,pause_resume,activity,pairs,accounts,conflicts,daemon}.rs` — depends on T009
- [x] T013 Implement `main.rs` — `#[tokio::main]`; parse `Cli`; derive daemon binary path (`current_exe().parent().join("adagio-daemon")`); call `DaemonClient::connect_or_start(&daemon_path)` for commands that require daemon; call `run(cli, client).await`; map `CliError` to `process::exit(code)` — depends on T012

**Checkpoint**: `cargo test --lib -p adagio-cli` — T006–T008 pass. `cargo build -p adagio-cli` compiles.

---

## Phase 3: User Story 1 — Check Sync Status from the Terminal (Priority: P1) 🎯 MVP

**Goal**: `adagio status` prints a table of all pairs with their current status.
`adagio status --json` prints valid JSON.

**Independent Test**: With daemon running and pairs configured, `adagio status` prints
a readable table; `adagio status --json | python3 -m json.tool` exits 0.

### Tests for User Story 1 (write first — must FAIL)

- [x] T014 [P] [US1] Write failing Rust unit test `status_human_output_contains_status_field` in `crates/adagio-cli/src/handlers/status.rs` test module — mock daemon returning `{"status":"idle"}`, call `run_status` with `json=false`, assert stdout contains "idle"
- [x] T015 [P] [US1] Write failing Rust unit test `status_json_output_is_parseable` in `crates/adagio-cli/src/handlers/status.rs` test module — mock daemon returning status object, call `run_status` with `json=true`, capture stdout, assert it parses as JSON

### Implementation for User Story 1

- [x] T016 [US1] Implement `pub async fn run_status(client: &DaemonClient, json: bool) -> Result<(), CliError>` in `crates/adagio-cli/src/handlers/status.rs` — calls `client.request(DaemonRequest::GetStatus)`; JSON mode: `print_json(&result)`; human mode: print single-row table with status, active transfers, last sync time; `///` doc comments
- [x] T017 [US1] Wire `Commands::Status` in `crates/adagio-cli/src/run.rs` — replace placeholder with `handlers::status::run_status(&client, cli.json).await?`

**Checkpoint**: `adagio status` prints a status table; `adagio status --json` outputs JSON.

---

## Phase 4: User Story 2 — Trigger Sync from a Script or Cron Job (Priority: P2)

**Goal**: `adagio sync` triggers a sync cycle. `adagio sync PAIR_ID` syncs one pair.
Auto-starts daemon if not running.

**Independent Test**: Kill daemon, run `adagio sync`, confirm it starts and exits 0 within 8 s.

### Tests for User Story 2 (write first — must FAIL)

- [x] T018 [P] [US2] Write failing Rust unit test `sync_with_pair_id_sends_trigger_sync_request` in `crates/adagio-cli/src/handlers/sync.rs` — mock client, call `run_sync` with `pair_id=Some("abc")`, assert `DaemonRequest::TriggerSync { pair_id: "abc" }` was sent
- [x] T019 [P] [US2] Write failing Rust unit test `sync_without_pair_id_triggers_all_pairs` in `crates/adagio-cli/src/handlers/sync.rs` — call with `pair_id=None`; assert `ListPairs` then `TriggerSync` for each pair are sent (or a specific "sync all" sentinel)
- [x] T020 [P] [US2] Write failing Rust unit test `activity_with_limit_sends_correct_limit` in `crates/adagio-cli/src/handlers/activity.rs` — mock client, call `run_activity` with `limit=10`, assert `DaemonRequest::GetActivityLog { limit: Some(10), filter: None }` sent

### Implementation for User Story 2

- [x] T021 [P] [US2] Implement `pub async fn run_sync(client: &DaemonClient, pair_id: Option<String>, json: bool)` in `crates/adagio-cli/src/handlers/sync.rs` — no pair_id: calls `ListPairs` then `TriggerSync` for each; with pair_id: calls `TriggerSync` once; prints "Sync triggered" / `{"ok":true}`
- [x] T022 [P] [US2] Implement `run_pause()` and `run_resume()` in `crates/adagio-cli/src/handlers/pause_resume.rs` — `PauseSyncAll` / `ResumeSyncAll`; prints "Sync paused"/"Sync resumed" / `{"ok":true}`
- [x] T023 [P] [US2] Implement `run_activity(client, limit, filter, json)` in `crates/adagio-cli/src/handlers/activity.rs` — calls `GetActivityLog`; human: table of time/action/file; JSON: `print_json`
- [x] T024 [US2] Wire `Commands::Sync`, `Commands::Pause`, `Commands::Resume`, `Commands::Activity` in `crates/adagio-cli/src/run.rs` — depends on T021–T023

**Checkpoint**: `adagio sync` triggers cycle; `adagio pause`/`adagio resume` work; `adagio activity` shows log.

---

## Phase 5: User Story 3 — Manage Conflicts from the Terminal (Priority: P3)

**Goal**: `adagio conflicts list [PAIR_ID]` shows pending conflicts.
`adagio conflicts resolve ID --keep local|remote|both` resolves one.
`adagio conflicts dismiss` clears all without file I/O.

**Independent Test**: Create a conflict, run `adagio conflicts list`, resolve with CLI, confirm list is empty.

### Tests for User Story 3 (write first — must FAIL)

- [x] T025 [P] [US3] Write failing Rust unit test `conflicts_list_no_pair_id_queries_all_pairs` in `crates/adagio-cli/src/handlers/conflicts.rs` — mock client with 2 pairs; call `run_conflicts_list` with `pair_id=None`; assert `ListConflicts` called twice (once per pair)
- [x] T026 [P] [US3] Write failing Rust unit test `conflicts_resolve_keep_local_sends_correct_side` — call `run_conflicts_resolve` with `keep="local"`; assert `DaemonRequest::ResolveConflict { id: "x", side: "local" }` sent
- [x] T027 [P] [US3] Write failing Rust unit test `conflicts_dismiss_sends_dismiss_all` — call `run_conflicts_dismiss`; assert `DaemonRequest::DismissAllConflicts` sent

### Implementation for User Story 3

- [x] T028 [US3] Implement `run_conflicts_list(client, pair_id, json)`, `run_conflicts_resolve(client, id, keep, json)`, `run_conflicts_dismiss(client, json)` in `crates/adagio-cli/src/handlers/conflicts.rs`:
  - `list` with no pair_id: calls `ListPairs` first, then `ListConflicts` per pair, combines results; human: table of conflict ID/file/local vs server size/detected-at; JSON: `print_json`
  - `resolve`: calls `ResolveConflict { id, side: keep }`; prints "Conflict resolved" / `{"ok":true}`
  - `dismiss`: calls `DismissAllConflicts`; prints "N conflicts dismissed" / `{"dismissed_count": N}`
- [x] T029 [US3] Wire `Commands::Conflicts { command }` in `crates/adagio-cli/src/run.rs` — depends on T028

**Checkpoint**: Full conflict workflow works end-to-end from terminal.

---

## Phase 6: User Story 4 — Use CLI Output in Shell Scripts (Priority: P4)

**Goal**: All commands produce valid JSON with `--json`. Pairs and accounts commands complete
the command set. Output contract verified.

**Independent Test**: Every command piped to `python3 -m json.tool` with `--json` exits 0.

### Tests for User Story 4 (write first — must FAIL)

- [x] T030 [P] [US4] Write failing Rust unit test `pairs_list_json_is_valid_json` in `crates/adagio-cli/src/handlers/pairs.rs` — mock client returning pairs, call `run_pairs_list` with `json=true`, assert stdout is parseable JSON
- [x] T031 [P] [US4] Write failing Rust unit test `accounts_list_json_is_valid_json` in `crates/adagio-cli/src/handlers/accounts.rs` — same pattern
- [x] T032 [P] [US4] Write failing Rust unit test `json_output_has_no_ansi_codes` in `crates/adagio-cli/src/output.rs` — call `print_json` with any value, capture stdout, assert no bytes in range 27 (ESC) or strings matching `\x1b[`

### Implementation for User Story 4

- [x] T033 [P] [US4] Implement `run_pairs_list(client, json)`, `run_pairs_add(client, local, remote, account, json)`, `run_pairs_remove(client, pair_id, delete_local_files, json)` in `crates/adagio-cli/src/handlers/pairs.rs` — list: `ListPairs` → table/JSON; add: `CreatePair`; remove: `DeletePair`; `///` doc comments
- [x] T034 [P] [US4] Implement `run_accounts_list(client, json)`, `run_accounts_remove(client, account_id, json)` in `crates/adagio-cli/src/handlers/accounts.rs` — list: `ListAccounts` → table/JSON; remove: `RemoveAccount`
- [x] T035 [US4] Wire `Commands::Pairs { command }` and `Commands::Accounts { command }` in `crates/adagio-cli/src/run.rs` — depends on T033, T034

**Checkpoint**: `adagio pairs list`, `adagio accounts list` work; all `--json` outputs pass JSON validation.

---

## Phase 7: User Story 5 — Control Daemon Lifecycle from the Terminal (Priority: P5)

**Goal**: `adagio daemon start` starts daemon in < 5 s. `adagio daemon stop` stops it gracefully.
`adagio daemon status` shows running state and uptime.

**Independent Test**: Kill daemon, `time adagio daemon start` exits 0 in < 5 s; `adagio daemon status --json` shows `running: true`.

### Tests for User Story 5 (write first — must FAIL)

- [x] T036 [P] [US5] Write failing Rust unit test `daemon_start_already_running_returns_already_running_message` in `crates/adagio-cli/src/handlers/daemon.rs` — mock connected stub client, call `run_daemon_start`, assert output contains "already running" (human) or `{"started":false}` (JSON)
- [x] T037 [P] [US5] Write failing Rust unit test `daemon_status_formats_uptime_seconds` — mock `Ping` returning `{"uptime_secs":7261}`, call `run_daemon_status` with `json=false`, assert output contains "2h" and "1m"
- [x] T038 [P] [US5] Write failing Rust unit test `daemon_stop_sends_stop_daemon_request` — mock client, call `run_daemon_stop`, assert `DaemonRequest::StopDaemon` sent

### Implementation for User Story 5

- [x] T039 [US5] Implement `run_daemon_start(daemon_binary_path, json)`, `run_daemon_stop(client, json)`, `run_daemon_status(client, json)` in `crates/adagio-cli/src/handlers/daemon.rs`:
  - `start`: calls `DaemonClient::connect_or_start(path)`; if already connected → "already running" / `{"started":false}`; if newly started → "Daemon started" / `{"started":true}`; exits 0 either way
  - `stop`: calls `StopDaemon`; prints "Daemon stopped" / `{"stopped":true}`
  - `status`: calls `Ping`; human: table of running/uptime (formatted as Xh Ym Zs)/version; JSON: `print_json`
- [x] T040 [US5] Wire `Commands::Daemon { command }` in `crates/adagio-cli/src/run.rs`; also wire the daemon-start fast path in `main.rs` — for `daemon start`, skip `connect_or_start` and go directly to spawning — depends on T039

**Checkpoint**: Full daemon lifecycle works from the terminal. `adagio daemon start/stop/status` all correct.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [x] T041 `cargo clippy -p adagio-cli -- -D warnings` — fix all warnings; zero suppressions
- [x] T042 [P] `cargo fmt --all --check` — run `cargo fmt --all` to fix formatting drift
- [x] T043 `cargo test --lib -p adagio-cli` — all new CLI tests pass
- [x] T044 [P] `cargo test --lib -p adagio-ipc` — 13 tests pass (verify `platform_config_dir()` export tests added)
- [x] T045 [P] `cargo test -p adagio-daemon` — 6 tests pass (no regressions from `platform_config_dir` migration)
- [x] T046 Manual `quickstart.md` validation — step through all 5 user stories per `specs/008-cli-binary/quickstart.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all story phases
- **Phase 3 (US1)**: Depends on Phase 2 — MVP; implement first
- **Phases 4–7 (US2–US5)**: All depend on Phase 2; can run in parallel after Phase 2 completes
- **Phase 8 (Polish)**: Depends on all prior phases

### Within Each Story Phase

- Test tasks (T014/T015, T018–T020, T025–T027, T030–T032, T036–T038) are [P] — write all simultaneously
- Handler implementation then wiring: handler file before run.rs wire task

### Parallel Opportunities

All test tasks within a story phase are parallel (different files).
After Phase 2: T016, T021, T025, T028, T033 can all begin simultaneously.

---

## Parallel Example: Phase 2 + Phase 3 sprint

```
Phase 2 tests (all parallel):
  T006: cli.rs argument tests
  T007: error.rs exit code tests
  T008: output.rs JSON tests

Phase 2 implementation (sequential within file, parallel across files):
  T009 (cli.rs) → T012 (run.rs) → T013 (main.rs)
  T010 (error.rs) — parallel with T009
  T011 (output.rs) — parallel with T009

Phase 3 tests (write while Phase 2 impl runs):
  T014, T015: status handler tests

Phase 3 impl:
  T016 (status handler) → T017 (wire in run.rs)
```

---

## Implementation Strategy

### MVP (US1 only — status command)

1. Phase 1 → Phase 2 → Phase 3
2. **STOP and VALIDATE**: `adagio status` shows pairs; `adagio status --json` returns JSON
3. `cargo test --lib -p adagio-cli` passes

### Full Delivery (all 5 stories)

1. MVP above
2. Phase 4 (US2): sync, pause, resume, activity
3. Phase 5 (US3): conflict management
4. Phase 6 (US4): pairs, accounts, output contract
5. Phase 7 (US5): daemon lifecycle
6. Phase 8: polish + quickstart validation

---

## Notes

- [P] tasks touch different files — launch together for speed
- Constitution Principle I is non-negotiable: every test must be written and confirmed failing before its paired implementation begins
- T019 (sync without pair_id) requires a design decision: either call `ListPairs` first and loop, OR add `TriggerSyncAll` to `DaemonRequest` — plan says "call ListPairs then loop"; no new DaemonRequest variant needed
- T028 (`conflicts list` without pair_id) also loops over pairs from `ListPairs` — same pattern
- T013 (`main.rs`) is the only task that handles the `CliError → process::exit` mapping; exit code 3 comes from `DaemonClient::connect_or_start` failing
- Total: **46 tasks** across 8 phases
