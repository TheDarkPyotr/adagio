// Use jemalloc on non-Windows platforms: it returns freed pages to the OS
// aggressively via background threads, preventing the glibc heap fragmentation
// that causes RSS to grow ~60-70% after large sync cycles (AC-3 violation).
#[cfg(not(target_os = "windows"))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

mod autostart;
mod dispatcher;
mod e2ee_runner;
mod events;
mod network_monitor;
mod server;
mod shutdown;

use adagio_core::account_manager::AccountManager;
use adagio_core::config::SyncPairManager;
use adagio_core::cycle::DefaultSyncEngine;
use adagio_core::journal::sqlite::SqliteJournal;
use adagio_core::network::NetworkPolicy;
use adagio_ipc::transport::{daemon_socket_path, ensure_socket_dir};
use dispatcher::DaemonProcess;
use events::EventBroadcaster;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Logging ───────────────────────────────────────────────────────────────
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "adagio=info,warn".to_string());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .init();

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "adagio-daemon starting"
    );

    // ── Config directory ──────────────────────────────────────────────────────
    // Accept --config-dir PATH from the desktop launcher so both processes
    // use the same data directory (avoids Tauri vs XDG path mismatch).
    let config_dir = {
        let args: Vec<String> = std::env::args().collect();
        if let Some(idx) = args.iter().position(|a| a == "--config-dir") {
            args.get(idx + 1)
                .map(std::path::PathBuf::from)
                .ok_or_else(|| anyhow::anyhow!("--config-dir requires a path argument"))?
        } else {
            adagio_ipc::transport::platform_config_dir()?
        }
    };
    std::fs::create_dir_all(&config_dir)?;

    // ── Single-instance guard ─────────────────────────────────────────────────
    let socket_path = daemon_socket_path();
    ensure_socket_dir(&socket_path)?;

    #[cfg(unix)]
    let listener = match tokio::net::UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(_) => {
            // Socket already exists — check if another daemon is alive.
            if ping_existing_daemon(&socket_path).await {
                info!("another daemon instance is already running — exiting");
                return Ok(());
            }
            // Stale socket — remove and retry.
            let _ = std::fs::remove_file(&socket_path);
            tokio::net::UnixListener::bind(&socket_path)?
        }
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
    }

    // ── Journal + engine ──────────────────────────────────────────────────────
    let db_path = config_dir.join("adagio.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let journal = Arc::new(SqliteJournal::open(&db_url).await?);

    let config_path = config_dir.join("config.json");
    let saved = load_saved_config(&config_path);

    let accounts = Arc::new(AccountManager::new());
    restore_accounts(&accounts, &saved);
    let mut pair_manager = SyncPairManager::new();
    restore_pairs(&mut pair_manager, &saved);
    let pairs = Arc::new(RwLock::new(pair_manager));

    let engine = Arc::new(DefaultSyncEngine::new());
    let events = EventBroadcaster::new(256);
    let e2ee_triggers: crate::dispatcher::E2eeTriggerMap =
        Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));

    // ── Network policy (loaded from config or defaulted) ──────────────────────
    let network_policy_val: NetworkPolicy =
        serde_json::from_value(saved.get("network_policy").cloned().unwrap_or_default())
            .unwrap_or_default();
    let network_policy = Arc::new(RwLock::new(network_policy_val));

    // ── Spawn pair runners ────────────────────────────────────────────────────
    let cancel = CancellationToken::new();
    let tracker = TaskTracker::new();

    {
        let pairs_guard = pairs.read().await;
        let all_pairs: Vec<_> = pairs_guard.all_pairs().into_iter().cloned().collect();
        drop(pairs_guard);

        for pair in all_pairs {
            // Ensure account + pair are registered in the journal DB so
            // journal_entries FK constraints are satisfied on the first cycle.
            if let Ok(accts) = accounts.list() {
                if let Some(acct) = accts.iter().find(|a| a.id == pair.account_id) {
                    let _ = journal.register_pair(acct, &pair).await;
                }
            }

            // E2EE pairs use a dedicated encrypted sync runner instead of the
            // plain-text PairRunner. Files are AES-128-GCM encrypted before upload
            // and decrypted after download; the server only stores ciphertext.
            if pair.e2ee_enabled {
                let client = build_nextcloud_client(&accounts, &pair).await;
                if let Some(nc_client) = client {
                    let cred_provider = Arc::new(e2ee_runner::DaemonCredentialProvider {
                        accounts: accounts.clone(),
                        pairs: pairs.clone(),
                    });
                    let e2ee_client = Arc::new(adagio_e2ee::provider::NcE2eeClient::new(
                        cred_provider,
                        journal.clone(),
                    ));
                    tracing::info!(pair_id = %pair.id, "E2EE pair: starting encrypted sync runner");
                    let (cancel, trigger_tx) = e2ee_runner::spawn_e2ee_runner(
                        pair.clone(),
                        e2ee_client,
                        Arc::new(nc_client),
                        journal.clone(),
                    );
                    e2ee_triggers
                        .lock()
                        .await
                        .insert(pair.id.clone(), (cancel, trigger_tx));
                } else {
                    tracing::warn!(pair_id = %pair.id, "E2EE pair: no credentials, runner not started");
                }
                continue;
            }

            let client = build_nextcloud_client(&accounts, &pair).await;
            if let Some(client) = client {
                if pair.vfs_enabled {
                    // VFS pairs use the metadata-only runner (US5 coexistence).
                    let provider = adagio_vfs::create_platform_provider();
                    engine.start_vfs_pair(pair, Arc::new(client), journal.clone(), provider);
                    continue;
                }
                engine
                    .start_pair(
                        pair,
                        Arc::new(client),
                        journal.clone(),
                        None, // conflict_tx: events wired via EventBroadcaster instead
                    )
                    .await;
            }
        }
    }

    // ── Network monitor ───────────────────────────────────────────────────────
    let network_monitor = network_monitor::spawn_network_monitor(
        engine.clone(),
        network_policy.clone(),
        &tracker,
        cancel.clone(),
    );

    // ── IPC server ────────────────────────────────────────────────────────────
    let state = Arc::new(DaemonProcess {
        engine: engine.clone(),
        journal: journal.clone(),
        accounts: accounts.clone(),
        pairs: pairs.clone(),
        events: events.clone(),
        started_at: Instant::now(),
        config_path,
        network_policy,
        network_monitor,
        e2ee_triggers,
    });

    #[cfg(unix)]
    {
        let state_srv = state.clone();
        let events_srv = events.clone();
        tracker.spawn(async move {
            server::accept_loop(listener, state_srv, move || events_srv.subscribe()).await;
        });
    }

    // ── Signal handling ───────────────────────────────────────────────────────
    wait_for_shutdown_signal().await;
    info!("shutdown signal received");

    events.emit_shutting_down("signal".to_string());

    // ── Graceful shutdown ─────────────────────────────────────────────────────
    shutdown::graceful_shutdown(cancel, tracker).await;

    // Checkpoint the WAL before dropping the pool.
    if let Err(e) = journal.checkpoint().await {
        warn!(error = %e, "WAL checkpoint failed");
    }

    // Clean up socket.
    #[cfg(unix)]
    let _ = std::fs::remove_file(&socket_path);

    info!("adagio-daemon stopped");
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// platform_config_dir() is now in adagio-ipc::transport — see ADR-012.

#[cfg(unix)]
async fn ping_existing_daemon(socket_path: &std::path::Path) -> bool {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    match tokio::time::timeout(
        std::time::Duration::from_millis(500),
        tokio::net::UnixStream::connect(socket_path),
    )
    .await
    {
        Ok(Ok(stream)) => {
            let (r, mut w) = stream.into_split();
            let ping = serde_json::json!({"id":1,"method":"ping","params":{}});
            let _ = w.write_all(format!("{ping}\n").as_bytes()).await;
            let mut reader = BufReader::new(r);
            let mut line = String::new();
            matches!(
                tokio::time::timeout(
                    std::time::Duration::from_millis(300),
                    reader.read_line(&mut line),
                )
                .await,
                Ok(Ok(n)) if n > 0
            )
        }
        _ => false,
    }
}

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.ok();
    }
}

fn load_saved_config(path: &std::path::Path) -> serde_json::Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({"version":1,"accounts":[],"pairs":[]}))
}

fn restore_accounts(manager: &AccountManager, saved: &serde_json::Value) {
    use adagio_core::types::{Account, AccountId};
    use chrono::Utc;
    if let Some(accounts) = saved["accounts"].as_array() {
        for a in accounts {
            let account = Account {
                id: AccountId(a["id"].as_str().unwrap_or_default().to_string()),
                display_name: a["display_name"].as_str().unwrap_or_default().to_string(),
                server_url: a["server_url"].as_str().unwrap_or_default().to_string(),
                username: a["username"].as_str().unwrap_or_default().to_string(),
                keychain_service_key: a["keychain_service_key"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                created_at: Utc::now(),
                upload_limit_kbps: a["upload_limit_kbps"].as_u64().unwrap_or(0),
                download_limit_kbps: a["download_limit_kbps"].as_u64().unwrap_or(0),
            };
            if let Err(e) = manager.add(account) {
                warn!(error = %e, "failed to restore account from config");
            }
        }
    }
}

fn restore_pairs(manager: &mut SyncPairManager, saved: &serde_json::Value) {
    use adagio_core::types::{
        AccountId, ConflictPolicy, LocalPath, PairId, PairStatus, RelativePath, RemotePath,
        SyncPair,
    };
    use chrono::Utc;
    if let Some(pairs) = saved["pairs"].as_array() {
        for p in pairs {
            let pair = SyncPair {
                id: PairId(p["id"].as_str().unwrap_or_default().to_string()),
                account_id: AccountId(p["account_id"].as_str().unwrap_or_default().to_string()),
                local_root: LocalPath::new(std::path::PathBuf::from(
                    p["local_root"].as_str().unwrap_or_default(),
                )),
                remote_root: RemotePath::new(p["remote_root"].as_str().unwrap_or_default()),
                status: PairStatus::Idle,
                exclude_patterns: p["exclude_patterns"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
                selective_paths: p["selective_paths"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(RelativePath::new))
                            .collect()
                    })
                    .unwrap_or_default(),
                created_at: Utc::now(),
                last_synced_at: None,
                scan_interval_secs: p["scan_interval_secs"].as_u64().unwrap_or(7200),
                scan_on_startup: p["scan_on_startup"].as_bool().unwrap_or(true),
                max_upload_concurrency: p["max_upload_concurrency"].as_u64().unwrap_or(3) as u8,
                max_download_concurrency: p["max_download_concurrency"].as_u64().unwrap_or(3) as u8,
                conflict_policy: ConflictPolicy::Ask,
                bulk_upload_workers: p["bulk_upload_workers"].as_u64().unwrap_or(8) as u8,
                bulk_upload_threshold_files: p["bulk_upload_threshold_files"].as_u64().unwrap_or(50)
                    as u32,
                bulk_upload_chunk_threshold_bytes: p["bulk_upload_chunk_threshold_bytes"]
                    .as_u64()
                    .unwrap_or(10 * 1024 * 1024),
                vfs_enabled: p["vfs_enabled"].as_bool().unwrap_or(false),
                vfs_cache_max_bytes: p["vfs_cache_max_bytes"]
                    .as_u64()
                    .unwrap_or(20 * 1024 * 1024 * 1024),
                vfs_eviction_threshold_bytes: p["vfs_eviction_threshold_bytes"]
                    .as_u64()
                    .unwrap_or(5 * 1024 * 1024 * 1024),
                e2ee_enabled: p["e2ee_enabled"].as_bool().unwrap_or(false),
                e2ee_account_id: p["e2ee_account_id"].as_str().map(|s| s.to_string()),
            };
            manager.register_full_pair(pair);
        }
    }
}

async fn build_nextcloud_client(
    accounts: &AccountManager,
    pair: &adagio_core::types::SyncPair,
) -> Option<adagio_nextcloud::client::NextcloudClient> {
    let account = accounts
        .list()
        .ok()?
        .into_iter()
        .find(|a| a.id == pair.account_id)?;
    let key = account.keychain_service_key.clone();
    let password =
        tokio::task::spawn_blocking(move || adagio_nextcloud::auth::retrieve_credentials(&key))
            .await
            .ok()?
            .ok()??;
    Some(adagio_nextcloud::client::NextcloudClient::new(
        &account.server_url,
        &account.username,
        &password,
    ))
}

#[cfg(test)]
mod tests {
    // T012 — single-instance guard: binding same socket twice fails.
    #[tokio::test]
    async fn single_instance_exits_when_socket_already_bound() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let sock = dir.path().join("test.sock");

        let _listener1 = tokio::net::UnixListener::bind(&sock).expect("first bind must succeed");
        let result = tokio::net::UnixListener::bind(&sock);
        assert!(result.is_err(), "second bind must fail — already in use");
    }
}
