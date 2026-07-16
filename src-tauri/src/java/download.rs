//! Automatic Java runtime download: fetches the right Eclipse Temurin JRE
//! from the Adoptium API and unpacks it into the app's managed Java folder.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::installers::{download_file, ExpectedChecksum, ProgressCallback};
use crate::java::{self, JavaInstall};

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

/// Downloads and unpacks a Temurin JRE with the given major version, then
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

    extract_archive(archive_path.clone(), managed_java_dir.to_path_buf()).await?;
    remove_archive_best_effort(&archive_path);

    find_installed(required_major, managed_java_dir).await
}

async fn latest_package(
    client: &reqwest::Client,
    required_major: u32,
) -> AppResult<AdoptiumPackage> {
    let url = format!(
        "{ADOPTIUM_API_BASE}/{required_major}/hotspot?os={}&architecture={}&image_type=jre&vendor=eclipse",
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
        eprintln!("failed to remove {}: {error}", archive_path.display());
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
