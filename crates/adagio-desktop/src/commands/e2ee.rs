use crate::state::AppState;
use adagio_ipc::DaemonRequest;
use tauri::State;

/// Initialise E2EE for a pair. Returns a one-time mnemonic.
#[tauri::command]
pub async fn e2ee_init(pair_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let resp = state
        .daemon
        .request(DaemonRequest::E2eeInit { pair_id })
        .await
        .map_err(|e| e.to_string())?;
    // Response is { "mnemonic": "..." }
    resp["mnemonic"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "E2EE init response missing mnemonic field".to_string())
}

/// Pair this device using the provided mnemonic.
#[tauri::command]
pub async fn e2ee_pair(
    pair_id: String,
    mnemonic: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::E2eePair { pair_id, mnemonic })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Return E2EE status for a pair.
#[tauri::command]
pub async fn e2ee_status(
    pair_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::E2eeStatus { pair_id })
        .await
        .map_err(|e| e.to_string())
}

/// Disable E2EE for a pair: deletes server metadata and clears local state.
#[tauri::command]
pub async fn e2ee_disable(pair_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::E2eeDisable { pair_id })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}
