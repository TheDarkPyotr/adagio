use adagio_cli::{cli::Cli, error::CliError, output, run};
use adagio_ipc::DaemonClient;
use clap::Parser;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match execute(cli).await {
        Ok(()) => {}
        Err(e) => {
            output::print_err(&e.to_string());
            std::process::exit(e.exit_code());
        }
    }
}

async fn execute(cli: Cli) -> Result<(), CliError> {
    let client = connect_with_spinner().await?;
    run::run(&cli, client).await
}

async fn connect_with_spinner() -> Result<Arc<DaemonClient>, CliError> {
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

    // Show a spinner while connecting / starting the daemon.
    // If stdout is not a TTY (piped), skip the spinner.
    let is_tty = crossterm::tty::IsTty::is_tty(&std::io::stderr());

    if is_tty {
        let sp = output::spinner("connecting to daemon");
        let result = DaemonClient::connect_or_start(&daemon_path).await;
        match result {
            Ok(client) => {
                output::spinner_ok(&sp, "");
                sp.finish_and_clear();
                Ok(client)
            }
            Err(e) => {
                output::spinner_err(&sp, &format!("cannot reach daemon: {e}"));
                Err(CliError::Unreachable(format!("daemon not reachable: {e}")))
            }
        }
    } else {
        DaemonClient::connect_or_start(&daemon_path)
            .await
            .map_err(|e| CliError::Unreachable(format!("daemon not reachable: {e}")))
    }
}
