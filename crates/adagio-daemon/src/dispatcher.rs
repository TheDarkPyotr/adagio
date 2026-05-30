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

use crate::events::EventBroadcaster;

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
            let status_dto = match state.engine.status() {
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
            };
            Ok(DaemonResponse::Status(status_dto))
        }

        DaemonRequest::TriggerSync { pair_id } => {
            use adagio_core::types::PairId;
            state
                .engine
                .trigger_pair(&PairId(pair_id))
                .await
                .map_err(|e| e.to_string())?;
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
            let entries = state
                .engine
                .activity_log(limit.unwrap_or(50) as usize)
                .await
                .map_err(|e| e.to_string())?;
            let dtos: Vec<serde_json::Value> = entries
                .into_iter()
                .map(|e| {
                    serde_json::json!({
                        "id": uuid::Uuid::new_v4().to_string(),
                        "who": "You",
                        "verb": &e.action,
                        "target": e.path.rsplit('/').next().unwrap_or(&e.path),
                        "with_whom": null,
                        "where_path": &e.path,
                        "at": e.occurred_at.timestamp_millis(),
                        "kind": if filter.as_deref() == Some("conflict") { "conflict" } else { "sync" }
                    })
                })
                .collect();
            Ok(DaemonResponse::ActivityLog(dtos))
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
            };
            let dto = serde_json::json!({
                "id": pair.id.to_string(),
                "account_id": pair.account_id.to_string(),
                "local_root": local_root,
                "remote_root": remote_root,
                "scan_interval_secs": pair.scan_interval_secs,
                "selective_paths": [],
                "vfs_enabled": pair.vfs_enabled,
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
                    state.engine.start_vfs_pair(pair.clone(), client, state.journal.clone(), provider);
                } else {
                    state.engine.start_pair(pair.clone(), client, state.journal.clone(), None).await;
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
            state.engine.stop_pair(&pid).await;
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

            // Find the pair to get its local_root and vfs_enabled flag.
            let (local_root, vfs_enabled) = {
                let pairs = state.pairs.read().await;
                let p = pairs
                    .get_pair(&pid)
                    .ok_or_else(|| format!("pair {pair_id} not found"))?;
                (p.local_root.0.clone(), p.vfs_enabled)
            };

            // Strip leading '/' so frontend's "/" becomes "" (root level).
            let sub = relative_path
                .as_deref()
                .unwrap_or("")
                .trim_start_matches('/')
                .to_string();

            if vfs_enabled {
                // VFS pair: serve entries from vfs_cache_metadata (journal),
                // not from the local filesystem. FUSE makes the directory
                // read-accessible, but we want remote metadata (size, mtime)
                // and VFS state for the status column.
                let all_entries = state.journal.all_vfs_entries(&pid).await
                    .map_err(|e| e.to_string())?;

                // Collect unique direct children of `sub`.
                let mut seen: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();
                for entry in &all_entries {
                    let p = entry.path.as_str();
                    let child_name = if sub.is_empty() {
                        let first = p.split('/').next().unwrap_or(p);
                        first.to_string()
                    } else {
                        let prefix = format!("{sub}/");
                        if !p.starts_with(&prefix) { continue; }
                        let rest = &p[prefix.len()..];
                        rest.split('/').next().unwrap_or(rest).to_string()
                    };
                    if child_name.is_empty() { continue; }

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

                    seen.insert(child_name.clone(), serde_json::json!({
                        "path": format!("/{rel_str}"),
                        "name": child_name,
                        "is_dir": is_dir,
                        "size": size,
                        "mtime": mtime,
                        "status": status,
                        "etag": serde_json::Value::Null,
                        "item_count": serde_json::Value::Null,
                    }));
                }
                let mut dtos: Vec<serde_json::Value> = seen.into_values().collect();
                dtos.sort_by(|a, b| {
                    let a_dir = a["is_dir"].as_bool().unwrap_or(false);
                    let b_dir = b["is_dir"].as_bool().unwrap_or(false);
                    match (b_dir, a_dir) {
                        (true, false) => std::cmp::Ordering::Greater,
                        (false, true) => std::cmp::Ordering::Less,
                        _ => a["name"].as_str().unwrap_or("").to_lowercase()
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
                    pairs.get_pair(&pid).cloned()
                        .ok_or_else(|| format!("pair {pid} not found"))?
                };
                let client = build_client(&state.accounts, &pair).await
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
                state.journal.unpin_path(&pid, &path).await
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
            adagio_core::vfs::evict_file(
                &pid,
                &path,
                &*state.journal,
                provider.as_ref(),
            )
            .await
            .map_err(|e| e.to_string())?;
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
    let account = accounts.list().ok()?.into_iter().find(|a| a.id == pair.account_id)?;
    let key = account.keychain_service_key.clone();
    let password = tokio::task::spawn_blocking(move || adagio_nextcloud::auth::retrieve_credentials(&key))
        .await.ok()?.ok()??;
    Some(adagio_nextcloud::client::NextcloudClient::new(&account.server_url, &account.username, &password))
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
