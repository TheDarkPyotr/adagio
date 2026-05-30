# IPC Protocol Contract: adagio-daemon ↔ adagio-desktop

**Version**: 1.0 | **Transport**: NDJSON over Unix socket / Windows named pipe

---

## Transport layer

| Platform | Type | Path |
|----------|------|------|
| Linux | Unix domain socket | `$XDG_RUNTIME_DIR/adagio/daemon.sock` |
| macOS | Unix domain socket | `~/Library/Application Support/adagio/daemon.sock` |
| Windows | Named pipe | `\\.\pipe\adagio-daemon-<user-sid>` |

**Security**: Socket created with `0600` permissions (Unix) / user-scoped DACL (Windows).
Only the owning OS user may connect.

---

## Connection types

Two distinct connection types share the same socket/pipe endpoint. The daemon
identifies them by the first message received.

### Type 1: RPC connection

Used for request/response command pairs.

**Client → Daemon** (one per line, UTF-8):
```json
{"id": 1, "method": "get_status", "params": {}}
```

**Daemon → Client** on success:
```json
{"id": 1, "result": {"status": "idle", "active_file_count": 0}}
```

**Daemon → Client** on error:
```json
{"id": 1, "error": {"code": -32000, "message": "pair not found: abc123"}}
```

Rules:
- `id` is a client-chosen positive integer; responses carry the same `id`.
- Multiple in-flight requests on a single connection are allowed (pipelined).
- The daemon processes requests serially per connection; clients may open multiple
  connections for concurrent requests.
- Connection is closed by either side sending EOF.

### Type 2: Subscription connection

Used for receiving push events. Identified by the first message being
`{"type":"subscribe"}`.

**Client → Daemon** (once, to establish subscription):
```json
{"type": "subscribe"}
```

**Daemon → Client** (pushed as events occur, indefinitely):
```json
{"event": "conflict_detected", "payload": {"pending_count": 2}}
{"event": "sync_status_changed", "payload": {"status": "syncing", "pair_id": "abc"}}
```

Rules:
- After the initial subscribe message, the client only reads (no further writes).
- The daemon sends events to all open subscription connections.
- When the daemon is shutting down, it sends a final `shutting_down` event before
  closing the connection.

---

## RPC method catalogue

### System

| Method | Params | Result |
|--------|--------|--------|
| `ping` | `{}` | `{ "version": string, "uptime_secs": integer }` |
| `stop_daemon` | `{}` | `{}` — daemon exits after 30 s drain |
| `set_start_at_login` | `{ "enabled": boolean }` | `{}` |

### Sync control

| Method | Params | Result |
|--------|--------|--------|
| `get_status` | `{}` | `SyncStatusDto` |
| `trigger_sync` | `{ "pair_id": string }` | `SyncReportDto` |
| `pause_sync_all` | `{}` | `{}` |
| `resume_sync_all` | `{}` | `{}` |

### Pairs

| Method | Params | Result |
|--------|--------|--------|
| `list_pairs` | `{}` | `PairDto[]` |
| `create_pair` | `CreatePairRequest` | `PairDto` |
| `delete_pair` | `{ "pair_id": string, "delete_local_files": boolean }` | `{}` |
| `list_synced_files` | `{ "pair_id": string, "relative_path": string? }` | `FileStatusDto[]` |
| `get_exclude_patterns` | `{}` | `string[]` |
| `list_remote_tree` | `{ "pair_id": string }` | `RemoteTreeItemDto[]` |

### Accounts

| Method | Params | Result |
|--------|--------|--------|
| `list_accounts` | `{}` | `AccountDto[]` |
| `add_account` | `AddAccountRequest` | `AccountDto` |
| `remove_account` | `{ "account_id": string }` | `{}` |
| `connect_account_oauth2` | `{ "server_url": string }` | `AccountDto` |

### Conflicts

| Method | Params | Result |
|--------|--------|--------|
| `list_conflicts` | `{ "pair_id": string }` | `ConflictDto[]` |
| `resolve_conflict` | `{ "id": string, "side": "local"\|"remote"\|"both" }` | `{}` |
| `dismiss_all_conflicts` | `{}` | `{ "dismissed_count": integer }` |

### Activity

| Method | Params | Result |
|--------|--------|--------|
| `get_activity_log` | `{ "limit": integer?, "filter": string? }` | `ActivityEntryDto[]` |

### Sharing

| Method | Params | Result |
|--------|--------|--------|
| `search_users` | `{ "account_id": string, "query": string }` | `UserSearchResult[]` |
| `create_share` | `CreateShareRequest` | `ShareResult` |

---

## Push event catalogue

| Event | Payload |
|-------|---------|
| `conflict_detected` | `{ "pending_count": integer }` |
| `conflict_resolved` | `{ "id": string, "pending_count": integer }` |
| `sync_status_changed` | `{ "status": string, "pair_id": string? }` |
| `transfer_progress` | `{ "pair_id": string, "path": string, "bytes_done": integer, "bytes_total": integer }` |
| `shutting_down` | `{ "reason": string }` |

---

## Error codes

| Code | Meaning |
|------|---------|
| `-32700` | Parse error — invalid JSON |
| `-32600` | Invalid request — missing required field |
| `-32601` | Method not found |
| `-32602` | Invalid params |
| `-32000` | Application error (see `message` for details) |
| `-32001` | Daemon shutting down — retry after restart |

---

## DTO shapes (unchanged from current Tauri API)

All DTO types (`SyncStatusDto`, `PairDto`, `AccountDto`, `ConflictDto`,
`ActivityEntryDto`, `FileStatusDto`) are byte-for-byte identical to those
currently serialised by the Tauri command handlers. The React frontend does not
need any changes.

---

## Versioning

The `ping` response includes `version`. If the desktop app's protocol version
does not match the daemon's, the app must restart the daemon (stop + spawn new).
Version mismatch is detected by comparing the major version prefix of the returned
version string.
