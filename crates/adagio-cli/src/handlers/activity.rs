use crate::error::CliError;
use crate::output::{print_json, print_table_header, print_table_row};
use adagio_ipc::{DaemonClient, DaemonRequest};
use std::sync::Arc;

/// Show recent activity log.
pub async fn run_activity(
    client: &Arc<DaemonClient>,
    limit: u32,
    filter: Option<String>,
    json: bool,
) -> Result<(), CliError> {
    let result = client
        .request(DaemonRequest::GetActivityLog {
            limit: Some(limit),
            filter,
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&result);
        return Ok(());
    }
    let entries = result.as_array().cloned().unwrap_or_default();
    if entries.is_empty() {
        println!("No activity recorded");
        return Ok(());
    }
    print_table_header(&["TIME", "ACTION", "FILE"], &[20, 14, 50]);
    for e in &entries {
        let at = e
            .get("at")
            .and_then(|v| v.as_i64())
            .map(|ts| {
                chrono::DateTime::from_timestamp(ts / 1000, 0)
                    .map(|dt: chrono::DateTime<chrono::Utc>| {
                        dt.format("%Y-%m-%d %H:%M:%S").to_string()
                    })
                    .unwrap_or_else(|| "—".to_string())
            })
            .unwrap_or_else(|| "—".to_string());
        let verb = e.get("verb").and_then(|v| v.as_str()).unwrap_or("—");
        let path = e.get("where_path").and_then(|v| v.as_str()).unwrap_or("—");
        print_table_row(&[&at, verb, path], &[20, 14, 50]);
    }
    Ok(())
}
