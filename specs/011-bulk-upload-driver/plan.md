# Implementation Plan: Bulk Upload Driver

**Branch**: `011-bulk-upload-driver` | **Date**: 2026-05-30 | **Spec**: [spec.md](spec.md)

---

## Summary

Add a `BulkUploadDriver` to `adagio-core` that activates transparently inside `SyncCycle::run()` when the remote is nearly empty and there are enough local files to upload. The driver uploads files in parallel using a `JoinSet`-based worker pool, reusing the existing `upload_chunked` / `upload_single` functions. Progress is forwarded via the existing `EventBroadcaster`. No new IPC variants are needed.

**Key ADR**: ADR-015 — BulkUploadDriver activates in `SyncCycle::run()` between reconciliation and propagation; reuses `upload_chunked`/`upload_single`; `JoinSet` worker pool; existing `TransferProgress` events; `upsert_batch` for atomic journal writes.

---

## Technical Context

**Language/Version**: Rust stable (edition 2021, MSRV 1.83)

**Primary Dependencies** (all existing):
- `tokio` — `JoinSet`, `Semaphore`, `mpsc` channels
- `adagio-nextcloud::chunked::upload_chunked` — chunked upload with resume
- `adagio-core::transfer::upload::upload_single` — single PUT upload
- `adagio-core::journal::sqlite::upsert_batch` — atomic batch journal writes
- `adagio-daemon::events::EventBroadcaster` — progress forwarding

**Storage**: `config.json` extended with three `#[serde(default)]` fields on `SavedPair`

**Testing**: `cargo test`, `MockRemoteClient`, call-count assertions for resume

**Target Platform**: Linux, macOS, Windows

**Performance Goals**: ≥ 6× throughput improvement over sequential with 8 workers

**Constraints**: No new IPC types; all new logic in `adagio-core`; `upsert_batch` for atomicity

---

## Constitution Check

| Gate | Principle | Status |
|------|-----------|--------|
| Tests authored and FAIL before implementation begins | I. Test-First | ✅ planned |
| All public Rust items have `///` doc comments | II. Documentation as Code | ✅ enforced |
| ADR-015 recorded in `docs/adr/` | II. Documentation as Code | ✅ planned |
| Structured logging for bulk driver lifecycle events | III. Observability | ✅ planned |
| No `println!` in production code | III. Observability | ✅ enforced |
| `BulkUploadDriver` in `adagio-core` — no direct daemon coupling | IV. Extensibility | ✅ planned |
| Driver accessed via function call — no tight coupling | IV. Extensibility | ✅ planned |
| Idle memory: no long-lived heap beyond active session | V. Performance-Oriented | ✅ JoinSet drops on completion |
| UI feedback ≤ 100 ms: progress events forwarded immediately | V. Performance-Oriented | ✅ existing event path |
| `cargo clippy -- -D warnings` passes | Dev Workflow | ✅ enforced |
| `cargo fmt --check` passes | Dev Workflow | ✅ enforced |
| No new `unsafe` blocks | Dev Workflow | ✅ not needed |
| All three platform CI targets pass | Technology | ✅ no platform-specific code |

---

## Architecture

```
adagio-core/src/
└── bulk_upload.rs           — BulkUploadDriver, BulkUploadResult, should_activate()

adagio-core/src/cycle/
└── mod.rs                   — activation check + driver call in SyncCycle::run()

adagio-core/src/types.rs     — 3 bulk fields added to SyncPair
adagio-core/src/lib.rs       — pub mod bulk_upload

adagio-desktop/src/config/mod.rs  — 3 bulk fields added to SavedPair
adagio-daemon/src/dispatcher.rs   — bulk fields in save_config + CreatePair

docs/adr/015-bulk-upload-driver.md
```

No new crates, no new IPC types, no UI changes.

---

## File Structure

| File | Action |
|------|--------|
| `crates/adagio-core/src/bulk_upload.rs` | CREATE — `BulkUploadDriver`, `BulkUploadResult`, `should_activate()` |
| `crates/adagio-core/src/lib.rs` | MODIFY — `pub mod bulk_upload` |
| `crates/adagio-core/src/types.rs` | MODIFY — 3 bulk fields on `SyncPair` |
| `crates/adagio-core/src/cycle/mod.rs` | MODIFY — activation check + driver call in `SyncCycle::run()` |
| `crates/adagio-desktop/src/config/mod.rs` | MODIFY — 3 bulk fields on `SavedPair` |
| `crates/adagio-daemon/src/dispatcher.rs` | MODIFY — bulk fields in `build_saved_config` + `save_config` |
| `docs/adr/015-bulk-upload-driver.md` | CREATE |

---

## Key Design Decisions

### BulkUploadDriver API
```rust
pub struct BulkUploadDriver<'a> {
    pair:        &'a SyncPair,
    client:      &'a dyn RemoteClient,
    journal:     &'a dyn Journal,
    broadcaster: Option<EventBroadcaster>,
}

impl<'a> BulkUploadDriver<'a> {
    /// True when bulk mode should activate for this cycle.
    pub fn should_activate(remote_count: usize, upload_count: usize, pair: &SyncPair) -> bool;

    /// Upload all pending ops in parallel; returns summary.
    pub async fn run(
        &self,
        upload_ops: Vec<SyncOp>,
        local_root: &LocalPath,
        remote_root: &RemotePath,
    ) -> BulkUploadResult;
}
```

### Activation in SyncCycle::run()
```
1. After reconcile: split plan.ops into upload_ops + other_ops
2. Check should_activate(remote_items.len(), upload_ops.len(), &pair)
3a. Bulk mode: BulkUploadDriver::run(upload_ops, ...)
    Then propagate other_ops via standard Propagator
3b. Standard mode: re-join upload_ops into plan.ops, run full Propagator as before
```

### Worker pool
Tokio `JoinSet` + `Arc<Semaphore>` with `bulk_upload_workers` permits. Each task acquires a permit, uploads one file (chunked or single based on size), writes journal entry, releases permit.

### Chunk threshold routing
```
if file_size >= pair.bulk_upload_chunk_threshold_bytes:
    upload_chunked(...)   // handles chunk resume internally
else:
    upload_single(...)
```

### Progress forwarding
Each worker sends `TransferProgress` events on an `mpsc` channel. A lightweight `tokio::spawn` task reads the channel and calls `broadcaster.emit_transfer_progress()` for each event.

### Journal atomicity
`upsert_batch` writes all completed-file entries in a single SQLite transaction. Written after each worker completes (not buffered until all workers finish), so interruption preserves partial progress.

---

## Shell Commands

```bash
# Build
cargo build -p adagio-core -p adagio-daemon

# Unit + lib tests
cargo test --lib -p adagio-core

# Full suite
cargo test --workspace

# Format + lint
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```
