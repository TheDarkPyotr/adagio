# Research: Desktop App Lifecycle

**Feature**: 002-desktop-app-lifecycle
**Date**: 2026-05-24

---

## Decision 1: Configuration File Format

**Decision**: JSON via `serde_json`

**Rationale**: `serde_json` is already a workspace dependency; JSON is human-readable and easy to
inspect or repair manually. No extra dependency required.

**Alternatives considered**:
- TOML — more human-friendly syntax but requires `toml` crate, adds a dep for minimal benefit
- RON — Rust-native, but almost no tooling outside the Rust ecosystem
- SQLite — overkill for a config file that contains at most a handful of accounts and pairs

---

## Decision 2: Configuration File Location

**Decision**: `app.path().app_config_dir()` from Tauri's `PathResolver`, file name `config.json`

**Rationale**: Tauri 2.x resolves `app_config_dir()` to the correct platform directory
automatically (`~/.config/adagio/` on Linux, `~/Library/Application Support/adagio/`
on macOS, `%APPDATA%\adagio\` on Windows). No manual path logic needed.

**Alternatives considered**:
- Hard-code `~/.adagio/` — breaks platform conventions on macOS and Windows
- Use the journal's SQLite database — mixes app config with ephemeral sync state; harder to
  wipe one without wiping the other

---

## Decision 3: Per-Pair Background Sync Runner

**Decision**: A `PairRunner` struct in `adagio-core/src/cycle/runner.rs` that holds a
`CancellationToken` and an `mpsc::Sender<()>` (immediate-trigger channel). The runner loop uses
`tokio::select!` over the scan interval tick, the trigger channel, and the cancellation token.

**Rationale**: Decouples the scheduling logic from the engine state. `DefaultSyncEngine` gains
`start_pair()` / `stop_pair()` / `trigger_pair()` methods backed by a
`HashMap<PairId, PairRunner>`.

**Alternatives considered**:
- Single background task iterating all pairs — harder to add/remove pairs dynamically at runtime
- `tokio_cron_scheduler` — heavyweight dep, unnecessary for a simple interval + trigger model
- OS-level timers — cross-platform complexity with no benefit over Tokio timers

---

## Decision 4: Credential Retrieval at Engine Startup

**Decision**: On app startup, retrieve credentials from the OS keychain via `spawn_blocking` in
the Tauri setup hook, construct one `NextcloudClient` per account, and pass it to
`engine.start_pair()`.

**Rationale**: Credentials must never leave the keychain; the client is constructed in-process
and passed to the runner by `Arc`. No credential is serialised to disk at any point.

**Alternatives considered**:
- Store credentials in the config file — violates the security constitution requirement
- Retrieve credentials per sync cycle — unnecessary keychain round-trips on every cycle

---

## Decision 5: File Browser Data Source

**Decision**: `list_synced_files` Tauri command performs a non-recursive `std::fs::read_dir` on
the requested path, then enriches each entry with its status from the SQLite journal.

**Rationale**: The filesystem is the source of truth for what files exist; the journal is the
source of truth for their sync state. Combining both gives the user an accurate, live view.

**Alternatives considered**:
- Journal-only listing — misses newly added local files not yet indexed by the engine
- Full recursive scan on every call — too slow for large folders; use expand-on-demand instead

---

## Decision 6: SyncPairManager Expansion

**Decision**: Update `SyncPairManager` to store full `SyncPair` structs (not just
`PairId → PathBuf`). Add `register_full_pair()`, `get_pair()`, `all_pairs()` methods.

**Rationale**: The current `HashMap<PairId, PathBuf>` is insufficient — the engine startup and
file browser both need the full pair config (remote root, scan interval, selective paths, etc.).
Fixing this in the manager avoids duplicating state elsewhere.

**Alternatives considered**:
- Store full pairs in `AppState` separately — duplicates the authoritative copy; harder to keep
  in sync with `SyncPairManager`
