# ADR 006: Conflict Resolution Execution Model

**Status**: Accepted
**Date**: 2026-05-27

## Context

When the user resolves a conflict (chooses "Keep Local", "Keep Server", or "Keep Both"), the application must perform file I/O — uploading, downloading, or renaming files — as a result of that choice. There are two natural places to trigger this work:

1. **In the Tauri command** (`resolve_conflict`) — file I/O executes immediately when the command is called, before it returns to the frontend.
2. **Via the propagator queue** — the command only updates the journal, then the background propagator picks up the resolution and executes the I/O on its next cycle.

## Decision

Execute file I/O **directly inside the `resolve_conflict` Tauri command**, not via the propagator queue.

The command:
1. Fetches the `ConflictRecord`, `SyncPair`, and `Account` from application state.
2. Retrieves credentials from the OS keychain via `spawn_blocking`.
3. Constructs a `NextcloudClient`.
4. Calls the appropriate resolver function (`conflict::resolve()` with `ConflictSide::Local/Remote/Both`).
5. Persists the resulting `ConflictResolution` to the journal.
6. Emits the `adagio://conflict-resolved` push event with the updated pending count.

File I/O must not block Tauri's async executor; any blocking keychain calls are wrapped in `tokio::task::spawn_blocking`.

## Rationale

- **Immediate user feedback**: the spinner the user sees resolves as soon as the I/O completes. With the queue approach, the user would see the wizard close but the file might not be in its final state until the propagator runs — potentially up to 2 hours later.
- **Simpler error reporting**: if the upload or download fails, the error surfaces directly in the UI response rather than silently failing in a background task with no clear path back to the user.
- **Determinism in tests**: unit tests can assert the full outcome (file content + journal state) synchronously without mocking a background scheduler.
- **No silent duplicates**: the command either succeeds completely or returns an error. There is no partial state where a journal record says "resolved" but the remote file is unchanged.

## Alternatives Considered

| Option | Rejected because |
|--------|-----------------|
| Propagator queue | Delayed execution gives no immediate feedback; failure is silent; complexity of correlating journal event back to the UX spinner |
| Separate worker task (tokio::spawn) | Same delayed-feedback problem; harder to propagate errors back to the command response |
| Background job with polling | Adds polling complexity; fundamentally incompatible with the 100 ms UX latency target (Constitution §V) |

## Consequences

- The `resolve_conflict` command is async and may take several seconds on slow connections; the frontend shows a loading spinner.
- If the network is unavailable, the command returns an error and the conflict remains pending — the user can retry.
- The propagator will not re-process already-resolved conflicts (it already skips records with `resolution.is_some()`).
