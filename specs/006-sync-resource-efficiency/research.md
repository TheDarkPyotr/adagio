# Research: Sync Resource Efficiency (006)

**Date**: 2026-05-29 | **Feature**: `006-sync-resource-efficiency`

---

## Decision 1 — Missed-tick behaviour in cycle scheduler

**Decision**: Change `tokio::time::interval` in `runner.rs` to use
`MissedTickBehavior::Skip` instead of the default `MissedTickBehavior::Burst`.

**Rationale**: Tokio's default `Burst` fires all accumulated ticks immediately
if a cycle runs longer than `scan_interval_secs`. On a loaded machine a single
slow network call could cause the next cycle to start before the previous one
fully completes, producing visible CPU spikes. `Skip` treats a late completion
as a single missed tick and waits one full interval before the next scan — the
correct semantics for a background sync client.

`Delay` was also evaluated but resets the timer from the missed-tick moment
rather than the next scheduled boundary, which produces a variable scan rhythm
that is harder to reason about in logs.

**Code location**: `crates/adagio-core/src/cycle/runner.rs:45` —
`tokio::time::interval(Duration::from_secs(interval_secs))`.

**Alternatives considered**:
- `MissedTickBehavior::Delay` — excluded; introduces non-deterministic drift in
  scan cadence.
- OS file-watch (inotify / FSEvents / ReadDirectoryChangesW) — out of scope for
  this feature; would reduce idle CPU further but requires platform-specific
  integration (see future feature 007).

---

## Decision 2 — Checksum skip guard is already implemented; needs observability

**Decision**: No change to the checksum-skip logic in `detection/local.rs`. Add
structured cycle-end metrics so the guard is auditable in production.

**Rationale**: The scanner at `local.rs:170-180` already skips `std::fs::read()`
when `cached.size == size && mtime_diff < 2s`. In a fully-synced idle folder,
100% of files hit this path. The gap is that there is no structured log event
recording how many files were skipped vs. recomputed, making the "under 1% CPU"
claim impossible to audit from logs alone.

**Alternatives considered**:
- Content-based deduplication (compare checksum against remote ETag on every
  stat) — unnecessary overhead; mtime+size guard is sufficient for detecting
  local changes.
- Switching from polling to OS-level file-watch for changed-file detection —
  deferred to feature 007; out of scope here.

---

## Decision 3 — Bounded channel for conflict notifications

**Decision**: Replace `Option<UnboundedSender<()>>` in `SyncCycle` with
`Option<Sender<()>>` (bounded, capacity 1).

**Rationale**: The conflict notification channel sends a unit signal `()`; it
communicates the *presence* of at least one conflict, not a count. An unbounded
channel here provides no additional value — the receiver only needs to know
"at least one conflict happened" before it queries the journal for details.
Making it bounded (capacity 1) provides backpressure for free: if the desktop
shell is slow to drain the event, the propagator's `try_send` returns
`TrySendError::Full` instead of silently accumulating entries.

**Alternatives considered**:
- Keep `UnboundedSender` — rejected; provides no observable benefit and is the
  only unbounded accumulator identified in the memory audit.
- Use a `watch::Sender<bool>` — possible, but requires a different API surface;
  bounded `Sender<()>` is the minimal change.

---

## Decision 4 — RSS sampling via platform-specific proc interfaces

**Decision**: Implement `MemorySampler` using `/proc/self/statm` (Linux),
`proc_info` / `RUSAGE_SELF` (macOS), and `GetProcessMemoryInfo` (Windows).
Sample at startup (baseline) and at each cycle completion. Emit as a structured
`DEBUG` log field `rss_bytes`. Emit `WARN` when `rss_now > baseline * 1.15`
(early warning) and `ERROR` when `rss_now > baseline * 1.20` (hard limit).

**Rationale**: The codebase already uses `tracing` for structured logs. Adding
RSS as a log field costs nothing at INFO level (the call is cheap) and makes the
AC-3 guarantee auditable without attaching a profiler. The 15% warning threshold
gives a one-week window to investigate before hitting the 20% SLA boundary.

**Alternatives considered**:
- `sysinfo` crate — adds a dependency; `/proc/self/statm` direct read is 3 lines
  and has zero overhead.
- Emit via Tauri event to frontend — out of scope; this is a backend metric.
- `jemalloc` stats — platform-specific and requires a different allocator.

---

## Decision 5 — Retry observability (no new rate limiter needed)

**Decision**: Add `retry_attempt` and `delay_ms` structured fields to the retry
log event in `propagator.rs`. No token-bucket rate limiter is needed.

**Rationale**: The existing exponential backoff starts at 1,000 ms base delay,
meaning at most one retry per second per path at the tightest interval. This
already satisfies the "≤ 3 identical requests per second" criterion. The gap is
that there are no log fields making this auditable, so a log grep cannot confirm
compliance. Adding two fields to the existing `tracing::warn!` in `with_retry`
closes the observability gap without adding a new data structure.

**Alternatives considered**:
- Per-path token bucket — unnecessary overhead given existing backoff already
  enforces ≤1 req/s; would add state and complexity.
- Global rate limiter (`tower::ServiceBuilder`) — over-engineering; the backoff
  already guarantees the bound.

---

## Summary of implementation scope

| AC | Root cause | Fix | Complexity |
|----|-----------|-----|------------|
| AC-1 (CPU ≤ 1%) | `MissedTickBehavior::Burst` can fire cycles back-to-back | Switch to `Skip`; add cycle-end metrics | Low |
| AC-2 (No file locks) | `std::fs::read()` semantics unclear | No code change; add test + doc | Low |
| AC-3 (Memory ≤ +20%) | `UnboundedSender<()>` has no backpressure | Bound to capacity 1; add RSS sampler | Low |
| AC-4 (≤ 3 req/s) | Retry log lacks timing fields | Add `retry_attempt` + `delay_ms` log fields | Low |
