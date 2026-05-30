# Research: Bulk Upload Driver (011)

## 1. Existing chunked upload API

### Decision: Reuse `upload_chunked` from `crates/adagio-nextcloud/src/chunked.rs`

**Signature:**
```rust
pub async fn upload_chunked(
    client: &dyn RemoteClient,
    local_path: &LocalPath,
    remote_path: &RemotePath,
    opts: &TransferOptions,   // chunked_threshold, chunk_size
    progress: mpsc::Sender<TransferProgress>,
) -> Result<UploadResult, TransferError>
```

**Chunk resume:** `client.list_uploaded_chunks(&session_url)` returns `Vec<u32>` of already-uploaded chunk indices. `upload_chunked` skips them automatically — no extra logic needed in the bulk driver.

**Alternatives considered:** Implement a new parallel chunked uploader. Rejected: the existing implementation already handles session creation, resume, and finalization. The bulk driver parallelizes across FILES (not within a single file's chunks), so the existing single-file chunked path is correct as-is.

---

## 2. Activation point in SyncCycle::run()

### Decision: Insert bulk driver check between reconciliation and propagation

In `crates/adagio-core/src/cycle/mod.rs`, `SyncCycle::run()`:
1. Local + remote snapshots built (lines ~148–175)
2. `reconciler::reconcile(...)` produces `plan.ops` (line ~178)
3. **← INSERT BULK CHECK HERE** — inspect `plan.ops` for Upload count and remote file count
4. `Propagator::execute(&plan.ops, ...)` runs (line ~198)

The bulk driver replaces step 4 for Upload ops when conditions are met. Non-Upload ops (Download, DeleteLocal, etc.) always go to the propagator.

**Condition check:**
```
remote_item_count < local_file_count × 0.10   // remote is "nearly empty"
AND upload_count > pair.bulk_upload_threshold_files  // enough to bulk
AND !bulk_already_in_progress(pair_id)
```

`remote_item_count` is `remote_items.len()` (already computed). `upload_count` is `plan.ops.uploads().len()`. No extra network calls.

---

## 3. Concurrency model

### Decision: Tokio `JoinSet` with semaphore-bounded workers, reusing propagator pattern

Current propagator: `Arc<Semaphore>` with `max_upload_concurrency` permits.

Bulk driver: same pattern with `bulk_upload_workers` as the permit count. `JoinSet` collects worker futures; each worker acquires a semaphore permit, uploads one file, releases the permit.

`max_upload_concurrency` (default 3) → `bulk_upload_workers` (default 8). The higher default is intentional: bulk mode is optimized for throughput on an empty remote where there's no steady-state maintenance overhead.

---

## 4. New fields on SyncPair / SavedPair

### Decision: Add three fields with `#[serde(default)]` to both structs

Neither `SyncPair` (core/src/types.rs) nor `SavedPair` (desktop/config/mod.rs) currently has bulk fields.

New fields (all with `#[serde(default)]` for backward compat):
```rust
pub bulk_upload_workers: u8,           // default 8
pub bulk_upload_threshold_files: u32,  // default 50
pub bulk_upload_chunk_threshold_bytes: u64,  // default 10_485_760 (10 MB)
```

`TransferOptions::chunked_threshold` is already used by `upload_chunked`; the bulk driver sets it from `bulk_upload_chunk_threshold_bytes` when building the opts.

---

## 5. Progress reporting

### Decision: Forward `TransferProgress` events directly to `EventBroadcaster`

The daemon's `EventBroadcaster::emit_transfer_progress()` broadcasts `DaemonEvent::TransferProgress { pair_id, path, bytes_done, bytes_total }` to all IPC subscribers. The desktop app and CLI already listen for these events.

The bulk driver receives an `mpsc::Sender<TransferProgress>` channel and a reference to the `EventBroadcaster`. A lightweight forwarder task reads from the channel and calls `emit_transfer_progress()`.

`upsert_batch()` in `SqliteJournal` wraps multiple journal writes in a single SQLite transaction — ideal for writing batches of completed uploads atomically.

---

## 6. ADR to record

**ADR-015**: BulkUploadDriver activates in `SyncCycle::run()` when remote is nearly empty and upload count exceeds threshold; reuses `upload_chunked`/`upload_single` with a `JoinSet`-based worker pool; progress forwarded via existing `EventBroadcaster`; no new IPC variants required.
