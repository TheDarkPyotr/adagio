# ADR-015: Bulk Upload Driver Architecture

**Status**: Accepted  
**Date**: 2026-05-30  
**Feature**: 011-bulk-upload-driver

---

## Context

The standard sync cycle uploads files one at a time through the propagator, which was designed for steady-state maintenance of an already-synced folder. For the initial-sync scenario — where the remote is empty and the local folder has thousands of files — this creates a severe performance bottleneck. A user syncing a 100 GB archive could wait days using the sequential path.

Three specific problems motivate a dedicated fast path:
1. **Sequential throughput**: The propagator's semaphore (`max_upload_concurrency`, default 3) limits parallelism; the bulk case can safely use more workers
2. **No file-level resume**: A network interruption or daemon restart forces a full restart
3. **Per-cycle overhead**: The standard cycle re-scans both sides on every iteration

---

## Decision

Add a `BulkUploadDriver` that activates inside `SyncCycle::run()` between reconciliation and propagation when two conditions are met:
1. The remote has fewer than 10% of the local file count (effectively empty on first sync)
2. The number of pending uploads exceeds `bulk_upload_threshold_files` (default 50)

The driver:
- Uploads files in parallel using `tokio::task::JoinSet` bounded by `Arc<Semaphore>` with `bulk_upload_workers` permits (default 8)
- Routes files ≥ `bulk_upload_chunk_threshold_bytes` (default 10 MiB) through `upload_chunked`, which handles chunk-level resume internally
- Routes smaller files through `upload_single`
- Skips files already in the journal with `status=Synced` or permanent errors on each invocation (file-level resume)
- Writes journal entries via `upsert` immediately on each file's completion (not batched)
- Forwards `TransferProgress` events to `EventBroadcaster` for UI visibility
- After completion, returns control to `SyncCycle::run()` which runs the standard propagator on non-upload ops (downloads, deletes, conflicts)

---

## Alternatives Considered

| Alternative | Rejected because |
|-------------|-----------------|
| Increase `max_upload_concurrency` in the propagator | Still sequential file-by-file processing; doesn't fix resume; propagator designed for steady-state |
| New daemon command `bulk_sync` | Requires UI/CLI changes and user manual intervention; auto-activation is a core requirement |
| Separate bulk upload binary | Unnecessary complexity; the logic belongs in `adagio-core` for testability |
| Batch journal writes with `upsert_batch` | Per-file `upsert` is safer: if interrupted after N files, exactly N entries are persisted without needing a commit boundary |
| Parallel chunks within a single file | Adds significant complexity; Nextcloud's chunked protocol is sequential by design; file-level parallelism is sufficient |

---

## Consequences

- `crates/adagio-core/src/types.rs`: three new `#[serde(default)]` fields on `SyncPair`
- `crates/adagio-core/src/bulk_upload.rs`: new module
- `crates/adagio-core/src/cycle/mod.rs`: activation check and driver invocation
- `crates/adagio-desktop/src/config/mod.rs`: three new fields on `SavedPair`
- No new IPC types, no UI changes — bulk upload is transparent to the API surface
- The `upload_chunked` function already handles chunk-level resume via `client.list_uploaded_chunks()`; no changes needed to the chunked upload protocol
