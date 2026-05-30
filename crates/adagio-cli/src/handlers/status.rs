use crate::error::CliError;
use crate::output::{print_json, print_table_header, print_table_row};
use adagio_ipc::{DaemonClient, DaemonRequest};
use std::sync::Arc;

/// Show current sync status.
pub async fn run_status(client: &Arc<DaemonClient>, json: bool) -> Result<(), CliError> {
    let result = client
        .request(DaemonRequest::GetStatus)
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&result);
        return Ok(());
    }
    let status = result["status"].as_str().unwrap_or("unknown");
    let active = result
        .get("active_file_count")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let last_sync = result
        .get("last_sync_at")
        .and_then(|v| v.as_i64())
        .filter(|&ts| ts > 0)
        .map(|ts| {
            chrono::DateTime::from_timestamp(ts / 1000, 0)
                .map(|dt: chrono::DateTime<chrono::Utc>| {
                    dt.format("%Y-%m-%d %H:%M UTC").to_string()
                })
                .unwrap_or_else(|| "—".to_string())
        })
        .unwrap_or_else(|| "never".to_string());
    print_table_header(&["STATUS", "ACTIVE FILES", "LAST SYNCED"], &[14, 14, 24]);
    let active_s = active.to_string();
    print_table_row(&[status, &active_s, &last_sync], &[14, 14, 24]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::output::format_uptime;
    #[test]
    fn format_uptime_in_handler() {
        assert_eq!(format_uptime(3600), "1h 0m 0s");
    }
}
