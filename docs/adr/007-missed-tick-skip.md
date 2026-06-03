# ADR-007: Use MissedTickBehavior::Skip for sync interval timer

**Date**: 2026-05-29
**Status**: Accepted
**Feature**: 006-sync-resource-efficiency

## Context

The sync cycle scheduler uses `tokio::time::interval()` in `PairRunner::spawn`
(`cycle/runner.rs`). Tokio's default `MissedTickBehavior` is `Burst`: if a cycle
runs longer than `scan_interval_secs`, all accumulated ticks fire back-to-back
immediately when the cycle completes. On a loaded machine — network latency,
large folder, slow Nextcloud response — a single over-time cycle triggers
several immediate follow-up cycles, causing a CPU/network burst.

## Decision

Set `MissedTickBehavior::Skip` immediately after constructing the interval:
```rust
ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
```

`Skip` discards any ticks that fired during the running cycle and waits for the
next scheduled boundary. The net effect: a cycle that takes longer than
`scan_interval_secs` simply defers the next scan to the following interval.

## Alternatives considered

**`MissedTickBehavior::Burst`** (default) — Rejected. It is the root cause of
back-to-back cycles after a slow scan. Violates AC-1 (CPU ≤ 1% at idle) when
the network is temporarily slow.

**`MissedTickBehavior::Delay`** — Evaluated. It resets the timer from the
moment the missed tick is detected, producing a variable cadence. For a sync
client, the absolute schedule matters less than the invariant that no more than
one cycle starts per `scan_interval_secs`. `Skip` provides that invariant
cleanly; `Delay` only approximates it.

## Consequences

- Sync cadence is slightly less regular under load (a slow cycle delays the
  next scan by up to one full interval rather than catching up).
- CPU stays idle between cycles even when previous cycles ran long.
- Out-of-band triggers (`PairRunner::trigger()`) remain unaffected; they bypass
  the interval entirely via the bounded `mpsc::channel::<()>(1)` trigger channel.
