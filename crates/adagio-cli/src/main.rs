use adagio_cli::{cli::Cli, error::CliError, run};
use adagio_ipc::DaemonClient;
use clap::Parser;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match execute(cli).await {
        Ok(()) => {}
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(e.exit_code());
        }
    }
}

async fn execute(cli: Cli) -> Result<(), CliError> {
    // Daemon subcommands manage the daemon process directly — they don't
    // pre-connect (daemon start handles its own connection logic).
    let client = connect_to_daemon().await?;
    run::run(&cli, client).await
}

async fn connect_to_daemon() -> Result<Arc<DaemonClient>, CliError> {
    let daemon_path = {
        let current = std::env::current_exe().map_err(|e| CliError::Unreachable(e.to_string()))?;
        let parent = current
            .parent()
            .ok_or_else(|| CliError::Unreachable("cannot find daemon binary".into()))?;
        #[cfg(windows)]
        {
            parent.join("adagio-daemon.exe")
        }
        #[cfg(not(windows))]
        {
            parent.join("adagio-daemon")
        }
    };

    DaemonClient::connect_or_start(&daemon_path)
        .await
        .map_err(|e| CliError::Unreachable(format!("daemon not reachable: {e}")))
}
