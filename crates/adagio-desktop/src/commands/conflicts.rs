use crate::state::AppState;
use adagio_core::journal::Journal;
use adagio_core::types::{ConflictResolution, PairId};
use serde::Serialize;
use tauri::State;

// ── DTOs ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ConflictDto {
    pub id: String,
    pub pair_id: String,
    pub path: String,
    pub local_mtime: String,
    pub remote_mtime: String,
    pub local_size: u64,
    pub remote_size: u64,
    pub policy: String,
    pub resolution: Option<String>,
    pub detected_at: String,
    pub resolved_at: Option<String>,
}

impl From<adagio_core::types::ConflictRecord> for ConflictDto {
    fn from(r: adagio_core::types::ConflictRecord) -> Self {
        let policy = format!("{:?}", r.policy).to_lowercase();
        let resolution = r
            .resolution
            .as_ref()
            .map(|res| format!("{:?}", res).to_lowercase());
        Self {
            id: r.id,
            pair_id: r.pair_id.to_string(),
            path: r.path.to_string(),
            local_mtime: r.local_mtime.to_rfc3339(),
            remote_mtime: r.remote_mtime.to_rfc3339(),
            local_size: r.local_size,
            remote_size: r.remote_size,
            policy,
            resolution,
            detected_at: r.detected_at.to_rfc3339(),
            resolved_at: r.resolved_at.map(|d| d.to_rfc3339()),
        }
    }
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// List all conflict records for a given pair (both resolved and unresolved).
#[tauri::command]
pub async fn list_conflicts(
    state: State<'_, AppState>,
    pair_id: String,
) -> Result<Vec<ConflictDto>, String> {
    let pid = PairId(pair_id);
    let records = state
        .journal
        .list_conflicts(&pid)
        .await
        .map_err(|e| e.to_string())?;
    Ok(records.into_iter().map(ConflictDto::from).collect())
}

/// Resolve a conflict by ID, applying the user's choice.
///
/// `side` must be `"local"`, `"remote"`, or `"both"`.
#[tauri::command]
pub async fn resolve_conflict(
    state: State<'_, AppState>,
    id: String,
    side: String,
) -> Result<(), String> {
    let resolution = match side.as_str() {
        "local" => ConflictResolution::KeptLocal,
        "remote" => ConflictResolution::KeptRemote,
        _ => {
            return Err(format!(
                "invalid side '{}' — expected 'local' or 'remote'",
                side
            ))
        }
    };

    state
        .journal
        .resolve_conflict(&id, resolution)
        .await
        .map_err(|e| e.to_string())
}
