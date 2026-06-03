use crate::state::AppState;
use adagio_ipc::DaemonRequest;
use tauri::State;

// ── Tauri commands (forwarded to adagio-daemon over IPC) ─────────────────────
// request() now returns serde_json::Value (the raw "result" field), so all
// commands simply return the value directly without matching on DaemonResponse.

/// Return the current sync engine status.
#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::GetStatus)
        .await
        .map_err(|e| e.to_string())
}

/// Pause all sync activity.
#[tauri::command]
pub async fn pause_sync(state: State<'_, AppState>) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::PauseSyncAll)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Resume sync from paused state.
#[tauri::command]
pub async fn resume_sync(state: State<'_, AppState>) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::ResumeSyncAll)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Return the last `limit` activity log entries, optionally filtered by kind.
#[tauri::command]
pub async fn get_activity_log(
    state: State<'_, AppState>,
    limit: Option<usize>,
    filter: Option<String>,
) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::GetActivityLog {
            limit: limit.map(|n| n as u32),
            filter,
        })
        .await
        .map_err(|e| e.to_string())
}

/// Return sidebar section counts (total synced files, recently modified) for a pair.
#[tauri::command]
pub async fn get_section_counts(
    state: State<'_, AppState>,
    pair_id: Option<String>,
) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::GetSectionCounts { pair_id })
        .await
        .map_err(|e| e.to_string())
}

/// Search synced file paths matching `query` across all pairs.
#[tauri::command]
pub async fn search_files(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::SearchFiles { query, limit })
        .await
        .map_err(|e| e.to_string())
}

/// Trigger an immediate sync cycle for a specific pair.
#[tauri::command]
pub async fn trigger_sync(state: State<'_, AppState>, pair_id: String) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::TriggerSync { pair_id })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Return all journal entries with Error status for the given pair.
#[tauri::command]
pub async fn get_error_items(
    state: State<'_, AppState>,
    pair_id: String,
) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::GetErrorItems { pair_id })
        .await
        .map_err(|e| e.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use tempfile::TempDir;

    fn stub_state(dir: &TempDir) -> AppState {
        AppState {
            daemon: adagio_ipc::DaemonClient::new_stub(),
            config_path: dir.path().join("config.json"),
            auth_flow: crate::auth_flow::AuthFlowSlot::default(),
        }
    }

    // T015 — get_status forwards to daemon (stub returns "not connected").
    #[tokio::test]
    async fn get_status_command_forwards_to_daemon() {
        let dir = TempDir::new().unwrap();
        let state = stub_state(&dir);
        let result = state.daemon.request(DaemonRequest::GetStatus).await;
        assert!(result.is_err(), "stub must fail — no socket connected");
        assert!(
            result.unwrap_err().to_string().contains("not connected"),
            "error must mention 'not connected'"
        );
    }
}
