use crate::cli::BandwidthCommand;
use crate::error::CliError;
use crate::output::{print_json, print_table_header, print_table_row};
use adagio_ipc::{DaemonClient, DaemonRequest};
use std::sync::Arc;

pub async fn run_bandwidth(
    client: &Arc<DaemonClient>,
    command: &BandwidthCommand,
    json: bool,
) -> Result<(), CliError> {
    match command {
        BandwidthCommand::Status => run_status(client, json).await,
        BandwidthCommand::Set {
            upload_kbps,
            download_kbps,
        } => run_set(client, *upload_kbps, *download_kbps, json).await,
        BandwidthCommand::Clear => run_clear(client, json).await,
    }
}

async fn run_status(client: &Arc<DaemonClient>, json: bool) -> Result<(), CliError> {
    let result = client
        .request(DaemonRequest::GetBandwidthStatus { account_id: None })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;

    if json {
        print_json(&result);
        return Ok(());
    }

    let ul_limit = result
        .get("upload_limit_kbps")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let dl_limit = result
        .get("download_limit_kbps")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let ul_rate = result
        .get("upload_rate_kbps")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let dl_rate = result
        .get("download_rate_kbps")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let ul_limit_s = if ul_limit == 0 {
        "Unlimited".to_string()
    } else {
        format!("{ul_limit} Kbps")
    };
    let dl_limit_s = if dl_limit == 0 {
        "Unlimited".to_string()
    } else {
        format!("{dl_limit} Kbps")
    };
    let ul_rate_s = format!("{ul_rate} Kbps");
    let dl_rate_s = format!("{dl_rate} Kbps");

    print_table_header(&["DIRECTION", "LIMIT", "CURRENT RATE"], &[12, 16, 16]);
    print_table_row(&["Upload", &ul_limit_s, &ul_rate_s], &[12, 16, 16]);
    print_table_row(&["Download", &dl_limit_s, &dl_rate_s], &[12, 16, 16]);
    Ok(())
}

async fn run_set(
    client: &Arc<DaemonClient>,
    upload_kbps: Option<u64>,
    download_kbps: Option<u64>,
    json: bool,
) -> Result<(), CliError> {
    if upload_kbps.is_none() && download_kbps.is_none() {
        return Err(CliError::DaemonError(
            "provide --upload-kbps and/or --download-kbps".to_string(),
        ));
    }

    // Fetch current limits so we only override the flags the user specified.
    let current = client
        .request(DaemonRequest::GetBandwidthStatus { account_id: None })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    let cur_up = current
        .get("upload_limit_kbps")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cur_dl = current
        .get("download_limit_kbps")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let new_up = upload_kbps.unwrap_or(cur_up);
    let new_dl = download_kbps.unwrap_or(cur_dl);

    client
        .request(DaemonRequest::SetBandwidthLimits {
            account_id: None,
            upload_kbps: new_up,
            download_kbps: new_dl,
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;

    if json {
        print_json(&serde_json::json!({"upload_kbps": new_up, "download_kbps": new_dl}));
    } else {
        let ul_s = if new_up == 0 {
            "Unlimited".to_string()
        } else {
            format!("{new_up} Kbps")
        };
        let dl_s = if new_dl == 0 {
            "Unlimited".to_string()
        } else {
            format!("{new_dl} Kbps")
        };
        println!("Bandwidth limits updated — upload: {ul_s}, download: {dl_s}");
    }
    Ok(())
}

async fn run_clear(client: &Arc<DaemonClient>, json: bool) -> Result<(), CliError> {
    client
        .request(DaemonRequest::ClearBandwidthLimits { account_id: None })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;

    if json {
        print_json(&serde_json::json!({"cleared": true}));
    } else {
        println!("Bandwidth limits cleared — transfers run at full speed");
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{BandwidthCommand, Cli, Commands};
    use clap::Parser;

    // T024-a: `bandwidth set --upload-kbps 200 --download-kbps 100` parses correctly.
    #[test]
    fn cli_parses_bandwidth_set_both_flags() {
        let args = [
            "adagio",
            "bandwidth",
            "set",
            "--upload-kbps",
            "200",
            "--download-kbps",
            "100",
        ];
        let cli = Cli::try_parse_from(args).expect("should parse");
        match cli.command.unwrap() {
            Commands::Bandwidth {
                command:
                    BandwidthCommand::Set {
                        upload_kbps,
                        download_kbps,
                    },
            } => {
                assert_eq!(upload_kbps, Some(200));
                assert_eq!(download_kbps, Some(100));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    // T024-b: `bandwidth clear` parses correctly.
    #[test]
    fn cli_parses_bandwidth_clear() {
        let args = ["adagio", "bandwidth", "clear"];
        let cli = Cli::try_parse_from(args).expect("should parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Bandwidth {
                command: BandwidthCommand::Clear
            })
        ));
    }

    // T024-c: `bandwidth status` parses correctly.
    #[test]
    fn cli_parses_bandwidth_status() {
        let args = ["adagio", "bandwidth", "status"];
        let cli = Cli::try_parse_from(args).expect("should parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Bandwidth {
                command: BandwidthCommand::Status
            })
        ));
    }
}
