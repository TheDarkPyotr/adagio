use crate::config::{load_config, save_config, CustomPaletteEntry};
use crate::state::AppState;
use tauri::State;
use tracing::{info, instrument};

const BUILTIN_PALETTES: &[&str] = &[
    "sienna", "slate", "bone", "ink", "plum", "azure", "iris", "citron", "forest", "rose",
    "midnight", "carbon",
];

// ── Built-in palette commands ─────────────────────────────────────────────────

/// Return the active palette name from config. Returns `"sienna"` if unset.
#[tauri::command]
#[instrument(skip(state))]
pub async fn get_palette(state: State<'_, AppState>) -> Result<String, String> {
    let cfg = load_config(&state.config_path);
    let palette = cfg.palette.unwrap_or_else(|| "sienna".to_string());
    info!(palette = %palette, "get_palette");
    Ok(palette)
}

/// Persist a palette selection. Accepts both built-in names and custom IDs
/// (any string starting with `"custom-"`).
#[tauri::command]
#[instrument(skip(state))]
pub async fn set_palette(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let is_custom = name.starts_with("custom-");
    if !is_custom && !BUILTIN_PALETTES.contains(&name.as_str()) {
        return Err(format!("Unknown palette: {name}"));
    }
    let mut cfg = load_config(&state.config_path);
    cfg.palette = Some(name.clone());
    save_config(&state.config_path, &cfg).map_err(|e| e.to_string())?;
    info!(palette = %name, "set_palette");
    Ok(())
}

// ── Custom palette commands ───────────────────────────────────────────────────

/// List all user-defined custom palettes.
#[tauri::command]
pub async fn list_custom_palettes(
    state: State<'_, AppState>,
) -> Result<Vec<CustomPaletteEntry>, String> {
    let cfg = load_config(&state.config_path);
    Ok(cfg.custom_palettes)
}

/// Save (create or update) a custom palette.
///
/// Generates a stable ID from the name if the entry is new.
/// Returns the full entry including its ID.
#[tauri::command]
pub async fn save_custom_palette(
    state: State<'_, AppState>,
    id: Option<String>,
    name: String,
    cream: String,
    ink: String,
    accent: String,
) -> Result<CustomPaletteEntry, String> {
    let mut cfg = load_config(&state.config_path);

    // Reuse existing ID on update, generate a new one on create.
    let palette_id = id.unwrap_or_else(|| {
        let slug = name
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ')
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-");
        format!("custom-{slug}")
    });

    let entry = CustomPaletteEntry {
        id: palette_id.clone(),
        name: name.trim().to_string(),
        cream: cream.trim().to_string(),
        ink: ink.trim().to_string(),
        accent: accent.trim().to_string(),
    };

    // Update existing or append.
    if let Some(existing) = cfg.custom_palettes.iter_mut().find(|p| p.id == palette_id) {
        *existing = entry.clone();
    } else {
        cfg.custom_palettes.push(entry.clone());
    }

    save_config(&state.config_path, &cfg).map_err(|e| e.to_string())?;
    info!(id = %palette_id, "custom palette saved");
    Ok(entry)
}

/// Delete a custom palette. If it was the active palette, clears the selection.
#[tauri::command]
pub async fn delete_custom_palette(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut cfg = load_config(&state.config_path);
    cfg.custom_palettes.retain(|p| p.id != id);
    // Clear active palette if it was the deleted one.
    if cfg.palette.as_deref() == Some(&id) {
        cfg.palette = None;
    }
    save_config(&state.config_path, &cfg).map_err(|e| e.to_string())?;
    info!(id = %id, "custom palette deleted");
    Ok(())
}
