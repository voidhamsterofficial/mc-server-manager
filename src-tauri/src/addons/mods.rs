//! Mod management for Forge-family servers: listing, enabling/disabling and
//! deleting the `.jar`s in a server's `mods/` folder, plus browsing and
//! installing mods from Modrinth and CurseForge.

use std::path::{Path, PathBuf};

use crate::addons;
use crate::addons::cache::MarketplaceCache;
use crate::addons::sources::{self, AddonSearchResult, InstalledAddonVersion, MarketplaceContext};
use crate::error::AppResult;
use crate::installers::ProgressCallback;

pub use crate::addons::sources::AddonUpdateStatus as ModUpdateStatus;
pub use crate::addons::InstalledAddon as InstalledMod;

const MODS_DIR: &str = "mods";
/// Modrinth's tag for this kind of project.
const PROJECT_TYPE: &str = "mod";

fn mods_dir(server_dir: &Path) -> PathBuf {
    server_dir.join(MODS_DIR)
}

pub fn list_installed(server_dir: &Path) -> AppResult<Vec<InstalledMod>> {
    addons::list_installed(&mods_dir(server_dir))
}

/// Copies a `.jar` dropped onto the Mods tab into the server's `mods/` folder.
pub fn import_jar(server_dir: &Path, source_path: &Path) -> AppResult<InstalledMod> {
    addons::import_jar(&mods_dir(server_dir), source_path)
}

pub fn set_enabled(server_dir: &Path, file_name: &str, enabled: bool) -> AppResult<String> {
    addons::set_enabled(&mods_dir(server_dir), file_name, enabled)
}

pub fn delete(server_dir: &Path, file_name: &str) -> AppResult<()> {
    addons::delete(&mods_dir(server_dir), file_name)
}

/// Searches a marketplace for mods compatible with `ctx`'s loader facet and
/// Minecraft version.
pub async fn search(
    client: &reqwest::Client,
    cache: &MarketplaceCache,
    ctx: MarketplaceContext<'_>,
    query: &str,
) -> AppResult<Vec<AddonSearchResult>> {
    sources::search(client, cache, ctx, query, PROJECT_TYPE).await
}

/// Downloads the newest compatible version of a mod into the server's
/// `mods/` folder.
pub async fn install(
    client: &reqwest::Client,
    server_dir: &Path,
    ctx: MarketplaceContext<'_>,
    project_id: &str,
    report_progress: &ProgressCallback,
) -> AppResult<InstalledAddonVersion> {
    sources::install(
        client,
        ctx,
        &mods_dir(server_dir),
        project_id,
        report_progress,
    )
    .await
}

/// Checks every installed mod ServerForge has provenance for against its
/// marketplace's newest version.
pub async fn check_for_updates(
    client: &reqwest::Client,
    cache: &MarketplaceCache,
    server_dir: &Path,
    records: &[crate::storage::db::PluginInstallRecord],
    curseforge_api_key: Option<&str>,
) -> AppResult<Vec<ModUpdateStatus>> {
    let installed = list_installed(server_dir)?;
    Ok(sources::check_for_updates(client, cache, &installed, records, curseforge_api_key).await)
}
