# Research: Background Sync Daemon (007)

**Date**: 2026-05-29 | **Feature**: `007-background-sync-daemon`

---

## Decision 1 — IPC transport: hand-rolled NDJSON over tokio sockets

**Decision**: Use newline-delimited JSON (NDJSON) framed over `tokio::net::UnixListener`
(Linux/macOS) and `tokio::net::windows::named_pipe::ServerOptions` (Windows). No new
crate is needed — both are built into tokio's `full` feature which the workspace already
uses.

**Protocol**: Two connection types on the same socket/pipe:
- **RPC connection** — one JSON object per line, request/response correlated by `id`:
  `{"id":1,"method":"get_status","params":{}}\n` → `{"id":1,"result":{...}}\n`
- **Subscription connection** — client sends `{"type":"subscribe"}\n`; server pushes
  NDJSON events indefinitely. No length prefix needed: `\n` is the frame delimiter.

**Rationale**: `jsonrpc-core` is unmaintained; `jsonrpsee` and `tarpc` have no built-in
Unix socket transport and require custom adapter layers that add more code than the
hand-rolled solution. The hand-rolled approach is 150–200 lines, testable in isolation,
and has zero additional dependencies.

**Alternatives considered**:
- `jsonrpsee` with custom transport — functional but adds a large dependency graph.
- `tarpc` — designed for async traits, not a line-protocol JSON-RPC API.
- gRPC/tonic — the proposal's long-term target; deferred to a future feature to avoid
  protobuf toolchain overhead in this extraction step.

---

## Decision 2 — Single-instance guard: socket-binding as the authority

**Decision**: On startup the daemon attempts to bind the Unix socket / named pipe. If
bind succeeds, the daemon is the sole instance. If bind fails (address already in use),
attempt a `ping` RPC. If ping succeeds, the daemon exits silently (another live instance
exists). If ping fails (stale socket from a crash), delete the socket file and retry bind.

**Rationale**: Socket binding is an atomic OS operation — no race condition. It is more
reliable than PID-file checks (TOCTOU race, stale PID reuse) and avoids adding the
`single-instance` crate. The stale-socket recovery handles the common crash-without-cleanup
case.

**Alternatives considered**:
- PID file with `flock` — race-prone; stale PID numbers can be reused.
- `single-instance` crate — adds a dependency for 20 lines of equivalent logic.

---

## Decision 3 — Auto-start registration: platform-specific file/registry writes

| Platform | Mechanism | How registered | How deregistered |
|----------|-----------|---------------|-----------------|
| Linux | XDG `.desktop` file in `~/.config/autostart/` | `std::fs::write` | `std::fs::remove_file` |
| macOS | LaunchAgent plist in `~/Library/LaunchAgents/` | `std::fs::write` + `launchctl load` | `launchctl unload` + `remove_file` |
| Windows | HKCU `Run` registry key | `winreg` crate (safe, no `unsafe`) | delete registry value |

**Decision**: Implement platform-specific auto-start registration entirely in Rust
inside `adagio-desktop` (as a Tauri command `set_start_at_login`). The daemon binary
path is determined at runtime (`std::env::current_exe()` from within the Tauri app,
knowing the daemon binary ships alongside it).

**Rationale**: `tauri-plugin-autostart` only registers the calling executable (the Tauri
GUI app), not a separate daemon binary. We need to register `adagio-daemon` at login,
not `adagio-desktop`. Writing the plist/desktop/registry entry is ≤ 20 lines per platform.

**Note**: Linux XDG autostart is preferred over systemd user services because it works
on all desktop environments (including non-systemd distros like Void, Devuan). On
systemd hosts, `systemd-xdg-autostart-generator` transparently converts it to a unit.

**New dependency**: `winreg = "0.52"` added to `adagio-desktop` as a Windows-only
conditional dependency (`[target.'cfg(windows)'.dependencies]`).

---

## Decision 4 — Daemon spawning: detached `std::process::Command`

**Decision**: Spawn `adagio-daemon` as a fully detached child process from `adagio-desktop`
using `std::process::Command` with all stdio handles redirected to `/dev/null` (Unix) or
`nul` (Windows). On Windows, set `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` creation
flags.

**Rationale**: Tauri's `tauri-plugin-shell` sidecar API terminates child processes when
the parent GUI exits (via SIGTERM/SIGKILL lifecycle hook — tracked in Tauri issue #3062).
This makes it unsuitable for a daemon that must outlive the GUI. Using `std::process::Command`
directly with detached semantics is the correct approach for a truly independent process.

**Health check before spawning**: The Tauri app connects to the socket with a 300 ms
timeout (three attempts × 100 ms). Connection success → daemon is already running, skip
spawn. Connection failure → spawn daemon, then retry health check for up to 5 seconds
before declaring startup failed.

---

## Decision 5 — Graceful shutdown: CancellationToken + TaskTracker + 30 s drain

**Decision**: The daemon uses `tokio_util::sync::CancellationToken` (already a workspace
dep via `tokio-util`) to broadcast shutdown to all `PairRunner` tasks. `TaskTracker`
wraps all spawned runner tasks and provides a `wait()` future. On SIGTERM/SIGINT, the
daemon cancels the token, closes the tracker, and waits up to 30 seconds for all tasks
to finish. SQLite WAL is checkpointed via `PRAGMA wal_checkpoint(RESTART)` before the
pool is dropped.

**Rationale**: `CancellationToken` is already used by `PairRunner::spawn` for its internal
loop. Adding it to the graceful-shutdown path is a natural extension. `TaskTracker` is
the tokio-recommended replacement for manual JoinSet management.

**Note**: `PairRunner` already has `cancel: CancellationToken` and a `stop()` method.
The shutdown coordinator calls `runner.stop()` on each registered runner, which cancels
the token and aborts the task. The 30 s drain gives in-flight HTTP calls time to complete
or time out at their own HTTP timeout (default: 30 s per request, already enforced by
`reqwest`).

---

## Decision 6 — AppState extraction: what moves where

| State | Current owner | After extraction |
|-------|---------------|-----------------|
| `engine: Arc<DefaultSyncEngine>` | `adagio-desktop` | `adagio-daemon` |
| `journal: Arc<SqliteJournal>` | `adagio-desktop` | `adagio-daemon` |
| `accounts: Arc<AccountManager>` | `adagio-desktop` | `adagio-daemon` |
| `pairs: Arc<RwLock<SyncPairManager>>` | `adagio-desktop` | `adagio-daemon` |
| `config_path: PathBuf` | `adagio-desktop` | both (daemon for engine config, desktop for UI prefs) |
| `daemon_client: Arc<DaemonClient>` | (new) | `adagio-desktop` only |

The new `AppState` in `adagio-desktop` holds a single `Arc<DaemonClient>`. All Tauri
commands become one-line wrappers: `daemon_client.request(method, params).await`.

Read-only commands (query only): `get_status`, `get_activity_log`, `list_conflicts`,
`list_accounts`, `list_pairs`, `list_synced_files`, `get_exclude_patterns`.

Write commands (mutate state): `pause_sync`, `resume_sync`, `trigger_sync`,
`resolve_conflict`, `dismiss_all_conflicts`, `connect_account_oauth2`, `add_account`,
`remove_account`, `create_pair`, `delete_pair`, `search_users`, `create_share`.

UI-only commands (stay in Tauri, no daemon needed): `get_palette`, `set_palette`.

---

## Decision 7 — New crates: adagio-ipc + adagio-daemon

Two new crates are added to the workspace:

**`crates/adagio-ipc`** (library) — shared by both `adagio-daemon` and `adagio-desktop`:
- `DaemonRequest` enum — all RPC method + params variants
- `DaemonResponse` enum — all result variants
- `DaemonEvent` enum — all push event variants
- `DaemonClient` struct — connect, `request()`, `subscribe()`, reconnection logic
- `transport` module — platform-specific socket/pipe connect/accept helpers

**`crates/adagio-daemon`** (binary):
- `main.rs` — tokio runtime, single-instance guard, signal handling, graceful shutdown
- `server.rs` — IPC server: accept connections, dispatch RPC, broadcast events
- `dispatcher.rs` — maps `DaemonRequest` variants to engine/journal calls
- `autostart.rs` — (exported via RPC) enable/disable daemon auto-start per platform

**Rationale for separate `adagio-ipc` crate**: Avoids `adagio-desktop` depending on
`adagio-daemon` (which would pull in the full engine + all server dependencies into the
Tauri build). Both crates share only the protocol types.
