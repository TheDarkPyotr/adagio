use std::path::{Path, PathBuf};

/// Validate that the resolved path stays inside `base` (prevents path traversal).
fn safe_join(base: &Path, rel: &str, name: &str) -> Result<PathBuf, String> {
    let candidate = base
        .join(rel.trim_start_matches('/'))
        .join(name.trim_start_matches('/'));
    let canonical_base = base.canonicalize().map_err(|e| e.to_string())?;
    // Resolve as far as possible; the final segment may not exist yet.
    let resolved = if let Ok(p) = candidate.canonicalize() {
        p
    } else if let Some(parent) = candidate.parent() {
        let parent_canon = parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf());
        parent_canon.join(candidate.file_name().unwrap_or_default())
    } else {
        candidate.clone()
    };
    if !resolved.starts_with(&canonical_base) {
        return Err("path traversal detected".to_string());
    }
    Ok(candidate)
}

/// Create a new sub-directory inside the current sync folder.
#[tauri::command]
pub async fn create_local_folder(
    local_root: String,
    rel_path: String,
    name: String,
) -> Result<(), String> {
    let base = PathBuf::from(&local_root);
    let path = safe_join(&base, &rel_path, &name)?;
    std::fs::create_dir_all(&path).map_err(|e| format!("create folder: {e}"))
}

/// Create a new text file (e.g. Markdown) with optional initial content.
#[tauri::command]
pub async fn create_local_file(
    local_root: String,
    rel_path: String,
    name: String,
    content: String,
) -> Result<(), String> {
    let base = PathBuf::from(&local_root);
    let path = safe_join(&base, &rel_path, &name)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }
    std::fs::write(&path, content).map_err(|e| format!("write file: {e}"))
}

/// Write one or more files (binary content) into the sync folder.
///
/// Used by both single-file upload (drag-and-drop / browse) and folder
/// upload (where `rel_subpath` carries the folder-relative sub-directory).
#[derive(serde::Deserialize)]
pub struct UploadEntry {
    /// Final filename (no path separators).
    pub name: String,
    /// Optional sub-directory relative to `rel_path` (from webkitRelativePath).
    pub rel_subpath: Option<String>,
    /// Raw file bytes as an array of 0-255 integers.
    pub content: Vec<u8>,
}

#[tauri::command]
pub async fn write_local_files(
    local_root: String,
    rel_path: String,
    files: Vec<UploadEntry>,
) -> Result<usize, String> {
    let base = PathBuf::from(&local_root);
    let dest = base.join(rel_path.trim_start_matches('/'));
    let mut written = 0usize;

    for entry in &files {
        // Build the target path (with optional sub-directory for folder uploads).
        let file_path = if let Some(sub) = &entry.rel_subpath {
            dest.join(sub.trim_start_matches('/')).join(&entry.name)
        } else {
            dest.join(&entry.name)
        };

        // Safety: every resolved path must stay under base.
        let canonical_base = base.canonicalize().map_err(|e| e.to_string())?;
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
            if let Ok(cp) = parent.canonicalize() {
                if !cp.starts_with(&canonical_base) {
                    return Err("path traversal detected".to_string());
                }
            }
        }

        std::fs::write(&file_path, &entry.content)
            .map_err(|e| format!("write {}: {e}", entry.name))?;
        written += 1;
    }

    Ok(written)
}
