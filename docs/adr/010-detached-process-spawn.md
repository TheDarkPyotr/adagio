# ADR-010: Detached Process Spawn for adagio-daemon

**Date**: 2026-05-29
**Status**: Accepted
**Feature**: 007-background-sync-daemon

## Context

`adagio-desktop` (the Tauri GUI app) needs to spawn `adagio-daemon` when it starts
and the daemon is not already running. The daemon must outlive the GUI process —
sync must continue after the user closes the app window.

## Decision

Use `std::process::Command` with **detached stdio** and platform-specific detach
flags. Do NOT use `tauri-plugin-shell` sidecar.

```rust
#[cfg(unix)]
fn spawn_daemon(path: &Path) -> std::io::Result<()> {
    std::process::Command::new(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

#[cfg(windows)]
fn spawn_daemon(path: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x00000008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
    std::process::Command::new(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()?;
    Ok(())
}
```

**Health check**: After spawning, the app retries connecting to the daemon's socket
with a 300 ms timeout per attempt, for up to 5 seconds total (≈16 attempts).

## Alternatives Considered

**tauri-plugin-shell sidecar**: Tauri's sidecar API registers a lifecycle hook that
sends SIGTERM/SIGKILL to child processes when the parent GUI exits. This is tracked
in tauri-plugins-workspace issue #3062 and is by design. Using the sidecar API
would cause the daemon to be killed when the user closes the app window — which
defeats the entire purpose of the extraction.

**nohup / systemd-run**: External wrappers are not portable and not testable. The
`DETACHED_PROCESS` flag on Windows and the simple spawn-with-null-stdio pattern on
Unix are the standard OS-native approaches.

## Consequences

- The GUI process has no handle to the daemon process after spawning. The daemon
  is managed exclusively via IPC (start: socket not answering → spawn; stop:
  `stop_daemon` RPC; health: periodic `ping`).
- On Unix, the daemon inherits the parent's process group by default. If the
  terminal that launched the GUI is closed with SIGHUP, the daemon may receive it.
  The daemon installs a SIGHUP handler that performs a config reload (not exit).
- The daemon binary path is determined at runtime as a sibling of the running
  executable: `std::env::current_exe()` → replace the binary name.
