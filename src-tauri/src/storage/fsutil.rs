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
