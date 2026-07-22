//! Server backups: zipping a server directory into the app's backups folder,
//! listing, restoring, and deleting archives.

use std::io::{Seek, Write};
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

/// Progress of an ongoing backup, emitted as `server:backup-progress`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupProgressEvent {
    pub server_id: String,
    pub processed_files: u64,
    pub total_files: u64,
}

/// Called with (processed_files, total_files) as zipping advances.
pub type ProgressCallback = Box<dyn Fn(u64, u64) + Send + Sync>;

/// Zips the entire server directory. Runs on a blocking thread because zip
/// I/O is synchronous.
pub async fn create(
    server_dir: PathBuf,
    backups_dir: PathBuf,
    report_progress: ProgressCallback,
) -> AppResult<BackupInfo> {
    let result = tokio::task::spawn_blocking(move || {
        create_sync(&server_dir, &backups_dir, &report_progress)
    })
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

fn create_sync(
    server_dir: &Path,
    backups_dir: &Path,
    report_progress: &ProgressCallback,
) -> AppResult<BackupInfo> {
    std::fs::create_dir_all(backups_dir)?;

    let file_name = format!(
        "backup-{}.zip",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
    );
    let archive_path = backups_dir.join(&file_name);

    // The zip is built in a sibling `.part` file and only renamed into place
    // once it is complete. `list` matches on the `.zip` suffix, so a backup
    // still being written is never listed — and therefore can't be restored
    // over a good world while it sits there at zero bytes.
    let partial_path = archive_path.with_extension("zip.part");
    if let Err(error) = write_archive(server_dir, backups_dir, &partial_path, report_progress) {
        discard_partial(&partial_path);
        return Err(error);
    }
    std::fs::rename(&partial_path, &archive_path)?;

    let metadata = std::fs::metadata(&archive_path)?;
    let info = BackupInfo {
        file_name,
        size_bytes: metadata.len(),
        created_at_unix: modified_unix_time(&metadata),
    };
    Ok(info)
}

/// Creates the archive file and zips the server directory into it.
fn write_archive(
    server_dir: &Path,
    backups_dir: &Path,
    destination_path: &Path,
    report_progress: &ProgressCallback,
) -> AppResult<()> {
    let archive_file = std::fs::File::create(destination_path)?;
    zip_directory(server_dir, backups_dir, archive_file, report_progress)
}

/// Removes a half-written archive. Best effort: the backup has already
/// failed, and a leftover `.part` file is never listed or restorable, so a
/// cleanup failure is worth a log line rather than masking the real error.
fn discard_partial(partial_path: &Path) {
    if let Err(error) = std::fs::remove_file(partial_path) {
        log::warn!(
            "failed to remove partial backup {}: {error}",
            partial_path.display()
        );
    }
}

/// Zips `source_dir`, skipping `excluded_dir` (the backups folder itself,
/// which by default lives inside the server directory — otherwise every
/// backup would swallow all previous ones).
fn zip_directory<W: Write + Seek>(
    source_dir: &Path,
    excluded_dir: &Path,
    destination: W,
    report_progress: &ProgressCallback,
) -> AppResult<()> {
    let mut writer = zip::ZipWriter::new(destination);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Compared in canonical form: the excluded folder may be an override
    // spelled differently from the paths walked out of `source_dir`, and a
    // mismatch here would zip the backups folder into every new backup.
    // Only directories are resolved — a file is never the excluded folder,
    // and canonicalizing every file would cost a syscall each.
    let excluded_dir = comparable_path(excluded_dir);
    let walker = walkdir::WalkDir::new(source_dir)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }
            let is_excluded = comparable_path(entry.path()) == excluded_dir;
            !is_excluded
        });

    // Collected up front so the total is known before the first file is
    // written — a progress bar needs a denominator, and walking a server
    // folder twice would double the I/O for a large world.
    let entries: Vec<walkdir::DirEntry> = walker
        .collect::<Result<Vec<_>, _>>()
        .map_err(|walk_error| AppError::Process(walk_error.to_string()))?;
    let total_files = entries.len() as u64;

    for (index, entry) in entries.iter().enumerate() {
        add_zip_entry(&mut writer, source_dir, entry.path(), options)?;
        report_progress(index as u64 + 1, total_files);
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

    // The world lock is held (and range-locked on Windows) while the server
    // runs — it's meaningless in a backup anyway.
    if entry_path
        .file_name()
        .is_some_and(|name| name == "session.lock")
    {
        return Ok(());
    }

    // Zip entries always use forward slashes, regardless of platform.
    let entry_name = relative.to_string_lossy().replace('\\', "/");

    if entry_path.is_dir() {
        writer.add_directory(entry_name, options)?;
        return Ok(());
    }

    // Read the whole file up front: a live server may hold byte-range locks
    // (os error 33 on Windows), and reading first means a locked file skips
    // cleanly instead of aborting mid-archive with a corrupt zip.
    let contents = match std::fs::read(entry_path) {
        Ok(bytes) => bytes,
        Err(read_error) => {
            log::warn!(
                "backup: skipping locked/unreadable file {}: {read_error}",
                entry_path.display()
            );
            return Ok(());
        }
    };

    writer.start_file(entry_name, options)?;
    write_into_zip(&contents, writer)?;
    Ok(())
}

fn write_into_zip<W: Write + Seek>(
    contents: &[u8],
    writer: &mut zip::ZipWriter<W>,
) -> AppResult<()> {
    writer.write_all(contents)?;
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

/// Resolves a path to a form safe to compare against another. The backups
/// folder may be a per-server override chosen in a folder dialog, while the
/// paths it is compared against are built by joining onto the server
/// directory — the same location can be spelled with different casing or
/// separators, which `PathBuf` equality treats as different paths. Falls back
/// to the path as given when it cannot be resolved (it may not exist yet).
fn comparable_path(path: &Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(resolved) => resolved,
        Err(_) => path.to_path_buf(),
    }
}

/// Empties the server folder before extraction — except the backups folder.
/// Restoring one backup must never destroy the others (or the archive
/// currently being read).
fn clear_server_dir_except_backups(server_dir: &Path, backups_dir: &Path) -> AppResult<()> {
    if !server_dir.exists() {
        return Ok(());
    }

    let protected_dir = comparable_path(backups_dir);
    clear_dir_around(server_dir, &protected_dir)
}

/// Removes everything under `dir` except `protected_dir`. A folder that
/// *contains* the backups folder is emptied around it rather than deleted,
/// so an override pointing somewhere nested inside the server directory
/// still survives a restore.
fn clear_dir_around(dir: &Path, protected_dir: &Path) -> AppResult<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let entry_path = entry.path();

        if !entry.file_type()?.is_dir() {
            std::fs::remove_file(&entry_path)?;
            continue;
        }

        let comparable_entry = comparable_path(&entry_path);
        if comparable_entry == *protected_dir {
            continue;
        }

        if protected_dir.starts_with(&comparable_entry) {
            clear_dir_around(&entry_path, protected_dir)?;
            continue;
        }

        std::fs::remove_dir_all(&entry_path)?;
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

    /// Most tests don't care about progress reporting.
    fn ignore_progress() -> ProgressCallback {
        Box::new(|_processed, _total| {})
    }

    /// A backup still being written must not appear in the list. It shows as
    /// a zero-byte archive while it grows, and restoring one would overwrite
    /// a good world with a truncated one.
    #[test]
    fn a_backup_in_progress_is_never_listed() {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::Arc;

        let temp_root =
            std::env::temp_dir().join(format!("serverforge-test-{}", uuid::Uuid::new_v4()));
        let server_dir = temp_root.join("server");
        let backups_dir = server_dir.join("backups");
        std::fs::create_dir_all(server_dir.join("world")).expect("create world dir");
        for index in 0..8 {
            std::fs::write(
                server_dir.join("world").join(format!("r{index}.mca")),
                "chunk",
            )
            .expect("write region file");
        }

        // Probe the listing from inside the backup, while the archive is
        // still being written.
        let probed_dir = backups_dir.clone();
        let most_seen_mid_backup = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&most_seen_mid_backup);
        let probe: ProgressCallback = Box::new(move |_processed, _total| {
            let listed = list(&probed_dir).expect("list during backup");
            counter.fetch_max(listed.len() as u64, Ordering::Relaxed);
        });

        create_sync(&server_dir, &backups_dir, &probe).expect("create backup");

        assert_eq!(
            most_seen_mid_backup.load(Ordering::Relaxed),
            0,
            "a backup must not be listed until it is complete"
        );
        assert_eq!(
            list(&backups_dir).expect("list after backup").len(),
            1,
            "the finished backup must be listed once it is renamed into place"
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup");
    }

    /// The backups folder lives inside the server folder by default, so the
    /// zipper must skip it — otherwise every backup would contain all
    /// previous backups and grow without bound.
    #[test]
    fn zipping_excludes_the_backups_folder() {
        let temp_root =
            std::env::temp_dir().join(format!("serverforge-test-{}", uuid::Uuid::new_v4()));
        let server_dir = temp_root.join("server");
        let backups_dir = server_dir.join("backups");
        std::fs::create_dir_all(server_dir.join("world")).expect("create world dir");
        std::fs::create_dir_all(&backups_dir).expect("create backups dir");
        std::fs::write(server_dir.join("server.properties"), "pvp=true").expect("write props");
        std::fs::write(server_dir.join("world").join("level.dat"), "data").expect("write level");
        std::fs::write(backups_dir.join("backup-old.zip"), "old archive").expect("write old zip");

        let mut buffer = Cursor::new(Vec::new());
        zip_directory(&server_dir, &backups_dir, &mut buffer, &ignore_progress()).expect("zip");

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
            std::env::temp_dir().join(format!("serverforge-test-{}", uuid::Uuid::new_v4()));
        let server_dir = temp_root.join("server");
        let backups_dir = server_dir.join("backups");
        std::fs::create_dir_all(server_dir.join("world")).expect("create world dir");
        std::fs::create_dir_all(&backups_dir).expect("create backups dir");
        std::fs::write(server_dir.join("world").join("level.dat"), "good world")
            .expect("write level");

        // Take a real backup of the good state.
        let good_backup =
            create_sync(&server_dir, &backups_dir, &ignore_progress()).expect("create backup");
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

    /// A per-server override can put the backups folder deeper inside the
    /// server directory. The folder holding it must be emptied around it
    /// rather than deleted wholesale, or restoring destroys every backup.
    #[test]
    fn restoring_preserves_a_backups_folder_nested_deeper() {
        let temp_root =
            std::env::temp_dir().join(format!("serverforge-test-{}", uuid::Uuid::new_v4()));
        let server_dir = temp_root.join("server");
        let backups_dir = server_dir.join("saves").join("backups");
        std::fs::create_dir_all(server_dir.join("world")).expect("create world dir");
        std::fs::create_dir_all(&backups_dir).expect("create backups dir");
        std::fs::write(server_dir.join("world").join("level.dat"), "good world")
            .expect("write level");

        let good_backup =
            create_sync(&server_dir, &backups_dir, &ignore_progress()).expect("create backup");
        std::fs::write(server_dir.join("world").join("level.dat"), "griefed world")
            .expect("corrupt level");
        // Written after the backup, so it is not in the archive: a sibling of
        // the backups folder that the restore is expected to clear away.
        std::fs::write(server_dir.join("saves").join("stale.dat"), "stale")
            .expect("write stale file");

        let archive_path = backups_dir.join(&good_backup.file_name);
        restore_sync(&server_dir, &backups_dir, &archive_path).expect("restore");

        assert!(
            archive_path.exists(),
            "a nested backups folder must survive a restore"
        );
        let restored = std::fs::read_to_string(server_dir.join("world").join("level.dat"))
            .expect("read restored level");
        assert_eq!(restored, "good world");
        assert!(
            !server_dir.join("saves").join("stale.dat").exists(),
            "the rest of the folder holding the backups must still be cleared"
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup");
    }

    /// The backups folder is compared against paths built from the server
    /// directory. An override that spells the same location differently must
    /// still be recognised, or it gets zipped into every new backup.
    #[test]
    fn zipping_excludes_a_backups_folder_spelled_differently() {
        let temp_root =
            std::env::temp_dir().join(format!("serverforge-test-{}", uuid::Uuid::new_v4()));
        let server_dir = temp_root.join("server");
        let backups_dir = server_dir.join("backups");
        std::fs::create_dir_all(&backups_dir).expect("create backups dir");
        std::fs::write(server_dir.join("server.properties"), "pvp=true").expect("write props");
        std::fs::write(backups_dir.join("backup-old.zip"), "old archive").expect("write old zip");

        // Same folder, non-canonical spelling — as a dialog-chosen override
        // can easily produce.
        let spelled_differently = server_dir.join(".").join("backups");

        let mut buffer = Cursor::new(Vec::new());
        zip_directory(
            &server_dir,
            &spelled_differently,
            &mut buffer,
            &ignore_progress(),
        )
        .expect("zip");

        let mut archive = zip::ZipArchive::new(buffer).expect("open archive");
        let entry_names: Vec<String> = (0..archive.len())
            .map(|index| archive.by_index(index).expect("entry").name().to_string())
            .collect();

        assert!(entry_names.iter().any(|name| name == "server.properties"));
        let contains_backups = entry_names.iter().any(|name| name.starts_with("backups"));
        assert!(
            !contains_backups,
            "archive must not contain the backups folder: {entry_names:?}"
        );

        std::fs::remove_dir_all(&temp_root).expect("cleanup");
    }
}
