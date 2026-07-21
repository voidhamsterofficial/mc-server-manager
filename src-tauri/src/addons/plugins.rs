//! Plugin management for Bukkit/Spigot-family servers and proxies: listing,
//! enabling/disabling and deleting the `.jar`s in a server's `plugins/`
//! folder, plus browsing and installing plugins from Modrinth and SpigotMC.

use std::path::{Path, PathBuf};

use crate::addons;
use crate::addons::cache::MarketplaceCache;
use crate::addons::sources::{self, AddonSearchResult, InstalledAddonVersion, MarketplaceContext};
use crate::error::AppResult;
use crate::installers::ProgressCallback;

pub use crate::addons::sources::AddonUpdateStatus as PluginUpdateStatus;
pub use crate::addons::InstalledAddon as InstalledPlugin;

const PLUGINS_DIR: &str = "plugins";
/// Modrinth/Spiget tag this software falls under.
const PROJECT_TYPE: &str = "plugin";

fn plugins_dir(server_dir: &Path) -> PathBuf {
    server_dir.join(PLUGINS_DIR)
}

pub fn list_installed(server_dir: &Path) -> AppResult<Vec<InstalledPlugin>> {
    addons::list_installed(&plugins_dir(server_dir))
}

/// Copies a `.jar` dropped onto the Plugins tab into the server's `plugins/`
/// folder.
pub fn import_jar(server_dir: &Path, source_path: &Path) -> AppResult<InstalledPlugin> {
    addons::import_jar(&plugins_dir(server_dir), source_path)
}

pub fn set_enabled(server_dir: &Path, file_name: &str, enabled: bool) -> AppResult<String> {
    addons::set_enabled(&plugins_dir(server_dir), file_name, enabled)
}

pub fn delete(server_dir: &Path, file_name: &str) -> AppResult<()> {
    addons::delete(&plugins_dir(server_dir), file_name)
}

/// Searches a marketplace for plugins compatible with `ctx`'s loader facet
/// and Minecraft version.
pub async fn search(
    client: &reqwest::Client,
    cache: &MarketplaceCache,
    ctx: MarketplaceContext<'_>,
    query: &str,
) -> AppResult<Vec<AddonSearchResult>> {
    sources::search(client, cache, ctx, query, PROJECT_TYPE).await
}

/// Downloads the newest compatible version of a plugin into the server's
/// `plugins/` folder.
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
        &plugins_dir(server_dir),
        project_id,
        report_progress,
    )
    .await
}

/// Checks every installed plugin ServerForge has provenance for against its
/// marketplace's newest version.
pub async fn check_for_updates(
    client: &reqwest::Client,
    cache: &MarketplaceCache,
    server_dir: &Path,
    records: &[crate::storage::db::PluginInstallRecord],
) -> AppResult<Vec<PluginUpdateStatus>> {
    let installed = list_installed(server_dir)?;
    Ok(sources::check_for_updates(client, cache, &installed, records, None).await)
}
