# Implementation Plan: Bandwidth Throttling

**Branch**: `009-bandwidth-throttling` | **Date**: 2026-05-30 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/009-bandwidth-throttling/spec.md`

---

## Summary

Add configurable upload and download speed limits to Adagio. A leaky-bucket
`TokenBucket` (already present in `adagio-core/src/bandwidth.rs`) is plumbed into
`upload_single` and `download_file` at the 64 KB chunk level. Limits are stored
per-account in `config.json`, configurable from both the desktop Settings panel and
the CLI (`adagio bandwidth status/set/clear`). Live throughput is measured via a
new `ThroughputMeter` and exposed through the daemon IPC.

**No new crates.** Changes span six existing crates and the React frontend.

---

## Technical Context

**Language/Version**: Rust stable 1.78+ (edition 2021); TypeScript 5.x / React 18

**Primary Dependencies** (all already in workspace):
- `tokio` — async chunk-by-chunk sleep
- `adagio-core/src/bandwidth.rs` — existing `TokenBucket` with `acquire(n) -> Duration`
- `serde_json` — JSON shapes for IPC and config
- `clap v4` — new `bandwidth` CLI subcommand

**Storage**: `config.json` — two new `u64` fields on `SavedAccount` with `#[serde(default)]`.
No database migration.

**Testing**: `cargo test --lib -p adagio-core` (unit + accuracy benchmark);
`cargo test --lib -p adagio-cli`.

**Target Platform**: Linux, macOS, Windows.

**Performance Goals**:
- Throughput within 10% of configured limit under sustained load (SC-001).
- Zero overhead (< 1% CPU) when no limit is configured (SC-002).
- Limit changes take effect within 1 second at the next 64 KB boundary (SC-003).

**Constraints**:
- `adagio-core` must not gain new crate dependencies.
- `upload_single` and `download_file` signatures change — all callers must be updated.
- Throttle is `None` by default; no behaviour change when limits are zero/absent.

**Scale/Scope**: ~400 lines new Rust code; ~80 lines new React code.

---

## Constitution Check

| Gate | Principle | Status |
|------|-----------|--------|
| Tests authored and FAIL before implementation begins | I. Test-First | ✅ |
| All public Rust items have `///` doc comments | II. Documentation as Code | ✅ (`ThroughputMeter`, modified signatures) |
| ADR recorded in `docs/adr/` for significant decisions | II. Documentation as Code | ✅ (ADR-013: chunk-level throttling) |
| Structured logging for new sync/network operations | III. Observability | ✅ (throttle sleep durations logged at TRACE) |
| No `println!` in production code paths | III. Observability | ✅ |
| New feature as independent module with no extra coupling | IV. Extensibility | ✅ (`TokenBucket` already decoupled; throttle is `Option`) |
| Cross-module calls through defined contracts | IV. Extensibility | ✅ (IPC contract in `contracts/bandwidth-ipc.md`) |
| Idle memory budget < 100 MB RSS | V. Performance-Oriented | ✅ (ThroughputMeter is a small deque; TokenBucket is tiny) |
| UI actions within 100 ms | V. Performance-Oriented | ✅ (Settings save triggers IPC; no blocking) |
| Benchmarks for hot-path changes | V. Performance-Oriented | ✅ (accuracy benchmark test added) |
| `cargo clippy -- -D warnings` | Dev Workflow | ✅ |
| `cargo fmt --check` | Dev Workflow | ✅ |
| All `unsafe` blocks have `// SAFETY:` | Dev Workflow | ✅ (no new unsafe) |
| All three platform CI targets pass | Technology | ✅ |

---

## Project Structure

### Documentation

```text
specs/009-bandwidth-throttling/
├── plan.md               ← this file
├── spec.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   └── bandwidth-ipc.md
└── tasks.md
```

### Source Code Changes

```text
docs/adr/013-chunk-level-throttling.md        ← NEW

crates/adagio-core/src/bandwidth.rs           ← ADD ThroughputMeter struct
crates/adagio-core/src/types.rs               ← ADD upload_limit_kbps, download_limit_kbps to Account
crates/adagio-core/src/transfer/upload.rs     ← ADD throttle param; chunk loop with acquire()
crates/adagio-core/src/transfer/download.rs   ← ADD throttle param; per-chunk acquire()
crates/adagio-core/src/transfer/engine.rs     ← Update callers of upload_single/download_file
crates/adagio-core/src/cycle/propagator.rs    ← ADD upload_throttle/download_throttle fields;
                                               ←   pass throttle to upload_single/download_file;
                                               ←   record bytes in ThroughputMeter
crates/adagio-core/src/cycle/mod.rs           ← Pass bandwidth limits from engine into SyncCycle;
                                               ←   DefaultSyncEngine stores per-account limits

crates/adagio-desktop/src/config/mod.rs       ← ADD upload_limit_kbps/download_limit_kbps to
                                               ←   SavedAccount; update restore_accounts()

crates/adagio-ipc/src/types.rs                ← ADD 3 new DaemonRequest variants

crates/adagio-daemon/src/dispatcher.rs        ← ADD handlers for bandwidth requests;
                                               ←   update build_saved_config()

crates/adagio-cli/src/cli.rs                  ← ADD Commands::Bandwidth { command: BandwidthCommand }
crates/adagio-cli/src/handlers/bandwidth.rs   ← NEW

crates/adagio-desktop/src-ui/src/tauri.ts     ← ADD getBandwidthStatus, setBandwidthLimits, clearBandwidthLimits
crates/adagio-desktop/src-ui/src/components/SettingsScene.tsx  ← ADD Bandwidth section
```

---

## Implementation Phases

### Phase 1: Setup — ADR

**Deliverables**:
1. `docs/adr/013-chunk-level-throttling.md` — document the decision to throttle
   at the 64 KB chunk level inside `upload_single` / `download_file` rather than
   using pre-flight per-file `acquire(file_size)`; explain why the latter causes
   bursts for large files.

---

### Phase 2: Core — ThroughputMeter + Account type changes

**Deliverables**:
1. `bandwidth.rs` — `ThroughputMeter` struct: `new()`, `record(bytes: u64)`,
   `rate_bytes_per_sec() -> u64`; `///` doc comments.
2. `types.rs` — add `upload_limit_kbps: u64` and `download_limit_kbps: u64` to
   `Account` with `#[serde(default)]`.

**Tests** (failing first):
- `throughput_meter_empty_returns_zero` — new meter returns 0.
- `throughput_meter_single_record` — record 10 000 bytes, read immediately, rate ≈ 0
  (bucket is fresh; test that rate grows with time mock).
- `throughput_meter_evicts_old_entries` — entries older than window_secs are pruned.
- `account_default_limits_are_zero` — `Account { ..Default::default() }` has zero limits.

---

### Phase 3: US1+US2 — Chunk-level throttling in upload/download

**Deliverables**:
1. `transfer/upload.rs` — add `throttle: Option<Arc<TokenBucket>>` parameter to
   `upload_single`; refactor from whole-file read to 64 KB chunk loop; call
   `throttle.acquire(chunk_len).await` before writing each chunk.
2. `transfer/download.rs` — add `throttle` parameter; call `acquire(chunk_len)` at
   the write boundary of each chunk.
3. `transfer/engine.rs` — update all callers of `upload_single` and `download_file`
   to pass `None` (no throttle — engine tests are unchanged).
4. `cycle/propagator.rs` — add `upload_throttle: Option<Arc<TokenBucket>>` and
   `download_throttle: Option<Arc<TokenBucket>>` fields; add
   `with_bandwidth(upload: Arc<TokenBucket>, download: Arc<TokenBucket>)` constructor;
   pass throttle to `upload_single` and `download_file`; call
   `upload_meter.record(bytes)` and `download_meter.record(bytes)` on completion.
5. `cycle/mod.rs` — `DefaultSyncEngine` stores
   `Arc<RwLock<HashMap<AccountId, (u64, u64)>>>` for limits; `SyncCycle::run()`
   reads the limits and constructs `TokenBucket` instances passed to `Propagator`.

**Tests** (failing first):
- `upload_single_with_throttle_slows_transfer` — mock client; set 100 Kbps limit;
  upload 200 KB; assert elapsed ≥ 1.8 s (within 10% tolerance).
- `download_file_with_throttle_slows_transfer` — equivalent download test.
- `throttle_none_does_not_sleep` — upload_single with `None` throttle should
  not call `tokio::time::sleep` (verify via timing — completes in < 100 ms for mock).
- `propagator_records_upload_bytes_in_meter` — after propagator executes Upload op,
  upload_meter.rate_bytes_per_sec() > 0.

---

### Phase 4: US3+US4 — IPC and daemon support

**Deliverables**:
1. `adagio-ipc/src/types.rs` — add `GetBandwidthStatus`, `SetBandwidthLimits`,
   `ClearBandwidthLimits` to `DaemonRequest`.
2. `adagio-daemon/src/dispatcher.rs` — handle the three new variants:
   - `GetBandwidthStatus` → read limits from `AccountManager` + rates from meters;
     return JSON.
   - `SetBandwidthLimits` → update `Account.upload_limit_kbps` / `download_limit_kbps`
     in `AccountManager`; update the engine's shared limits map; persist config.
   - `ClearBandwidthLimits` → equivalent to `SetBandwidthLimits { 0, 0 }`.
3. `adagio-desktop/src/config/mod.rs` — add fields to `SavedAccount`; update
   `restore_accounts()` to read them.

**Tests** (failing first):
- `dispatcher_get_bandwidth_status_returns_correct_limits` — set limits on account,
  call dispatch, assert response JSON has correct values.
- `dispatcher_set_bandwidth_limits_persists_to_config` — set limits, verify
  `config.json` contains the new values.
- `dispatcher_clear_bandwidth_limits_zeros_both` — clear, assert both zero.

---

### Phase 5: US4 — CLI bandwidth subcommand

**Deliverables**:
1. `adagio-cli/src/cli.rs` — add `Commands::Bandwidth { command: BandwidthCommand }`;
   add `BandwidthCommand { Status, Set { upload_kbps, download_kbps }, Clear }`.
2. `adagio-cli/src/handlers/bandwidth.rs` — implement `run_bandwidth_status`,
   `run_bandwidth_set`, `run_bandwidth_clear`; human and JSON output modes.
3. `adagio-cli/src/run.rs` — wire `Commands::Bandwidth`.

**Tests** (failing first):
- `cli_parses_bandwidth_set_both_flags` — assert correct variant and values.
- `cli_parses_bandwidth_clear` — assert `BandwidthCommand::Clear`.
- `bandwidth_status_json_is_valid` — stub client, call handler with json=true,
  assert output is parseable JSON.

---

### Phase 6: US3+US5 — Desktop Settings panel + live display

**Deliverables**:
1. `tauri.ts` — add `getBandwidthStatus()`, `setBandwidthLimits(upload, download)`,
   `clearBandwidthLimits()` bindings.
2. `SettingsScene.tsx` — add "Bandwidth" sub-section in the Sync tab:
   - Upload limit input (Kbps; blank = unlimited)
   - Download limit input (Kbps; blank = unlimited)
   - Save button → calls `setBandwidthLimits`
   - Live throughput: "Current upload: X KB/s", "Current download: X KB/s"
     (polls `getBandwidthStatus` every 2 s while section is visible)
   - Validation: numeric > 0 or blank; red border on invalid input

**Tests** (failing first):
- `SettingsScene renders bandwidth section when Sync tab open` — assert inputs exist.
- `bandwidth save calls setBandwidthLimits with parsed values` — enter 300, assert
  invoke called with upload=300, download=0.
- `bandwidth invalid input shows error` — enter "abc", assert error visible, save
  button disabled.

---

### Phase 7: Polish & Quality Gates

1. `cargo clippy -- -D warnings` — 0 warnings
2. `cargo fmt --all --check` — passes
3. `cargo test --lib -p adagio-core` — all tests pass (133+ existing + new bandwidth)
4. `cargo test --lib -p adagio-ipc` — all pass
5. `cargo test -p adagio-daemon` — all pass
6. `cargo test --lib -p adagio-desktop` — all pass
7. `npm run test` — all pass
8. `cargo bench -p adagio-core -- bandwidth` — accuracy benchmark passes
9. `quickstart.md` manual validation — all 5 user stories pass

---

## Phase Dependencies

- **Phase 1** (ADR): No dependencies — start immediately.
- **Phase 2** (ThroughputMeter + types): Depends on Phase 1.
- **Phase 3** (throttling in transfer layer): Depends on Phase 2.
- **Phase 4** (IPC + daemon): Depends on Phase 2; can begin in parallel with Phase 3.
- **Phase 5** (CLI): Depends on Phase 4.
- **Phase 6** (Settings UI): Depends on Phase 4.
- **Phase 7** (Polish): Depends on all prior phases.

Phases 3, 4, and 6 can proceed in parallel once Phase 2 is complete.
