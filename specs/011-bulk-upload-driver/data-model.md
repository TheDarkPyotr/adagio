# Data Model: Bulk Upload Driver (011)

## New / modified entities

### BulkUploadConfig (embedded in SyncPair, persisted in config.json)

Three new fields on the existing `SyncPair` struct (all `#[serde(default)]`):

```
bulk_upload_workers:              u8   // default 8, range 1–32
bulk_upload_threshold_files:      u32  // default 50
bulk_upload_chunk_threshold_bytes: u64 // default 10_485_760 (10 MiB)
```

Same fields mirrored on `SavedPair` in the desktop config crate.

**Validation rules:**
- `bulk_upload_workers` clamped to 1–32 on load; out-of-range values use the default
- `bulk_upload_threshold_files` must be ≥ 1
- `bulk_upload_chunk_threshold_bytes` must be > 0; values below 1 MiB are silently raised to 1 MiB (prevents degenerate session counts)

---

### BulkUploadDriver (runtime only — not persisted)

```
BulkUploadDriver {
    pair:       SyncPair             // full pair config including bulk fields
    client:     Arc<dyn RemoteClient>
    journal:    Arc<dyn Journal>
    broadcaster: Option<EventBroadcaster>  // for progress forwarding
}
```

**Methods:**
- `should_activate(remote_count, upload_ops_count) -> bool` — pure function, no side effects
- `run(upload_ops, local_root, remote_root, cancel) -> BulkUploadResult`

---

### BulkUploadResult (runtime only)

```
BulkUploadResult {
    uploaded:  u32   // files successfully uploaded
    skipped:   u32   // files excluded (already Synced, permanent error)
    errors:    u32   // files that failed with transient or permanent error
    bytes:     u64   // total bytes successfully transferred
}
```

Returned from `BulkUploadDriver::run()` back to `SyncCycle::run()` for logging.

---

### TransferOptions extension (existing type, no schema change)

The existing `TransferOptions` struct already has `chunked_threshold` and `chunk_size`. The bulk driver builds `TransferOptions` from the pair's `bulk_upload_chunk_threshold_bytes`:

```rust
TransferOptions {
    chunked_threshold: pair.bulk_upload_chunk_threshold_bytes,
    chunk_size: 5 * 1024 * 1024,  // 5 MiB fixed chunk size
    bandwidth_cap: None,           // throttle applied at transfer layer
}
```

---

## Activation condition (derived, not stored)

```
should_activate = 
    remote_item_count < (local_item_count as f64 * 0.10).ceil() as usize
    AND upload_op_count >= pair.bulk_upload_threshold_files as usize
```

`remote_item_count` and `upload_op_count` are computed from the already-fetched snapshots in `SyncCycle::run()` — no extra network calls.

---

## Config.json schema (delta for SavedPair)

```json
{
  "pairs": [
    {
      "id": "...",
      "account_id": "...",
      "local_root": "...",
      "remote_root": "...",
      "bulk_upload_workers": 8,
      "bulk_upload_threshold_files": 50,
      "bulk_upload_chunk_threshold_bytes": 10485760
    }
  ]
}
```

Missing fields → defaults. Fully backward compatible with existing config.json files.
