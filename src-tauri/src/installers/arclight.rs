//! Arclight installer — jars are published as GitHub release assets named
//! `arclight-<edition>-<mc>-<build>.jar`. We install the Forge edition.

use std::path::Path;

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::installers::forgelike::sort_minecraft_versions_desc;
use crate::installers::vanilla::{McVersion, SERVER_JAR_NAME};
use crate::installers::{download_file, ExpectedChecksum, ProgressCallback};

const ARCLIGHT_RELEASES_URL: &str =
    "https://api.github.com/repos/IzzelAliz/Arclight/releases?per_page=30";

const EDITION_PREFIX: &str = "arclight-forge-";

#[derive(Debug, Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// `arclight-forge-1.21.1-1.0.3.jar` -> `1.21.1`.
fn asset_mc_version(asset_name: &str) -> Option<String> {
    let remainder = asset_name.strip_prefix(EDITION_PREFIX)?;
    if !remainder.ends_with(".jar") {
        return None;
    }
    let (mc_version, _build) = remainder.rsplit_once('-')?;
    Some(mc_version.to_string())
}

async fn recent_releases(client: &reqwest::Client) -> AppResult<Vec<Release>> {
    let releases = client
        .get(ARCLIGHT_RELEASES_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(releases)
}

pub async fn list_versions(client: &reqwest::Client) -> AppResult<Vec<McVersion>> {
    let releases = recent_releases(client).await?;

    let mut mc_versions: Vec<String> = Vec::new();
    for release in &releases {
        for asset in &release.assets {
            let Some(mc_version) = asset_mc_version(&asset.name) else {
                continue;
            };
            if !mc_versions.contains(&mc_version) {
                mc_versions.push(mc_version);
            }
        }
    }
    sort_minecraft_versions_desc(&mut mc_versions);

    let entries = mc_versions
        .into_iter()
        .map(|id| McVersion {
            id,
            kind: "release".to_string(),
            release_time: String::new(),
        })
        .collect();
    Ok(entries)
}

pub async fn install(
    client: &reqwest::Client,
    mc_version: &str,
    server_dir: &Path,
    report_progress: &ProgressCallback,
) -> AppResult<()> {
    let releases = recent_releases(client).await?;

    // Releases are newest-first; the first matching asset is the newest
    // build for this Minecraft version.
    let wanted_prefix = format!("{EDITION_PREFIX}{mc_version}-");
    let matching_asset = releases
        .iter()
        .flat_map(|release| release.assets.iter())
        .find(|asset| asset.name.starts_with(&wanted_prefix) && asset.name.ends_with(".jar"))
        .ok_or_else(|| AppError::UnknownMinecraftVersion(mc_version.to_string()))?;

    let jar_path = server_dir.join(SERVER_JAR_NAME);
    download_file(
        client,
        &matching_asset.browser_download_url,
        &jar_path,
        ExpectedChecksum::None,
        report_progress,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mc_version_from_asset_names() {
        assert_eq!(
            asset_mc_version("arclight-forge-1.21.1-1.0.3.jar"),
            Some("1.21.1".to_string())
        );
        assert_eq!(asset_mc_version("arclight-fabric-1.20.1-1.0.0.jar"), None);
        assert_eq!(asset_mc_version("arclight-forge-1.20.1-1.0.0.zip"), None);
    }
}
