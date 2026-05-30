use crate::cli::AccountsCommand;
use crate::error::CliError;
use crate::output::{print_json, print_ok, print_table_header, print_table_row};
use adagio_ipc::{DaemonClient, DaemonRequest};
use std::sync::Arc;

/// Dispatch accounts subcommands.
pub async fn run_accounts(
    client: &Arc<DaemonClient>,
    command: &AccountsCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        AccountsCommand::List => run_accounts_list(client, json).await,
        AccountsCommand::Remove { account_id } => {
            run_accounts_remove(client, account_id, json).await
        }
    }
}

async fn run_accounts_list(client: &Arc<DaemonClient>, json: bool) -> Result<(), CliError> {
    let result = client
        .request(DaemonRequest::ListAccounts)
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&result);
        return Ok(());
    }
    let accounts = result.as_array().cloned().unwrap_or_default();
    if accounts.is_empty() {
        println!("No accounts connected");
        return Ok(());
    }
    print_table_header(
        &["ACCOUNT ID", "DISPLAY NAME", "SERVER", "USERNAME"],
        &[36, 20, 40, 20],
    );
    for a in &accounts {
        let id = a.get("id").and_then(|v| v.as_str()).unwrap_or("—");
        let name = a
            .get("display_name")
            .and_then(|v| v.as_str())
            .unwrap_or("—");
        let server = a.get("server_url").and_then(|v| v.as_str()).unwrap_or("—");
        let user = a.get("username").and_then(|v| v.as_str()).unwrap_or("—");
        print_table_row(&[id, name, server, user], &[36, 20, 40, 20]);
    }
    Ok(())
}

async fn run_accounts_remove(
    client: &Arc<DaemonClient>,
    account_id: &str,
    json: bool,
) -> Result<(), CliError> {
    client
        .request(DaemonRequest::RemoveAccount {
            account_id: account_id.to_string(),
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&serde_json::json!({"ok": true}));
    } else {
        print_ok("Account removed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // T031 integration test requires running daemon.
    #[test]
    fn placeholder() {}
}
