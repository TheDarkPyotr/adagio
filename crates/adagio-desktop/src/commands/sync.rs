use crate::state::AppState;
use adagio_core::cycle::{ActivityEntry, EngineStatus, SyncEngine};
use serde::Serialize;
use tauri::State;

// ── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SyncStatusDto {
    pub status: String,
    pub pair_id: Option<String>,
    pub error: Option<String>,
}

impl From<EngineStatus> for SyncStatusDto {
    fn from(s: EngineStatus) -> Self {
        match s {
            EngineStatus::Idle => Self {
                status: "idle".into(),
                pair_id: None,
                error: None,
            },
            EngineStatus::Syncing { pair_id } => Self {
                status: "syncing".into(),
                pair_id: Some(pair_id.to_string()),
                error: None,
            },
            EngineStatus::Paused => Self {
                status: "paused".into(),
                pair_id: None,
                error: None,
            },
            EngineStatus::Error(msg) => Self {
                status: "error".into(),
                pair_id: None,
                error: Some(msg),
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ActivityEntryDto {
    pub pair_id: String,
    pub path: String,
    pub action: String,
    pub occurred_at: String,
    pub bytes: Option<u64>,
}

impl From<ActivityEntry> for ActivityEntryDto {
    fn from(e: ActivityEntry) -> Self {
        Self {
            pair_id: e.pair_id.to_string(),
            path: e.path,
            action: e.action,
            occurred_at: e.occurred_at.to_rfc3339(),
            bytes: e.bytes,
        }
    }
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Return the current sync engine status.
#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<SyncStatusDto, String> {
    Ok(state.engine.status().into())
}

/// Pause all sync activity.
#[tauri::command]
pub async fn pause_sync(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.pause().await.map_err(|e| e.to_string())
}

/// Resume sync from paused state.
#[tauri::command]
pub async fn resume_sync(state: State<'_, AppState>) -> Result<(), String> {
    state.engine.resume().await.map_err(|e| e.to_string())
}

/// Return the last `limit` activity log entries.
#[tauri::command]
pub async fn get_activity_log(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<ActivityEntryDto>, String> {
    let entries = state
        .engine
        .activity_log(limit.unwrap_or(50))
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(ActivityEntryDto::from).collect())
}

/// Trigger an immediate sync cycle for a specific pair.
///
/// Sends a trigger signal to the pair's background runner and returns `Ok(())`
/// immediately — the cycle runs asynchronously.
#[tauri::command]
pub async fn trigger_sync(state: State<'_, AppState>, pair_id: String) -> Result<(), String> {
    use adagio_core::types::PairId;
    let pid = PairId(pair_id);
    state
        .engine
        .trigger_pair(&pid)
        .await
        .map_err(|e| e.to_string())
}

// ── Error item DTO ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ErrorItemDto {
    pub path: String,
    pub error_message: Option<String>,
    pub retry_count: u32,
    pub updated_at: String,
}

impl From<adagio_core::types::JournalEntry> for ErrorItemDto {
    fn from(e: adagio_core::types::JournalEntry) -> Self {
        Self {
            path: e.path.as_str().to_string(),
            error_message: e.error_message,
            retry_count: e.retry_count,
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

/// Return all journal entries with Error status for the given pair.
///
/// Used by the Dashboard to display items that need user attention.
#[tauri::command]
pub async fn get_error_items(
    state: State<'_, AppState>,
    pair_id: String,
) -> Result<Vec<ErrorItemDto>, String> {
    use adagio_core::journal::Journal;
    use adagio_core::types::{PairId, SyncStatus};
    let pid = PairId(pair_id);
    let entries = state
        .journal
        .entries_by_status(&pid, &SyncStatus::Error)
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(ErrorItemDto::from).collect())
}
