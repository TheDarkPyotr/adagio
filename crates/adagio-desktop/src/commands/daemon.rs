use crate::state::AppState;
use adagio_ipc::DaemonRequest;
use serde::Serialize;
use tauri::State;

/// Current status of the background daemon process.
#[derive(Debug, Serialize)]
pub struct DaemonStatusDto {
    /// Whether the daemon is connected and responding.
    pub running: bool,
    /// Daemon uptime in seconds, if known.
    pub uptime_secs: Option<u64>,
    /// Human-readable connection state: "connected", "reconnecting", "stopped", "failed".
    pub connection_state: String,
}

/// Return the current daemon connection state and uptime.
#[tauri::command]
pub async fn get_daemon_status(state: State<'_, AppState>) -> Result<DaemonStatusDto, String> {
    use adagio_ipc::ConnectionState;
    let conn_state = state.daemon.connection_state().borrow().clone();
    let state_str = match &conn_state {
        ConnectionState::Connected => "connected",
        ConnectionState::Reconnecting { .. } => "reconnecting",
        ConnectionState::Stopped => "stopped",
        ConnectionState::Failed => "failed",
    };
    let running = conn_state == ConnectionState::Connected;
    let uptime_secs = if running {
        state
            .daemon
            .request(DaemonRequest::Ping)
            .await
            .ok()
            .and_then(|v| v.get("uptime_secs").and_then(|n| n.as_u64()))
    } else {
        None
    };
    Ok(DaemonStatusDto {
        running,
        uptime_secs,
        connection_state: state_str.to_string(),
    })
}

/// Start the background daemon process (if not already running).
///
/// Used by the Settings UI "Start background sync" button and the "Restart sync"
/// button in the daemon-failed overlay.  After `reconnect_loop` gives up and sets
/// state to `Failed`, the socket monitor task has exited — a plain IPC ping cannot
/// reach the daemon.  We must call `connect_or_upgrade` to re-open the socket,
/// update `state_tx` (→ Connected), and spawn a fresh monitor task.
#[tauri::command]
pub async fn start_daemon(state: State<'_, AppState>) -> Result<(), String> {
    use adagio_ipc::ConnectionState;
    if state.daemon.connection_state().borrow().clone() == ConnectionState::Connected {
        return Ok(());
    }
    let config_dir = state
        .config_path
        .parent()
        .ok_or_else(|| "cannot determine config dir".to_string())?
        .to_path_buf();
    crate::lifecycle::connect_or_upgrade(&state.daemon, &config_dir)
        .await
        .map_err(|e| e.to_string())
}

/// Stop the background daemon process gracefully.
///
/// Used by the Settings UI "Stop background sync" button.
/// Marks the client as Stopped so auto-restart is disabled.
#[tauri::command]
pub async fn stop_daemon(state: State<'_, AppState>) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::StopDaemon)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Enable or disable auto-start of the daemon at OS login.
///
/// Forwards to the daemon, which writes the appropriate platform entry
/// (XDG `.desktop` on Linux, LaunchAgent plist on macOS, registry key on Windows).
#[tauri::command]
pub async fn set_start_at_login(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::SetStartAtLogin { enabled })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn stub_state(dir: &TempDir) -> AppState {
        AppState {
            daemon: adagio_ipc::DaemonClient::new_stub(),
            config_path: dir.path().join("config.json"),
            auth_flow: crate::auth_flow::AuthFlowSlot::default(),
        }
    }

    // T033 — set_start_at_login forwards SetStartAtLogin to daemon.
    #[tokio::test]
    async fn set_start_at_login_forwards_to_daemon() {
        let dir = TempDir::new().unwrap();
        let state = stub_state(&dir);
        let result = state
            .daemon
            .request(DaemonRequest::SetStartAtLogin { enabled: true })
            .await;
        assert!(result.is_err(), "stub must fail — no socket");
    }

    // T043 — stop_daemon forwards StopDaemon request.
    #[tokio::test]
    async fn stop_daemon_forwards_stop_daemon_request() {
        let dir = TempDir::new().unwrap();
        let state = stub_state(&dir);
        let result = state.daemon.request(DaemonRequest::StopDaemon).await;
        assert!(result.is_err(), "stub must fail — no socket");
    }

    // T044 — start_daemon returns error when daemon not reachable.
    #[tokio::test]
    async fn start_daemon_returns_error_when_not_reachable() {
        let dir = TempDir::new().unwrap();
        let state = stub_state(&dir);
        // Stub has no connection, pings fail → start_daemon times out and errors.
        let result = state.daemon.request(DaemonRequest::Ping).await;
        assert!(result.is_err(), "stub ping must fail");
    }

    // DaemonStatusDto serialises correctly.
    #[test]
    fn daemon_status_dto_serialises() {
        let dto = DaemonStatusDto {
            running: true,
            uptime_secs: Some(42),
            connection_state: "connected".to_string(),
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"running\":true"));
        assert!(json.contains("\"uptime_secs\":42"));
    }
}
