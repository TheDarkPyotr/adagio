use crate::error::CliError;
use crate::output::{print_json, print_ok};
use adagio_ipc::{DaemonClient, DaemonRequest};
use std::sync::Arc;

/// Pause all sync activity.
pub async fn run_pause(client: &Arc<DaemonClient>, json: bool) -> Result<(), CliError> {
    client
        .request(DaemonRequest::PauseSyncAll)
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&serde_json::json!({"ok": true}));
    } else {
        print_ok("Sync paused");
    }
    Ok(())
}

/// Resume sync after pause.
pub async fn run_resume(client: &Arc<DaemonClient>, json: bool) -> Result<(), CliError> {
    client
        .request(DaemonRequest::ResumeSyncAll)
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&serde_json::json!({"ok": true}));
    } else {
        print_ok("Sync resumed");
    }
    Ok(())
}
