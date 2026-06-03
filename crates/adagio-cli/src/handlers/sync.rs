use crate::error::CliError;
use crate::output::{print_json, print_ok};
use adagio_ipc::{DaemonClient, DaemonRequest};
use std::sync::Arc;

/// Trigger an immediate sync cycle.
pub async fn run_sync(
    client: &Arc<DaemonClient>,
    pair_id: Option<String>,
    json: bool,
) -> Result<(), CliError> {
    if let Some(id) = pair_id {
        client
            .request(DaemonRequest::TriggerSync { pair_id: id })
            .await
            .map_err(|e| CliError::DaemonError(e.to_string()))?;
        if json {
            print_json(&serde_json::json!({"ok": true}));
        } else {
            print_ok("Sync triggered");
        }
    } else {
        // Sync all pairs: list them first, then trigger each.
        let pairs_val = client
            .request(DaemonRequest::ListPairs)
            .await
            .map_err(|e| CliError::DaemonError(e.to_string()))?;
        let pairs = pairs_val.as_array().cloned().unwrap_or_default();
        if pairs.is_empty() {
            if json {
                print_json(&serde_json::json!({"ok": true, "pairs_triggered": 0}));
            } else {
                print_ok("No pairs configured — nothing to sync");
            }
            return Ok(());
        }
        for pair in &pairs {
            if let Some(id) = pair.get("id").and_then(|v| v.as_str()) {
                client
                    .request(DaemonRequest::TriggerSync {
                        pair_id: id.to_string(),
                    })
                    .await
                    .map_err(|e| CliError::DaemonError(e.to_string()))?;
            }
        }
        if json {
            print_json(&serde_json::json!({"ok": true, "pairs_triggered": pairs.len()}));
        } else {
            print_ok(&format!("Sync triggered for {} pair(s)", pairs.len()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // T018/T019 integration tests require a running daemon.
    #[test]
    fn placeholder() {}
}
