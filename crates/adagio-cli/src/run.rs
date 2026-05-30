use std::sync::Arc;

use adagio_ipc::DaemonClient;

use crate::cli::{Cli, Commands};
use crate::error::CliError;

/// Dispatch the parsed CLI command to the appropriate handler.
///
/// The `client` is already connected to the daemon (or was just started).
/// Returns `Ok(())` on success or a `CliError` with the appropriate exit code.
pub async fn run(cli: &Cli, client: Arc<DaemonClient>) -> Result<(), CliError> {
    match &cli.command {
        Commands::Status => handlers::status::run_status(&client, cli.json).await,
        Commands::Sync { pair_id } => {
            handlers::sync::run_sync(&client, pair_id.clone(), cli.json).await
        }
        Commands::Pause => handlers::pause_resume::run_pause(&client, cli.json).await,
        Commands::Resume => handlers::pause_resume::run_resume(&client, cli.json).await,
        Commands::Activity { limit, filter } => {
            handlers::activity::run_activity(&client, *limit, filter.clone(), cli.json).await
        }
        Commands::Pairs { command } => handlers::pairs::run_pairs(&client, command, cli.json).await,
        Commands::Accounts { command } => {
            handlers::accounts::run_accounts(&client, command, cli.json).await
        }
        Commands::Conflicts { command } => {
            handlers::conflicts::run_conflicts(&client, command, cli.json).await
        }
        Commands::Daemon { command } => handlers::daemon::run_daemon(command, cli.json).await,
        Commands::Bandwidth { command } => {
            handlers::bandwidth::run_bandwidth(&client, command, cli.json).await
        }
        Commands::Network { command } => {
            handlers::network::run_network(&client, command, cli.json).await
        }
    }
}

use crate::handlers;
