use std::sync::Arc;

use adagio_ipc::DaemonClient;

use crate::cli::{Cli, Commands};
use crate::error::CliError;

/// Dispatch the parsed CLI command to the appropriate handler.
pub async fn run(cli: &Cli, client: Arc<DaemonClient>) -> Result<(), CliError> {
    match &cli.command {
        // No subcommand → interactive live dashboard.
        None => {
            crate::tui::run_dashboard(client).await;
            Ok(())
        }

        Some(Commands::Status) => handlers::status::run_status(&client, cli.json).await,
        Some(Commands::Sync { pair_id }) => {
            handlers::sync::run_sync(&client, pair_id.clone(), cli.json).await
        }
        Some(Commands::Pause) => handlers::pause_resume::run_pause(&client, cli.json).await,
        Some(Commands::Resume) => handlers::pause_resume::run_resume(&client, cli.json).await,
        Some(Commands::Activity { limit, filter }) => {
            handlers::activity::run_activity(&client, *limit, filter.clone(), cli.json).await
        }
        Some(Commands::Pairs { command }) => {
            handlers::pairs::run_pairs(&client, command, cli.json).await
        }
        Some(Commands::Accounts { command }) => {
            handlers::accounts::run_accounts(&client, command, cli.json).await
        }
        Some(Commands::Conflicts { command }) => {
            handlers::conflicts::run_conflicts(&client, command, cli.json).await
        }
        Some(Commands::Daemon { command }) => handlers::daemon::run_daemon(command, cli.json).await,
        Some(Commands::Bandwidth { command }) => {
            handlers::bandwidth::run_bandwidth(&client, command, cli.json).await
        }
        Some(Commands::Network { command }) => {
            handlers::network::run_network(&client, command, cli.json).await
        }
        Some(Commands::Vfs { command }) => handlers::vfs::run_vfs(&client, command, cli.json).await,
        Some(Commands::E2ee { command }) => {
            handlers::e2ee::run_e2ee(&client, command, cli.json).await
        }
    }
}

use crate::handlers;
