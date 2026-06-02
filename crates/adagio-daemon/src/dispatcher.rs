use adagio_ipc::{DaemonRequest, DaemonResponse};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::instrument;

use adagio_core::account_manager::AccountManager;
use adagio_core::config::SyncPairManager;
use adagio_core::cycle::{DefaultSyncEngine, SyncEngine};
use adagio_core::journal::sqlite::SqliteJournal;
use adagio_core::journal::Journal;
use adagio_core::network::{NetworkMonitor, NetworkPolicy};
use adagio_core::types::PairId;
use tokio_util::sync::CancellationToken;

use crate::events::EventBroadcaster;

/// Cancel token + trigger sender for one E2EE runner, stored per pair.
pub type E2eeRunnerHandle = (CancellationToken, tokio::sync::mpsc::Sender<()>);
/// Map of active E2EE runners keyed by pair ID.
pub type E2eeTriggerMap =
    Arc<tokio::sync::Mutex<std::collections::HashMap<PairId, E2eeRunnerHandle>>>;

/// All the shared state that the dispatcher needs to handle RPC calls.
pub struct DaemonProcess {
    pub engine: Arc<DefaultSyncEngine>,
    pub journal: Arc<SqliteJournal>,
    pub accounts: Arc<AccountManager>,
    pub pairs: Arc<RwLock<SyncPairManager>>,
    pub events: EventBroadcaster,
    pub started_at: Instant,
    /// Path to the config file on disk (for commands that mutate config).
    pub config_path: std::path::PathBuf,
    /// Shared network policy — updated by IPC, read by the NetworkMonitor.
    pub network_policy: Arc<RwLock<NetworkPolicy>>,
    /// Live network state — updated by the NetworkMonitor poll loop.
    pub network_monitor: Arc<NetworkMonitor>,
    /// Handles for E2EE sync runners (keyed by pair ID).
    /// Each entry holds the cancel token (to stop the runner) and the trigger
    /// sender (to fire an immediate cycle).
    pub e2ee_triggers: E2eeTriggerMap,
}

/// Route a `DaemonRequest` to the appropriate engine/journal call and return
/// the serialisable response.
///
/// This is the single dispatch function the server calls for every RPC request.
/// It mirrors the Tauri command handler bodies from `adagio-desktop/src/commands/`.
#[instrument(skip(state), fields(method = ?req))]
pub async fn dispatch(req: DaemonRequest, state: &DaemonProcess) -> Result<DaemonResponse, String> {
    match req {
        // ── System ────────────────────────────────────────────────────────────
        DaemonRequest::Ping => {
            let uptime_secs = state.started_at.elapsed().as_secs();
            let version = env!("CARGO_PKG_VERSION").to_string();
            Ok(DaemonResponse::Pong {
                version,
                uptime_secs,
            })
        }

        DaemonRequest::StopDaemon => {
            // The server handles the actual shutdown; just ack here.
            Ok(DaemonResponse::Unit {})
        }

        DaemonRequest::SetStartAtLogin { enabled } => {
            let daemon_path = std::env::current_exe()
                .map_err(|e| format!("cannot determine daemon path: {e}"))?;
            crate::autostart::set_start_at_login(enabled, &daemon_path)
                .map_err(|e| format!("autostart error: {e}"))?;
            Ok(DaemonResponse::Unit {})
        }

        // ── Sync control ─────────────────────────────────────────────────────
        DaemonRequest::GetStatus => {
            use adagio_core::cycle::EngineStatus;
            // Collect pair IDs that have E2EE enabled.
            let e2ee_pairs: Vec<String> = {
                let pairs = state.pairs.read().await;
                pairs
                    .all_pairs()
                    .iter()
                    .filter(|p| p.e2ee_enabled)
                    .map(|p| p.id.0.clone())
                    .collect()
            };
            let mut status_dto = match state.engine.status() {
                EngineStatus::Idle => {
                    serde_json::json!({"status":"idle","active_file_count":0,"total_bytes":0,"transferred_bytes":0,"eta_seconds":null,"last_sync_at":null})
                }
                EngineStatus::Syncing { pair_id } => {
                    serde_json::json!({"status":"syncing","pair_id": pair_id.to_string(),"active_file_count":0,"total_bytes":0,"transferred_bytes":0,"eta_seconds":null,"last_sync_at":null})
                }
                EngineStatus::Paused => {
                    serde_json::json!({"status":"paused","active_file_count":0,"total_bytes":0,"transferred_bytes":0,"eta_seconds":null,"last_sync_at":null})
                }
                EngineStatus::Error(msg) => {
                    serde_json::json!({"status":"error","error":msg,"active_file_count":0,"total_bytes":0,"transferred_bytes":0,"eta_seconds":null,"last_sync_at":null})
                }
                EngineStatus::ServerMaintenance => {
                    serde_json::json!({"status":"maintenance","active_file_count":0,"total_bytes":0,"transferred_bytes":0,"eta_seconds":null,"last_sync_at":null})
                }
                EngineStatus::ServerUnreachable => {
                    serde_json::json!({"status":"unreachable","active_file_count":0,"total_bytes":0,"transferred_bytes":0,"eta_seconds":null,"last_sync_at":null})
                }
            };
            status_dto["e2ee_pairs"] = serde_json::json!(e2ee_pairs);
            Ok(DaemonResponse::Status(status_dto))
        }

        DaemonRequest::TriggerSync { pair_id } => {
            let pid = PairId(pair_id);
            // E2EE pairs run a separate runner; send to its trigger channel.
            let triggered_e2ee = {
                let triggers = state.e2ee_triggers.lock().await;
                if let Some((_, tx)) = triggers.get(&pid) {
                    let _ = tx.try_send(());
                    true
                } else {
                    false
                }
            };
            if !triggered_e2ee {
                state
                    .engine
                    .trigger_pair(&pid)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Ok(DaemonResponse::Unit {})
        }

        DaemonRequest::PauseSyncAll => {
            state.engine.pause().await.map_err(|e| e.to_string())?;
            Ok(DaemonResponse::Unit {})
        }

        DaemonRequest::ResumeSyncAll => {
            state.engine.resume().await.map_err(|e| e.to_string())?;
            Ok(DaemonResponse::Unit {})
        }

        DaemonRequest::GetActivityLog { limit, filter } => {
            // The journal is the authoritative record of sync activity.
            // Query it directly — the in-memory ActivityLog ring-buffer is only
            // populated by the legacy engine path and is always empty for VFS / E2EE pairs.
            let n = limit.unwrap_or(50) as i64;
            let sql = match filter.as_deref() {
                Some("conflict") => {
                    "SELECT pair_id, path, status, updated_at FROM journal_entries \
                     WHERE status = 'conflict' ORDER BY updated_at DESC LIMIT ?"
                }
                Some("sync") | Some("edit") => {
                    "SELECT pair_id, path, status, updated_at FROM journal_entries \
                     WHERE status = 'synced' ORDER BY updated_at DESC LIMIT ?"
                }
                _ => {
                    "SELECT pair_id, path, status, updated_at FROM journal_entries \
                     WHERE status IN ('synced', 'conflict') ORDER BY updated_at DESC LIMIT ?"
                }
            };
            let rows: Vec<(String, String, String, String)> = sqlx::query_as(sql)
                .bind(n)
                .fetch_all(state.journal.pool())
                .await
                .map_err(|e| e.to_string())?;
            let dtos: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|(_pair_id, path, status, updated_at_str)| {
                    let at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
                        .map(|dt| dt.timestamp_millis())
                        .unwrap_or(0);
                    let filename = path.rsplit('/').next().unwrap_or(&path).to_string();
                    let (kind, verb) = if status == "conflict" {
                        ("conflict", "conflict")
                    } else {
                        ("sync", "synced")
                    };
                    serde_json::json!({
                        "id": uuid::Uuid::new_v4().to_string(),
                        "who": "You",
                        "verb": verb,
                        "target": filename,
                        "with_whom": null,
                        "where_path": path,
                        "at": at,
                        "kind": kind,
                    })
                })
                .collect();
            Ok(DaemonResponse::ActivityLog(dtos))
        }

        DaemonRequest::SearchFiles { query, limit } => {
            let q = query.trim().to_lowercase();
            if q.is_empty() {
                return Ok(DaemonResponse::Status(serde_json::json!([])));
            }
            let n = limit.unwrap_or(20) as i64;
            // Search all known paths (any status except error/excluded) containing
            // the query, case-insensitive via LOWER. Most recent activity first.
            let rows: Vec<(String, String)> = sqlx::query_as(
                "SELECT pair_id, path FROM journal_entries \
                 WHERE status NOT IN ('error', 'excluded') AND LOWER(path) LIKE ? \
                 ORDER BY updated_at DESC LIMIT ?",
            )
            .bind(format!("%{q}%"))
            .bind(n)
            .fetch_all(state.journal.pool())
            .await
            .map_err(|e| e.to_string())?;

            let results: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|(pair_id, path)| {
                    let filename = path.rsplit('/').next().unwrap_or(&path).to_string();
                    serde_json::json!({
                        "pair_id": pair_id,
                        "path": path,
                        "filename": filename,
                    })
                })
                .collect();
            Ok(DaemonResponse::Status(serde_json::json!(results)))
        }

        DaemonRequest::GetSectionCounts { pair_id } => {
            // Count synced (non-error, non-excluded) files and those updated in
            // the last 7 days, optionally filtered to a single pair.
            let (total, recent): (i64, i64) = if let Some(ref pid) = pair_id {
                sqlx::query_as(
                    "SELECT \
                       COUNT(*), \
                       SUM(CASE WHEN updated_at > datetime('now', '-7 days') THEN 1 ELSE 0 END) \
                     FROM journal_entries \
                     WHERE pair_id = ? AND status NOT IN ('error', 'excluded')",
                )
                .bind(pid)
                .fetch_one(state.journal.pool())
                .await
                .unwrap_or((0, 0))
            } else {
                sqlx::query_as(
                    "SELECT \
                       COUNT(*), \
                       SUM(CASE WHEN updated_at > datetime('now', '-7 days') THEN 1 ELSE 0 END) \
                     FROM journal_entries \
                     WHERE status NOT IN ('error', 'excluded')",
                )
                .fetch_one(state.journal.pool())
                .await
                .unwrap_or((0, 0))
            };
            Ok(DaemonResponse::Status(serde_json::json!({
                "total": total,
                "recent": recent,
            })))
        }

        DaemonRequest::GetErrorItems { pair_id } => {
            use adagio_core::types::PairId;
            let entries = state
                .journal
                .entries_by_status(&PairId(pair_id), &adagio_core::types::SyncStatus::Error)
                .await
                .map_err(|e| e.to_string())?;
            let dtos: Vec<serde_json::Value> = entries
                .into_iter()
                .map(|e| {
                    serde_json::json!({
                        "path": e.path.to_string(),
                        "message": e.error_message.unwrap_or_default(),
                        "retry_count": e.retry_count
                    })
                })
                .collect();
            Ok(DaemonResponse::ErrorItems(dtos))
        }

        // ── Pairs ─────────────────────────────────────────────────────────────
        DaemonRequest::ListPairs => {
            let pairs = state.pairs.read().await;
            let dtos: Vec<serde_json::Value> = pairs
                .all_pairs()
                .into_iter()
                .map(|p| {
                    serde_json::json!({
                        "id": p.id.to_string(),
                        "account_id": p.account_id.to_string(),
                        "local_root": p.local_root.0.to_string_lossy(),
                        "remote_root": p.remote_root.as_str(),
                        "scan_interval_secs": p.scan_interval_secs,
                        "selective_paths": p.selective_paths.iter().map(|r| r.as_str()).collect::<Vec<_>>(),
                        "vfs_enabled": p.vfs_enabled,
                        "e2ee_enabled": p.e2ee_enabled,
                    })
                })
                .collect();
            Ok(DaemonResponse::Pairs(dtos))
        }

        DaemonRequest::CreatePair {
            account_id,
            local_root,
            remote_root,
            vfs_enabled,
            vfs_cache_max_bytes,
            vfs_eviction_threshold_bytes,
        } => {
            use adagio_core::types::{
                AccountId, LocalPath, PairId, PairStatus, RemotePath, SyncPair,
            };
            use chrono::Utc;
            let pair = SyncPair {
                id: PairId::new(),
                account_id: AccountId(account_id),
                local_root: LocalPath::new(std::path::PathBuf::from(&local_root)),
                remote_root: RemotePath::new(&remote_root),
                status: PairStatus::Idle,
                exclude_patterns: vec![],
                selective_paths: vec![],
                created_at: Utc::now(),
                last_synced_at: None,
                scan_interval_secs: 7200,
                scan_on_startup: true,
                max_upload_concurrency: 3,
                max_download_concurrency: 3,
                conflict_policy: adagio_core::types::ConflictPolicy::Ask,
                bulk_upload_workers: 8,
                bulk_upload_threshold_files: 50,
                bulk_upload_chunk_threshold_bytes: 10 * 1024 * 1024,
                vfs_enabled,
                vfs_cache_max_bytes,
                vfs_eviction_threshold_bytes,
                e2ee_enabled: false,
                e2ee_account_id: None,
            };
            let dto = serde_json::json!({
                "id": pair.id.to_string(),
                "account_id": pair.account_id.to_string(),
                "local_root": local_root,
                "remote_root": remote_root,
                "scan_interval_secs": pair.scan_interval_secs,
                "selective_paths": [],
                "vfs_enabled": pair.vfs_enabled,
                "e2ee_enabled": pair.e2ee_enabled,
            });
            {
                let mut pairs = state.pairs.write().await;
                pairs.register_full_pair(pair.clone());
            } // release write lock before save_config acquires read lock

            // Register in journal so journal_entries FK constraint is satisfied.
            let accounts = state.accounts.list().map_err(|e| e.to_string())?;
            if let Some(account) = accounts.iter().find(|a| a.id == pair.account_id) {
                let _ = state.journal.register_pair(account, &pair).await;
            }

            // Start the sync runner immediately — don't wait for daemon restart.
            if let Some(client) = build_client(&state.accounts, &pair).await {
                let client = std::sync::Arc::new(client);
                if pair.vfs_enabled {
                    let provider = adagio_vfs::create_platform_provider();
                    state.engine.start_vfs_pair(
                        pair.clone(),
                        client,
                        state.journal.clone(),
                        provider,
                    );
                } else {
                    state
                        .engine
                        .start_pair(pair.clone(), client, state.journal.clone(), None)
                        .await;
                }
            }

            save_config(state).await?;
            Ok(DaemonResponse::Pair(dto))
        }

        DaemonRequest::DeletePair {
            pair_id,
            delete_local_files,
        } => {
            use adagio_core::types::PairId;
            let pid = PairId(pair_id);
            // Stop the VFS / regular runner via the engine.
            state.engine.stop_pair(&pid).await;
            // Cancel the E2EE runner if one is registered for this pair.
            {
                let mut triggers = state.e2ee_triggers.lock().await;
                if let Some((cancel, _)) = triggers.remove(&pid) {
                    cancel.cancel();
                }
            }
            {
                let mut pairs = state.pairs.write().await;
                pairs.remove_pair(&pid);
            }
            let _ = state.journal.clear_pair(&pid).await;
            save_config(state).await?;
            let _ = delete_local_files; // handled by caller if needed
            Ok(DaemonResponse::Unit {})
        }

        DaemonRequest::ListSyncedFiles {
            pair_id,
            relative_path,
        } => {
            use adagio_core::types::{PairId, RelativePath};
            use adagio_core::vfs::types::VfsState;
            let pid = PairId(pair_id.clone());

            // Find the pair to get its local_root and mode flags.
            let (local_root, vfs_enabled, e2ee_enabled) = {
                let pairs = state.pairs.read().await;
                let p = pairs
                    .get_pair(&pid)
                    .ok_or_else(|| format!("pair {pair_id} not found"))?;
                (p.local_root.0.clone(), p.vfs_enabled, p.e2ee_enabled)
            };

            // Strip leading '/' so frontend's "/" becomes "" (root level).
            let sub = relative_path
                .as_deref()
                .unwrap_or("")
                .trim_start_matches('/')
                .to_string();

            if e2ee_enabled {
                // E2EE pair: files are stored with real names on the local filesystem
                // (decrypted on download). Fall back to the same local-filesystem listing
                // as a regular pair — the E2EE layer is transparent to the file browser.
                // If the local root doesn't exist yet (never synced), return an empty list
                // with a single placeholder entry to prompt the user to sync.
                let root_exists = tokio::fs::metadata(&local_root)
                    .await
                    .map(|m| m.is_dir())
                    .unwrap_or(false);
                let is_empty = if root_exists {
                    tokio::fs::read_dir(&local_root)
                        .await
                        .ok()
                        .map(|_| false)
                        .unwrap_or(true)
                } else {
                    true
                };
                if !root_exists || is_empty {
                    // No local files yet — direct the user to sync first.
                    return Ok(DaemonResponse::SyncedFiles(vec![serde_json::json!({
                        "path": "/.e2ee_placeholder",
                        "name": "Encrypted folder — sync to view files",
                        "is_dir": false,
                        "size": 0,
                        "mtime": null,
                        "status": "sync",
                        "e2ee": true,
                    })]));
                }
                // Local files exist — fall through to the standard local-filesystem listing
                // below, which will show the decrypted real filenames.
            }

            if vfs_enabled {
                // VFS pair: serve entries from vfs_cache_metadata (journal),
                // not from the local filesystem. FUSE makes the directory
                // read-accessible, but we want remote metadata (size, mtime)
                // and VFS state for the status column.
                let all_entries = state
                    .journal
                    .all_vfs_entries(&pid)
                    .await
                    .map_err(|e| e.to_string())?;

                // Collect unique direct children of `sub`.
                let mut seen: std::collections::HashMap<String, serde_json::Value> =
                    std::collections::HashMap::new();
                for entry in &all_entries {
                    let p = entry.path.as_str();
                    let child_name = if sub.is_empty() {
                        let first = p.split('/').next().unwrap_or(p);
                        first.to_string()
                    } else {
                        let prefix = format!("{sub}/");
                        if !p.starts_with(&prefix) {
                            continue;
                        }
                        let rest = &p[prefix.len()..];
                        rest.split('/').next().unwrap_or(rest).to_string()
                    };
                    if child_name.is_empty() {
                        continue;
                    }

                    let rel_str = if sub.is_empty() {
                        child_name.clone()
                    } else {
                        format!("{sub}/{child_name}")
                    };
                    let is_dir = entry.path.as_str().len() > rel_str.len()
                        && entry.path.as_str()[rel_str.len()..].starts_with('/');

                    if seen.contains_key(&child_name) {
                        // Mark as dir if any entry under this name is deeper.
                        if is_dir {
                            if let Some(v) = seen.get_mut(&child_name) {
                                v["is_dir"] = serde_json::json!(true);
                            }
                        }
                        continue;
                    }

                    let status = match &entry.state {
                        VfsState::CloudOnly => "cloud",
                        VfsState::Pinned { .. } => "pin",
                        VfsState::LocallyAvailable { .. } => "ok",
                    };
                    let size = if is_dir { 0 } else { entry.remote_size };
                    let mtime = entry.remote_mtime.timestamp_millis();

                    seen.insert(
                        child_name.clone(),
                        serde_json::json!({
                            "path": format!("/{rel_str}"),
                            "name": child_name,
                            "is_dir": is_dir,
                            "size": size,
                            "mtime": mtime,
                            "status": status,
                            "etag": serde_json::Value::Null,
                            "item_count": serde_json::Value::Null,
                        }),
                    );
                }
                let mut dtos: Vec<serde_json::Value> = seen.into_values().collect();
                dtos.sort_by(|a, b| {
                    let a_dir = a["is_dir"].as_bool().unwrap_or(false);
                    let b_dir = b["is_dir"].as_bool().unwrap_or(false);
                    match (b_dir, a_dir) {
                        (true, false) => std::cmp::Ordering::Greater,
                        (false, true) => std::cmp::Ordering::Less,
                        _ => a["name"]
                            .as_str()
                            .unwrap_or("")
                            .to_lowercase()
                            .cmp(&b["name"].as_str().unwrap_or("").to_lowercase()),
                    }
                });
                return Ok(DaemonResponse::SyncedFiles(dtos));
            }

            // Copy-sync pair: read local filesystem.
            let target_dir = if sub.is_empty() {
                local_root.clone()
            } else {
                local_root.join(&sub)
            };

            // Read local filesystem entries — same logic as the original
            // desktop `list_files_internal`, preserving directory visibility.
            let read_dir = tokio::task::spawn_blocking(move || {
                std::fs::read_dir(&target_dir)
                    .map_err(|e| format!("cannot read {:?}: {e}", target_dir))
                    .map(|d| d.collect::<Vec<_>>())
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

            let mut dtos: Vec<serde_json::Value> = Vec::new();
            for entry_result in read_dir {
                let entry = entry_result.map_err(|e| e.to_string())?;
                let file_name = entry.file_name().to_string_lossy().to_string();
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let is_dir = meta.is_dir();
                let mtime: Option<i64> = meta.modified().ok().and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_millis() as i64)
                });
                let rel_str = if sub.is_empty() {
                    file_name.clone()
                } else {
                    format!("{}/{}", sub.trim_end_matches('/'), file_name)
                };
                let rel_path = RelativePath::new(&rel_str);
                let journal_entry = state.journal.get(&pid, &rel_path).await.ok().flatten();
                let status = match journal_entry.as_ref().map(|e| &e.status) {
                    Some(adagio_core::types::SyncStatus::Synced) => "ok",
                    Some(adagio_core::types::SyncStatus::Conflict) => "conflict",
                    Some(adagio_core::types::SyncStatus::Error) => "conflict",
                    Some(_) => "sync",
                    None => "ok",
                };
                let etag = journal_entry.as_ref().and_then(|e| e.etag.clone());
                let size: u64 = if is_dir { 0 } else { meta.len() };
                let item_count: Option<u32> = if is_dir {
                    std::fs::read_dir(entry.path())
                        .ok()
                        .map(|d| d.count() as u32)
                } else {
                    None
                };
                dtos.push(serde_json::json!({
                    "path": format!("/{rel_str}"),
                    "name": file_name,
                    "is_dir": is_dir,
                    "size": size,
                    "mtime": mtime,
                    "status": status,
                    "etag": etag,
                    "item_count": item_count,
                    "e2ee": e2ee_enabled,
                }));
            }

            // Sort: directories first, then files, each group alphabetically.
            dtos.sort_by(|a, b| {
                let a_dir = a["is_dir"].as_bool().unwrap_or(false);
                let b_dir = b["is_dir"].as_bool().unwrap_or(false);
                match (b_dir, a_dir) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    _ => a["name"]
                        .as_str()
                        .unwrap_or("")
                        .to_lowercase()
                        .cmp(&b["name"].as_str().unwrap_or("").to_lowercase()),
                }
            });

            Ok(DaemonResponse::SyncedFiles(dtos))
        }

        DaemonRequest::GetExcludePatterns => {
            // Global exclude patterns are stored in config.
            Ok(DaemonResponse::Strings(vec![]))
        }

        DaemonRequest::ListRemoteTree { pair_id: _ } => Ok(DaemonResponse::RemoteTree(vec![])),

        // ── Accounts ──────────────────────────────────────────────────────────
        DaemonRequest::ListAccounts => {
            let accounts = state.accounts.list().map_err(|e| e.to_string())?;
            let dtos: Vec<serde_json::Value> = accounts
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "id": a.id.to_string(),
                        "display_name": a.display_name,
                        "server_url": a.server_url,
                        "username": a.username,
                    })
                })
                .collect();
            Ok(DaemonResponse::Accounts(dtos))
        }

        DaemonRequest::AddAccount {
            server_url,
            username,
            display_name,
            secret,
        } => {
            use adagio_core::types::{Account, AccountId};
            use chrono::Utc;
            let id = AccountId::new();
            let key = format!("adagio/{}", id.0);
            // Store credential in keychain.
            tokio::task::spawn_blocking({
                let key = key.clone();
                let secret = secret.clone();
                move || adagio_nextcloud::auth::store_credentials(&key, &secret)
            })
            .await
            .map_err(|e| format!("spawn_blocking error: {e}"))?
            .map_err(|e| format!("keychain error: {e}"))?;

            let account = Account {
                id: id.clone(),
                display_name: display_name.clone(),
                server_url: server_url.clone(),
                username: username.clone(),
                keychain_service_key: key,
                created_at: Utc::now(),
                upload_limit_kbps: 0,
                download_limit_kbps: 0,
            };
            state.accounts.add(account).map_err(|e| e.to_string())?;
            save_config(state).await?;
            let dto = serde_json::json!({
                "id": id.to_string(),
                "display_name": display_name,
                "server_url": server_url,
                "username": username,
            });
            Ok(DaemonResponse::Account(dto))
        }

        DaemonRequest::RemoveAccount { account_id } => {
            use adagio_core::types::AccountId;
            let aid = AccountId(account_id);
            // Stop all pairs for this account.
            let pair_ids: Vec<_> = {
                let pairs = state.pairs.read().await;
                pairs
                    .all_pairs()
                    .into_iter()
                    .filter(|p| p.account_id == aid)
                    .map(|p| p.id.clone())
                    .collect()
            };
            for pid in &pair_ids {
                state.engine.stop_pair(pid).await;
                let _ = state.journal.clear_pair(pid).await;
                // Cancel E2EE runner for this pair if one exists.
                let mut triggers = state.e2ee_triggers.lock().await;
                if let Some((cancel, _)) = triggers.remove(pid) {
                    cancel.cancel();
                }
            }
            {
                let mut pairs = state.pairs.write().await;
                for pid in &pair_ids {
                    pairs.remove_pair(pid);
                }
            }
            state.accounts.remove(&aid).map_err(|e| e.to_string())?;
            save_config(state).await?;
            Ok(DaemonResponse::Unit {})
        }

        DaemonRequest::ConnectAccountOAuth2 { server_url } => {
            // OAuth2 flow requires the desktop app's browser; daemon just returns an error.
            // The desktop app's existing command handles the actual flow locally.
            Err(format!(
                "OAuth2 flow must be initiated from the desktop app for server: {server_url}"
            ))
        }

        // ── Conflicts ─────────────────────────────────────────────────────────
        DaemonRequest::ListConflicts { pair_id } => {
            use adagio_core::types::PairId;
            let records = state
                .journal
                .list_conflicts(&PairId(pair_id))
                .await
                .map_err(|e| e.to_string())?;
            let dtos: Vec<serde_json::Value> = records
                .into_iter()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "pair_id": r.pair_id.to_string(),
                        "path": r.path.as_str(),
                        "local_mtime": r.local_mtime.to_rfc3339(),
                        "remote_mtime": r.remote_mtime.to_rfc3339(),
                        "local_size": r.local_size,
                        "remote_size": r.remote_size,
                        "policy": format!("{:?}", r.policy).to_lowercase(),
                        "resolution": r.resolution.as_ref().map(|res| format!("{:?}", res).to_lowercase()),
                        "detected_at": r.detected_at.to_rfc3339(),
                        "resolved_at": r.resolved_at.map(|t| t.to_rfc3339()),
                        "is_dir": r.is_dir,
                        "conflict_kind": serde_json::to_string(&r.conflict_kind).unwrap_or_default().trim_matches('"').to_string(),
                    })
                })
                .collect();
            Ok(DaemonResponse::Conflicts(dtos))
        }

        DaemonRequest::ResolveConflict { id, side } => {
            // Minimal implementation: just mark as resolved in journal.
            // Full I/O resolution deferred to a future task.
            use adagio_core::types::ConflictResolution;
            let resolution = match side.as_str() {
                "local" => ConflictResolution::KeptLocal,
                "remote" => ConflictResolution::KeptRemote,
                "both" => ConflictResolution::BothKept {
                    conflict_copy_path: adagio_core::types::RelativePath::new(""),
                },
                other => return Err(format!("invalid side: {other}")),
            };
            state
                .journal
                .resolve_conflict(&id, resolution)
                .await
                .map_err(|e| e.to_string())?;
            // Emit event to all subscribers.
            let pending = count_pending_conflicts(state).await;
            state.events.emit_conflict_resolved(id, pending);
            Ok(DaemonResponse::Unit {})
        }

        DaemonRequest::DismissAllConflicts => {
            use adagio_core::types::ConflictResolution;
            let pair_ids: Vec<_> = {
                let pairs = state.pairs.read().await;
                pairs
                    .all_pairs()
                    .into_iter()
                    .map(|p| p.id.clone())
                    .collect()
            };
            let mut dismissed = 0usize;
            for pid in &pair_ids {
                if let Ok(records) = state.journal.list_conflicts(pid).await {
                    for r in records.iter().filter(|r| r.resolution.is_none()) {
                        let _ = state
                            .journal
                            .resolve_conflict(&r.id, ConflictResolution::KeptRemote)
                            .await;
                        dismissed += 1;
                    }
                }
            }
            state
                .events
                .emit_conflict_resolved("bulk-dismiss".to_string(), 0);
            Ok(DaemonResponse::DismissedCount {
                dismissed_count: dismissed,
            })
        }

        // ── Sharing ───────────────────────────────────────────────────────────
        DaemonRequest::SearchUsers { account_id, query } => {
            if query.trim().is_empty() {
                return Ok(DaemonResponse::Users(vec![]));
            }
            let account = state
                .accounts
                .list()
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|a| a.id.0 == account_id)
                .ok_or_else(|| format!("account not found: {account_id}"))?;
            let _ = account; // Sharing search deferred to future task
            Ok(DaemonResponse::Users(vec![]))
        }

        DaemonRequest::CreateShare { .. } => {
            Err("share creation requires active Nextcloud client — deferred".to_string())
        }

        DaemonRequest::Subscribe => {
            // Handled at the server level, not the dispatcher.
            Ok(DaemonResponse::Unit {})
        }

        // ── Bandwidth throttling ──────────────────────────────────────────────
        DaemonRequest::GetBandwidthStatus { account_id } => {
            let accounts = state.accounts.list().map_err(|e| e.to_string())?;
            let acct = resolve_account(&accounts, account_id.as_deref())?;
            let limits = state.engine.bandwidth_limits.read().await;
            let (up_kbps, dn_kbps) = limits
                .get(&acct.id)
                .copied()
                .unwrap_or((acct.upload_limit_kbps, acct.download_limit_kbps));
            Ok(DaemonResponse::Status(serde_json::json!({
                "upload_limit_kbps": up_kbps,
                "download_limit_kbps": dn_kbps,
                "upload_bytes_per_sec": 0u64,   // TODO: expose from propagator meter
                "download_bytes_per_sec": 0u64, // TODO: expose from propagator meter
            })))
        }

        DaemonRequest::SetBandwidthLimits {
            account_id,
            upload_kbps,
            download_kbps,
        } => {
            let accounts = state.accounts.list().map_err(|e| e.to_string())?;
            let acct = resolve_account(&accounts, account_id.as_deref())?;
            // Update the engine's in-memory limits (takes effect on next chunk).
            state
                .engine
                .update_bandwidth_limits(&acct.id, upload_kbps, download_kbps)
                .await;
            // Persist by re-saving config — save_config reads from AccountManager
            // which holds the saved limits; update it via remove+re-add.
            state.accounts.remove(&acct.id).ok();
            let mut updated_acct = acct.clone();
            updated_acct.upload_limit_kbps = upload_kbps;
            updated_acct.download_limit_kbps = download_kbps;
            state
                .accounts
                .add(updated_acct)
                .map_err(|e| e.to_string())?;
            save_config(state).await?;
            Ok(DaemonResponse::Unit {})
        }

        DaemonRequest::ClearBandwidthLimits { account_id } => {
            let accounts = state.accounts.list().map_err(|e| e.to_string())?;
            let acct = resolve_account(&accounts, account_id.as_deref())?;
            state.engine.update_bandwidth_limits(&acct.id, 0, 0).await;
            save_config(state).await?;
            Ok(DaemonResponse::Unit {})
        }

        // ── Network awareness ─────────────────────────────────────────────────
        DaemonRequest::GetNetworkStatus => {
            let ns = state.network_monitor.state.lock().await;
            let policy = state.network_policy.read().await;
            let ea = adagio_core::network::derive_effective_action(&policy, &ns);
            let json = serde_json::json!({
                "metered": ns.metered,
                "on_battery": ns.on_battery,
                "ssid": ns.ssid,
                "effective_action": format!("{:?}", ea.action).to_lowercase(),
                "throttle_kbps": ea.throttle_kbps,
                "reason": ea.reason,
                "policy": {
                    "on_metered": format!("{:?}", policy.on_metered).to_lowercase(),
                    "on_battery": format!("{:?}", policy.on_battery).to_lowercase(),
                    "throttle_kbps": policy.throttle_kbps,
                    "blocked_ssids": policy.blocked_ssids,
                }
            });
            Ok(DaemonResponse::Status(json))
        }

        DaemonRequest::SetNetworkPolicy {
            on_metered,
            on_battery,
            throttle_kbps,
        } => {
            use adagio_core::network::NetworkAction;
            let parse_action = |s: &str| match s {
                "allow" => Ok(NetworkAction::Allow),
                "throttle" => Ok(NetworkAction::Throttle),
                "pause" => Ok(NetworkAction::Pause),
                other => Err(format!("invalid action: {other}")),
            };
            let mut policy = state.network_policy.write().await;
            if let Some(ref s) = on_metered {
                policy.on_metered = parse_action(s)?;
            }
            if let Some(ref s) = on_battery {
                policy.on_battery = parse_action(s)?;
            }
            if let Some(kbps) = throttle_kbps {
                policy.throttle_kbps = kbps;
            }
            drop(policy);
            save_config(state).await?;
            Ok(DaemonResponse::Unit {})
        }

        DaemonRequest::ManageBlockedSsid { action, ssid } => {
            let mut policy = state.network_policy.write().await;
            match action.as_str() {
                "add" => {
                    let s = ssid.ok_or_else(|| "ssid required for add".to_string())?;
                    let trimmed = s.trim().to_string();
                    if !policy.blocked_ssids.contains(&trimmed) {
                        policy.blocked_ssids.push(trimmed);
                    }
                    drop(policy);
                    save_config(state).await?;
                    Ok(DaemonResponse::Unit {})
                }
                "remove" => {
                    let s = ssid.ok_or_else(|| "ssid required for remove".to_string())?;
                    let trimmed = s.trim().to_string();
                    policy.blocked_ssids.retain(|b| b != &trimmed);
                    drop(policy);
                    save_config(state).await?;
                    Ok(DaemonResponse::Unit {})
                }
                "list" => {
                    let list = policy.blocked_ssids.clone();
                    drop(policy);
                    Ok(DaemonResponse::Strings(list))
                }
                other => Err(format!("unknown action: {other}")),
            }
        }

        // ── VFS (on-demand files) ─────────────────────────────────────────────
        DaemonRequest::GetVfsStats { pair_id } => {
            use adagio_core::types::PairId;
            use adagio_core::vfs::get_vfs_stats;
            let pid = PairId(pair_id);
            let pairs = state.pairs.read().await;
            let pair = pairs
                .get_pair(&pid)
                .cloned()
                .ok_or_else(|| format!("pair {pid} not found"))?;
            drop(pairs);
            let stats = get_vfs_stats(&state.journal, &pair).await;
            Ok(DaemonResponse::Status(
                serde_json::to_value(&stats).unwrap_or_default(),
            ))
        }

        DaemonRequest::SetVfsPin {
            pair_id,
            path,
            pinned,
        } => {
            use adagio_core::types::PairId;
            let pid = PairId(pair_id);
            // Paths from the UI always carry a leading '/'; strip it before
            // looking up vfs_cache_metadata rows which store bare relative paths.
            let path = path.trim_start_matches('/').to_string();

            if pinned {
                // Full pin: writes DB row + spawns background download.
                let pair = {
                    let pairs = state.pairs.read().await;
                    pairs
                        .get_pair(&pid)
                        .cloned()
                        .ok_or_else(|| format!("pair {pid} not found"))?
                };
                let client = build_client(&state.accounts, &pair)
                    .await
                    .ok_or_else(|| "credentials unavailable for this pair".to_string())?;
                let provider = adagio_vfs::create_platform_provider();
                adagio_core::vfs::pin_path(
                    &pid,
                    &path,
                    state.journal.clone(),
                    std::sync::Arc::new(client),
                    &pair,
                    provider,
                )
                .await
                .map_err(|e| e.to_string())?;
            } else {
                // Unpin: remove the pin row; file stays locally available.
                state
                    .journal
                    .unpin_path(&pid, &path)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Ok(DaemonResponse::Unit {})
        }

        DaemonRequest::EvictVfsFile { pair_id, path } => {
            use adagio_core::types::PairId;
            let pid = PairId(pair_id);
            let path = path.trim_start_matches('/').to_string();
            // Full evict: removes local file content + updates journal state.
            let provider = adagio_vfs::create_platform_provider();
            adagio_core::vfs::evict_file(&pid, &path, &state.journal, provider.as_ref())
                .await
                .map_err(|e| e.to_string())?;
            Ok(DaemonResponse::Unit {})
        }

        // ── E2EE ─────────────────────────────────────────────────────────────
        DaemonRequest::E2eeInit { pair_id } => {
            use adagio_core::types::PairId;
            use adagio_e2ee::{
                keys::{
                    build_csr, generate_metadata_key, generate_mnemonic, generate_rsa_keypair,
                    key_fingerprint, private_key_to_pem, public_key_der, wrap_private_key,
                },
                metadata::{build_outer, canonical_inner_json, cms_sign, metadata_key_checksum},
                E2eeMetadata,
            };
            use rsa::RsaPublicKey;

            let pid = PairId(pair_id.clone());

            // Look up pair → account credentials and remote_root.
            let (pair_account, nc_password, remote_root) = {
                let pairs = state.pairs.read().await;
                let pair = pairs
                    .get_pair(&pid)
                    .ok_or_else(|| format!("pair {pair_id} not found"))?;
                let aid = pair.account_id.clone();
                let remote = pair.remote_root.clone();
                let accs = state.accounts.list().map_err(|e| e.to_string())?;
                let acc = accs
                    .into_iter()
                    .find(|a| a.id == aid)
                    .ok_or_else(|| format!("account {} not found", aid.0))?;
                let key = acc.keychain_service_key.clone();
                let pw = tokio::task::spawn_blocking(move || {
                    adagio_nextcloud::auth::retrieve_credentials(&key)
                })
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "credentials not found in keychain".to_string())?;
                (acc, pw, remote)
            };

            let ocs = adagio_nextcloud::E2eeOcsClient::new(
                &pair_account.server_url,
                &pair_account.username,
                &nc_password,
            );

            // Generate mnemonic and RSA-4096 key pair (blocking — RSA keygen is slow).
            let mnemonic = generate_mnemonic().map_err(|e| e.to_string())?;
            let mnemonic_for_wrap = mnemonic.clone();
            let privkey = tokio::task::spawn_blocking(generate_rsa_keypair)
                .await
                .map_err(|e| e.to_string())?
                .map_err(|e| e.to_string())?;

            // Submit public key as CSR; server returns a signed certificate.
            let pub_der = public_key_der(&privkey);
            // Build a proper PKCS#10 CSR — Nextcloud requires a valid signed CSR,
            // not just the raw public key bytes.
            let csr_pem = build_csr(&privkey, &pair_account.username)
                .map_err(|e| format!("E2EE step 1 failed (build_csr): {e}"))?;
            // Step 1: submit public key as CSR → server returns signed certificate.
            // If the server returns 409 (key already registered from a previous partial
            // init), delete the stale server key pair and re-register with the fresh key.
            tracing::info!(pair_id = %pair_id, "E2EE step 1/6: submitting PKCS#10 CSR to server CA");
            let cert_pem = match ocs.post_public_key(&csr_pem).await {
                Ok(resp) => {
                    tracing::debug!(pair_id = %pair_id, resp = %resp, "post_public_key response");
                    resp["ocs"]["data"]["public-key"]
                        .as_str()
                        .ok_or_else(|| {
                            "E2EE step 1 failed: server did not return a signed certificate. \
                                 Ensure the End-to-End Encryption app ≥ 2.0 is enabled."
                                .to_string()
                        })?
                        .to_string()
                }
                Err(e) if e.to_string().contains("conflict") => {
                    // 409 Conflict: keys already registered from a previous partial init.
                    // Delete both and re-register so the user gets a clean state + new mnemonic.
                    tracing::warn!(
                        pair_id = %pair_id,
                        "E2EE step 1: server already has keys (previous partial init detected) — \
                         deleting and re-registering with fresh key pair"
                    );
                    let _ = ocs.delete_private_key().await;
                    ocs.delete_public_key().await.map_err(|e| {
                        format!(
                            "E2EE step 1 failed (could not delete stale server keys): {e}. \
                                 Delete your E2EE keys manually in Nextcloud Settings → Security \
                                 → End-to-End Encryption."
                        )
                    })?;
                    // Re-register with the freshly generated key.
                    let resp = ocs.post_public_key(&csr_pem).await.map_err(|e| {
                        format!("E2EE step 1 failed (re-register after delete): {e}")
                    })?;
                    tracing::info!(pair_id = %pair_id, "E2EE step 1: fresh key registered after stale key cleanup");
                    resp["ocs"]["data"]["public-key"]
                        .as_str()
                        .ok_or_else(|| "E2EE step 1 failed: no cert after re-register".to_string())?
                        .to_string()
                }
                Err(e) => {
                    return Err(format!("E2EE step 1 failed (post_public_key): {e}"));
                }
            };
            let fingerprint = key_fingerprint(&pub_der);
            tracing::info!(
                pair_id = %pair_id,
                fingerprint = %fingerprint,
                "E2EE step 1/6 complete: certificate obtained"
            );

            // Step 2: wrap private key with mnemonic and upload to server.
            tracing::info!(pair_id = %pair_id, "E2EE step 2/6: uploading encrypted private key");
            let blob = wrap_private_key(&privkey, &mnemonic_for_wrap)
                .map_err(|e| format!("E2EE step 2 failed (wrap_private_key): {e}"))?;
            ocs.post_private_key(&blob)
                .await
                .map_err(|e| format!("E2EE step 2 failed (post_private_key): {e}"))?;
            tracing::info!(pair_id = %pair_id, "E2EE step 2/6 complete: private key stored on server");

            // Step 3: persist certificate and private key locally.
            tracing::info!(pair_id = %pair_id, "E2EE step 3/6: saving keys locally");
            let aid_str = pair_account.id.0.clone();
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT INTO e2ee_account_keys(account_id, certificate, key_fingerprint, paired_at, created_at) \
                 VALUES(?, ?, ?, ?, ?) \
                 ON CONFLICT(account_id) DO UPDATE SET \
                 certificate=excluded.certificate, \
                 key_fingerprint=excluded.key_fingerprint, \
                 paired_at=excluded.paired_at",
            )
            .bind(&aid_str)
            .bind(&cert_pem)
            .bind(&fingerprint)
            .bind(&now)
            .bind(&now)
            .execute(state.journal.pool())
            .await
            .map_err(|e| format!("E2EE step 3 failed (save certificate to DB): {e}"))?;

            let pem_str = private_key_to_pem(&privkey)
                .map_err(|e| format!("E2EE step 3 failed (encode private key): {e}"))?
                .to_string();
            let keychain_key = format!("adagio-e2ee/{aid_str}");
            tokio::task::spawn_blocking(move || {
                keyring::Entry::new("adagio", &keychain_key)
                    .map_err(|e| e.to_string())?
                    .set_password(&pem_str)
                    .map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| format!("E2EE step 3 failed (keychain write): {e}"))??;
            tracing::info!(pair_id = %pair_id, "E2EE step 3/6 complete: keys saved locally");

            // Step 4: ensure the remote folder exists and resolve its Nextcloud numeric file ID.
            tracing::info!(
                pair_id = %pair_id,
                folder = %remote_root.as_str(),
                "E2EE step 4/6: ensuring remote folder exists and resolving file ID"
            );
            let nc_client = adagio_nextcloud::client::NextcloudClient::new(
                &pair_account.server_url,
                &pair_account.username,
                &nc_password,
            );
            let folder_id = {
                use adagio_core::remote::RemoteClient;

                // Use Depth: 0 stat() to get the folder's own metadata (including file_id).
                // list() strips the folder entry itself; stat() always returns it.
                let item = match nc_client
                    .stat(
                        &remote_root,
                        Some("Mozilla/5.0 (Linux) mirall/3.12.0 (adagio; linux-openssl)"),
                    )
                    .await
                {
                    Ok(Some(item)) => item,
                    Ok(None) | Err(adagio_core::error::ClientError::Permanent(..)) => {
                        // Folder not found — create it via MKCOL then stat again.
                        tracing::info!(
                            pair_id = %pair_id,
                            folder = %remote_root.as_str(),
                            "E2EE step 4: folder not found — creating it via MKCOL"
                        );
                        nc_client.create_dir(&remote_root).await.map_err(|e| {
                            format!("E2EE step 4 failed (MKCOL '{}'): {e}", remote_root.as_str())
                        })?;
                        nc_client
                            .stat(
                                &remote_root,
                                Some("Mozilla/5.0 (Linux) mirall/3.12.0 (adagio; linux-openssl)"),
                            )
                            .await
                            .map_err(|e| format!("E2EE step 4 failed (stat after MKCOL): {e}"))?
                            .ok_or_else(|| {
                                format!(
                                    "E2EE step 4 failed: folder '{}' not visible after MKCOL",
                                    remote_root.as_str()
                                )
                            })?
                    }
                    Err(e) => {
                        return Err(format!(
                            "E2EE step 4 failed (stat '{}'): {e}",
                            remote_root.as_str()
                        ));
                    }
                };

                if item.file_id.is_empty() {
                    return Err(format!(
                        "E2EE step 4 failed: folder '{}' has no file_id in PROPFIND response. \
                         Check that the Nextcloud oc: namespace is supported.",
                        remote_root.as_str()
                    ));
                }
                item.file_id
            };
            tracing::info!(
                pair_id = %pair_id,
                folder_id = %folder_id,
                "E2EE step 4/6 complete: folder ID resolved"
            );

            // Step 5: create initial metadata inside a lock window.
            //
            // In the V2 API, POST /meta-data also requires the e2e-token from a
            // prior lock call — the folder must be locked before metadata can be
            // created or updated.
            tracing::info!(pair_id = %pair_id, "E2EE step 5/6: creating initial metadata on server");
            let pub_key = RsaPublicKey::from(&privkey);
            let metadata_key = generate_metadata_key();
            let init_meta = E2eeMetadata {
                version: "2.0".to_string(),
                counter: 1,
                key_checksums: vec![metadata_key_checksum(&metadata_key)],
                ..Default::default()
            };
            let outer = build_outer(
                &init_meta,
                &metadata_key,
                &pair_account.username,
                &cert_pem,
                &pub_key,
            )
            .map_err(|e| format!("E2EE step 5 failed (build_outer): {e}"))?;
            let outer_json = serde_json::to_string(&outer)
                .map_err(|e| format!("E2EE step 5 failed (serialise metadata): {e}"))?;

            let init_inner_json = canonical_inner_json(&init_meta)
                .map_err(|e| format!("E2EE step 5 failed (canonical_inner_json): {e}"))?;
            let init_signature = cms_sign(&init_inner_json, &privkey, &cert_pem)
                .map_err(|e| format!("E2EE step 5 failed (cms_sign): {e}"))?;
            let init_token = ocs
                .lock_folder(&folder_id, 1)
                .await
                .map_err(|e| format!("E2EE step 5 failed (lock_folder): {e}"))?;
            match ocs
                .post_metadata(&folder_id, &outer_json, &init_token, &init_signature)
                .await
            {
                Ok(_) => {}
                Err(e) if e.to_string().contains("conflict") => {
                    tracing::info!(pair_id = %pair_id, "E2EE step 5: metadata already exists");
                }
                Err(e) => {
                    let _ = ocs.unlock_folder(&folder_id, &init_token).await;
                    return Err(format!("E2EE step 5 failed (post_metadata): {e}"));
                }
            }
            ocs.unlock_folder(&folder_id, &init_token)
                .await
                .map_err(|e| format!("E2EE step 5 failed (unlock_folder): {e}"))?;

            tracing::info!(pair_id = %pair_id, "E2EE step 5/6 complete: metadata ready on server");

            // Step 6: mark folder as E2EE on the server.
            tracing::info!(pair_id = %pair_id, "E2EE step 6/6: marking folder as E2EE on server");
            ocs.mark_folder_encrypted(&folder_id)
                .await
                .map_err(|e| format!("E2EE step 6 failed (mark_folder_encrypted): {e}"))?;
            tracing::info!(pair_id = %pair_id, "E2EE step 6/6 complete: folder marked as E2EE");

            // Persist folder state — counter=1 because the finalization PUT used counter 1.
            sqlx::query(
                "INSERT INTO e2ee_folder_state(pair_id, folder_id, metadata_version, counter, key_checksums, updated_at) \
                 VALUES(?, ?, '2.0', 1, ?, ?) \
                 ON CONFLICT(pair_id) DO UPDATE SET \
                 folder_id=excluded.folder_id, metadata_version='2.0', \
                 counter=1, key_checksums=excluded.key_checksums, updated_at=excluded.updated_at",
            )
            .bind(&pair_id)
            .bind(&folder_id)
            .bind(metadata_key_checksum(&metadata_key))
            .bind(&now)
            .execute(state.journal.pool())
            .await
            .map_err(|e| e.to_string())?;

            // Mark pair as E2EE-enabled in memory and on disk.
            let updated_pair = {
                let mut pairs = state.pairs.write().await;
                if let Some(existing) = pairs.get_pair(&pid).cloned() {
                    let mut updated = existing;
                    updated.e2ee_enabled = true;
                    updated.e2ee_account_id = Some(aid_str.clone());
                    pairs.register_full_pair(updated.clone());
                    Some(updated)
                } else {
                    None
                }
            };
            save_config(state).await?;

            // Start the E2EE runner immediately — no daemon restart needed.
            // Cancel any stale runner first (idempotent if called twice).
            if let Some(pair) = updated_pair {
                let client = crate::dispatcher::build_client(&state.accounts, &pair).await;
                if let Some(nc_client) = client {
                    let cred_provider = Arc::new(crate::e2ee_runner::DaemonCredentialProvider {
                        accounts: state.accounts.clone(),
                        pairs: state.pairs.clone(),
                    });
                    let e2ee_client = Arc::new(adagio_e2ee::provider::NcE2eeClient::new(
                        cred_provider,
                        state.journal.clone(),
                    ));
                    let mut triggers = state.e2ee_triggers.lock().await;
                    // Cancel any stale runner for this pair.
                    if let Some((old_cancel, _)) = triggers.remove(&pid) {
                        old_cancel.cancel();
                    }
                    let (cancel, trigger_tx) = crate::e2ee_runner::spawn_e2ee_runner(
                        pair,
                        e2ee_client,
                        Arc::new(nc_client),
                        state.journal.clone(),
                    );
                    triggers.insert(pid.clone(), (cancel, trigger_tx));
                }
            }

            tracing::info!(
                pair_id = %pair_id,
                fingerprint = %fingerprint,
                "E2EE initialised successfully"
            );
            // Return mnemonic — shown once, NEVER logged.
            Ok(DaemonResponse::Status(
                serde_json::json!({ "mnemonic": mnemonic }),
            ))
        }

        DaemonRequest::E2eePair { pair_id, mnemonic } => {
            use adagio_core::types::PairId;
            use adagio_e2ee::keys::{
                key_fingerprint, private_key_to_pem, public_key_der, unwrap_private_key,
            };

            let pid = PairId(pair_id.clone());

            // Look up the pair → account.
            let (account_id, pair_account) = {
                let pairs = state.pairs.read().await;
                let pair = pairs
                    .get_pair(&pid)
                    .ok_or_else(|| format!("pair {pair_id} not found"))?;
                let aid = pair.account_id.clone();
                let accs = state.accounts.list().map_err(|e| e.to_string())?;
                let acc = accs
                    .into_iter()
                    .find(|a| a.id == aid)
                    .ok_or_else(|| format!("account {} not found", aid.0))?;
                (aid, acc)
            };

            // Retrieve Nextcloud password from keychain.
            let key = pair_account.keychain_service_key.clone();
            let nc_password = tokio::task::spawn_blocking(move || {
                adagio_nextcloud::auth::retrieve_credentials(&key)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "credentials not found in keychain".to_string())?;

            // Create OCS client and download the encrypted private key blob.
            let ocs = adagio_nextcloud::E2eeOcsClient::new(
                &pair_account.server_url,
                &pair_account.username,
                &nc_password,
            );
            let pk_resp = ocs.get_private_key().await.map_err(|e| e.to_string())?;
            let blob = pk_resp["ocs"]["data"]["private-key"]
                .as_str()
                .ok_or_else(|| "server returned no private key blob".to_string())?
                .to_string();

            // Decrypt the private key with the supplied mnemonic.
            // Run on a blocking thread — RSA key operations are CPU-intensive.
            let mnemonic_clone = mnemonic.clone();
            let privkey =
                tokio::task::spawn_blocking(move || unwrap_private_key(&blob, &mnemonic_clone))
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;

            // Fetch the certificate so we can verify and store it.
            let uid = pair_account.username.clone();
            let cert_resp = ocs
                .get_public_key(&[uid.as_str()])
                .await
                .map_err(|e| e.to_string())?;
            let cert_pem = cert_resp["ocs"]["data"]["public-keys"][&uid]
                .as_str()
                .unwrap_or("")
                .to_string();

            // Compute fingerprint from the public key.
            let pub_der = public_key_der(&privkey);
            let fingerprint = key_fingerprint(&pub_der);

            // Persist certificate to e2ee_account_keys.
            let aid_str = account_id.0.clone();
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT INTO e2ee_account_keys(account_id, certificate, key_fingerprint, paired_at, created_at) \
                 VALUES(?, ?, ?, ?, ?) \
                 ON CONFLICT(account_id) DO UPDATE SET certificate=excluded.certificate, \
                 key_fingerprint=excluded.key_fingerprint, paired_at=excluded.paired_at",
            )
            .bind(&aid_str)
            .bind(&cert_pem)
            .bind(&fingerprint)
            .bind(&now)
            .bind(&now)
            .execute(state.journal.pool())
            .await
            .map_err(|e| e.to_string())?;

            // Store the private key in the OS keychain. Must run on a blocking thread.
            let pem = private_key_to_pem(&privkey).map_err(|e| e.to_string())?;
            let pem_str = pem.to_string();
            let keychain_key = format!("adagio-e2ee/{aid_str}");
            tokio::task::spawn_blocking(move || {
                let entry =
                    keyring::Entry::new("adagio", &keychain_key).map_err(|e| e.to_string())?;
                entry.set_password(&pem_str).map_err(|e| e.to_string())
            })
            .await
            .map_err(|e| e.to_string())??;

            tracing::info!(
                pair_id = %pair_id,
                fingerprint = %fingerprint,
                "E2EE device paired successfully"
            );
            // Mnemonic MUST NOT appear in any log — it was consumed above and is now dropped.
            Ok(DaemonResponse::Unit {})
        }

        DaemonRequest::E2eeStatus { pair_id } => {
            use adagio_core::types::PairId;
            let pid = PairId(pair_id.clone());

            let e2ee_enabled = {
                let pairs = state.pairs.read().await;
                pairs
                    .get_pair(&pid)
                    .map(|p| p.e2ee_enabled)
                    .unwrap_or(false)
            };

            // Load folder state and certificate from DB.
            let folder_state: Option<(String, String, i64)> = sqlx::query_as(
                "SELECT folder_id, metadata_version, counter \
                 FROM e2ee_folder_state WHERE pair_id = ?",
            )
            .bind(&pair_id)
            .fetch_optional(state.journal.pool())
            .await
            .unwrap_or(None);

            let fingerprint: Option<String> = sqlx::query_as(
                "SELECT key_fingerprint FROM e2ee_account_keys \
                 WHERE account_id = (SELECT account_id FROM sync_pairs WHERE id = ? LIMIT 1)",
            )
            .bind(&pair_id)
            .fetch_optional(state.journal.pool())
            .await
            .ok()
            .flatten()
            .map(|(f,): (String,)| f);

            let (metadata_version, counter) = folder_state
                .map(|(_, ver, cnt)| (Some(ver), cnt as u64))
                .unwrap_or((None, 0));

            // Count locally-synced files as a proxy for encrypted file count.
            let encrypted_file_count: u64 = {
                let pairs = state.pairs.read().await;
                if let Some(pair) = pairs.get_pair(&pid) {
                    let root = &pair.local_root.0;
                    if root.is_dir() {
                        std::fs::read_dir(root)
                            .map(|d| d.count() as u64)
                            .unwrap_or(0)
                    } else {
                        0
                    }
                } else {
                    0
                }
            };

            let dto = adagio_e2ee::E2eeStatusDto {
                pair_id: pair_id.clone(),
                enabled: e2ee_enabled,
                metadata_version,
                counter,
                key_fingerprint: fingerprint,
                encrypted_file_count,
            };
            Ok(DaemonResponse::Status(
                serde_json::to_value(dto).unwrap_or_default(),
            ))
        }

        DaemonRequest::E2eeDisable { pair_id } => {
            use adagio_core::types::PairId;

            let pid = PairId(pair_id.clone());

            // Look up the pair → account for OCS client construction.
            let pair_account = {
                let pairs = state.pairs.read().await;
                let pair = pairs
                    .get_pair(&pid)
                    .ok_or_else(|| format!("pair {pair_id} not found"))?;
                let aid = pair.account_id.clone();
                state
                    .accounts
                    .list()
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .find(|a| a.id == aid)
                    .ok_or_else(|| format!("account {} not found", aid.0))?
            };

            // Retrieve Nextcloud password.
            let key = pair_account.keychain_service_key.clone();
            let nc_password = tokio::task::spawn_blocking(move || {
                adagio_nextcloud::auth::retrieve_credentials(&key)
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "credentials not found in keychain".to_string())?;

            // Delete server metadata (best-effort).
            let ocs = adagio_nextcloud::E2eeOcsClient::new(
                &pair_account.server_url,
                &pair_account.username,
                &nc_password,
            );
            // Load folder_id from local state.
            let folder_id: Option<String> =
                sqlx::query_as("SELECT folder_id FROM e2ee_folder_state WHERE pair_id = ?")
                    .bind(&pair_id)
                    .fetch_optional(state.journal.pool())
                    .await
                    .ok()
                    .flatten()
                    .map(|(f,): (String,)| f);

            if let Some(fid) = folder_id {
                let _ = ocs.delete_metadata(&fid).await;
            }

            // Remove local E2EE journal row.
            sqlx::query("DELETE FROM e2ee_folder_state WHERE pair_id = ?")
                .bind(&pair_id)
                .execute(state.journal.pool())
                .await
                .map_err(|e| e.to_string())?;

            // Mark pair as non-E2EE in the in-memory pair manager.
            {
                let mut pairs = state.pairs.write().await;
                if let Some(existing) = pairs.get_pair(&pid).cloned() {
                    let mut updated = existing;
                    updated.e2ee_enabled = false;
                    updated.e2ee_account_id = None;
                    pairs.register_full_pair(updated);
                }
            }
            save_config(state).await?;

            tracing::info!(pair_id = %pair_id, "E2EE disabled");
            Ok(DaemonResponse::Unit {})
        }
    }
}

/// Resolve account by ID, or pick the first if `account_id` is None.
/// Build a Nextcloud client for a pair, returning None if credentials are unavailable.
async fn build_client(
    accounts: &adagio_core::account_manager::AccountManager,
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

fn resolve_account(
    accounts: &[adagio_core::types::Account],
    account_id: Option<&str>,
) -> Result<adagio_core::types::Account, String> {
    if let Some(id) = account_id {
        accounts
            .iter()
            .find(|a| a.id.0 == id)
            .cloned()
            .ok_or_else(|| format!("account not found: {id}"))
    } else {
        accounts
            .first()
            .cloned()
            .ok_or_else(|| "no accounts configured".to_string())
    }
}

async fn count_pending_conflicts(state: &DaemonProcess) -> usize {
    let pairs = state.pairs.read().await;
    let mut count = 0usize;
    for pair in pairs.all_pairs() {
        if let Ok(records) = state.journal.list_conflicts(&pair.id).await {
            count += records.iter().filter(|r| r.resolution.is_none()).count();
        }
    }
    count
}

async fn save_config(state: &DaemonProcess) -> Result<(), String> {
    let accounts = state.accounts.list().map_err(|e| e.to_string())?;
    let pairs = state.pairs.read().await;
    let policy = state.network_policy.read().await.clone();
    let mut saved = build_saved_config(&accounts, &pairs);
    saved["network_policy"] = serde_json::to_value(&policy).unwrap_or_default();

    // Preserve desktop-owned fields (palette, custom_palettes, …) that the
    // daemon has no knowledge of. Read the current file and merge: daemon-owned
    // keys overwrite, everything else is kept from disk.
    if let Ok(existing_json) = std::fs::read_to_string(&state.config_path) {
        if let Ok(existing) = serde_json::from_str::<serde_json::Value>(&existing_json) {
            if let (Some(existing_obj), Some(saved_obj)) =
                (existing.as_object(), saved.as_object_mut())
            {
                for (key, value) in existing_obj {
                    if !saved_obj.contains_key(key) {
                        saved_obj.insert(key.clone(), value.clone());
                    }
                }
            }
        }
    }

    let json = serde_json::to_string_pretty(&saved).map_err(|e| e.to_string())?;
    std::fs::write(&state.config_path, json).map_err(|e| e.to_string())?;
    Ok(())
}

fn build_saved_config(
    accounts: &[adagio_core::types::Account],
    pairs: &adagio_core::config::SyncPairManager,
) -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "accounts": accounts.iter().map(|a| serde_json::json!({
            "id": a.id.to_string(),
            "display_name": a.display_name,
            "server_url": a.server_url,
            "username": a.username,
            "keychain_service_key": a.keychain_service_key,
            "upload_limit_kbps": a.upload_limit_kbps,
            "download_limit_kbps": a.download_limit_kbps,
        })).collect::<Vec<_>>(),
        "pairs": pairs.all_pairs().iter().map(|p| serde_json::json!({
            "id": p.id.to_string(),
            "account_id": p.account_id.to_string(),
            "local_root": p.local_root.0.to_string_lossy(),
            "remote_root": p.remote_root.as_str(),
            "scan_interval_secs": p.scan_interval_secs,
            "scan_on_startup": p.scan_on_startup,
            "max_upload_concurrency": p.max_upload_concurrency,
            "max_download_concurrency": p.max_download_concurrency,
            "selective_paths": p.selective_paths.iter().map(|r| r.as_str()).collect::<Vec<_>>(),
            "exclude_patterns": p.exclude_patterns,
            "bulk_upload_workers": p.bulk_upload_workers,
            "bulk_upload_threshold_files": p.bulk_upload_threshold_files,
            "bulk_upload_chunk_threshold_bytes": p.bulk_upload_chunk_threshold_bytes,
            "vfs_enabled": p.vfs_enabled,
            "vfs_cache_max_bytes": p.vfs_cache_max_bytes,
            "vfs_eviction_threshold_bytes": p.vfs_eviction_threshold_bytes,
            "e2ee_enabled": p.e2ee_enabled,
            "e2ee_account_id": p.e2ee_account_id,
        })).collect::<Vec<_>>(),
    })
}

// ── Test helpers ──────────────────────────────────────────────────────────────

#[cfg(test)]
pub struct MockDaemonState {
    pub inner: DaemonProcess,
}

#[cfg(test)]
impl MockDaemonState {
    pub async fn new_async() -> (Self, tempfile::TempDir) {
        use adagio_core::account_manager::AccountManager;
        use adagio_core::config::SyncPairManager;
        use adagio_core::cycle::DefaultSyncEngine;
        use std::sync::Arc;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let db_url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let journal = SqliteJournal::open(&db_url).await.unwrap();
        let config_path = dir.path().join("config.json");

        let engine = Arc::new(DefaultSyncEngine::new());
        let policy = Arc::new(RwLock::new(NetworkPolicy::default()));
        let monitor = Arc::new(NetworkMonitor::new(
            Arc::new(adagio_core::network::detector::MockNetworkDetector::default()),
            policy.clone(),
            engine.clone(),
        ));
        let state = Self {
            inner: DaemonProcess {
                engine,
                journal: Arc::new(journal),
                accounts: Arc::new(AccountManager::new()),
                pairs: Arc::new(RwLock::new(SyncPairManager::new())),
                events: EventBroadcaster::new(16),
                started_at: Instant::now(),
                config_path,
                network_policy: policy,
                network_monitor: monitor,
                e2ee_triggers: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            },
        };
        (state, dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T010 — dispatch(Ping) returns Pong with version and uptime.
    #[tokio::test]
    async fn dispatcher_routes_ping_to_pong() {
        let (state, _dir) = MockDaemonState::new_async().await;
        let resp = dispatch(adagio_ipc::DaemonRequest::Ping, &state.inner)
            .await
            .unwrap();
        match resp {
            adagio_ipc::DaemonResponse::Pong {
                version,
                uptime_secs,
            } => {
                assert!(!version.is_empty(), "version must not be empty");
                let _ = uptime_secs;
            }
            other => panic!("expected Pong, got {:?}", other),
        }
    }

    // T011 — dispatch(GetStatus) returns Status payload.
    #[tokio::test]
    async fn dispatcher_routes_get_status() {
        let (state, _dir) = MockDaemonState::new_async().await;
        let resp = dispatch(adagio_ipc::DaemonRequest::GetStatus, &state.inner)
            .await
            .unwrap();
        match resp {
            adagio_ipc::DaemonResponse::Status(v) => {
                assert_eq!(v["status"].as_str().unwrap(), "idle");
            }
            other => panic!("expected Status, got {:?}", other),
        }
    }
}
