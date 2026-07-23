//! Automatic Java runtime download: fetches the right Eclipse Temurin JDK
//! from the Adoptium API and unpacks it into the app's managed Java folder.
//!
//! This deliberately downloads the full JDK, not the smaller JRE: Adoptium's
//! `jre` image type is jlink-trimmed down to the modules its static analysis
//! can see a JVM needs, which misses `jdk.crypto.ec` — it's only ever pulled
//! in via a security-provider service lookup, not a normal `requires`. Forge
//! and NeoForge's bootstrapper needs that module (for the EC crypto used to
//! validate Mojang auth JWTs via nimbus-jose-jwt) and throws
//! `java.lang.module.FindException: Module jdk.crypto.ec not found` on the
//! trimmed JRE. The full JDK isn't jlink-reduced, so it always has it.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::installers::{download_file, ExpectedChecksum, ProgressCallback};
use crate::java::{self, JavaInstall};
use crate::storage::fsutil::remove_dir_best_effort;

/// Where a JDK is unpacked before being moved into place.
///
/// Unpacking straight into the managed folder is not safe to interrupt: the
/// archive lists `bin/java.exe` before the `bin/jli.dll` it needs, so a run
/// that dies partway (app closed, disk full, antivirus) leaves a runtime that
/// *looks* installed but cannot execute. Detection would then keep probing it
/// on every start. Staging keeps a partial unpack invisible until it is
/// complete: discovery only looks one level down, at `<entry>/bin/java`, and
/// a staged runtime sits a level deeper than that.
const STAGING_DIR_NAME: &str = ".staging";

const ADOPTIUM_API_BASE: &str = "https://api.adoptium.net/v3/assets/latest";
const ADOPTIUM_RELEASES_URL: &str = "https://api.adoptium.net/v3/info/available_releases";

#[derive(Debug, Deserialize)]
struct AvailableReleases {
    most_recent_lts: u32,
}

/// The newest LTS Java major Adoptium ships — what unmapped (newest)
/// Minecraft versions should run on.
pub async fn most_recent_lts(client: &reqwest::Client) -> AppResult<u32> {
    let releases: AvailableReleases = client
        .get(ADOPTIUM_RELEASES_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(releases.most_recent_lts)
}

#[derive(Debug, Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}

#[derive(Debug, Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Debug, Deserialize)]
struct AdoptiumPackage {
    link: String,
    name: String,
    /// SHA-256 of the archive, hex-encoded.
    checksum: String,
}

/// Downloads and unpacks a Temurin JDK with the given major version, then
/// returns the freshly usable install.
pub async fn install_temurin(
    client: &reqwest::Client,
    required_major: u32,
    managed_java_dir: &Path,
    report_progress: &ProgressCallback,
) -> AppResult<JavaInstall> {
    let package = latest_package(client, required_major).await?;

    let archive_path = managed_java_dir.join(&package.name);
    download_file(
        client,
        &package.link,
        &archive_path,
        ExpectedChecksum::Sha256(&package.checksum),
        report_progress,
    )
    .await?;

    let unpacked = unpack_into_place(&archive_path, managed_java_dir).await;
    remove_archive_best_effort(&archive_path);
    unpacked?;

    find_installed(required_major, managed_java_dir).await
}

/// Unpacks the archive via a staging directory, so the managed folder only
/// ever gains a runtime that finished unpacking.
async fn unpack_into_place(archive_path: &Path, managed_java_dir: &Path) -> AppResult<()> {
    let staging_dir = managed_java_dir.join(STAGING_DIR_NAME);
    // A leftover from an attempt that died partway would otherwise be
    // published as if it were this download's output.
    remove_dir_best_effort(&staging_dir);
    std::fs::create_dir_all(&staging_dir)?;

    let extracted = extract_archive(archive_path.to_path_buf(), staging_dir.clone()).await;
    if let Err(extract_error) = extracted {
        remove_dir_best_effort(&staging_dir);
        return Err(extract_error);
    }

    let published = publish_staged_runtimes(&staging_dir, managed_java_dir);
    remove_dir_best_effort(&staging_dir);
    published
}

/// Moves each unpacked runtime out of staging and into the managed folder,
/// replacing any earlier copy of the same runtime (which may be the broken
/// one this download exists to repair).
fn publish_staged_runtimes(staging_dir: &Path, managed_java_dir: &Path) -> AppResult<()> {
    // Collected before the loop starts moving things: publishing displaces
    // replaced runtimes back into the staging directory, and a directory
    // being read must not gain entries that would then be published in turn.
    let staged_runtimes = staged_runtimes(staging_dir)?;

    for staged_runtime in staged_runtimes {
        let destination = managed_java_dir.join(&staged_runtime.name);
        publish_one_runtime(&staged_runtime, &destination, staging_dir)?;
    }
    Ok(())
}

/// One unpacked runtime waiting to be published.
struct StagedRuntime {
    path: PathBuf,
    name: std::ffi::OsString,
}

fn staged_runtimes(staging_dir: &Path) -> AppResult<Vec<StagedRuntime>> {
    let mut runtimes = Vec::new();

    for entry in std::fs::read_dir(staging_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        // Nothing an Adoptium archive contains is named this, but publishing
        // an entry called `.staging` would move the staging directory into
        // itself, so refuse it rather than trust the archive's layout.
        if name == STAGING_DIR_NAME {
            log::warn!("ignoring a Java archive entry named {STAGING_DIR_NAME}");
            continue;
        }
        runtimes.push(StagedRuntime {
            path: entry.path(),
            name,
        });
    }
    Ok(runtimes)
}

/// Puts one unpacked runtime at its final path.
///
/// Any runtime already there is moved aside and only deleted once the new one
/// is in place. Deleting it where it stands would be the more obvious order,
/// but `remove_dir_all` can stop partway — a JVM still running from that
/// folder, an antivirus holding a file open — and a half-deleted runtime is
/// exactly the broken install this whole path exists to prevent. Moving is a
/// single atomic operation: it either happens or it doesn't.
fn publish_one_runtime(
    staged_runtime: &StagedRuntime,
    destination: &Path,
    staging_dir: &Path,
) -> AppResult<()> {
    if !destination.exists() {
        std::fs::rename(&staged_runtime.path, destination)?;
        return Ok(());
    }

    // Kept inside staging so it is invisible to runtime discovery (which only
    // looks one level down) and swept up by the staging cleanup either now or
    // on the next attempt.
    let mut displaced_name = staged_runtime.name.clone();
    displaced_name.push(".replaced");
    let displaced = staging_dir.join(displaced_name);
    std::fs::rename(destination, &displaced)?;

    if let Err(rename_error) = std::fs::rename(&staged_runtime.path, destination) {
        // Put the old runtime back: a broken Java is still better than the
        // empty space a failed swap would otherwise leave behind.
        if let Err(restore_error) = std::fs::rename(&displaced, destination) {
            log::error!(
                "could not restore {} after a failed swap: {restore_error}",
                destination.display()
            );
        }
        return Err(rename_error.into());
    }

    remove_dir_best_effort(&displaced);
    Ok(())
}

async fn latest_package(
    client: &reqwest::Client,
    required_major: u32,
) -> AppResult<AdoptiumPackage> {
    let url = format!(
        "{ADOPTIUM_API_BASE}/{required_major}/hotspot?os={}&architecture={}&image_type=jdk&vendor=eclipse",
        adoptium_os_name(),
        adoptium_architecture(),
    );

    let assets: Vec<AdoptiumAsset> = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let first_asset = assets
        .into_iter()
        .next()
        .ok_or(AppError::NoSuitableJava { required_major })?;
    Ok(first_asset.binary.package)
}

fn adoptium_os_name() -> &'static str {
    if cfg!(windows) {
        return "windows";
    }
    if cfg!(target_os = "macos") {
        return "mac";
    }
    "linux"
}

fn adoptium_architecture() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        return "aarch64";
    }
    "x64"
}

/// Unpacks a `.zip` (Windows builds) or `.tar.gz` (macOS/Linux builds)
/// archive into the managed Java directory on a blocking thread.
async fn extract_archive(archive_path: PathBuf, destination: PathBuf) -> AppResult<()> {
    let result = tokio::task::spawn_blocking(move || extract_sync(&archive_path, &destination))
        .await
        .map_err(|join_error| AppError::Process(join_error.to_string()))?;
    result
}

fn extract_sync(archive_path: &Path, destination: &Path) -> AppResult<()> {
    let file_name = archive_path.to_string_lossy();
    if file_name.ends_with(".zip") {
        return extract_zip(archive_path, destination);
    }
    if file_name.ends_with(".tar.gz") {
        return extract_tar_gz(archive_path, destination);
    }

    let message = format!("unsupported Java archive format: {file_name}");
    Err(AppError::Process(message))
}

fn extract_zip(archive_path: &Path, destination: &Path) -> AppResult<()> {
    let archive_file = std::fs::File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(archive_file)?;
    archive.extract(destination)?;
    Ok(())
}

fn extract_tar_gz(archive_path: &Path, destination: &Path) -> AppResult<()> {
    let archive_file = std::fs::File::open(archive_path)?;
    let decompressed = flate2::read::GzDecoder::new(archive_file);
    let mut archive = tar::Archive::new(decompressed);
    archive.unpack(destination)?;
    Ok(())
}

fn remove_archive_best_effort(archive_path: &Path) {
    if let Err(error) = std::fs::remove_file(archive_path) {
        log::warn!("failed to remove {}: {error}", archive_path.display());
    }
}

/// Probes the managed directory for the runtime that was just unpacked.
async fn find_installed(required_major: u32, managed_java_dir: &Path) -> AppResult<JavaInstall> {
    let installs = java::detect_installs(managed_java_dir).await;
    let matching = installs
        .into_iter()
        .find(|install| install.major_version == required_major);
    matching.ok_or(AppError::NoSuitableJava { required_major })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_managed_java_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("serverforge-java-{label}"));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// Writes a runtime-shaped folder: `<root>/bin/java.exe` plus whichever
    /// libraries the caller wants next to it.
    fn write_runtime(root: &Path, java_contents: &str, libraries: &[&str]) {
        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).expect("create bin");
        std::fs::write(bin.join("java.exe"), java_contents).expect("write java");
        for library in libraries {
            std::fs::write(bin.join(library), "library").expect("write library");
        }
    }

    #[test]
    fn publishing_installs_a_runtime_that_is_not_there_yet() {
        let managed_java_dir = temp_managed_java_dir("publish-fresh");
        let staging_dir = managed_java_dir.join(STAGING_DIR_NAME);
        write_runtime(&staging_dir.join("jdk-21"), "fresh", &["jli.dll"]);

        publish_staged_runtimes(&staging_dir, &managed_java_dir).expect("publish");

        let published_bin = managed_java_dir.join("jdk-21").join("bin");
        assert!(published_bin.join("java.exe").is_file());
        assert!(published_bin.join("jli.dll").is_file());

        std::fs::remove_dir_all(&managed_java_dir).ok();
    }

    /// The repair path: a runtime left broken by an interrupted unpack must be
    /// replaced wholesale rather than merged with, so no stale file survives —
    /// and the replaced copy must end up inside staging, where discovery can't
    /// see it and cleanup will collect it.
    #[test]
    fn publishing_replaces_a_broken_runtime_of_the_same_name() {
        let managed_java_dir = temp_managed_java_dir("publish-replaces");
        write_runtime(&managed_java_dir.join("jdk-21"), "stale", &["leftover.dll"]);

        let staging_dir = managed_java_dir.join(STAGING_DIR_NAME);
        write_runtime(&staging_dir.join("jdk-21"), "fresh", &["jli.dll"]);

        publish_staged_runtimes(&staging_dir, &managed_java_dir).expect("publish");

        let published_bin = managed_java_dir.join("jdk-21").join("bin");
        let java_contents =
            std::fs::read_to_string(published_bin.join("java.exe")).expect("read java");
        assert_eq!(java_contents, "fresh");
        assert!(published_bin.join("jli.dll").is_file());
        assert!(
            !published_bin.join("leftover.dll").exists(),
            "the replaced runtime's files must not survive alongside the new one"
        );

        std::fs::remove_dir_all(&managed_java_dir).ok();
    }

    /// An archive entry named like the staging directory would otherwise make
    /// publishing move staging into itself.
    #[test]
    fn publishing_refuses_to_move_the_staging_directory() {
        let managed_java_dir = temp_managed_java_dir("publish-staging-name");
        let staging_dir = managed_java_dir.join(STAGING_DIR_NAME);
        write_runtime(&staging_dir.join(STAGING_DIR_NAME), "hostile", &[]);
        write_runtime(&staging_dir.join("jdk-21"), "fresh", &["jli.dll"]);

        publish_staged_runtimes(&staging_dir, &managed_java_dir).expect("publish");

        assert!(staging_dir.is_dir(), "staging must still be where it was");
        assert!(managed_java_dir.join("jdk-21").join("bin").is_dir());

        std::fs::remove_dir_all(&managed_java_dir).ok();
    }
}
