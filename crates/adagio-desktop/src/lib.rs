pub mod auth_flow;
pub mod commands;
pub mod config;
pub mod lifecycle;
pub mod oauth2_callback;
pub mod state;

use tauri::{Emitter, Manager};
use tauri_plugin_shell::ShellExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    adagio_core::telemetry::init("adagio=info,warn");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
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
                auth_flow: crate::auth_flow::AuthFlowSlot::default(),
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
                        // Update state_tx so get_daemon_status reflects the failure
                        // rather than staying at the initial Reconnecting value.
                        stub_daemon.mark_failed();
                        let _ = handle.emit(
                            "adagio://daemon-connection-state",
                            serde_json::json!({ "state": "failed" }),
                        );
                    }
                }
            });

            // ── Build tray menu ───────────────────────────────────────────────
            //
            // On Linux, libappindicator requires a GTK menu before the
            // ubuntu-appindicators GNOME extension will show the icon. We
            // build a full functional menu (not just a placeholder) so clicking
            // any item works without needing to open the rich popover first.
            //
            // On macOS / Windows, the same menu is available on right-click;
            // left-click still toggles the WebView popover directly.
            let tray = app
                .tray_by_id("main")
                .or_else(|| app.tray_by_id("ai.neuralagent.adagio"));
            if tray.is_none() {
                tracing::warn!(
                    "no tray icon registered — tray host may be unavailable \
                     (GNOME without AppIndicator extension, or unsupported Wayland compositor)"
                );
            }
            if let Some(ref tray) = tray {
                build_tray_menu(app.handle(), tray);
            }

            // ── Wire menu events ──────────────────────────────────────────────
            let handle_menu = app.handle().clone();
            app.handle().on_menu_event(move |_app, event| {
                on_tray_menu_event(&handle_menu, event.id().as_ref());
            });

            // ── Wire left-click (macOS / Windows; skipped on Linux) ──────────
            //
            // On Linux with libappindicator, left-click opens the GTK menu (the
            // "Show Adagio" menu item handles the popover). The AppIndicator backend
            // also fires a synthetic TrayIconEvent::Click with position (0,0) after
            // each menu interaction, which would incorrectly toggle the popover shut.
            // All Linux interaction goes through on_menu_event instead.
            #[cfg(not(target_os = "linux"))]
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
                        toggle_tray_popover_at(&app_handle, (position.x, position.y));
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
                tracing::debug!("tray window lost focus, hiding");
                window.hide().ok();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::account::connect_account_oauth2,
            commands::account::add_account,
            commands::account::remove_account,
            commands::account::list_accounts,
            commands::account::get_account_avatar,
            commands::daemon::get_daemon_status,
            commands::daemon::start_daemon,
            commands::daemon::stop_daemon,
            commands::daemon::set_start_at_login,
            commands::pair::create_pair,
            commands::pair::delete_pair,
            commands::pair::list_pairs,
            commands::pair::get_exclude_patterns,
            commands::pair::list_synced_files,
            commands::files::create_local_folder,
            commands::files::create_local_file,
            commands::files::write_local_files,
            commands::sync::get_status,
            commands::sync::pause_sync,
            commands::sync::resume_sync,
            commands::sync::get_activity_log,
            commands::sync::get_section_counts,
            commands::sync::search_files,
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
            commands::vfs::get_vfs_stats,
            commands::vfs::set_vfs_pin,
            commands::vfs::evict_vfs_file,
            commands::e2ee::e2ee_init,
            commands::e2ee::e2ee_pair,
            commands::e2ee::e2ee_status,
            commands::e2ee::e2ee_disable,
            commands::network::get_network_status,
            commands::network::set_network_policy,
            commands::network::add_blocked_ssid,
            commands::network::remove_blocked_ssid,
            commands::network::list_blocked_ssids,
            commands::onboarding::probe_server,
            commands::onboarding::begin_auth_flow,
            commands::onboarding::pick_folder,
            commands::onboarding::get_account_remote_stats,
            commands::onboarding::complete_onboarding,
            commands::get_platform,
        ])
        .run(tauri::generate_context!())
        .expect("error running Tauri application");
}

// ── Tray helpers ─────────────────────────────────────────────────────────────

/// Build and attach the tray icon menu.
///
/// Called once from `setup()`. Creates labelled items so `on_menu_event` can
/// match by ID. Errors on individual items are logged and skipped; the menu is
/// still attached even if some items fail.
fn build_tray_menu<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    tray: &tauri::tray::TrayIcon<R>,
) {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};

    let Ok(menu) = Menu::new(handle) else { return };

    let items: &[(&str, &str)] = &[
        ("show-adagio", "Show Adagio"),
        ("---", ""),
        ("open-folder", "Open Adagio folder"),
        ("open-browser", "Open in browser"),
        ("---", ""),
        ("pause-resume", "Pause syncing"),
        ("---", ""),
        ("preferences", "Preferences…"),
        ("---", ""),
        ("quit", "Quit Adagio"),
    ];

    for &(id, label) in items {
        if id == "---" {
            if let Ok(sep) = PredefinedMenuItem::separator(handle) {
                let _ = menu.append(&sep);
            }
        } else if let Ok(item) = MenuItem::with_id(handle, id, label, true, None::<&str>) {
            let _ = menu.append(&item);
        }
    }

    if let Err(e) = tray.set_menu(Some(menu)) {
        tracing::warn!(error = %e, "failed to attach tray menu");
    } else {
        tracing::debug!("tray menu attached");
    }
}

/// Handle a click on any tray menu item.
///
/// Spawns async tasks for actions that require daemon I/O. Synchronous actions
/// (show popover, preferences, quit) execute inline on the event thread.
fn on_tray_menu_event<R: tauri::Runtime>(app: &tauri::AppHandle<R>, id: &str) {
    match id {
        "show-adagio" => {
            // Toggle the WebView popover. On Linux/Wayland the icon position
            // is not retrievable from the AppIndicator protocol, so we pass
            // (0.0, 0.0) which compute_popover_position maps to the top-right
            // corner fallback — the natural GNOME tray area.
            toggle_tray_popover_at(app, (0.0, 0.0));
        }

        "open-folder" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(root) = load_first_pair_root(&app).await {
                    #[allow(deprecated)]
                    if let Err(e) = app.shell().open(&root, None) {
                        tracing::warn!(error = %e, "failed to open local folder");
                    }
                } else {
                    tracing::warn!("no sync pair configured — cannot open folder");
                }
            });
        }

        "open-browser" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(url) = load_first_server_url(&app).await {
                    #[allow(deprecated)]
                    if let Err(e) = app.shell().open(&url, None) {
                        tracing::warn!(error = %e, "failed to open browser");
                    }
                } else {
                    tracing::warn!("no account configured — cannot open browser");
                }
            });
        }

        "pause-resume" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                use adagio_ipc::DaemonRequest;
                let state = app.state::<state::AppState>();
                // Query current status to decide whether to pause or resume.
                let is_paused = match state.daemon.request(DaemonRequest::GetStatus).await {
                    Ok(v) => v.get("status").and_then(|s| s.as_str()) == Some("paused"),
                    _ => false,
                };
                let req = if is_paused {
                    DaemonRequest::ResumeSyncAll
                } else {
                    DaemonRequest::PauseSyncAll
                };
                if let Err(e) = state.daemon.request(req).await {
                    tracing::warn!(error = %e, "pause/resume failed");
                }
            });
        }

        "preferences" => {
            // Raise the main window and tell it to navigate to the settings page.
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
                let _ = win.emit("adagio://open-settings", serde_json::json!({}));
            }
        }

        "quit" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                for win in app.webview_windows().values() {
                    let _ = win.close();
                }
            });
        }

        other => tracing::debug!(id = other, "unhandled menu event"),
    }
}

/// Show or hide the tray popover, positioning it relative to `click_pos`.
fn toggle_tray_popover_at<R: tauri::Runtime>(app: &tauri::AppHandle<R>, click_pos: (f64, f64)) {
    let Some(tray_win) = app.get_webview_window("tray") else {
        tracing::warn!("tray window not found — check tauri.conf.json");
        return;
    };
    if tray_win.is_visible().unwrap_or(false) {
        tray_win.hide().ok();
        return;
    }
    let win_size = tray_win
        .outer_size()
        .unwrap_or(tauri::PhysicalSize::new(380, 520));
    let screen = app.primary_monitor().ok().flatten();
    let (screen_w, screen_h) = screen
        .map(|m| (m.size().width, m.size().height))
        .unwrap_or((1920, 1080));
    let (x, y) = lifecycle::compute_popover_position(
        click_pos,
        (win_size.width, win_size.height),
        screen_w,
        screen_h,
    );
    tray_win
        .set_position(tauri::PhysicalPosition::new(x, y))
        .ok();
    tray_win.show().ok();
    tray_win.set_focus().ok();
}

/// Query the daemon for the local sync root of the first configured pair.
///
/// Uses the daemon (same source as the TrayPopover's `listPairs`) rather than
/// reading the config file directly, so both surfaces always show the same path.
async fn load_first_pair_root<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<String> {
    let state = app.state::<state::AppState>();
    match state
        .daemon
        .request(adagio_ipc::DaemonRequest::ListPairs)
        .await
    {
        Ok(v) => v
            .as_array()
            .and_then(|a| a.first())
            .and_then(|p| p.get("local_root"))
            .and_then(|r| r.as_str())
            .map(|s| s.to_string()),
        Err(e) => {
            tracing::warn!(error = %e, "failed to list pairs from daemon");
            None
        }
    }
}

/// Query the daemon for the server URL of the first configured account.
async fn load_first_server_url<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Option<String> {
    let state = app.state::<state::AppState>();
    match state
        .daemon
        .request(adagio_ipc::DaemonRequest::ListAccounts)
        .await
    {
        Ok(v) => v
            .as_array()
            .and_then(|a| a.first())
            .and_then(|p| p.get("server_url"))
            .and_then(|r| r.as_str())
            .map(|s| s.to_string()),
        Err(e) => {
            tracing::warn!(error = %e, "failed to list accounts from daemon");
            None
        }
    }
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
