use crate::state::AppState;
use adagio_ipc::DaemonRequest;
use tauri::State;

/// Return VFS cache statistics for a pair.
#[tauri::command]
pub async fn get_vfs_stats(
    pair_id: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::GetVfsStats { pair_id })
        .await
        .map_err(|e| e.to_string())
}

/// Pin or unpin a path for offline access.
#[tauri::command]
pub async fn set_vfs_pin(
    pair_id: String,
    path: String,
    pinned: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::SetVfsPin {
            pair_id,
            path,
            pinned,
        })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Evict local content for a path, returning it to cloud-only state.
#[tauri::command]
pub async fn evict_vfs_file(
    pair_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::EvictVfsFile { pair_id, path })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}
