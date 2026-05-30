# Data Model: Background Sync Daemon (007)

**Date**: 2026-05-29 | **Feature**: `007-background-sync-daemon`

This feature introduces no new SQLite tables and no new Tauri IPC commands visible to
the React frontend. All changes are at the process boundary and protocol layer.

---

## New: `DaemonRequest` (protocol enum, not persisted)

Every call from `adagio-desktop` to `adagio-daemon` maps to one variant.
Serialises to `{"method":"<snake_case>","params":{...}}`.

```rust
pub enum DaemonRequest {
    Ping,
    GetStatus,
    ListPairs,
    CreatePair { req: CreatePairRequest },
    DeletePair { pair_id: String, delete_local_files: bool },
    ListAccounts,
    AddAccount { req: AddAccountRequest },
    RemoveAccount { account_id: String },
    ConnectAccountOAuth2 { server_url: String },
    TriggerSync { pair_id: String },
    PauseSyncAll,
    ResumeSyncAll,
    GetActivityLog { limit: Option<u32>, filter: Option<String> },
    ListConflicts { pair_id: String },
    ResolveConflict { id: String, side: String },
    DismissAllConflicts,
    ListSyncedFiles { pair_id: String, relative_path: Option<String> },
    GetExcludePatterns,
    ListRemoteTree { pair_id: String },
    SearchUsers { account_id: String, query: String },
    CreateShare { request: CreateShareRequest },
    SetStartAtLogin { enabled: bool },
    StopDaemon,
    SubscribeEvents, // sent on the subscription connection
}
```

---

## New: `DaemonResponse` (protocol enum, not persisted)

Successful result for each request. Serialises to `{"id":<n>,"result":{...}}`.
Errors serialise to `{"id":<n>,"error":{"code":<n>,"message":"..."}}`.

```rust
pub enum DaemonResponse {
    Pong { version: String, uptime_secs: u64 },
    Status(SyncStatusDto),
    Pairs(Vec<PairDto>),
    Pair(PairDto),
    Accounts(Vec<AccountDto>),
    Account(AccountDto),
    SyncReport(SyncReportDto),
    Unit,
    ActivityLog(Vec<ActivityEntryDto>),
    Conflicts(Vec<ConflictDto>),
    DismissedCount(usize),
    SyncedFiles(Vec<FileStatusDto>),
    ExcludePatterns(Vec<String>),
    RemoteTree(Vec<RemoteTreeItemDto>),
    Users(Vec<UserSearchResult>),
    Share(ShareResult),
}
```

---

## New: `DaemonEvent` (push notification, not persisted)

Emitted by the daemon to all subscribed connections whenever state changes.
Serialises to `{"event":"<snake_case>","payload":{...}}\n`.

```rust
pub enum DaemonEvent {
    /// A new Ask-policy conflict was detected.
    ConflictDetected { pending_count: usize },
    /// A conflict was resolved (or dismissed).
    ConflictResolved { id: String, pending_count: usize },
    /// Engine-level status changed (idle → syncing → idle, error, paused).
    SyncStatusChanged { status: String, pair_id: Option<String> },
    /// Byte-level transfer progress for a file.
    TransferProgress { pair_id: String, path: String, bytes_done: u64, bytes_total: u64 },
    /// The daemon is about to shut down (sent before process exit).
    ShuttingDown { reason: String },
}
```

---

## New: `DaemonClient` (runtime struct, not persisted)

Held in `adagio-desktop`'s `AppState`. Manages the IPC connection lifecycle.

```
DaemonClient {
    rpc_stream: Mutex<Option<BufStream>>   // the RPC connection
    event_rx:   broadcast::Receiver<DaemonEvent>
    state:      watch::Sender<ConnectionState>
    next_id:    AtomicU64
}

ConnectionState {
    Connected,
    Reconnecting { attempt: u8 },
    Failed,       // after 3 consecutive failed restarts
}
```

Methods:
- `request(DaemonRequest) -> Result<DaemonResponse>` — sends RPC, awaits response
- `subscribe() -> broadcast::Receiver<DaemonEvent>` — get a copy of the event stream
- `connection_state() -> watch::Receiver<ConnectionState>` — watch reconnect state

---

## New: `DaemonProcess` (runtime struct, not persisted)

Held inside `adagio-daemon`'s `main`. Represents the running daemon.

```
DaemonProcess {
    engine:        Arc<DefaultSyncEngine>
    journal:       Arc<SqliteJournal>
    accounts:      Arc<AccountManager>
    pairs:         Arc<RwLock<SyncPairManager>>
    event_tx:      broadcast::Sender<DaemonEvent>
    cancel:        CancellationToken
    task_tracker:  TaskTracker
    socket_path:   PathBuf
    started_at:    Instant
}
```

---

## New: `AutoStartConfig` (persisted to platform-specific location, not SQLite)

Tracks whether auto-start is enabled. Written by `set_start_at_login`.

| Platform | Storage location | Format |
|----------|-----------------|--------|
| Linux | `~/.config/autostart/adagio-daemon.desktop` | XDG Desktop Entry |
| macOS | `~/Library/LaunchAgents/com.adagio.daemon.plist` | plist XML |
| Windows | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\adagio-daemon` | String (exe path) |

Presence of the file/key indicates enabled; absence indicates disabled.

---

## Socket / pipe path convention

| Platform | Path |
|----------|------|
| Linux | `$XDG_RUNTIME_DIR/adagio/daemon.sock` (fallback: `/tmp/adagio-<uid>.sock`) |
| macOS | `~/Library/Application Support/adagio/daemon.sock` |
| Windows | `\\.\pipe\adagio-daemon-<sid>` (SID prevents cross-user access) |

---

## No database schema changes

All existing SQLite tables (`accounts`, `sync_pairs`, `journal_entries`,
`conflict_records`, `transfers`) are unchanged. The daemon reads/writes them with
the same `SqliteJournal` and `AccountManager` types as the current embedded design.
There is no migration required for existing data.
