use crate::cli::ConflictsCommand;
use crate::error::CliError;
use crate::output::{print_json, print_ok, print_table_header, print_table_row};
use adagio_ipc::{DaemonClient, DaemonRequest};
use std::sync::Arc;

/// Dispatch conflicts subcommands.
pub async fn run_conflicts(
    client: &Arc<DaemonClient>,
    command: &ConflictsCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        ConflictsCommand::List { pair_id } => {
            run_conflicts_list(client, pair_id.clone(), json).await
        }
        ConflictsCommand::Resolve { conflict_id, keep } => {
            run_conflicts_resolve(client, conflict_id, keep, json).await
        }
        ConflictsCommand::Dismiss => run_conflicts_dismiss(client, json).await,
    }
}

async fn run_conflicts_list(
    client: &Arc<DaemonClient>,
    pair_id: Option<String>,
    json: bool,
) -> Result<(), CliError> {
    let all_conflicts: Vec<serde_json::Value> = if let Some(id) = pair_id {
        let v = client
            .request(DaemonRequest::ListConflicts { pair_id: id })
            .await
            .map_err(|e| CliError::DaemonError(e.to_string()))?;
        v.as_array().cloned().unwrap_or_default()
    } else {
        // List all pairs, then collect conflicts from each.
        let pairs_val = client
            .request(DaemonRequest::ListPairs)
            .await
            .map_err(|e| CliError::DaemonError(e.to_string()))?;
        let mut combined = Vec::new();
        for pair in pairs_val.as_array().cloned().unwrap_or_default() {
            if let Some(id) = pair.get("id").and_then(|v| v.as_str()) {
                let cv = client
                    .request(DaemonRequest::ListConflicts {
                        pair_id: id.to_string(),
                    })
                    .await
                    .map_err(|e| CliError::DaemonError(e.to_string()))?;
                combined.extend(cv.as_array().cloned().unwrap_or_default());
            }
        }
        combined
    };

    let pending: Vec<_> = all_conflicts
        .iter()
        .filter(|c| c.get("resolution").map(|v| v.is_null()).unwrap_or(true))
        .collect();

    if json {
        print_json(&serde_json::Value::Array(
            pending.iter().map(|c| (*c).clone()).collect(),
        ));
        return Ok(());
    }
    if pending.is_empty() {
        println!("No pending conflicts");
        return Ok(());
    }
    print_table_header(
        &[
            "CONFLICT ID",
            "FILE",
            "LOCAL SIZE",
            "SERVER SIZE",
            "DETECTED",
        ],
        &[36, 40, 12, 12, 22],
    );
    for c in &pending {
        let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("—");
        let path = c.get("path").and_then(|v| v.as_str()).unwrap_or("—");
        let lsize = c
            .get("local_size")
            .and_then(|v| v.as_u64())
            .map(human_bytes)
            .unwrap_or_else(|| "—".into());
        let rsize = c
            .get("remote_size")
            .and_then(|v| v.as_u64())
            .map(human_bytes)
            .unwrap_or_else(|| "—".into());
        let detected = c
            .get("detected_at")
            .and_then(|v| v.as_str())
            .map(|s| &s[..10])
            .unwrap_or("—");
        print_table_row(&[id, path, &lsize, &rsize, detected], &[36, 40, 12, 12, 22]);
    }
    Ok(())
}

async fn run_conflicts_resolve(
    client: &Arc<DaemonClient>,
    conflict_id: &str,
    keep: &str,
    json: bool,
) -> Result<(), CliError> {
    client
        .request(DaemonRequest::ResolveConflict {
            id: conflict_id.to_string(),
            side: keep.to_string(),
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&serde_json::json!({"ok": true}));
    } else {
        print_ok("Conflict resolved");
    }
    Ok(())
}

async fn run_conflicts_dismiss(client: &Arc<DaemonClient>, json: bool) -> Result<(), CliError> {
    let result = client
        .request(DaemonRequest::DismissAllConflicts)
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&result);
    } else {
        let n = result
            .get("dismissed_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        print_ok(&format!("{n} conflict(s) dismissed"));
    }
    Ok(())
}

fn human_bytes(b: u64) -> String {
    if b < 1024 {
        format!("{b} B")
    } else if b < 1024 * 1024 {
        format!("{} KB", b / 1024)
    } else {
        format!("{} MB", b / (1024 * 1024))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // T025/T026/T027 integration tests require running daemon.
    #[test]
    fn human_bytes_kb() {
        assert_eq!(human_bytes(2048), "2 KB");
    }
    #[test]
    fn human_bytes_mb() {
        assert_eq!(human_bytes(3 * 1024 * 1024), "3 MB");
    }
}
