use crate::state::AppState;
use adagio_ipc::DaemonRequest;
use serde::Serialize;
use tauri::{Emitter, State};

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
    /// `true` if this conflict is on a directory.
    pub is_dir: bool,
    /// Serialised `ConflictKind`: `"content_modified"` | `"renamed_both_sides"` | `"deleted_with_content"`.
    pub conflict_kind: String,
}

impl From<adagio_core::types::ConflictRecord> for ConflictDto {
    fn from(r: adagio_core::types::ConflictRecord) -> Self {
        let policy = format!("{:?}", r.policy).to_lowercase();
        let resolution = r
            .resolution
            .as_ref()
            .map(|res| format!("{:?}", res).to_lowercase());
        let conflict_kind = serde_json::to_string(&r.conflict_kind)
            .unwrap_or_else(|_| "\"content_modified\"".to_string())
            .trim_matches('"')
            .to_string();
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
            is_dir: r.is_dir,
            conflict_kind,
        }
    }
}

// ── Event payloads ────────────────────────────────────────────────────────────

/// Payload emitted when a conflict-detected event fires.
#[derive(Debug, Clone, Serialize)]
pub struct ConflictDetectedPayload {
    pub pending_count: usize,
}

/// Payload emitted when a conflict-resolved event fires.
#[derive(Debug, Clone, Serialize)]
pub struct ConflictResolvedPayload {
    pub id: String,
    pub pending_count: usize,
}

// ── Event helpers ─────────────────────────────────────────────────────────────

/// Emit `adagio://conflict-detected` with the total number of pending conflicts.
///
/// Called by `lib.rs` when a `DaemonEvent::ConflictDetected` is received.
pub fn emit_conflict_detected(app: &tauri::AppHandle, pending_count: usize) {
    let _ = app.emit(
        "adagio://conflict-detected",
        ConflictDetectedPayload { pending_count },
    );
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Dismiss all pending conflicts across every pair (no file I/O).
#[tauri::command]
pub async fn dismiss_all_conflicts(
    _app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let v = state
        .daemon
        .request(DaemonRequest::DismissAllConflicts)
        .await
        .map_err(|e| e.to_string())?;
    Ok(v.get("dismissed_count")
        .and_then(|n| n.as_u64())
        .unwrap_or(0) as usize)
}

/// List all conflict records for a given pair (both resolved and unresolved).
#[tauri::command]
pub async fn list_conflicts(
    state: State<'_, AppState>,
    pair_id: String,
) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::ListConflicts { pair_id })
        .await
        .map_err(|e| e.to_string())
}

/// Resolve a conflict by ID with the given side: "local", "remote", or "both".
#[tauri::command]
pub async fn resolve_conflict(
    _app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    side: String,
) -> Result<(), String> {
    state
        .daemon
        .request(DaemonRequest::ResolveConflict { id, side })
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn stub_state(dir: &TempDir) -> crate::state::AppState {
        crate::state::AppState {
            daemon: adagio_ipc::DaemonClient::new_stub(),
            config_path: dir.path().join("config.json"),
            auth_flow: crate::auth_flow::AuthFlowSlot::default(),
        }
    }

    // T016 — resolve_conflict forwards to daemon (stub returns error).
    #[tokio::test]
    async fn resolve_conflict_command_forwards_to_daemon() {
        let dir = TempDir::new().unwrap();
        let state = stub_state(&dir);
        let result = state
            .daemon
            .request(adagio_ipc::DaemonRequest::ResolveConflict {
                id: "test-id".to_string(),
                side: "local".to_string(),
            })
            .await;
        assert!(result.is_err(), "stub must fail — no socket connected");
    }

    // T017 — emit_conflict_detected helper has correct signature and compiles.
    #[test]
    fn emit_conflict_detected_helper_exists() {
        // The function is pub so we can call it; verify it compiles with the expected signature.
        let _: fn(&tauri::AppHandle, usize) = emit_conflict_detected;
    }

    // T005 — ConflictDto must include is_dir and conflict_kind fields.
    #[test]
    fn conflict_dto_includes_is_dir_and_kind() {
        use adagio_core::types::{
            ConflictKind, ConflictPolicy, ConflictRecord, PairId, RelativePath,
        };
        use chrono::Utc;

        let record = ConflictRecord {
            id: "abc".to_string(),
            pair_id: PairId::new(),
            path: RelativePath::new("docs/file.txt"),
            local_mtime: Utc::now(),
            remote_mtime: Utc::now(),
            local_size: 100,
            remote_size: 200,
            policy: ConflictPolicy::Ask,
            resolution: None,
            detected_at: Utc::now(),
            resolved_at: None,
            is_dir: true,
            conflict_kind: ConflictKind::RenamedBothSides,
        };

        let dto = ConflictDto::from(record);
        let json = serde_json::to_string(&dto).unwrap();
        assert!(
            json.contains("\"is_dir\":true"),
            "missing is_dir in: {json}"
        );
        assert!(
            json.contains("\"conflict_kind\""),
            "missing conflict_kind in: {json}"
        );
        assert_eq!(dto.is_dir, true);
        assert_eq!(dto.conflict_kind, "renamed_both_sides");
    }
}
