//! Marketplaces plugins and mods can be browsed from and installed out of:
//! Modrinth (plugins and mods), SpigotMC (plugins, via the community Spiget
//! API), and CurseForge (mods, needs a user-supplied API key).
//!
//! Each marketplace module exposes `search`, `latest_version` and `install`
//! with the same shapes so `plugins.rs`/`mods.rs` can treat them uniformly.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::addons::cache::{self, MarketplaceCache};
use crate::addons::{self, InstalledAddon};
use crate::error::{AppError, AppResult};
use crate::installers::{download_file, ExpectedChecksum, ProgressCallback};
use crate::storage::db::PluginInstallRecord;

/// How many results one browse returns. Deliberately a single page: the
/// browser has no pagination and no infinite scroll, so this is the whole
/// list the user sees, and it keeps a search to exactly one API request.
pub const SEARCH_RESULT_LIMIT: usize = 20;

/// Which marketplace an addon was found on or installed from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AddonSource {
    Modrinth,
    Spigot,
    Curseforge,
}

impl AddonSource {
    pub fn as_db_str(self) -> &'static str {
        match self {
            AddonSource::Modrinth => "modrinth",
            AddonSource::Spigot => "spigot",
            AddonSource::Curseforge => "curseforge",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        match value {
            "modrinth" => Some(AddonSource::Modrinth),
            "spigot" => Some(AddonSource::Spigot),
            "curseforge" => Some(AddonSource::Curseforge),
            _ => None,
        }
    }
}

/// One addon project found via a marketplace search.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonSearchResult {
    pub source: AddonSource,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub author: String,
}

/// The newest version of an addon available for a given loader/MC version,
/// or after installing/updating it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonVersion {
    pub version_id: String,
    pub version_number: String,
}

/// The result of installing (or updating) an addon: the file now on disk,
/// plus the version info to remember for future update checks.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledAddonVersion {
    #[serde(flatten)]
    pub addon: InstalledAddon,
    pub version: AddonVersion,
}

// --- Modrinth -------------------------------------------------------------

pub mod modrinth {
    use super::*;

    const MODRINTH_API: &str = "https://api.modrinth.com/v2";

    #[derive(Debug, Deserialize)]
    struct SearchResponse {
        hits: Vec<Hit>,
    }

    #[derive(Debug, Deserialize)]
    struct Hit {
        project_id: String,
        slug: String,
        title: String,
        description: String,
        downloads: u64,
        icon_url: Option<String>,
        author: String,
    }

    #[derive(Debug, Deserialize)]
    struct Version {
        id: String,
        version_number: String,
        files: Vec<VersionFile>,
        date_published: String,
    }

    #[derive(Debug, Deserialize)]
    struct VersionFile {
        url: String,
        filename: String,
        primary: bool,
        hashes: VersionHashes,
    }

    #[derive(Debug, Deserialize)]
    struct VersionHashes {
        sha1: Option<String>,
        sha512: Option<String>,
    }

    /// Searches Modrinth for `project_type` ("plugin" or "mod") projects
    /// compatible with the given loader facet and Minecraft version.
    pub async fn search(
        client: &reqwest::Client,
        query: &str,
        project_type: &str,
        loader_facet: &str,
        mc_version: &str,
    ) -> AppResult<Vec<AddonSearchResult>> {
        // Proxies (empty mc_version) aren't tagged by Minecraft version, so we
        // only filter by version when one is given.
        let mut facet_groups = vec![
            format!(r#"["project_type:{project_type}"]"#),
            format!(r#"["categories:{loader_facet}"]"#),
        ];
        if !mc_version.is_empty() {
            facet_groups.push(format!(r#"["versions:{mc_version}"]"#));
        }
        let facets = format!("[{}]", facet_groups.join(","));

        let limit = SEARCH_RESULT_LIMIT.to_string();
        let response: SearchResponse = client
            .get(format!("{MODRINTH_API}/search"))
            .query(&[
                ("query", query),
                ("limit", &limit),
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
            .map(|hit| AddonSearchResult {
                source: AddonSource::Modrinth,
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

    async fn newest_version(
        client: &reqwest::Client,
        project_id: &str,
        loader_facet: &str,
        mc_version: &str,
    ) -> AppResult<Version> {
        let loaders = format!(r#"["{loader_facet}"]"#);
        let game_versions = format!(r#"["{mc_version}"]"#);
        let mut params: Vec<(&str, &str)> = vec![("loaders", loaders.as_str())];
        if !mc_version.is_empty() {
            params.push(("game_versions", game_versions.as_str()));
        }
        let mut versions: Vec<Version> = client
            .get(format!("{MODRINTH_API}/project/{project_id}/version"))
            .query(&params)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // The API returns versions newest-first, but sort defensively.
        versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        versions
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Process("no compatible version found on Modrinth".to_string()))
    }

    /// The newest version available for a project, without downloading it.
    pub async fn latest_version(
        client: &reqwest::Client,
        project_id: &str,
        loader_facet: &str,
        mc_version: &str,
    ) -> AppResult<AddonVersion> {
        let version = newest_version(client, project_id, loader_facet, mc_version).await?;
        Ok(AddonVersion {
            version_id: version.id,
            version_number: version.version_number,
        })
    }

    /// Downloads the newest compatible version of a Modrinth project into
    /// `dir`, verifying its published checksum.
    pub async fn install(
        client: &reqwest::Client,
        dir: &Path,
        project_id: &str,
        loader_facet: &str,
        mc_version: &str,
        report_progress: &ProgressCallback,
    ) -> AppResult<InstalledAddonVersion> {
        let version = newest_version(client, project_id, loader_facet, mc_version).await?;
        let file = version
            .files
            .iter()
            .find(|file| file.primary)
            .or_else(|| version.files.first())
            .ok_or_else(|| AppError::Process("Modrinth version has no files".to_string()))?;

        let file_name = addons::safe_file_name(&file.filename)?.to_string();
        std::fs::create_dir_all(dir)?;
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
        Ok(InstalledAddonVersion {
            addon: InstalledAddon {
                display_name: addons::display_name(&file_name),
                enabled: true,
                size_bytes,
                file_name,
            },
            version: AddonVersion {
                version_id: version.id,
                version_number: version.version_number,
            },
        })
    }
}

// --- SpigotMC (via the community Spiget API) ------------------------------

pub mod spigot {
    use super::*;

    const SPIGET_API: &str = "https://api.spiget.org/v2";

    #[derive(Debug, Deserialize)]
    struct SearchHit {
        id: u64,
        name: String,
        tag: Option<String>,
        downloads: u64,
        #[serde(default)]
        premium: bool,
        #[serde(default)]
        external: Option<serde_json::Value>,
    }

    #[derive(Debug, Deserialize)]
    struct ResourceDetails {
        name: String,
        #[serde(default)]
        premium: bool,
        #[serde(default)]
        external: Option<serde_json::Value>,
    }

    #[derive(Debug, Deserialize)]
    struct LatestVersion {
        id: u64,
        name: String,
    }

    /// Searches SpigotMC resources by name. Premium/externally-hosted
    /// resources are filtered out since ServerForge can't download them
    /// directly.
    pub async fn search(
        client: &reqwest::Client,
        query: &str,
    ) -> AppResult<Vec<AddonSearchResult>> {
        let query_path = if query.trim().is_empty() {
            "minecraft"
        } else {
            query.trim()
        };
        let hits: Vec<SearchHit> = client
            .get(format!(
                "{SPIGET_API}/search/resources/{}",
                urlencoding_path(query_path)
            ))
            .query(&[
                ("field", "name"),
                ("size", &SEARCH_RESULT_LIMIT.to_string()),
                ("sort", "-downloads"),
                ("fields", "id,name,tag,downloads,premium,external"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let results = hits
            .into_iter()
            .filter(|hit| !hit.premium && hit.external.is_none())
            .map(|hit| AddonSearchResult {
                source: AddonSource::Spigot,
                project_id: hit.id.to_string(),
                slug: hit.id.to_string(),
                title: hit.name,
                description: hit.tag.unwrap_or_default(),
                downloads: hit.downloads,
                // Spiget serves each resource's icon at a fixed, ID-based
                // route regardless of where the underlying image lives.
                icon_url: Some(format!("{SPIGET_API}/resources/{}/icon", hit.id)),
                author: "SpigotMC".to_string(),
            })
            .collect();
        Ok(results)
    }

    async fn details(client: &reqwest::Client, resource_id: &str) -> AppResult<ResourceDetails> {
        client
            .get(format!("{SPIGET_API}/resources/{resource_id}"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .map_err(Into::into)
    }

    async fn latest(client: &reqwest::Client, resource_id: &str) -> AppResult<LatestVersion> {
        client
            .get(format!(
                "{SPIGET_API}/resources/{resource_id}/versions/latest"
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .map_err(Into::into)
    }

    pub async fn latest_version(
        client: &reqwest::Client,
        resource_id: &str,
    ) -> AppResult<AddonVersion> {
        let version = latest(client, resource_id).await?;
        Ok(AddonVersion {
            version_id: version.id.to_string(),
            version_number: version.name,
        })
    }

    /// Downloads a SpigotMC resource's latest version into `dir`. Spiget
    /// doesn't hand back a checksum, so the file is trusted as-is once
    /// downloaded from the API.
    pub async fn install(
        client: &reqwest::Client,
        dir: &Path,
        resource_id: &str,
        report_progress: &ProgressCallback,
    ) -> AppResult<InstalledAddonVersion> {
        let resource = details(client, resource_id).await?;
        if resource.premium || resource.external.is_some() {
            return Err(AppError::InvalidInput(
                "this SpigotMC resource can't be downloaded directly".to_string(),
            ));
        }
        let version = latest(client, resource_id).await?;

        let file_name = addons::sanitize_jar_name(&resource.name, resource_id);
        std::fs::create_dir_all(dir)?;
        let destination = dir.join(&file_name);

        download_file(
            client,
            &format!("{SPIGET_API}/resources/{resource_id}/download"),
            &destination,
            ExpectedChecksum::None,
            report_progress,
        )
        .await?;

        let size_bytes = std::fs::metadata(&destination)
            .map(|m| m.len())
            .unwrap_or(0);
        Ok(InstalledAddonVersion {
            addon: InstalledAddon {
                display_name: addons::display_name(&file_name),
                enabled: true,
                size_bytes,
                file_name,
            },
            version: AddonVersion {
                version_id: version.id.to_string(),
                version_number: version.name,
            },
        })
    }

    /// Percent-encodes a search term for use as a Spiget URL path segment.
    fn urlencoding_path(value: &str) -> String {
        value
            .bytes()
            .map(|byte| match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (byte as char).to_string()
                }
                b' ' => "%20".to_string(),
                _ => format!("%{byte:02X}"),
            })
            .collect()
    }
}

// --- CurseForge -------------------------------------------------------------

pub mod curseforge {
    use super::*;

    const CURSEFORGE_API: &str = "https://api.curseforge.com/v1";
    /// Minecraft, per CurseForge's game catalog.
    const MINECRAFT_GAME_ID: u32 = 432;
    /// "Mods" section of the Minecraft catalog.
    const MODS_CLASS_ID: u32 = 6;

    /// CurseForge's `ModLoaderType` enum values for the loaders ServerForge
    /// can install mods for.
    fn mod_loader_type(loader_facet: &str) -> Option<u32> {
        match loader_facet {
            "forge" => Some(1),
            "fabric" => Some(4),
            "quilt" => Some(5),
            "neoforge" => Some(6),
            _ => None,
        }
    }

    #[derive(Debug, Deserialize)]
    struct SearchResponse {
        data: Vec<ModEntry>,
    }

    #[derive(Debug, Deserialize)]
    struct ModEntry {
        id: u64,
        slug: String,
        name: String,
        summary: String,
        #[serde(default)]
        download_count: u64,
        logo: Option<ModLogo>,
        #[serde(default)]
        authors: Vec<ModAuthor>,
    }

    #[derive(Debug, Deserialize)]
    struct ModLogo {
        thumbnail_url: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct ModAuthor {
        name: String,
    }

    #[derive(Debug, Deserialize)]
    struct FilesResponse {
        data: Vec<ModFile>,
    }

    #[derive(Debug, Deserialize)]
    struct ModFile {
        id: u64,
        display_name: String,
        file_name: String,
        download_url: Option<String>,
    }

    fn require_api_key(api_key: Option<&str>) -> AppResult<&str> {
        api_key.filter(|key| !key.is_empty()).ok_or_else(|| {
            AppError::InvalidInput(
                "add a CurseForge API key in Settings to browse CurseForge mods".to_string(),
            )
        })
    }

    pub async fn search(
        client: &reqwest::Client,
        api_key: Option<&str>,
        query: &str,
        loader_facet: &str,
        mc_version: &str,
    ) -> AppResult<Vec<AddonSearchResult>> {
        let api_key = require_api_key(api_key)?;
        let mut query_params: Vec<(&str, String)> = vec![
            ("gameId", MINECRAFT_GAME_ID.to_string()),
            ("classId", MODS_CLASS_ID.to_string()),
            ("searchFilter", query.to_string()),
            ("sortField", "2".to_string()),
            ("sortOrder", "desc".to_string()),
            ("pageSize", SEARCH_RESULT_LIMIT.to_string()),
        ];
        if !mc_version.is_empty() {
            query_params.push(("gameVersion", mc_version.to_string()));
        }
        if let Some(loader_type) = mod_loader_type(loader_facet) {
            query_params.push(("modLoaderType", loader_type.to_string()));
        }

        let response: SearchResponse = client
            .get(format!("{CURSEFORGE_API}/mods/search"))
            .header("x-api-key", api_key)
            .query(&query_params)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let results = response
            .data
            .into_iter()
            .map(|entry| AddonSearchResult {
                source: AddonSource::Curseforge,
                project_id: entry.id.to_string(),
                slug: entry.slug,
                title: entry.name,
                description: entry.summary,
                downloads: entry.download_count,
                icon_url: entry.logo.and_then(|logo| logo.thumbnail_url),
                author: entry
                    .authors
                    .first()
                    .map(|author| author.name.clone())
                    .unwrap_or_default(),
            })
            .collect();
        Ok(results)
    }

    async fn newest_file(
        client: &reqwest::Client,
        api_key: &str,
        mod_id: &str,
        loader_facet: &str,
        mc_version: &str,
    ) -> AppResult<ModFile> {
        let mut query_params: Vec<(&str, String)> = vec![("pageSize", "10".to_string())];
        if !mc_version.is_empty() {
            query_params.push(("gameVersion", mc_version.to_string()));
        }
        if let Some(loader_type) = mod_loader_type(loader_facet) {
            query_params.push(("modLoaderType", loader_type.to_string()));
        }

        let response: FilesResponse = client
            .get(format!("{CURSEFORGE_API}/mods/{mod_id}/files"))
            .header("x-api-key", api_key)
            .query(&query_params)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        response
            .data
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Process("no compatible file found on CurseForge".to_string()))
    }

    pub async fn latest_version(
        client: &reqwest::Client,
        api_key: Option<&str>,
        mod_id: &str,
        loader_facet: &str,
        mc_version: &str,
    ) -> AppResult<AddonVersion> {
        let api_key = require_api_key(api_key)?;
        let file = newest_file(client, api_key, mod_id, loader_facet, mc_version).await?;
        Ok(AddonVersion {
            version_id: file.id.to_string(),
            version_number: file.display_name,
        })
    }

    /// Downloads a CurseForge mod's newest compatible file into `dir`.
    pub async fn install(
        client: &reqwest::Client,
        api_key: Option<&str>,
        dir: &Path,
        mod_id: &str,
        loader_facet: &str,
        mc_version: &str,
        report_progress: &ProgressCallback,
    ) -> AppResult<InstalledAddonVersion> {
        let api_key = require_api_key(api_key)?;
        let file = newest_file(client, api_key, mod_id, loader_facet, mc_version).await?;
        let download_url = file.download_url.clone().ok_or_else(|| {
            AppError::InvalidInput(
                "this mod's author has disabled third-party downloads".to_string(),
            )
        })?;

        let file_name = addons::safe_file_name(&file.file_name)?.to_string();
        std::fs::create_dir_all(dir)?;
        let destination = dir.join(&file_name);

        download_file(
            client,
            &download_url,
            &destination,
            ExpectedChecksum::None,
            report_progress,
        )
        .await?;

        let size_bytes = std::fs::metadata(&destination)
            .map(|m| m.len())
            .unwrap_or(0);
        Ok(InstalledAddonVersion {
            addon: InstalledAddon {
                display_name: addons::display_name(&file_name),
                enabled: true,
                size_bytes,
                file_name,
            },
            version: AddonVersion {
                version_id: file.id.to_string(),
                version_number: file.display_name,
            },
        })
    }
}

// --- Cross-marketplace dispatch --------------------------------------------

/// Everything the marketplace dispatch functions need beyond the query/project
/// id itself: which marketplace, which loader/MC version to filter by, and
/// (for CurseForge only) the user's API key.
#[derive(Debug, Clone, Copy)]
pub struct MarketplaceContext<'a> {
    pub source: AddonSource,
    pub loader_facet: &'a str,
    pub mc_version: &'a str,
    pub curseforge_api_key: Option<&'a str>,
}

/// Searches whichever marketplace `ctx.source` points to for `project_type`
/// ("plugin" or "mod") projects, returning at most [`SEARCH_RESULT_LIMIT`]
/// results. Repeats within the cache's TTL are served from memory.
pub async fn search(
    client: &reqwest::Client,
    cache: &MarketplaceCache,
    ctx: MarketplaceContext<'_>,
    query: &str,
    project_type: &str,
) -> AppResult<Vec<AddonSearchResult>> {
    let key = cache::search_key(
        ctx.source.as_db_str(),
        project_type,
        ctx.loader_facet,
        ctx.mc_version,
        query,
    );
    if let Some(cached) = cache.search(&key).await {
        return Ok(cached);
    }

    let mut results = search_marketplace(client, ctx, query, project_type).await?;
    // Not every marketplace honours its own limit parameter, so the cap is
    // enforced here too rather than trusted.
    results.truncate(SEARCH_RESULT_LIMIT);
    cache.store_search(key, results.clone()).await;
    Ok(results)
}

async fn search_marketplace(
    client: &reqwest::Client,
    ctx: MarketplaceContext<'_>,
    query: &str,
    project_type: &str,
) -> AppResult<Vec<AddonSearchResult>> {
    match ctx.source {
        AddonSource::Modrinth => {
            modrinth::search(
                client,
                query,
                project_type,
                ctx.loader_facet,
                ctx.mc_version,
            )
            .await
        }
        AddonSource::Spigot => spigot::search(client, query).await,
        AddonSource::Curseforge => {
            curseforge::search(
                client,
                ctx.curseforge_api_key,
                query,
                ctx.loader_facet,
                ctx.mc_version,
            )
            .await
        }
    }
}

/// The newest version available for a project on whichever marketplace
/// `ctx.source` points to, without downloading it.
pub async fn latest_version(
    client: &reqwest::Client,
    cache: &MarketplaceCache,
    ctx: MarketplaceContext<'_>,
    project_id: &str,
) -> AppResult<AddonVersion> {
    let key = cache::version_key(
        ctx.source.as_db_str(),
        ctx.loader_facet,
        ctx.mc_version,
        project_id,
    );
    if let Some(cached) = cache.latest_version(&key).await {
        return Ok(cached);
    }

    let version = fetch_latest_version(client, ctx, project_id).await?;
    cache.store_latest_version(key, version.clone()).await;
    Ok(version)
}

async fn fetch_latest_version(
    client: &reqwest::Client,
    ctx: MarketplaceContext<'_>,
    project_id: &str,
) -> AppResult<AddonVersion> {
    match ctx.source {
        AddonSource::Modrinth => {
            modrinth::latest_version(client, project_id, ctx.loader_facet, ctx.mc_version).await
        }
        AddonSource::Spigot => spigot::latest_version(client, project_id).await,
        AddonSource::Curseforge => {
            curseforge::latest_version(
                client,
                ctx.curseforge_api_key,
                project_id,
                ctx.loader_facet,
                ctx.mc_version,
            )
            .await
        }
    }
}

/// Downloads the newest compatible version of a project from whichever
/// marketplace `ctx.source` points to, into `dir`.
pub async fn install(
    client: &reqwest::Client,
    ctx: MarketplaceContext<'_>,
    dir: &Path,
    project_id: &str,
    report_progress: &ProgressCallback,
) -> AppResult<InstalledAddonVersion> {
    match ctx.source {
        AddonSource::Modrinth => {
            modrinth::install(
                client,
                dir,
                project_id,
                ctx.loader_facet,
                ctx.mc_version,
                report_progress,
            )
            .await
        }
        AddonSource::Spigot => spigot::install(client, dir, project_id, report_progress).await,
        AddonSource::Curseforge => {
            curseforge::install(
                client,
                ctx.curseforge_api_key,
                dir,
                project_id,
                ctx.loader_facet,
                ctx.mc_version,
                report_progress,
            )
            .await
        }
    }
}

// --- Update checking --------------------------------------------------------

/// Whether a newer version than the one on disk is available for one
/// installed addon ServerForge knows the provenance of.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddonUpdateStatus {
    pub file_name: String,
    pub display_name: String,
    /// Which marketplace this addon was installed from, so the browse list
    /// can recognize it's already installed.
    pub source: AddonSource,
    pub project_id: String,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub has_update: bool,
}

/// Checks every addon ServerForge has an install record for against its
/// marketplace's newest version. Addons dropped in by hand (no record) are
/// skipped — there's no provenance to check them against.
pub async fn check_for_updates(
    client: &reqwest::Client,
    cache: &MarketplaceCache,
    installed: &[InstalledAddon],
    records: &[PluginInstallRecord],
    curseforge_api_key: Option<&str>,
) -> Vec<AddonUpdateStatus> {
    let mut statuses = Vec::new();
    for addon in installed {
        let Some(record) = records
            .iter()
            .find(|record| record.file_name == addon.file_name)
        else {
            continue;
        };
        let Some(source) = AddonSource::from_db_str(&record.source) else {
            continue;
        };
        let Some(project_id) = &record.project_id else {
            continue;
        };
        let loader_facet = record.loader_facet.as_deref().unwrap_or("");
        let mc_version = record.mc_version.as_deref().unwrap_or("");

        let ctx = MarketplaceContext {
            source,
            loader_facet,
            mc_version,
            curseforge_api_key,
        };
        let latest = latest_version(client, cache, ctx, project_id).await.ok();

        let has_update = match (&latest, &record.version_id) {
            (Some(latest), Some(current_version_id)) => &latest.version_id != current_version_id,
            _ => false,
        };

        statuses.push(AddonUpdateStatus {
            file_name: addon.file_name.clone(),
            display_name: addon.display_name.clone(),
            source,
            project_id: project_id.clone(),
            current_version: record.version_number.clone(),
            latest_version: latest.map(|version| version.version_number),
            has_update,
        });
    }
    statuses
}
