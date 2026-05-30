use std::sync::Arc;

use adagio_ipc::{DaemonClient, DaemonRequest};

use crate::output::{bold, cyan, dim, format_uptime, green, red, shorten_path, status_icon, yellow};

// ── Entry point ───────────────────────────────────────────────────────────────

/// Display a formatted status overview and exit.
///
/// This is what `adagio` (no subcommand) shows: a clean at-a-glance view of
/// the daemon and all sync pairs, similar to `git status`.
pub async fn run_dashboard(client: Arc<DaemonClient>) {
    let engine = fetch_engine(&client).await;
    let uptime = fetch_uptime(&client).await;
    let account = fetch_account(&client).await;
    let pairs = fetch_pairs(&client).await;

    // ── Header ────────────────────────────────────────────────────────────────
    println!();
    if !account.is_empty() {
        println!("  {}  {}", bold("adagio"), dim(&format!("· {account}")));
    } else {
        println!("  {}", bold("adagio"));
    }
    println!();

    // ── Engine status ─────────────────────────────────────────────────────────
    let (engine_icon, engine_label) = match engine.as_str() {
        "syncing" => (cyan("⟳"), cyan("syncing")),
        "paused"  => (yellow("⏸"), yellow("paused")),
        "error"   => (red("✗"), red("error")),
        _         => (green("✓"), dim("idle")),
    };
    let uptime_s = if uptime > 0 {
        format!("  {}", dim(&format!("daemon up {}", format_uptime(uptime))))
    } else {
        String::new()
    };
    println!("  {engine_icon}  {engine_label}{uptime_s}");
    println!();

    // ── Pairs ─────────────────────────────────────────────────────────────────
    if pairs.is_empty() {
        println!("  {}", dim("No sync pairs configured."));
        println!("  {}", dim("Add one with: adagio pairs add --local <dir> --remote <path> --account <id>"));
    } else {
        for pair in &pairs {
            let icon  = status_icon(&engine);
            let root  = shorten_path(&pair.local_root, 48);
            let arrow = dim("→");
            let dest  = dim(&pair.remote_root);
            let colored_icon = match engine.as_str() {
                "syncing" => cyan(icon),
                "paused"  => yellow(icon),
                "error"   => red(icon),
                _         => green(icon),
            };
            println!("  {colored_icon}  {root}  {arrow}  {dest}");
        }
    }
    println!();

    // ── Hints ─────────────────────────────────────────────────────────────────
    let hints = [
        ("adagio sync",           "trigger sync now"),
        ("adagio status",         "engine status"),
        ("adagio activity",       "recent activity"),
        ("adagio pairs list",     "manage pairs"),
        ("adagio --help",         "all commands"),
    ];
    for (cmd, desc) in hints {
        println!("  {}  {}", dim(&format!("{cmd:<24}")), dim(desc));
    }
    println!();
}

// ── Data fetchers ─────────────────────────────────────────────────────────────

async fn fetch_engine(client: &Arc<DaemonClient>) -> String {
    client
        .request(DaemonRequest::GetStatus)
        .await
        .ok()
        .and_then(|v| v["status"].as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "idle".to_string())
}

async fn fetch_uptime(client: &Arc<DaemonClient>) -> u64 {
    client
        .request(DaemonRequest::Ping)
        .await
        .ok()
        .and_then(|v| v["uptime_secs"].as_u64())
        .unwrap_or(0)
}

async fn fetch_account(client: &Arc<DaemonClient>) -> String {
    client
        .request(DaemonRequest::ListAccounts)
        .await
        .ok()
        .and_then(|v| v.as_array().and_then(|a| a.first()).cloned())
        .map(|a| {
            let name = a["display_name"].as_str().unwrap_or("").to_string();
            let url = a["server_url"]
                .as_str()
                .unwrap_or("")
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .to_string();
            if name.is_empty() { url } else { format!("{name} @ {url}") }
        })
        .unwrap_or_default()
}

struct PairInfo {
    local_root: String,
    remote_root: String,
}

async fn fetch_pairs(client: &Arc<DaemonClient>) -> Vec<PairInfo> {
    client
        .request(DaemonRequest::ListPairs)
        .await
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|p| PairInfo {
            local_root:  p["local_root"].as_str().unwrap_or("?").to_string(),
            remote_root: p["remote_root"].as_str().unwrap_or("/").to_string(),
        })
        .collect()
}
