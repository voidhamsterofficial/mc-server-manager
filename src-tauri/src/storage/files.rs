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

        let child_rel = join_rel(rel_path, &name);

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

/// Copies a file from anywhere on disk into `rel_dir` inside the server
/// folder, keeping its original name and replacing any file already there.
/// Returns the name it landed under. Folders are rejected — the browser
/// imports single files (plugin jars, configs), not trees.
pub fn import_file(server_dir: &Path, rel_dir: &str, source_path: &Path) -> AppResult<String> {
    if source_path.is_dir() {
        return Err(AppError::InvalidInput(
            "folders can't be dropped in — drop the files inside it".to_string(),
        ));
    }

    let file_name = source_path
        .file_name()
        .ok_or_else(|| AppError::InvalidInput("that file has no name".to_string()))?
        .to_string_lossy()
        .to_string();

    let dir = safe_join(server_dir, rel_dir)?;
    if !dir.is_dir() {
        return Err(AppError::InvalidInput(
            "that destination isn't a folder".to_string(),
        ));
    }

    let destination = safe_join(server_dir, &join_rel(rel_dir, &file_name))?;
    std::fs::copy(source_path, destination)?;
    Ok(file_name)
}

/// Joins a child name onto a server-relative directory path, using the
/// forward slashes the UI works in.
fn join_rel(rel_dir: &str, name: &str) -> String {
    if rel_dir.is_empty() {
        return name.to_string();
    }
    let joined = format!("{}/{}", rel_dir.trim_end_matches('/'), name);
    joined
}

/// Rejects a name that isn't a single, plain file or folder name — no path
/// separators, no traversal. `safe_join` would catch an escape anyway, but a
/// clear message beats "path escapes the server folder" for a typo.
fn validate_entry_name(name: &str) -> AppResult<&str> {
    let trimmed = name.trim();
    let is_plain = !trimmed.is_empty()
        && !trimmed.contains('/')
        && !trimmed.contains('\\')
        && trimmed != "."
        && trimmed != "..";
    if !is_plain {
        return Err(AppError::InvalidInput(
            "that name can't contain slashes or be . or ..".to_string(),
        ));
    }
    Ok(trimmed)
}

/// Creates an empty file in `rel_dir`, refusing to clobber anything already
/// there. Returns its path relative to the server folder.
pub fn create_file(server_dir: &Path, rel_dir: &str, name: &str) -> AppResult<String> {
    let file_name = validate_entry_name(name)?;
    let rel_path = join_rel(rel_dir, file_name);
    let target = safe_join(server_dir, &rel_path)?;
    if target.exists() {
        return Err(AppError::InvalidInput(format!(
            "{file_name} already exists"
        )));
    }

    std::fs::write(&target, b"")?;
    Ok(rel_path)
}

/// Creates a folder in `rel_dir`. Returns its path relative to the server
/// folder.
pub fn create_dir(server_dir: &Path, rel_dir: &str, name: &str) -> AppResult<String> {
    let dir_name = validate_entry_name(name)?;
    let rel_path = join_rel(rel_dir, dir_name);
    let target = safe_join(server_dir, &rel_path)?;
    if target.exists() {
        return Err(AppError::InvalidInput(format!("{dir_name} already exists")));
    }

    std::fs::create_dir(&target)?;
    Ok(rel_path)
}

/// Renames a file or folder in place, within its own directory. Returns the
/// new path relative to the server folder.
pub fn rename_entry(server_dir: &Path, rel_path: &str, new_name: &str) -> AppResult<String> {
    let file_name = validate_entry_name(new_name)?;
    let source = safe_join(server_dir, rel_path)?;
    if !source.exists() {
        return Err(AppError::InvalidInput(
            "that file no longer exists".to_string(),
        ));
    }

    let parent_rel = match rel_path.rsplit_once('/') {
        Some((parent, _)) => parent,
        None => "",
    };
    let target_rel = join_rel(parent_rel, file_name);
    let target = safe_join(server_dir, &target_rel)?;
    // Renaming `Foo.txt` to `foo.txt` is a real rename on Linux but a no-op
    // comparison on Windows, so only reject a genuinely different entry.
    if target.exists() && target != source {
        return Err(AppError::InvalidInput(format!(
            "{file_name} already exists"
        )));
    }

    std::fs::rename(&source, &target)?;
    Ok(target_rel)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_server_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("serverforge-files-{label}"));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn rejects_names_that_would_escape_the_folder() {
        assert!(validate_entry_name("ops.json").is_ok());
        assert!(validate_entry_name("  spaced.txt  ").is_ok());
        assert!(validate_entry_name("../evil").is_err());
        assert!(validate_entry_name("sub/child").is_err());
        assert!(validate_entry_name("sub\\child").is_err());
        assert!(validate_entry_name("..").is_err());
        assert!(validate_entry_name("   ").is_err());
    }

    #[test]
    fn creates_files_and_folders_without_clobbering() {
        let dir = temp_server_dir("create");

        let rel = create_file(&dir, "", "notes.txt").expect("create file");
        assert_eq!(rel, "notes.txt");
        assert!(dir.join("notes.txt").is_file());
        assert!(create_file(&dir, "", "notes.txt").is_err());

        let dir_rel = create_dir(&dir, "", "plugins").expect("create dir");
        assert_eq!(dir_rel, "plugins");
        assert!(dir.join("plugins").is_dir());

        let nested = create_file(&dir, "plugins", "config.yml").expect("create nested");
        assert_eq!(nested, "plugins/config.yml");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn renames_within_the_same_folder() {
        let dir = temp_server_dir("rename");
        create_dir(&dir, "", "plugins").expect("create dir");
        create_file(&dir, "plugins", "old.yml").expect("create file");

        let renamed = rename_entry(&dir, "plugins/old.yml", "new.yml").expect("rename");
        assert_eq!(renamed, "plugins/new.yml");
        assert!(dir.join("plugins/new.yml").is_file());
        assert!(!dir.join("plugins/old.yml").exists());

        create_file(&dir, "plugins", "taken.yml").expect("create file");
        assert!(rename_entry(&dir, "plugins/new.yml", "taken.yml").is_err());
        assert!(rename_entry(&dir, "plugins/new.yml", "../escape.yml").is_err());

        std::fs::remove_dir_all(&dir).ok();
    }
}
