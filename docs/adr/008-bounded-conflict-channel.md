# ADR-008: Use a bounded channel (capacity 1) for conflict notifications

**Date**: 2026-05-29
**Status**: Accepted
**Feature**: 006-sync-resource-efficiency

## Context

When the sync propagator detects an Ask-policy conflict it sends a unit signal
`()` to a channel so the desktop shell can emit the `adagio://conflict-detected`
Tauri event. The channel was originally created with
`tokio::sync::mpsc::unbounded_channel::<()>()` (`lifecycle.rs`,
`propagator.rs:64`).

An unbounded channel has no back-pressure. If the desktop receiver is slow —
e.g., the Tauri runtime is busy rendering — the sender accumulates entries
without limit. In practice, the signal is unit `()` and the receiver should
drain immediately, but the absence of a bound is an unnecessary risk.

## Decision

Replace `unbounded_channel::<()>()` with `channel::<()>(1)`:
```rust
let (conflict_tx, conflict_rx) = tokio::sync::mpsc::channel::<()>(1);
```

The sender uses `try_send(())`:
- If the channel is empty, the notification is delivered.
- If the channel already holds a `()` (`TrySendError::Full`), the sender drops
  the new signal silently. This is correct: the receiver already has a pending
  notification and will process it; sending a duplicate is redundant.
- If the receiver is dropped (`TrySendError::Closed`), the error is logged
  at `WARN` once and subsequent sends are skipped.

## Alternatives considered

**Keep `UnboundedSender`** — Rejected. Provides no benefit over a bounded
channel in this use-case; only adds a potential (if small) unbounded growth
vector.

**`watch::Sender<bool>`** — Evaluated. Would work but requires a distinct API
shape (`send(true)` vs `send(())`); the bounded `Sender<()>` is the minimal
change with the same semantics.

**`oneshot::Sender`** — Rejected. A oneshot is consumed by the first send;
subsequent conflicts within the same session would not be delivered.

## Consequences

- No functional change visible to end users or the Tauri frontend.
- Eliminates the only unbounded accumulation path identified in the memory
  audit (AC-3 compliance).
- `Propagator::with_conflict_channel()` now accepts `Sender<()>` instead of
  `UnboundedSender<()>` — a breaking change to the internal API, which is
  acceptable since there are no external callers.
