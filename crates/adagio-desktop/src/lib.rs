pub mod commands;
pub mod config;
pub mod lifecycle;
pub mod oauth2_callback;
pub mod state;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    adagio_core::telemetry::init("adagio=info,warn");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let app_state = state::AppState::new(config_dir)?;
            app.manage(app_state);

            // Spawn engine runners for all configured pairs after state is managed.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<state::AppState>();
                lifecycle::start_engine_for_all_pairs(&state).await;
            });

            Ok(())
        })
        .on_window_event(|_window, event| {
            // Hide the window instead of quitting so background sync continues.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                _window.hide().ok();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::account::connect_account_oauth2,
            commands::account::add_account,
            commands::account::remove_account,
            commands::account::list_accounts,
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
        ])
        .run(tauri::generate_context!())
        .expect("error running Tauri application");
}
