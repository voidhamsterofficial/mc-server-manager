//! Server backups: zipping a server directory into the app's backups folder,
//! listing, restoring, and deleting archives.

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::error::{AppError, AppResult};

/// One archive on disk, as listed in the Backups tab.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub file_name: String,
    pub size_bytes: u64,
    pub created_at_unix: u64,
}

/// Zips the entire server directory. Runs on a blocking thread because zip
/// I/O is synchronous.
pub async fn create(server_dir: PathBuf, backups_dir: PathBuf) -> AppResult<BackupInfo> {
    let result = tokio::task::spawn_blocking(move || create_sync(&server_dir, &backups_dir))
        .await
        .map_err(|join_error| AppError::Process(join_error.to_string()))?;
    result
}

/// Replaces the server directory's contents with the chosen archive,
/// preserving the backups folder. Callers must ensure the server is
/// stopped first.
pub async fn restore(
    server_dir: PathBuf,
    backups_dir: PathBuf,
    archive_path: PathBuf,
) -> AppResult<()> {
    let result =
        tokio::task::spawn_blocking(move || restore_sync(&server_dir, &backups_dir, &archive_path))
            .await
            .map_err(|join_error| AppError::Process(join_error.to_string()))?;
    result
}

pub fn list(backups_dir: &Path) -> AppResult<Vec<BackupInfo>> {
    if !backups_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();
    for entry in std::fs::read_dir(backups_dir)? {
        let entry = entry?;
        let Some(info) = describe_archive(&entry) else {
            continue;
        };
        backups.push(info);
    }

    backups.sort_by_key(|backup| std::cmp::Reverse(backup.created_at_unix));
    Ok(backups)
}

/// Deletes the oldest archives beyond `keep_newest`. Returns how many were
/// removed.
pub fn prune(backups_dir: &Path, keep_newest: u32) -> AppResult<u32> {
    let archives = list(backups_dir)?;

    let mut removed_count = 0;
    for stale in archives.iter().skip(keep_newest as usize) {
        delete(backups_dir, &stale.file_name)?;
        removed_count += 1;
    }
    Ok(removed_count)
}

pub fn delete(backups_dir: &Path, file_name: &str) -> AppResult<()> {
    let archive_path = safe_archive_path(backups_dir, file_name)?;
    std::fs::remove_file(archive_path)?;
    Ok(())
}

/// Resolves a backup file name inside the backups directory, rejecting names
/// that try to escape it.
pub fn safe_archive_path(backups_dir: &Path, file_name: &str) -> AppResult<PathBuf> {
    let is_plain_file_name = !file_name.contains('/')
        && !file_name.contains('\\')
        && !file_name.contains("..")
        && file_name.ends_with(".zip");
    if !is_plain_file_name {
        let message = format!("invalid backup file name: {file_name}");
        return Err(AppError::InvalidInput(message));
    }

    let archive_path = backups_dir.join(file_name);
    Ok(archive_path)
}

fn create_sync(server_dir: &Path, backups_dir: &Path) -> AppResult<BackupInfo> {
    std::fs::create_dir_all(backups_dir)?;

    let file_name = format!(
        "backup-{}.zip",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
    );
    let archive_path = backups_dir.join(&file_name);

    let archive_file = std::fs::File::create(&archive_path)?;
    zip_directory(server_dir, backups_dir, archive_file)?;

    let metadata = std::fs::metadata(&archive_path)?;
    let info = BackupInfo {
        file_name,
        size_bytes: metadata.len(),
        created_at_unix: modified_unix_time(&metadata),
    };
    Ok(info)
}

/// Zips `source_dir`, skipping `excluded_dir` (the backups folder itself,
/// which by default lives inside the server directory — otherwise every
/// backup would swallow all previous ones).
fn zip_directory<W: Write + Seek>(
    source_dir: &Path,
    excluded_dir: &Path,
    destination: W,
) -> AppResult<()> {
    let mut writer = zip::ZipWriter::new(destination);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let walker = walkdir::WalkDir::new(source_dir)
        .into_iter()
        .filter_entry(|entry| entry.path() != excluded_dir);
    for entry in walker {
        let entry = entry.map_err(|walk_error| AppError::Process(walk_error.to_string()))?;
        add_zip_entry(&mut writer, source_dir, entry.path(), options)?;
    }

    writer.finish()?;
    Ok(())
}

fn add_zip_entry<W: Write + Seek>(
    writer: &mut zip::ZipWriter<W>,
    source_dir: &Path,
    entry_path: &Path,
    options: zip::write::SimpleFileOptions,
) -> AppResult<()> {
    let relative = entry_path
        .strip_prefix(source_dir)
        .map_err(|strip_error| AppError::Process(strip_error.to_string()))?;
    if relative.as_os_str().is_empty() {
        return Ok(());
    }

    // Zip entries always use forward slashes, regardless of platform.
    let entry_name = relative.to_string_lossy().replace('\\', "/");

    if entry_path.is_dir() {
        writer.add_directory(entry_name, options)?;
        return Ok(());
    }

    writer.start_file(entry_name, options)?;
    let mut source_file = std::fs::File::open(entry_path)?;
    copy_into_zip(&mut source_file, writer)?;
    Ok(())
}

fn copy_into_zip<R: Read, W: Write + Seek>(
    source: &mut R,
    writer: &mut zip::ZipWriter<W>,
) -> AppResult<()> {
    std::io::copy(source, writer)?;
    Ok(())
}

fn restore_sync(server_dir: &Path, backups_dir: &Path, archive_path: &Path) -> AppResult<()> {
    let archive_file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(archive_file)?;

    clear_server_dir_except_backups(server_dir, backups_dir)?;
    std::fs::create_dir_all(server_dir)?;

    // Archives never contain a backups folder (excluded at creation), so
    // extraction cannot overwrite it either.
    archive.extract(server_dir)?;
    Ok(())
}

/// Empties the server folder before extraction — except the backups folder.
/// Restoring one backup must never destroy the others (or the archive
/// currently being read).
fn clear_server_dir_except_backups(server_dir: &Path, backups_dir: &Path) -> AppResult<()> {
    if !server_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(server_dir)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path == backups_dir {
            continue;
        }
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(&entry_path)?;
        } else {
            std::fs::remove_file(&entry_path)?;
        }
    }
    Ok(())
}

fn describe_archive(entry: &std::fs::DirEntry) -> Option<BackupInfo> {
    let file_name = entry.file_name().to_string_lossy().to_string();
    if !file_name.ends_with(".zip") {
        return None;
    }

    let metadata = entry.metadata().ok()?;
    let info = BackupInfo {
        file_name,
        size_bytes: metadata.len(),
        created_at_unix: modified_unix_time(&metadata),
    };
    Some(info)
}

fn modified_unix_time(metadata: &std::fs::Metadata) -> u64 {
    let modified = metadata.modified().ok();
    let since_epoch = modified.and_then(|time| time.duration_since(UNIX_EPOCH).ok());
    let seconds = since_epoch.map(|duration| duration.as_secs());
    seconds.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// The backups folder lives inside the server folder by default, so the
    /// zipper must skip it — otherwise every backup would contain all
    /// previous backups and grow without bound.
    #[test]
    fn zipping_excludes_the_backups_folder() {
        let temp_root =
            std::env::temp_dir().join(format!("blockparty-test-{}", uuid::Uuid::new_v4()));
        let server_dir = temp_root.join("server");
        let backups_dir = server_dir.join("backups");
        std::fs::create_dir_all(server_dir.join("world")).expect("create world dir");
        std::fs::create_dir_all(&backups_dir).expect("create backups dir");
        std::fs::write(server_dir.join("server.properties"), "pvp=true").expect("write props");
        std::fs::write(server_dir.join("world").join("level.dat"), "data").expect("write level");
        std::fs::write(backups_dir.join("backup-old.zip"), "old archive").expect("write old zip");

        let mut buffer = Cursor::new(Vec::new());
        zip_directory(&server_dir, &backups_dir, &mut buffer).expect("zip");

        let mut archive = zip::ZipArchive::new(buffer).expect("open archive");
        let entry_names: Vec<String> = (0..archive.len())
            .map(|index| archive.by_index(index).expect("entry").name().to_string())
            .collect();

        assert!(entry_names.iter().any(|name| name == "server.properties"));
        assert!(entry_names.iter().any(|name| name == "world/level.dat"));
        let contains_old_backups = entry_names.iter().any(|name| name.starts_with("backups"));
        assert!(
            !contains_old_backups,
            "archive must not contain the backups folder: {entry_names:?}"
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup");
    }

    /// Restoring wipes the server folder to make room for the archive — but
    /// never the backups folder, or one restore would delete every backup.
    #[test]
    fn restoring_preserves_the_backups_folder() {
        let temp_root =
            std::env::temp_dir().join(format!("blockparty-test-{}", uuid::Uuid::new_v4()));
        let server_dir = temp_root.join("server");
        let backups_dir = server_dir.join("backups");
        std::fs::create_dir_all(server_dir.join("world")).expect("create world dir");
        std::fs::create_dir_all(&backups_dir).expect("create backups dir");
        std::fs::write(server_dir.join("world").join("level.dat"), "good world")
            .expect("write level");

        // Take a real backup of the good state.
        let good_backup = create_sync(&server_dir, &backups_dir).expect("create backup");
        std::fs::write(backups_dir.join("older-backup.zip"), "unrelated archive")
            .expect("write older backup");

        // The world goes bad; restore the good backup over it.
        std::fs::write(server_dir.join("world").join("level.dat"), "griefed world")
            .expect("corrupt level");
        let archive_path = backups_dir.join(&good_backup.file_name);
        restore_sync(&server_dir, &backups_dir, &archive_path).expect("restore");

        let restored = std::fs::read_to_string(server_dir.join("world").join("level.dat"))
            .expect("read restored level");
        assert_eq!(restored, "good world");
        assert!(
            archive_path.exists(),
            "the restored archive itself must survive"
        );
        assert!(
            backups_dir.join("older-backup.zip").exists(),
            "other backups must survive a restore"
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup");
    }
}
