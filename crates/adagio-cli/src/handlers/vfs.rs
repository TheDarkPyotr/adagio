use crate::error::CliError;
use crate::output::{dim, format_bytes, print_json, print_ok, print_table_header, print_table_row};
use adagio_ipc::{DaemonClient, DaemonRequest};
use std::sync::Arc;

/// Top-level VFS command dispatcher.
pub async fn run_vfs(
    client: &Arc<DaemonClient>,
    command: &crate::cli::VfsCommand,
    json: bool,
) -> Result<(), CliError> {
    use crate::cli::VfsCommand;
    match command {
        VfsCommand::Status { pair_id } => status(client, pair_id.clone(), json).await,
        VfsCommand::Pin { path, pair_id } => pin(client, pair_id.clone(), path, true, json).await,
        VfsCommand::Unpin { path, pair_id } => {
            pin(client, pair_id.clone(), path, false, json).await
        }
        VfsCommand::Evict { path, pair_id, all } => {
            evict(client, pair_id.clone(), path.clone(), *all, json).await
        }
    }
}

async fn status(
    client: &Arc<DaemonClient>,
    pair_id: Option<String>,
    json: bool,
) -> Result<(), CliError> {
    // If pair_id given: show stats for that pair. Otherwise show all pairs and aggregate.
    let pid = pair_id.unwrap_or_default();
    if pid.is_empty() {
        // List all pairs and print stats for each.
        let pairs = client
            .request(DaemonRequest::ListPairs)
            .await
            .map_err(|e| CliError::DaemonError(e.to_string()))?;
        let arr = pairs.as_array().cloned().unwrap_or_default();
        if arr.is_empty() {
            println!("  {}", dim("No sync pairs configured."));
            return Ok(());
        }
        if json {
            let mut all = Vec::new();
            for p in &arr {
                let pid = p["id"].as_str().unwrap_or("").to_string();
                if let Ok(stats) = client
                    .request(DaemonRequest::GetVfsStats { pair_id: pid })
                    .await
                {
                    all.push(stats);
                }
            }
            print_json(&serde_json::json!(all));
        } else {
            print_table_header(
                &["PAIR", "CLOUD-ONLY", "CACHED", "PINNED", "USED"],
                &[36, 10, 10, 8, 12],
            );
            for p in &arr {
                let pid = p["id"].as_str().unwrap_or("").to_string();
                let root = p["local_root"].as_str().unwrap_or("?");
                if let Ok(stats) = client
                    .request(DaemonRequest::GetVfsStats { pair_id: pid })
                    .await
                {
                    let cloud = stats["cloud_only_count"].as_u64().unwrap_or(0).to_string();
                    let avail = stats["locally_available_count"]
                        .as_u64()
                        .unwrap_or(0)
                        .to_string();
                    let pinned = stats["pinned_count"].as_u64().unwrap_or(0).to_string();
                    let used = format_bytes(stats["cached_bytes"].as_u64().unwrap_or(0));
                    print_table_row(
                        &[root, &cloud, &avail, &pinned, &used],
                        &[36, 10, 10, 8, 12],
                    );
                }
            }
        }
    } else {
        let stats = client
            .request(DaemonRequest::GetVfsStats { pair_id: pid })
            .await
            .map_err(|e| CliError::DaemonError(e.to_string()))?;
        if json {
            print_json(&stats);
            return Ok(());
        }
        let cloud = stats["cloud_only_count"].as_u64().unwrap_or(0);
        let avail = stats["locally_available_count"].as_u64().unwrap_or(0);
        let pinned = stats["pinned_count"].as_u64().unwrap_or(0);
        let used = stats["cached_bytes"].as_u64().unwrap_or(0);
        let max = stats["cache_max_bytes"].as_u64().unwrap_or(0);
        println!("  {}  cloud-only", dim(&format!("{cloud:>6}")));
        println!("  {}  locally available", dim(&format!("{avail:>6}")));
        println!("  {}  pinned", dim(&format!("{pinned:>6}")));
        println!(
            "  {}  cached  {}  max",
            dim(&format_bytes(used)),
            dim(&format_bytes(max))
        );
    }
    Ok(())
}

async fn pin(
    client: &Arc<DaemonClient>,
    pair_id: Option<String>,
    path: &str,
    pinned: bool,
    json: bool,
) -> Result<(), CliError> {
    let pid = require_pair_id(pair_id)?;
    client
        .request(DaemonRequest::SetVfsPin {
            pair_id: pid,
            path: path.to_string(),
            pinned,
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&serde_json::json!({ "pinned": pinned, "path": path }));
    } else if pinned {
        print_ok(&format!("Pinned: {path}"));
    } else {
        print_ok(&format!("Unpinned: {path}"));
    }
    Ok(())
}

async fn evict(
    client: &Arc<DaemonClient>,
    pair_id: Option<String>,
    path: Option<String>,
    all: bool,
    json: bool,
) -> Result<(), CliError> {
    let pid = require_pair_id(pair_id)?;
    if all {
        // Evict by listing all locally-available entries.
        let _stats = client
            .request(DaemonRequest::GetVfsStats {
                pair_id: pid.clone(),
            })
            .await
            .map_err(|e| CliError::DaemonError(e.to_string()))?;
        // Just report — actual per-file eviction would need a ListVfsEntries call (future).
        println!("  {}", dim("Bulk eviction not yet implemented (future). Use  adagio vfs evict <path>  per file."));
        return Ok(());
    }
    let p = path.ok_or_else(|| CliError::DaemonError("path required".to_string()))?;
    client
        .request(DaemonRequest::EvictVfsFile {
            pair_id: pid,
            path: p.clone(),
        })
        .await
        .map_err(|e| CliError::DaemonError(e.to_string()))?;
    if json {
        print_json(&serde_json::json!({ "evicted": p }));
    } else {
        print_ok(&format!("Evicted: {p}"));
    }
    Ok(())
}

fn require_pair_id(id: Option<String>) -> Result<String, CliError> {
    id.filter(|s| !s.is_empty()).ok_or_else(|| {
        CliError::DaemonError("--pair-id is required for this subcommand".to_string())
    })
}
