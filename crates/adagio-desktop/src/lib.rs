pub mod commands;
pub mod config;
pub mod lifecycle;
pub mod oauth2_callback;
pub mod state;

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    adagio_core::telemetry::init("adagio=info,warn");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // ── Register AppState SYNCHRONOUSLY with a stub DaemonClient.
            //
            // This avoids a race where commands fired immediately on startup
            // (listAccounts, listPairs, etc.) arrive before the async daemon
            // connection completes and fail with "state not managed".
            //
            // The stub starts with no socket connection; all requests return
            // "not connected" until upgrade_connection() succeeds below.
            let config_dir = app.path().app_config_dir()?;
            let stub_daemon = adagio_ipc::DaemonClient::new_stub();
            let app_state = state::AppState {
                daemon: stub_daemon.clone(),
                config_path: config_dir.join("config.json"),
            };
            app.manage(app_state);

            let handle = app.handle().clone();
            let config_dir_owned = config_dir.clone();

            // ── Connect to (or start) the daemon and upgrade the stub.
            tauri::async_runtime::spawn(async move {
                let mut event_rx = stub_daemon.subscribe();
                let state_rx = stub_daemon.connection_state();

                // Wire event forwarding and connection-state broadcast
                // BEFORE attempting connection so no events are missed.
                let handle2 = handle.clone();
                tokio::spawn(async move {
                    loop {
                        match event_rx.recv().await {
                            Ok(event) => forward_daemon_event(&handle2, event),
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        }
                    }
                });

                let handle3 = handle.clone();
                tokio::spawn(async move {
                    let mut rx = state_rx;
                    // Emit the current (initial) state immediately so the
                    // frontend can show "connecting…" without waiting for a change.
                    let initial_str = match *rx.borrow() {
                        adagio_ipc::ConnectionState::Connected => "connected",
                        adagio_ipc::ConnectionState::Reconnecting { .. } => "reconnecting",
                        adagio_ipc::ConnectionState::Stopped => "stopped",
                        adagio_ipc::ConnectionState::Failed => "failed",
                    };
                    let _ = handle3.emit(
                        "adagio://daemon-connection-state",
                        serde_json::json!({ "state": initial_str }),
                    );
                    loop {
                        if rx.changed().await.is_err() {
                            break;
                        }
                        let state_str = match *rx.borrow() {
                            adagio_ipc::ConnectionState::Connected => "connected",
                            adagio_ipc::ConnectionState::Reconnecting { .. } => "reconnecting",
                            adagio_ipc::ConnectionState::Stopped => "stopped",
                            adagio_ipc::ConnectionState::Failed => "failed",
                        };
                        let _ = handle3.emit(
                            "adagio://daemon-connection-state",
                            serde_json::json!({ "state": state_str }),
                        );
                    }
                });

                // Now upgrade the stub to a real connection. Passes the Tauri
                // config dir to the daemon as --config-dir so both processes
                // read the same accounts/pairs database.
                match lifecycle::connect_or_upgrade(&stub_daemon, &config_dir_owned).await {
                    Ok(()) => {
                        tracing::info!("adagio-daemon connection established");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to connect to adagio-daemon");
                        let _ = handle.emit(
                            "adagio://daemon-connection-state",
                            serde_json::json!({ "state": "failed" }),
                        );
                    }
                }
            });

            // Wire tray icon left-click.
            let tray = app
                .tray_by_id("main")
                .or_else(|| app.tray_by_id("ai.neuralagent.adagio"));
            if let Some(tray) = tray {
                let app_handle = app.handle().clone();
                tray.on_tray_icon_event(move |_icon, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        if let Some(tray_win) = app_handle.get_webview_window("tray") {
                            if tray_win.is_visible().unwrap_or(false) {
                                tray_win.hide().ok();
                            } else {
                                let win_size = tray_win
                                    .outer_size()
                                    .unwrap_or(tauri::PhysicalSize::new(380, 520));
                                let x = (position.x as i32) - (win_size.width as i32 / 2);
                                let y = (position.y as i32) - (win_size.height as i32) - 8;
                                tray_win
                                    .set_position(tauri::PhysicalPosition::new(x.max(0), y.max(0)))
                                    .ok();
                                tray_win.show().ok();
                                tray_win.set_focus().ok();
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                window.hide().ok();
            }
            tauri::WindowEvent::Focused(false) if window.label() == "tray" => {
                window.hide().ok();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::account::connect_account_oauth2,
            commands::account::add_account,
            commands::account::remove_account,
            commands::account::list_accounts,
            commands::daemon::get_daemon_status,
            commands::daemon::start_daemon,
            commands::daemon::stop_daemon,
            commands::daemon::set_start_at_login,
            commands::pair::create_pair,
            commands::pair::delete_pair,
            commands::pair::list_pairs,
            commands::pair::get_exclude_patterns,
            commands::pair::list_synced_files,
            commands::sync::get_status,
            commands::sync::pause_sync,
            commands::sync::resume_sync,
            commands::sync::get_activity_log,
            commands::sync::trigger_sync,
            commands::sync::get_error_items,
            commands::pair::list_remote_tree,
            commands::conflicts::list_conflicts,
            commands::conflicts::resolve_conflict,
            commands::conflicts::dismiss_all_conflicts,
            commands::prefs::get_palette,
            commands::prefs::set_palette,
            commands::prefs::list_custom_palettes,
            commands::prefs::save_custom_palette,
            commands::prefs::delete_custom_palette,
            commands::sharing::search_users,
            commands::sharing::create_share,
            commands::bandwidth::get_bandwidth_status,
            commands::bandwidth::set_bandwidth_limits,
            commands::bandwidth::clear_bandwidth_limits,
            commands::network::get_network_status,
            commands::network::set_network_policy,
            commands::network::add_blocked_ssid,
            commands::network::remove_blocked_ssid,
            commands::network::list_blocked_ssids,
        ])
        .run(tauri::generate_context!())
        .expect("error running Tauri application");
}

// ── Event forwarding ──────────────────────────────────────────────────────────

fn forward_daemon_event(app: &tauri::AppHandle, event: adagio_ipc::DaemonEvent) {
    use adagio_ipc::DaemonEvent;
    match event {
        DaemonEvent::ConflictDetected { pending_count } => {
            commands::conflicts::emit_conflict_detected(app, pending_count);
        }
        DaemonEvent::ConflictResolved { id, pending_count } => {
            let _ = app.emit(
                "adagio://conflict-resolved",
                serde_json::json!({ "id": id, "pending_count": pending_count }),
            );
        }
        DaemonEvent::SyncStatusChanged { status, pair_id } => {
            let _ = app.emit(
                "adagio://sync-status-changed",
                serde_json::json!({ "status": status, "pair_id": pair_id }),
            );
        }
        DaemonEvent::TransferProgress {
            pair_id,
            path,
            bytes_done,
            bytes_total,
        } => {
            let _ = app.emit(
                "adagio://transfer-progress",
                serde_json::json!({
                    "pair_id": pair_id,
                    "path": path,
                    "bytes_done": bytes_done,
                    "bytes_total": bytes_total,
                }),
            );
        }
        DaemonEvent::ShuttingDown { .. } => {}
    }
}
