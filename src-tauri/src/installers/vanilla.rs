//! Vanilla server installation via Mojang's piston-meta version manifest.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::installers::{download_file, ExpectedChecksum, ProgressCallback};

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// The file name every managed server's launch jar is stored under.
pub const SERVER_JAR_NAME: &str = "server.jar";

/// One available Minecraft version, as shown in the create-server wizard.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub release_time: String,
}

#[derive(Debug, Deserialize)]
struct VersionManifest {
    versions: Vec<ManifestVersion>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestVersion {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    url: String,
    release_time: String,
}

#[derive(Debug, Deserialize)]
struct VersionDetails {
    downloads: VersionDownloads,
}

#[derive(Debug, Deserialize)]
struct VersionDownloads {
    server: Option<DownloadEntry>,
}

#[derive(Debug, Deserialize)]
struct DownloadEntry {
    url: String,
    sha1: String,
}

/// Lists every version Mojang publishes, newest first.
pub async fn list_versions(client: &reqwest::Client) -> AppResult<Vec<McVersion>> {
    let manifest = fetch_manifest(client).await?;

    let versions = manifest
        .versions
        .into_iter()
        .map(|version| McVersion {
            id: version.id,
            kind: version.kind,
            release_time: version.release_time,
        })
        .collect();
    Ok(versions)
}

/// Downloads the official server jar for `mc_version` into `server_dir`.
pub async fn install(
    client: &reqwest::Client,
    mc_version: &str,
    server_dir: &Path,
    report_progress: &ProgressCallback,
) -> AppResult<()> {
    let manifest = fetch_manifest(client).await?;
    let manifest_entry = manifest
        .versions
        .into_iter()
        .find(|version| version.id == mc_version)
        .ok_or_else(|| AppError::UnknownMinecraftVersion(mc_version.to_string()))?;

    let details: VersionDetails = client
        .get(&manifest_entry.url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let server_download = details
        .downloads
        .server
        .ok_or_else(|| AppError::UnknownMinecraftVersion(mc_version.to_string()))?;

    let jar_path = server_dir.join(SERVER_JAR_NAME);
    download_file(
        client,
        &server_download.url,
        &jar_path,
        ExpectedChecksum::Sha1(&server_download.sha1),
        report_progress,
    )
    .await?;
    Ok(())
}

async fn fetch_manifest(client: &reqwest::Client) -> AppResult<VersionManifest> {
    let manifest = client
        .get(VERSION_MANIFEST_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(manifest)
}
