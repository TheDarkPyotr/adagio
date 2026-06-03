# Research: Bandwidth Throttling (009)

**Date**: 2026-05-30 | **Feature**: `009-bandwidth-throttling`

---

## Decision 1 — Reuse the existing TokenBucket from bandwidth.rs

**Decision**: The existing `TokenBucket` in `crates/adagio-core/src/bandwidth.rs` with
its `acquire(n: u64) -> Duration` API is used as the throttling primitive. No new rate
limiter is written.

**API**:
```rust
// token_bucket.acquire(bytes) → returns how long to sleep before transferring
let delay = token_bucket.acquire(chunk_size).await;
tokio::time::sleep(delay).await;
```

Rate 0 means unlimited (no sleep). The bucket refills continuously at the configured
rate (bytes/sec). The existing implementation already handles burst capacity and
sliding-window averaging.

**Rationale**: The crate already has a correct, tested leaky-bucket implementation.
Using it avoids duplicate logic. `BandwidthSchedule` (time-based scheduling) and the
existing benchmark tests are also already present.

**Alternatives considered**: Writing a custom `tokio::time::Interval`-based throttler
— not needed when the existing code already does this correctly.

---

## Decision 2 — Apply throttling chunk-by-chunk inside upload_single and download_file

**Decision**: Modify `upload_single` and `download_file` in `adagio-core/src/transfer/`
to accept an `Option<Arc<TokenBucket>>` parameter and call `acquire(chunk_size)` between
chunks. Default chunk size: 64 KB.

**Rationale**: The spec requires accuracy within 10% over any 10-second window. A
pre-flight `acquire(file_size)` for large files causes bursts: the entire file transfers
at full speed in a narrow window, violating the average. Chunk-level throttling prevents
this. `upload_single` currently reads the whole file into `Bytes`; it is refactored
to yield chunks. `download_file` already streams from the client; chunks are applied
at the write boundary.

**Current state**: `upload_single` does `bytes::Bytes::from(std::fs::read(path))`
then creates `stream::once(data)`. The refactor splits this into a chunked iterator.

**Alternatives considered**:
- Pre-flight `acquire(file_size)` — simple but inaccurate for files > 1 MB.
- Wrapping `reqwest::Body` with a throttled stream — possible but requires changes
  to `adagio-nextcloud/src/client.rs`; containment in `adagio-core` is cleaner.

---

## Decision 3 — Store BandwidthConfig per account in SavedConfig

**Decision**: Add `upload_limit_kbps: u64` and `download_limit_kbps: u64` (both
defaulting to 0 = unlimited) to `SavedAccount` in
`crates/adagio-desktop/src/config/mod.rs`, and corresponding fields to
`adagio-core::types::Account`.

**Rationale**: The spec specifies per-account limits. `SavedAccount` is the persistence
type for account data in `config.json`. Adding two `u64` fields with `#[serde(default)]`
is backward-compatible (existing configs read cleanly without the fields).

**Where NOT to store**:
- `SavedPair` — the spec explicitly says per-account, not per-pair.
- A separate file — unnecessary complexity.

**The daemon dispatcher** reads the `AccountManager` for the current limits at the start
of each `SyncCycle::run()` and passes `TokenBucket` instances into the `Propagator`.

---

## Decision 4 — Share TokenBucket via Arc for live updates without restarting cycles

**Decision**: The daemon holds `Arc<RwLock<(u64, u64)>>` per account (upload_kbps,
download_kbps). `SyncCycle::run()` reads the current value when creating the `Propagator`.
When limits change via `SetBandwidthLimits`, the daemon updates the stored value and
also updates a shared `Arc<TokenBucket>` passed into running runners.

**Practical simplification**: The `Propagator` is created fresh per cycle. For the
"takes effect within 1 second" requirement (FR-005), the new limit takes effect at
the next `acquire()` call — which for chunk-level throttling happens every 64 KB ≈
every few milliseconds. The 1-second target is easily met without complex mid-cycle
reconfiguration.

**How**: `DefaultSyncEngine` holds `Arc<RwLock<HashMap<AccountId, (u64, u64)>>>`.
`SyncCycle` reads this to build `TokenBucket` instances. When the dispatcher calls
`SetBandwidthLimits`, it writes the new values; the next `acquire()` in a running
propagator picks up the change.

---

## Decision 5 — Three new DaemonRequest variants

**Decision**: Add to `adagio-ipc/src/types.rs`:
```rust
GetBandwidthStatus { account_id: Option<String> },
SetBandwidthLimits { account_id: Option<String>, upload_kbps: u64, download_kbps: u64 },
ClearBandwidthLimits { account_id: Option<String> },
```

`account_id: None` applies to the first (and usually only) account.

**Rationale**: Clean IPC contract that separates read (`GetBandwidthStatus`) from write
(`SetBandwidthLimits`, `ClearBandwidthLimits`). The CLI maps directly to these.

---

## Decision 6 — Live throughput measured in Propagator via TransferProgress events

**Decision**: Add a `ThroughputMeter` struct to `adagio-core/src/bandwidth.rs` that
subscribes to `TransferProgress` events and maintains a sliding 10-second average.
The daemon queries it when `GetBandwidthStatus` is called.

`ThroughputMeter` is a rolling window: a `VecDeque<(Instant, u64)>` of
`(timestamp, bytes)` pairs. On each call, entries older than 10 seconds are evicted
and the rate is `total_bytes / elapsed_seconds`.

**Rationale**: `TransferProgress` is already emitted per chunk from the upload/download
functions. This is the correct data source; no additional instrumentation is needed
in the network layer.

---

## Decision 7 — CLI: bandwidth subcommand added to adagio-cli

**Decision**: Add `bandwidth` as a new top-level subcommand to `adagio-cli/src/cli.rs`
with sub-subcommands `status`, `set`, and `clear`. Handler in
`crates/adagio-cli/src/handlers/bandwidth.rs`. Maps to the three new `DaemonRequest`
variants.

**Rationale**: Follows the existing pattern (`conflicts`, `pairs`, `accounts`, `daemon`).
No new crates or dependencies needed.

---

## Scope summary

| Layer | Changes |
|---|---|
| `adagio-core/src/bandwidth.rs` | Add `ThroughputMeter` struct |
| `adagio-core/src/transfer/upload.rs` | Add `throttle: Option<Arc<TokenBucket>>` param; chunk loop |
| `adagio-core/src/transfer/download.rs` | Add `throttle: Option<Arc<TokenBucket>>` param; per-chunk acquire |
| `adagio-core/src/cycle/propagator.rs` | Pass throttle to upload/download; read from `ThroughputMeter` |
| `adagio-core/src/cycle/mod.rs` | Pass bandwidth limits from engine into SyncCycle |
| `adagio-core/src/types.rs` | Add `upload_limit_kbps`, `download_limit_kbps` to `Account` |
| `adagio-desktop/src/config/mod.rs` | Add fields to `SavedAccount`; `SavedPair` unchanged |
| `adagio-ipc/src/types.rs` | Add 3 new `DaemonRequest` variants + bandwidth response types |
| `adagio-daemon/src/dispatcher.rs` | Handle new bandwidth requests |
| `adagio-cli/src/cli.rs` | Add `bandwidth` subcommand |
| `adagio-cli/src/handlers/bandwidth.rs` | New handler file |
| React SettingsScene | Add Bandwidth section (upload/download fields + live display) |
| React `tauri.ts` | Add bandwidth bindings |
