//! Global application settings, stored as YAML. The file lives beside the
//! executable when that location is writable (dev builds, portable
//! installs); otherwise it falls back to the per-user config directory
//! (installed copies under Program Files can't write beside the binary).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::servers::ServerConfig;

pub const GLOBAL_SETTINGS_FILE: &str = "blockparty.yaml";

/// Name of the folder created inside the chosen base location.
const DEFAULT_SERVERS_FOLDER_NAME: &str = "Blockparty Servers";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSettings {
    /// Parent directory new servers are created under.
    pub servers_base_dir: PathBuf,
    /// Every managed server's folder; each holds its own settings YAML.
    #[serde(default)]
    pub server_dirs: Vec<PathBuf>,
}

/// Backwards-compat alias: commands still speak of "AppSettings".
pub type AppSettings = GlobalSettings;

impl GlobalSettings {
    pub fn save(&self, path: &Path) -> AppResult<()> {
        let serialized = serde_yaml::to_string(self)
            .map_err(|yaml_error| AppError::Process(yaml_error.to_string()))?;
        crate::fsutil::atomic_write(path, serialized.as_bytes())?;
        Ok(())
    }
}

/// Where the global settings YAML lives. An existing file wins wherever it
/// is; otherwise prefer beside the exe when writable, else the user config
/// dir.
pub fn global_settings_path(app: &AppHandle) -> AppResult<PathBuf> {
    let beside_exe = executable_dir().map(|dir| dir.join(GLOBAL_SETTINGS_FILE));
    if let Some(path) = &beside_exe {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    let config_dir = app.path().app_config_dir()?;
    let in_config_dir = config_dir.join(GLOBAL_SETTINGS_FILE);
    if in_config_dir.exists() {
        return Ok(in_config_dir);
    }

    if let Some(path) = beside_exe {
        let Some(parent) = path.parent() else {
            return Ok(in_config_dir);
        };
        if dir_is_writable(parent) {
            return Ok(path);
        }
    }

    std::fs::create_dir_all(&config_dir)?;
    Ok(in_config_dir)
}

/// Loads global settings, migrating from the pre-YAML JSON files in app
/// data on first run after the upgrade.
pub fn load_or_migrate(
    app: &AppHandle,
    settings_path: &Path,
    data_dir: &Path,
) -> AppResult<GlobalSettings> {
    if settings_path.exists() {
        let contents = std::fs::read_to_string(settings_path)?;
        let loaded = serde_yaml::from_str(&contents)
            .map_err(|yaml_error| AppError::Process(yaml_error.to_string()))?;
        return Ok(loaded);
    }

    let migrated = migrate_from_json(data_dir);
    let settings = migrated.unwrap_or_else(|| GlobalSettings {
        servers_base_dir: default_servers_base_dir(app, data_dir),
        server_dirs: Vec::new(),
    });
    settings.save(settings_path)?;
    Ok(settings)
}

/// One-time migration from the old servers.json registry + settings.json:
/// writes each server's YAML into its folder and returns the global view.
fn migrate_from_json(data_dir: &Path) -> Option<GlobalSettings> {
    #[derive(Deserialize)]
    struct OldSettings {
        #[serde(rename = "serversBaseDir")]
        servers_base_dir: PathBuf,
    }
    #[derive(Deserialize)]
    struct OldRegistry {
        servers: Vec<ServerConfig>,
    }

    let old_settings: OldSettings =
        serde_json::from_str(&std::fs::read_to_string(data_dir.join("settings.json")).ok()?)
            .ok()?;
    let old_registry: OldRegistry =
        serde_json::from_str(&std::fs::read_to_string(data_dir.join("servers.json")).ok()?).ok()?;

    let mut server_dirs = Vec::new();
    for mut config in old_registry.servers {
        // Legacy entries stored an empty dir and lived in app data.
        if config.dir.as_os_str().is_empty() {
            config.dir = data_dir.join("servers").join(&config.id);
        }
        if let Err(error) = crate::servers::save_server_settings(&config) {
            log::warn!("migration: could not write {}: {error}", config.name);
            continue;
        }
        server_dirs.push(config.dir);
    }

    Some(GlobalSettings {
        servers_base_dir: old_settings.servers_base_dir,
        server_dirs,
    })
}

fn executable_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    Some(dir.to_path_buf())
}

fn dir_is_writable(dir: &Path) -> bool {
    let probe = dir.join(".blockparty-write-probe");
    let outcome = std::fs::write(&probe, b"probe");
    if outcome.is_ok() {
        let cleanup = std::fs::remove_file(&probe);
        if let Err(error) = cleanup {
            log::warn!("could not remove write probe: {error}");
        }
        return true;
    }
    false
}

/// Somewhere the user can actually find their worlds: the Documents folder,
/// falling back to the home directory, then app data as a last resort.
fn default_servers_base_dir(app: &AppHandle, app_data_dir: &Path) -> PathBuf {
    if let Ok(documents_dir) = app.path().document_dir() {
        return documents_dir.join(DEFAULT_SERVERS_FOLDER_NAME);
    }
    if let Ok(home_dir) = app.path().home_dir() {
        return home_dir.join(DEFAULT_SERVERS_FOLDER_NAME);
    }
    app_data_dir.join("servers")
}
