//! Small filesystem helpers shared across the persistence modules.

use std::path::{Path, PathBuf};

use crate::error::AppResult;

/// Writes `contents` to `path` atomically: write a sibling temp file, then
/// rename it over the target. `rename` is atomic on the same filesystem, so a
/// crash or power loss mid-write leaves the previous file intact instead of a
/// half-written, corrupt one (roster, settings, and scheduled-task JSON all
/// rely on this).
pub fn atomic_write(path: &Path, contents: &[u8]) -> AppResult<()> {
    let mut temp = path.as_os_str().to_os_string();
    temp.push(".tmp");
    let temp_path = PathBuf::from(temp);

    std::fs::write(&temp_path, contents)?;
    if let Err(error) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(error.into());
    }
    Ok(())
}

/// Deletes a directory tree if it is there, logging rather than failing.
///
/// Only for scratch directories the caller can carry on without: a half-built
/// server folder, a staging area, a runtime already replaced. Never call it on
/// something a later step depends on — `remove_dir_all` can stop partway (a
/// file locked by antivirus or a running process), and a partly deleted
/// directory still exists and still looks usable to anything that finds it.
pub fn remove_dir_best_effort(directory: &Path) {
    if !directory.exists() {
        return;
    }
    if let Err(error) = std::fs::remove_dir_all(directory) {
        log::warn!("failed to clean up {}: {error}", directory.display());
    }
}
