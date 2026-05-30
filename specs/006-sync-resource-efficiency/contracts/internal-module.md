# Internal Module Contracts: Sync Resource Efficiency (006)

This feature has no new public Tauri IPC commands or user-facing API surface.
All contracts are internal boundaries between `adagio-core` modules.

---

## Contract 1: `MemorySampler` — observability interface

**Module**: `adagio_core::observability`

```rust
impl MemorySampler {
    /// Create a new sampler. Does not read RSS yet.
    pub fn new() -> Self;

    /// Record the RSS baseline. MUST be called exactly once,
    /// at least 60 seconds after process start and after the
    /// first sync cycle has completed.
    ///
    /// Panics if called a second time (programming error guard).
    pub fn set_baseline(&mut self, rss: u64);

    /// Read current RSS from the OS. Returns `None` on OS error.
    /// Never panics. Thread-safe (no internal mutable state).
    pub fn sample_rss() -> Option<u64>;

    /// Read RSS, build a `MemorySample`, emit structured logs,
    /// and return the sample. If `sample_rss()` returns `None`,
    /// emits a single WARN and returns a sample with all fields
    /// `None`.
    pub fn observe(&self) -> MemorySample;
}
```

**Guarantees**:
- `sample_rss()` completes in < 1 ms on all three platforms.
- `observe()` never blocks on I/O; the `/proc/self/statm` read is a single
  `read(2)` syscall on Linux.
- `observe()` is safe to call from async context (no `spawn_blocking` needed).

---

## Contract 2: `CycleMetrics` — scan cycle telemetry

**Module**: `adagio_core::cycle`

`CycleMetrics` is constructed and emitted inside `SyncCycle::run`. Its fields
are populated by aggregating counters incremented during the local scan pass.
The scan must update these counters:

| Counter | Incremented when |
|---------|-----------------|
| `path_count` | Any file or directory examined by the scanner |
| `checksum_recomputed` | `compute_checksum()` is called (mtime/size changed) |
| `checksum_from_cache` | Cached checksum is returned (mtime/size unchanged) |

**Invariant**: `checksum_recomputed + checksum_from_cache == path_count`
(every examined path falls into exactly one bucket).

---

## Contract 3: `conflict_tx` channel — bounded notification

**Modules**: `adagio_core::cycle::propagator` (sender) → `adagio_desktop::lib`
(receiver)

```
Sender side (propagator):
  - Use `conflict_tx.try_send(())`.
  - If `TrySendError::Full`: drop silently.
    (Receiver already has a pending notification; re-sending is redundant.)
  - If `TrySendError::Closed`: log WARN once; do not retry.

Receiver side (adagio-desktop lib):
  - Drain the channel promptly (within one event-loop tick of receiving).
  - After draining, query the journal for the actual conflict count.
```

**Capacity**: 1. This is intentional — the channel signals presence of at least
one conflict; the receiver is expected to query the journal for details.

---

## Contract 4: `PairRunner` interval — missed-tick semantics

The timer in `PairRunner::spawn` MUST use `MissedTickBehavior::Skip`. No caller
may override this to `Burst` or `Delay`. The only way to trigger an immediate
cycle out-of-band is via `PairRunner::trigger()`, which sends on the existing
bounded `mpsc::channel::<()>(1)`.

This contract prevents the "catch-up storm" where a slow cycle causes the
scheduler to fire multiple scans in rapid succession immediately after.
