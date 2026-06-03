# Data Model: Sync Resource Efficiency (006)

**Date**: 2026-05-29 | **Feature**: `006-sync-resource-efficiency`

This feature is purely behavioural — it introduces no new database tables and no
new Tauri IPC commands. The changes are confined to:
1. A new `CycleMetrics` log-event struct.
2. A new `MemorySampler` utility struct (no persistent state).
3. Modified field types on two existing structs (`PairRunner`, `SyncCycle`).

---

## New: `CycleMetrics` (log event, not stored)

Emitted as a single structured `tracing::debug!` event at the end of each scan
cycle. It is never persisted; it exists only in the log stream.

```rust
/// Structured telemetry emitted at the end of every scan cycle.
///
/// Consumed by log aggregators to verify AC-1 and AC-4.
#[derive(Debug)]
pub struct CycleMetrics {
    /// Total files examined during this cycle.
    pub path_count: usize,
    /// Files whose checksum was recomputed (mtime/size changed).
    pub checksum_recomputed: usize,
    /// Files whose cached checksum was reused (mtime/size unchanged).
    pub checksum_from_cache: usize,
    /// Wall-clock duration of the full scan + propagation pass.
    pub duration_ms: u64,
}
```

**Location**: `crates/adagio-core/src/cycle/mod.rs` (emitted in `SyncCycle::run`
after propagation completes).

---

## New: `MemorySample` (log event, not stored)

Emitted as a structured `tracing::debug!` event at cycle end, and as a `WARN`
when growth exceeds 15%.

```rust
/// One RSS measurement taken at the end of a scan cycle.
#[derive(Debug)]
pub struct MemorySample {
    /// Resident set size at this moment, in bytes.
    pub rss_bytes: u64,
    /// RSS at first-minute baseline. `None` until baseline is set.
    pub baseline_rss: Option<u64>,
    /// Growth ratio relative to baseline. `None` until baseline is set.
    pub growth_ratio: Option<f64>,
}
```

**Location**: `crates/adagio-core/src/observability.rs` (new `MemorySampler`
struct; sample emitted from `SyncCycle::run`).

---

## New: `MemorySampler` struct

```rust
/// Samples resident set size using platform-specific OS interfaces.
///
/// Platform implementations:
/// - Linux: reads `/proc/self/statm` (field 1 × page size).
/// - macOS: calls `proc_info(PROC_PID_RUSAGE, ...)`.
/// - Windows: calls `GetProcessMemoryInfo`.
pub struct MemorySampler {
    baseline_rss: Option<u64>,
}

impl MemorySampler {
    pub fn new() -> Self { ... }

    /// Set the baseline RSS. Called once, 60 s after startup.
    pub fn set_baseline(&mut self, rss: u64);

    /// Read current RSS. Returns `None` if the OS call fails.
    pub fn sample_rss() -> Option<u64>;

    /// Compute and log a `MemorySample`. Emits WARN if growth > 15%.
    pub fn observe(&self) -> MemorySample { ... }
}
```

**Location**: `crates/adagio-core/src/observability.rs`.

---

## Modified: `PairRunner` — interval tick behaviour

```rust
// Before (runner.rs:45)
let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));

// After
let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
```

No field-level change to `PairRunner`. One line added in `PairRunner::spawn`.

---

## Modified: `SyncCycle` — bounded conflict notification channel

```rust
// Before (cycle/mod.rs)
pub conflict_tx: Option<&'a tokio::sync::mpsc::UnboundedSender<()>>,

// After
pub conflict_tx: Option<&'a tokio::sync::mpsc::Sender<()>>,
```

The channel capacity is set to `1` at construction:

```rust
// Before (lifecycle.rs or lib.rs, wherever the channel is created)
let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<()>();

// After
let (tx, rx) = tokio::sync::mpsc::channel::<()>(1);
```

Downstream callers of `conflict_tx.send(()).await` become
`conflict_tx.try_send(())` with `TrySendError::Full` silently ignored (full
channel means the consumer already has a pending notification).

---

## Modified: retry log event in `propagator.rs`

```rust
// Before — in with_retry() on transient error
tracing::warn!(attempt, "transient error; retrying");

// After
tracing::warn!(
    retry_attempt = attempt,
    delay_ms = delay.as_millis() as u64,
    "transient error — retrying after backoff"
);
```

No struct changes. Two new fields added to an existing `tracing::warn!` call.

---

## No database changes

This feature adds no new SQLite tables, columns, or migrations.

## No new Tauri IPC commands

All new behaviour is internal to `adagio-core`. The `adagio-desktop` crate
requires only the channel-type change in the caller that creates the conflict
notification channel.
