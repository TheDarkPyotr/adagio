use crate::cli::PairsCommand;
use crate::error::CliError;
use crate::output::{print_json, print_ok, print_table_header, print_table_row};
use adagio_ipc::{DaemonClient, DaemonRequest};
use std::sync::Arc;

/// Dispatch pairs subcommands.
pub async fn run_pairs(
    client: &Arc<DaemonClient>,
    command: &PairsCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        PairsCommand::List => run_pairs_list(client, json).await,
        PairsCommand::Add {
            local,
            remote,
            account,
        } => run_pairs_add(client, local, remote, account, json).await,
        PairsCommand::Remove {
            pair_id,
            delete_local_files,
        } => run_pairs_remove(client, pair_id, *delete_local_files, json).await,
    }
}

async fn run_pairs_list(client: &Arc<DaemonClient>, json: bool) -> Result<(), CliError> {
    let result = client
        .request(DaemonRequest::ListPairs)
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&result);
        return Ok(());
    }
    let pairs = result.as_array().cloned().unwrap_or_default();
    if pairs.is_empty() {
        println!("No sync pairs configured");
        return Ok(());
    }
    print_table_header(&["PAIR ID", "LOCAL", "REMOTE"], &[36, 40, 40]);
    for p in &pairs {
        let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("—");
        let local = p.get("local_root").and_then(|v| v.as_str()).unwrap_or("—");
        let remote = p.get("remote_root").and_then(|v| v.as_str()).unwrap_or("—");
        print_table_row(&[id, local, remote], &[36, 40, 40]);
    }
    Ok(())
}

async fn run_pairs_add(
    client: &Arc<DaemonClient>,
    local: &str,
    remote: &str,
    account: &str,
    json: bool,
) -> Result<(), CliError> {
    let result = client
        .request(DaemonRequest::CreatePair {
            account_id: account.to_string(),
            local_root: local.to_string(),
            remote_root: remote.to_string(),
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&result);
    } else {
        let id = result.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        print_ok(&format!("Pair created: {id}"));
    }
    Ok(())
}

async fn run_pairs_remove(
    client: &Arc<DaemonClient>,
    pair_id: &str,
    delete_local_files: bool,
    json: bool,
) -> Result<(), CliError> {
    client
        .request(DaemonRequest::DeletePair {
            pair_id: pair_id.to_string(),
            delete_local_files,
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&serde_json::json!({"ok": true}));
    } else {
        print_ok("Pair removed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // T030 integration test requires running daemon.
    #[test]
    fn placeholder() {}
}
