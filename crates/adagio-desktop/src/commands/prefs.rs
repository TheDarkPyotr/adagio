use crate::config::{load_config, save_config};
use crate::state::AppState;
use tauri::State;
use tracing::{info, instrument};

const VALID_PALETTES: &[&str] = &[
    "sienna", "slate", "bone", "ink", "plum", "azure", "iris", "citron",
    "forest", "rose", "midnight", "carbon",
];

/// Return the active palette name from config.
///
/// Returns `"sienna"` if no palette has been persisted yet.
#[tauri::command]
#[instrument(skip(state))]
pub async fn get_palette(state: State<'_, AppState>) -> Result<String, String> {
    let cfg = load_config(&state.config_path);
    let palette = cfg.palette.unwrap_or_else(|| "sienna".to_string());
    info!(palette = %palette, "get_palette");
    Ok(palette)
}

/// Persist a palette selection to config.
///
/// Returns an error if the name is not one of the supported palettes.
#[tauri::command]
#[instrument(skip(state))]
pub async fn set_palette(state: State<'_, AppState>, name: String) -> Result<(), String> {
    if !VALID_PALETTES.contains(&name.as_str()) {
        return Err(format!("Unknown palette: {name}"));
    }
    let mut cfg = load_config(&state.config_path);
    cfg.palette = Some(name.clone());
    save_config(&state.config_path, &cfg).map_err(|e| e.to_string())?;
    info!(palette = %name, "set_palette");
    Ok(())
}
