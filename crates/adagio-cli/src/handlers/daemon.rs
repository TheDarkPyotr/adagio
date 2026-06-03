use crate::cli::DaemonCommand;
use crate::error::CliError;
use crate::output::{format_uptime, print_json, print_ok};
use adagio_ipc::{
    transport::{daemon_socket_path, platform_config_dir},
    DaemonClient, DaemonRequest,
};

/// Dispatch daemon subcommands.
pub async fn run_daemon(command: &DaemonCommand, json: bool) -> Result<(), CliError> {
    match command {
        DaemonCommand::Start => run_daemon_start(json).await,
        DaemonCommand::Stop => run_daemon_stop(json).await,
        DaemonCommand::Status => run_daemon_status(json).await,
    }
}

async fn run_daemon_start(json: bool) -> Result<(), CliError> {
    let daemon_path = daemon_binary_path().map_err(|e| CliError::Unreachable(e.to_string()))?;
    let config_dir = platform_config_dir().map_err(|e| CliError::Unreachable(e.to_string()))?;

    // Try connecting first — if already running, report that.
    let _socket = daemon_socket_path();
    if let Ok(client) = DaemonClient::connect_or_start(&daemon_path).await {
        // Check if it was already running by pinging.
        if let Ok(v) = client.request(DaemonRequest::Ping).await {
            let started = v.get("uptime_secs").and_then(|u| u.as_u64()).unwrap_or(0) > 0;
            if json {
                print_json(
                    &serde_json::json!({"started": true, "already_running": !started, "config_dir": config_dir.to_string_lossy()}),
                );
            } else {
                print_ok(if started {
                    "Daemon started"
                } else {
                    "Daemon is already running"
                });
            }
            return Ok(());
        }
    }
    Err(CliError::Unreachable("Could not start daemon".into()))
}

async fn run_daemon_stop(json: bool) -> Result<(), CliError> {
    let daemon_path = daemon_binary_path().map_err(|e| CliError::Unreachable(e.to_string()))?;
    match DaemonClient::connect_or_start(&daemon_path).await {
        Ok(client) => {
            client
                .request(DaemonRequest::StopDaemon)
                .await
                .map_err(|e| CliError::DaemonError(e.to_string()))?;
            if json {
                print_json(&serde_json::json!({"stopped": true}));
            } else {
                print_ok("Daemon stopped");
            }
        }
        Err(_) => {
            if json {
                print_json(
                    &serde_json::json!({"stopped": false, "message": "Daemon was not running"}),
                );
            } else {
                print_ok("Daemon is not running");
            }
        }
    }
    Ok(())
}

async fn run_daemon_status(json: bool) -> Result<(), CliError> {
    let daemon_path = daemon_binary_path().map_err(|e| CliError::Unreachable(e.to_string()))?;
    match DaemonClient::connect_or_start(&daemon_path).await {
        Ok(client) => {
            let v = client
                .request(DaemonRequest::Ping)
                .await
                .map_err(|e| CliError::DaemonError(e.to_string()))?;
            if json {
                print_json(
                    &serde_json::json!({"running": true, "version": v["version"], "uptime_secs": v["uptime_secs"]}),
                );
                return Ok(());
            }
            let version = v.get("version").and_then(|s| s.as_str()).unwrap_or("?");
            let uptime = v.get("uptime_secs").and_then(|u| u.as_u64()).unwrap_or(0);
            println!("Status:   running");
            println!("Version:  {version}");
            println!("Uptime:   {}", format_uptime(uptime));
        }
        Err(_) => {
            if json {
                print_json(&serde_json::json!({"running": false}));
            } else {
                println!("Status:   stopped");
            }
        }
    }
    Ok(())
}

fn daemon_binary_path() -> anyhow::Result<std::path::PathBuf> {
    let current = std::env::current_exe()?;
    let parent = current
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot determine daemon path"))?;
    #[cfg(windows)]
    {
        Ok(parent.join("adagio-daemon.exe"))
    }
    #[cfg(not(windows))]
    {
        Ok(parent.join("adagio-daemon"))
    }
}

#[cfg(test)]
mod tests {
    use crate::output::format_uptime;
    // T036/T037/T038 integration tests require running daemon.
    #[test]
    fn daemon_status_formats_uptime_seconds() {
        // 2h 1m 1s = 7261s
        assert_eq!(format_uptime(7261), "2h 1m 1s");
    }
}
