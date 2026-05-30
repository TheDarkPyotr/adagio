use crate::state::AppState;
use adagio_ipc::DaemonRequest;
use tauri::State;

/// Return current bandwidth limits and live throughput for the first account.
#[tauri::command]
pub async fn get_bandwidth_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::GetBandwidthStatus { account_id: None })
        .await
        .map_err(|e| e.to_string())
}

/// Set upload and/or download speed limits in Kbps (0 = unlimited).
#[tauri::command]
pub async fn set_bandwidth_limits(
    upload_kbps: u64,
    download_kbps: u64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::SetBandwidthLimits {
            account_id: None,
            upload_kbps,
            download_kbps,
        })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Remove all bandwidth limits.
#[tauri::command]
pub async fn clear_bandwidth_limits(state: State<'_, AppState>) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::ClearBandwidthLimits { account_id: None })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}
