# IPC Contract: Bulk Upload Driver (011)

## No new DaemonRequest variants needed

The bulk upload driver reuses the existing IPC surface entirely:

| Existing mechanism | How bulk driver uses it |
|--------------------|------------------------|
| `DaemonEvent::TransferProgress` | Emitted per-file during bulk upload; desktop app and CLI already handle it |
| `DaemonEvent::SyncStatusChanged` | Emitted when bulk mode starts (`status: "syncing"`) and when it completes (`status: "idle"`) |
| `DaemonRequest::GetStatus` | Returns `status: "syncing"` while bulk is running; no schema change needed |
| `DaemonRequest::CreatePair` | Bulk config fields are stored per-pair; the `CreatePair` handler already writes pair config |

---

## CreatePair — extended pair config (no protocol change)

The `CreatePair` request already passes pair settings that get persisted. Bulk config fields are stored directly in the pair's config.json entry:

```json
{
  "method": "create_pair",
  "params": {
    "account_id": "...",
    "local_root": "/path/to/folder",
    "remote_root": "/",
    "bulk_upload_workers": 8,
    "bulk_upload_threshold_files": 50,
    "bulk_upload_chunk_threshold_bytes": 10485760
  }
}
```

Omitting bulk fields → defaults apply. Existing `CreatePair` callers are unaffected.

---

## TransferProgress event (existing, unchanged)

```json
{
  "event": "transfer_progress",
  "payload": {
    "pair_id": "...",
    "path": "photos/IMG_001.jpg",
    "bytes_done": 1048576,
    "bytes_total": 5242880
  }
}
```

Emitted at least once per file completion during bulk upload. The UI's existing progress bar and the CLI's status command consume these without modification.

---

## Tauri commands — no new commands

The bulk upload is transparent to the UI. The existing commands suffice:

| Command | Bulk upload behavior |
|---------|---------------------|
| `get_status` | Returns `status: "syncing"` with `active_file_count` reflecting bulk progress |
| `get_activity_log` | Lists completed uploads from the bulk session |
| `get_error_items` | Lists files that failed with permanent errors during bulk upload |

---

## CLI — no new subcommands

`adagio status` already shows sync state and active file count. No new CLI commands are needed for bulk upload — the feature is automatic and its progress is visible through existing status mechanisms.

---

## TypeScript types — no changes needed

The existing `SyncStatusDto`, `ActivityEntryDto`, and progress event handlers in the UI all work without modification. The bulk driver is transparent at the IPC boundary.
