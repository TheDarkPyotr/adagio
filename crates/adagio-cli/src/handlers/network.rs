use crate::cli::NetworkCommand;
use crate::error::CliError;
use crate::output::{print_json, print_table_header, print_table_row};
use adagio_ipc::{DaemonClient, DaemonRequest};
use std::sync::Arc;

pub async fn run_network(
    client: &Arc<DaemonClient>,
    command: &NetworkCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        NetworkCommand::Status => run_status(client, json).await,
        NetworkCommand::Set {
            on_metered,
            on_battery,
            throttle_kbps,
        } => {
            run_set(
                client,
                on_metered.clone(),
                on_battery.clone(),
                *throttle_kbps,
                json,
            )
            .await
        }
        NetworkCommand::BlockSsid { ssid } => run_block_ssid(client, ssid, json).await,
        NetworkCommand::UnblockSsid { ssid } => run_unblock_ssid(client, ssid, json).await,
        NetworkCommand::ListBlocked => run_list_blocked(client, json).await,
    }
}

async fn run_status(client: &Arc<DaemonClient>, json: bool) -> Result<(), CliError> {
    let result = client
        .request(DaemonRequest::GetNetworkStatus)
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;

    if json {
        print_json(&result);
        return Ok(());
    }

    let metered = result
        .get("metered")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let on_battery = result
        .get("on_battery")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let ssid = result
        .get("ssid")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let effective = result
        .get("effective_action")
        .and_then(|v| v.as_str())
        .unwrap_or("allow");
    let reason = result.get("reason").and_then(|v| v.as_str()).unwrap_or("");

    let on_metered_policy = result
        .get("policy")
        .and_then(|p| p.get("on_metered"))
        .and_then(|v| v.as_str())
        .unwrap_or("allow");
    let on_battery_policy = result
        .get("policy")
        .and_then(|p| p.get("on_battery"))
        .and_then(|v| v.as_str())
        .unwrap_or("allow");
    let throttle_kbps = result
        .get("policy")
        .and_then(|p| p.get("throttle_kbps"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let on_battery_policy_s = if on_battery_policy == "throttle" && throttle_kbps > 0 {
        format!("Throttle ({throttle_kbps} Kbps)")
    } else {
        on_battery_policy.to_string()
    };
    let on_metered_policy_s = if on_metered_policy == "throttle" && throttle_kbps > 0 {
        format!("Throttle ({throttle_kbps} Kbps)")
    } else {
        on_metered_policy.to_string()
    };

    print_table_header(&["CONDITION", "STATE", "POLICY"], &[14, 16, 24]);
    print_table_row(
        &[
            "Metered",
            if metered { "Yes" } else { "No" },
            &on_metered_policy_s,
        ],
        &[14, 16, 24],
    );
    print_table_row(
        &[
            "Battery",
            if on_battery { "Yes" } else { "No" },
            &on_battery_policy_s,
        ],
        &[14, 16, 24],
    );
    print_table_row(&["SSID", ssid, "—"], &[14, 16, 24]);

    let reason_s = if reason.is_empty() {
        String::new()
    } else {
        format!(" ({reason})")
    };
    println!("\nEffective action: {effective}{reason_s}");
    Ok(())
}

async fn run_set(
    client: &Arc<DaemonClient>,
    on_metered: Option<String>,
    on_battery: Option<String>,
    throttle_kbps: Option<u64>,
    json: bool,
) -> Result<(), CliError> {
    if on_metered.is_none() && on_battery.is_none() && throttle_kbps.is_none() {
        return Err(CliError::DaemonError(
            "provide at least one of --on-metered, --on-battery, or --throttle-kbps".to_string(),
        ));
    }
    client
        .request(DaemonRequest::SetNetworkPolicy {
            on_metered,
            on_battery,
            throttle_kbps,
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;

    if json {
        print_json(&serde_json::json!({"ok": true}));
    } else {
        println!("Network policy updated");
    }
    Ok(())
}

async fn run_block_ssid(
    client: &Arc<DaemonClient>,
    ssid: &str,
    json: bool,
) -> Result<(), CliError> {
    client
        .request(DaemonRequest::ManageBlockedSsid {
            action: "add".to_string(),
            ssid: Some(ssid.to_string()),
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;

    if json {
        print_json(&serde_json::json!({"blocked": ssid}));
    } else {
        println!("SSID blocked: {ssid}");
    }
    Ok(())
}

async fn run_unblock_ssid(
    client: &Arc<DaemonClient>,
    ssid: &str,
    json: bool,
) -> Result<(), CliError> {
    client
        .request(DaemonRequest::ManageBlockedSsid {
            action: "remove".to_string(),
            ssid: Some(ssid.to_string()),
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;

    if json {
        print_json(&serde_json::json!({"unblocked": ssid}));
    } else {
        println!("SSID unblocked: {ssid}");
    }
    Ok(())
}

async fn run_list_blocked(client: &Arc<DaemonClient>, json: bool) -> Result<(), CliError> {
    let result = client
        .request(DaemonRequest::ManageBlockedSsid {
            action: "list".to_string(),
            ssid: None,
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;

    if json {
        print_json(&result);
        return Ok(());
    }

    if let Some(arr) = result.as_array() {
        if arr.is_empty() {
            println!("No blocked SSIDs");
        } else {
            for item in arr {
                println!("{}", item.as_str().unwrap_or(""));
            }
        }
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::cli::{Cli, Commands, NetworkCommand};
    use clap::Parser;

    // T013/T020/T032: Parse network subcommands.
    #[test]
    fn cli_parses_network_status() {
        let cli = Cli::try_parse_from(["adagio", "network", "status"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Network {
                command: NetworkCommand::Status
            }
        ));
    }

    #[test]
    fn cli_parses_network_set_on_metered_pause() {
        let cli =
            Cli::try_parse_from(["adagio", "network", "set", "--on-metered", "pause"]).unwrap();
        match cli.command {
            Commands::Network {
                command: NetworkCommand::Set { on_metered, .. },
            } => assert_eq!(on_metered, Some("pause".to_string())),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn cli_parses_network_set_on_battery_throttle() {
        let cli = Cli::try_parse_from([
            "adagio",
            "network",
            "set",
            "--on-battery",
            "throttle",
            "--throttle-kbps",
            "200",
        ])
        .unwrap();
        match cli.command {
            Commands::Network {
                command:
                    NetworkCommand::Set {
                        on_battery,
                        throttle_kbps,
                        ..
                    },
            } => {
                assert_eq!(on_battery, Some("throttle".to_string()));
                assert_eq!(throttle_kbps, Some(200));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn cli_parses_network_block_ssid() {
        let cli =
            Cli::try_parse_from(["adagio", "network", "block-ssid", "CoffeeShop-Guest"]).unwrap();
        match cli.command {
            Commands::Network {
                command: NetworkCommand::BlockSsid { ssid },
            } => assert_eq!(ssid, "CoffeeShop-Guest"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn cli_parses_network_unblock_ssid() {
        let cli =
            Cli::try_parse_from(["adagio", "network", "unblock-ssid", "CoffeeShop-Guest"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Network {
                command: NetworkCommand::UnblockSsid { .. }
            }
        ));
    }

    #[test]
    fn cli_parses_network_list_blocked() {
        let cli = Cli::try_parse_from(["adagio", "network", "list-blocked"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Network {
                command: NetworkCommand::ListBlocked
            }
        ));
    }
}
