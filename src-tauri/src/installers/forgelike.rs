//! Forge and NeoForge installers. Both download an installer jar and run it
//! with `--installServer`; modern versions produce args files under
//! `libraries/` that the launch logic picks up (see `process.rs`).

use std::path::Path;

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::installers::vanilla::McVersion;
use crate::installers::{download_file, fetch_maven_checksum, run_java_tool, ProgressCallback};
use crate::servers::Loader;

const FORGE_PROMOTIONS_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const FORGE_MAVEN_BASE: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";

const NEOFORGE_VERSIONS_URL: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
const NEOFORGE_MAVEN_BASE: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge";

const INSTALLER_FILE: &str = "installer.jar";

#[derive(Debug, Deserialize)]
struct ForgePromotions {
    promos: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct NeoForgeVersions {
    versions: Vec<String>,
}

pub async fn list_versions(client: &reqwest::Client, loader: Loader) -> AppResult<Vec<McVersion>> {
    let mut mc_versions = match loader {
        Loader::Forge => forge_mc_versions(client).await?,
        Loader::NeoForge => neoforge_mc_versions(client).await?,
        other => {
            let message = format!("{other:?} is not a Forge-family loader");
            return Err(AppError::InvalidInput(message));
        }
    };

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
    loader: Loader,
    mc_version: &str,
    server_dir: &Path,
    java_executable: &Path,
    report_progress: &ProgressCallback,
) -> AppResult<()> {
    let installer_url = match loader {
        Loader::Forge => forge_installer_url(client, mc_version).await?,
        Loader::NeoForge => neoforge_installer_url(client, mc_version).await?,
        other => {
            let message = format!("{other:?} is not a Forge-family loader");
            return Err(AppError::InvalidInput(message));
        }
    };

    // This jar is executed with --installServer, so verify it against the
    // Maven checksum before running it.
    let checksum = fetch_maven_checksum(client, &installer_url).await?;
    let installer_path = server_dir.join(INSTALLER_FILE);
    download_file(
        client,
        &installer_url,
        &installer_path,
        checksum.as_expected(),
        report_progress,
    )
    .await?;

    // Both installers accept --installServer (NeoForge kept the alias).
    run_java_tool(
        java_executable,
        server_dir,
        &["-jar", INSTALLER_FILE, "--installServer"],
    )
    .await?;

    remove_best_effort(&installer_path);
    remove_best_effort(&server_dir.join(format!("{INSTALLER_FILE}.log")));
    Ok(())
}

async fn forge_promotions(client: &reqwest::Client) -> AppResult<ForgePromotions> {
    let promotions = client
        .get(FORGE_PROMOTIONS_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(promotions)
}

async fn forge_mc_versions(client: &reqwest::Client) -> AppResult<Vec<String>> {
    let promotions = forge_promotions(client).await?;

    let mut mc_versions: Vec<String> = promotions
        .promos
        .keys()
        .filter_map(|key| key.strip_suffix("-latest").map(str::to_string))
        .collect();
    mc_versions.dedup();
    Ok(mc_versions)
}

async fn forge_installer_url(client: &reqwest::Client, mc_version: &str) -> AppResult<String> {
    let promotions = forge_promotions(client).await?;

    let recommended = promotions.promos.get(&format!("{mc_version}-recommended"));
    let latest = promotions.promos.get(&format!("{mc_version}-latest"));
    let forge_version = recommended
        .or(latest)
        .ok_or_else(|| AppError::UnknownMinecraftVersion(mc_version.to_string()))?;

    let full_version = format!("{mc_version}-{forge_version}");
    let url = format!("{FORGE_MAVEN_BASE}/{full_version}/forge-{full_version}-installer.jar");
    Ok(url)
}

async fn neoforge_all_versions(client: &reqwest::Client) -> AppResult<Vec<String>> {
    let listing: NeoForgeVersions = client
        .get(NEOFORGE_VERSIONS_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(listing.versions)
}

/// NeoForge `21.1.208` targets Minecraft `1.21.1`; `21.0.x` targets `1.21`.
/// From the year-based Minecraft versions on (25.x+), NeoForge `26.2.x.y`
/// targets Minecraft `26.2` directly.
fn neoforge_to_mc_version(neoforge_version: &str) -> Option<String> {
    let mut parts = neoforge_version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;

    let major_number: u32 = major.parse().ok()?;
    if major_number >= 22 {
        return Some(format!("{major}.{minor}"));
    }
    if minor == "0" {
        return Some(format!("1.{major}"));
    }
    Some(format!("1.{major}.{minor}"))
}

async fn neoforge_mc_versions(client: &reqwest::Client) -> AppResult<Vec<String>> {
    let all = neoforge_all_versions(client).await?;

    let mut mc_versions: Vec<String> = Vec::new();
    for neoforge_version in all {
        let Some(mc_version) = neoforge_to_mc_version(&neoforge_version) else {
            continue;
        };
        if !mc_versions.contains(&mc_version) {
            mc_versions.push(mc_version);
        }
    }
    Ok(mc_versions)
}

async fn neoforge_installer_url(client: &reqwest::Client, mc_version: &str) -> AppResult<String> {
    let all = neoforge_all_versions(client).await?;

    // The maven listing is oldest-to-newest; the last match is the newest
    // build for this Minecraft version.
    let newest_match = all
        .into_iter()
        .rfind(|candidate| neoforge_to_mc_version(candidate).as_deref() == Some(mc_version))
        .ok_or_else(|| AppError::UnknownMinecraftVersion(mc_version.to_string()))?;

    let url = format!("{NEOFORGE_MAVEN_BASE}/{newest_match}/neoforge-{newest_match}-installer.jar");
    Ok(url)
}

/// Sorts Minecraft version ids newest-first by numeric segments.
pub(crate) fn sort_minecraft_versions_desc(versions: &mut [String]) {
    let numeric_key = |version: &String| -> Vec<u32> {
        version
            .split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    };
    versions.sort_by_key(|version| std::cmp::Reverse(numeric_key(version)));
}

fn remove_best_effort(path: &Path) {
    if !path.exists() {
        return;
    }
    if let Err(error) = std::fs::remove_file(path) {
        eprintln!("could not remove {}: {error}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_neoforge_versions_to_minecraft() {
        assert_eq!(
            neoforge_to_mc_version("21.1.208"),
            Some("1.21.1".to_string())
        );
        assert_eq!(neoforge_to_mc_version("21.0.4"), Some("1.21".to_string()));
        assert_eq!(
            neoforge_to_mc_version("20.4.196"),
            Some("1.20.4".to_string())
        );
        // Year-based Minecraft versions map directly.
        assert_eq!(
            neoforge_to_mc_version("26.2.0.21-beta"),
            Some("26.2".to_string())
        );
    }

    #[test]
    fn sorts_minecraft_versions_numerically() {
        let mut versions = vec![
            "1.9.4".to_string(),
            "1.21.1".to_string(),
            "1.20.4".to_string(),
            "1.21".to_string(),
        ];
        sort_minecraft_versions_desc(&mut versions);
        assert_eq!(versions, ["1.21.1", "1.21", "1.20.4", "1.9.4"]);
    }
}
