//! Plugin management for Bukkit/Spigot-family servers and proxies: listing,
//! enabling/disabling and deleting the `.jar`s in a server's `plugins/`
//! folder, plus browsing and installing plugins from Modrinth.
//!
//! Enable/disable is done by renaming `foo.jar` <-> `foo.jar.disabled`, the
//! convention Bukkit-family servers already understand.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::installers::{download_file, ExpectedChecksum, ProgressCallback};

const PLUGINS_DIR: &str = "plugins";
const DISABLED_SUFFIX: &str = ".disabled";

/// One plugin jar already present in a server's `plugins/` folder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPlugin {
    /// The on-disk file name, e.g. `EssentialsX.jar` or `EssentialsX.jar.disabled`.
    pub file_name: String,
    /// A friendlier name for display (extension and `.disabled` stripped).
    pub display_name: String,
    pub enabled: bool,
    pub size_bytes: u64,
}

fn plugins_dir(server_dir: &Path) -> PathBuf {
    server_dir.join(PLUGINS_DIR)
}

/// Rejects anything that isn't a plain file name, so callers can never escape
/// the plugins folder with `..` or a path separator.
fn safe_file_name(file_name: &str) -> AppResult<&str> {
    let is_plain = !file_name.is_empty()
        && !file_name.contains('/')
        && !file_name.contains('\\')
        && file_name != "."
        && file_name != "..";
    if !is_plain {
        return Err(AppError::InvalidInput(
            "invalid plugin file name".to_string(),
        ));
    }
    Ok(file_name)
}

/// A friendlier label from a jar file name: drops `.disabled`, the `.jar`
/// extension, and a trailing `-<version>` if present.
fn display_name(file_name: &str) -> String {
    let without_disabled = file_name.strip_suffix(DISABLED_SUFFIX).unwrap_or(file_name);
    let stem = without_disabled
        .strip_suffix(".jar")
        .unwrap_or(without_disabled);
    stem.to_string()
}

/// Lists the plugins in a server's `plugins/` folder (enabled and disabled),
/// alphabetically. A missing folder simply means no plugins yet.
pub fn list_installed(server_dir: &Path) -> AppResult<Vec<InstalledPlugin>> {
    let dir = plugins_dir(server_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut plugins = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let enabled = file_name.ends_with(".jar");
        let disabled = file_name.ends_with(".jar.disabled");
        if !enabled && !disabled {
            continue;
        }

        plugins.push(InstalledPlugin {
            display_name: display_name(&file_name),
            enabled,
            size_bytes: metadata.len(),
            file_name,
        });
    }

    plugins.sort_by_key(|plugin| plugin.display_name.to_lowercase());
    Ok(plugins)
}

/// Enables or disables a plugin by renaming its jar. Returns the new file name.
pub fn set_enabled(server_dir: &Path, file_name: &str, enabled: bool) -> AppResult<String> {
    let file_name = safe_file_name(file_name)?;
    let dir = plugins_dir(server_dir);
    let current = dir.join(file_name);
    if !current.is_file() {
        return Err(AppError::InvalidInput("plugin not found".to_string()));
    }

    let target_name = if enabled {
        match file_name.strip_suffix(DISABLED_SUFFIX) {
            Some(base) => base.to_string(),
            None => return Ok(file_name.to_string()), // already enabled
        }
    } else if file_name.ends_with(DISABLED_SUFFIX) {
        return Ok(file_name.to_string()); // already disabled
    } else {
        format!("{file_name}{DISABLED_SUFFIX}")
    };

    let target = dir.join(&target_name);
    std::fs::rename(&current, &target)?;
    Ok(target_name)
}

/// Deletes a plugin jar from the plugins folder.
pub fn delete(server_dir: &Path, file_name: &str) -> AppResult<()> {
    let file_name = safe_file_name(file_name)?;
    let path = plugins_dir(server_dir).join(file_name);
    if !path.is_file() {
        return Err(AppError::InvalidInput("plugin not found".to_string()));
    }
    std::fs::remove_file(path)?;
    Ok(())
}

// --- Modrinth browsing & installation ------------------------------------

const MODRINTH_API: &str = "https://api.modrinth.com/v2";

/// One plugin project from a Modrinth search.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSearchResult {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub author: String,
}

#[derive(Debug, Deserialize)]
struct ModrinthSearchResponse {
    hits: Vec<ModrinthHit>,
}

#[derive(Debug, Deserialize)]
struct ModrinthHit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    downloads: u64,
    icon_url: Option<String>,
    author: String,
}

#[derive(Debug, Deserialize)]
struct ModrinthVersion {
    files: Vec<ModrinthFile>,
    date_published: String,
}

#[derive(Debug, Deserialize)]
struct ModrinthFile {
    url: String,
    filename: String,
    primary: bool,
    hashes: ModrinthHashes,
}

#[derive(Debug, Deserialize)]
struct ModrinthHashes {
    sha1: Option<String>,
    sha512: Option<String>,
}

/// Searches Modrinth for plugins compatible with the given loader facet and
/// Minecraft version.
pub async fn search(
    client: &reqwest::Client,
    query: &str,
    loader_facet: &str,
    mc_version: &str,
) -> AppResult<Vec<PluginSearchResult>> {
    // Proxies (empty mc_version) aren't tagged by Minecraft version, so we
    // only filter by version for game servers.
    let mut facet_groups = vec![
        r#"["project_type:plugin"]"#.to_string(),
        format!(r#"["categories:{loader_facet}"]"#),
    ];
    if !mc_version.is_empty() {
        facet_groups.push(format!(r#"["versions:{mc_version}"]"#));
    }
    let facets = format!("[{}]", facet_groups.join(","));

    let response: ModrinthSearchResponse = client
        .get(format!("{MODRINTH_API}/search"))
        .query(&[
            ("query", query),
            ("limit", "30"),
            ("index", "relevance"),
            ("facets", &facets),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let results = response
        .hits
        .into_iter()
        .map(|hit| PluginSearchResult {
            project_id: hit.project_id,
            slug: hit.slug,
            title: hit.title,
            description: hit.description,
            downloads: hit.downloads,
            icon_url: hit.icon_url,
            author: hit.author,
        })
        .collect();
    Ok(results)
}

/// Downloads the newest compatible version of a Modrinth plugin into the
/// server's `plugins/` folder, verifying its published checksum.
pub async fn install_from_modrinth(
    client: &reqwest::Client,
    server_dir: &Path,
    project_id: &str,
    loader_facet: &str,
    mc_version: &str,
    report_progress: &ProgressCallback,
) -> AppResult<InstalledPlugin> {
    let loaders = format!(r#"["{loader_facet}"]"#);
    let game_versions = format!(r#"["{mc_version}"]"#);
    let mut params: Vec<(&str, &str)> = vec![("loaders", loaders.as_str())];
    if !mc_version.is_empty() {
        params.push(("game_versions", game_versions.as_str()));
    }
    let mut versions: Vec<ModrinthVersion> = client
        .get(format!("{MODRINTH_API}/project/{project_id}/version"))
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // The API returns versions newest-first, but sort defensively.
    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    let newest = versions
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Process("no compatible plugin version found".to_string()))?;

    let file = newest
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| newest.files.first())
        .ok_or_else(|| AppError::Process("plugin version has no files".to_string()))?;

    let file_name = safe_file_name(&file.filename)?.to_string();
    let dir = plugins_dir(server_dir);
    std::fs::create_dir_all(&dir)?;
    let destination = dir.join(&file_name);

    let checksum = file
        .hashes
        .sha512
        .as_deref()
        .map(ExpectedChecksum::Sha512)
        .or_else(|| file.hashes.sha1.as_deref().map(ExpectedChecksum::Sha1))
        .unwrap_or(ExpectedChecksum::None);

    download_file(client, &file.url, &destination, checksum, report_progress).await?;

    let size_bytes = std::fs::metadata(&destination)
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(InstalledPlugin {
        display_name: display_name(&file_name),
        enabled: true,
        size_bytes,
        file_name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_strips_extension_and_disabled() {
        assert_eq!(display_name("EssentialsX.jar"), "EssentialsX");
        assert_eq!(display_name("EssentialsX.jar.disabled"), "EssentialsX");
        assert_eq!(display_name("LuckPerms-5.4.jar"), "LuckPerms-5.4");
    }

    #[test]
    fn safe_file_name_rejects_paths() {
        assert!(safe_file_name("ok.jar").is_ok());
        assert!(safe_file_name("../evil.jar").is_err());
        assert!(safe_file_name("sub/evil.jar").is_err());
        assert!(safe_file_name("..").is_err());
        assert!(safe_file_name("").is_err());
    }
}
