//! A file browser scoped strictly to a server's own folder. Every path from
//! the UI is resolved and checked to stay inside the server directory, so
//! the browser can never read or write elsewhere.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{AppError, AppResult};

/// The largest text file the editor will open, to avoid loading a giant
/// world file into the UI.
const MAX_EDITABLE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    /// Path relative to the server directory, using forward slashes.
    pub rel_path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
}

/// Resolves `rel_path` inside `server_dir`, rejecting anything that escapes
/// it (via `..`, absolute paths, or symlinks).
fn safe_join(server_dir: &Path, rel_path: &str) -> AppResult<PathBuf> {
    let candidate = server_dir.join(rel_path);

    // Canonicalize the deepest existing ancestor and confirm containment.
    let base =
        std::fs::canonicalize(server_dir).map_err(|error| AppError::Process(error.to_string()))?;
    let resolved = match std::fs::canonicalize(&candidate) {
        Ok(path) => path,
        // Doesn't exist yet (e.g. a new file): validate its parent instead.
        Err(_) => {
            let parent = candidate
                .parent()
                .ok_or_else(|| AppError::InvalidInput("invalid path".to_string()))?;
            let parent_real = std::fs::canonicalize(parent)
                .map_err(|error| AppError::Process(error.to_string()))?;
            if !parent_real.starts_with(&base) {
                return Err(AppError::InvalidInput(
                    "path escapes the server folder".to_string(),
                ));
            }
            return Ok(candidate);
        }
    };

    if !resolved.starts_with(&base) {
        return Err(AppError::InvalidInput(
            "path escapes the server folder".to_string(),
        ));
    }
    Ok(resolved)
}

/// Lists one directory (relative to the server root), folders first then
/// files, each alphabetical.
pub fn list_dir(server_dir: &Path, rel_path: &str) -> AppResult<Vec<DirEntry>> {
    let dir = safe_join(server_dir, rel_path)?;

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let name = entry.file_name().to_string_lossy().to_string();

        let child_rel = if rel_path.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", rel_path.trim_end_matches('/'), name)
        };

        entries.push(DirEntry {
            name,
            rel_path: child_rel,
            is_dir: metadata.is_dir(),
            size_bytes: metadata.len(),
        });
    }

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(entries)
}

/// Reads a text file's contents, refusing files that are too large or not
/// valid UTF-8 (those are edited elsewhere, not here).
pub fn read_text(server_dir: &Path, rel_path: &str) -> AppResult<String> {
    let file = safe_join(server_dir, rel_path)?;
    let metadata = std::fs::metadata(&file)?;
    if metadata.len() > MAX_EDITABLE_BYTES {
        return Err(AppError::InvalidInput(
            "file is too large to edit here".to_string(),
        ));
    }

    let bytes = std::fs::read(&file)?;
    String::from_utf8(bytes).map_err(|_| AppError::InvalidInput("not a text file".to_string()))
}

pub fn write_text(server_dir: &Path, rel_path: &str, contents: &str) -> AppResult<()> {
    let file = safe_join(server_dir, rel_path)?;
    std::fs::write(file, contents)?;
    Ok(())
}

pub fn delete_entry(server_dir: &Path, rel_path: &str) -> AppResult<()> {
    let target = safe_join(server_dir, rel_path)?;
    if target.is_dir() {
        std::fs::remove_dir_all(target)?;
    } else {
        std::fs::remove_file(target)?;
    }
    Ok(())
}
