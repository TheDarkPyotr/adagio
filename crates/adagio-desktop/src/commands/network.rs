use crate::state::AppState;
use adagio_ipc::DaemonRequest;
use tauri::State;

/// Return current network state, effective action, and configured policy.
#[tauri::command]
pub async fn get_network_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::GetNetworkStatus)
        .await
        .map_err(|e| e.to_string())
}

/// Update the global network awareness policy.
#[tauri::command]
pub async fn set_network_policy(
    on_metered: Option<String>,
    on_battery: Option<String>,
    throttle_kbps: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::SetNetworkPolicy {
            on_metered,
            on_battery,
            throttle_kbps,
        })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Add an SSID to the block list.
#[tauri::command]
pub async fn add_blocked_ssid(ssid: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::ManageBlockedSsid {
            action: "add".to_string(),
            ssid: Some(ssid),
        })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Remove an SSID from the block list.
#[tauri::command]
pub async fn remove_blocked_ssid(ssid: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::ManageBlockedSsid {
            action: "remove".to_string(),
            ssid: Some(ssid),
        })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Return the list of blocked SSIDs.
#[tauri::command]
pub async fn list_blocked_ssids(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::ManageBlockedSsid {
            action: "list".to_string(),
            ssid: None,
        })
        .await
        .map_err(|e| e.to_string())
}
