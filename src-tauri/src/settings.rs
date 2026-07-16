//! User-facing application settings, persisted as `settings.json` in the
//! app-data directory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::AppResult;

/// Name of the folder created inside the chosen base location.
const DEFAULT_SERVERS_FOLDER_NAME: &str = "Blockparty Servers";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Parent directory new servers are created under.
    pub servers_base_dir: PathBuf,
}

impl AppSettings {
    /// Loads persisted settings, or builds defaults on first launch (or if
    /// the file is unreadable).
    pub fn load_or_default(path: &Path, app: &AppHandle, app_data_dir: &Path) -> Self {
        let Some(loaded) = Self::load(path) else {
            let defaults = Self {
                servers_base_dir: default_servers_base_dir(app, app_data_dir),
            };
            return defaults;
        };
        loaded
    }

    fn load(path: &Path) -> Option<Self> {
        let contents = std::fs::read_to_string(path).ok()?;
        let settings = serde_json::from_str(&contents).ok()?;
        Some(settings)
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        let serialized = serde_json::to_string_pretty(self)?;
        std::fs::write(path, serialized)?;
        Ok(())
    }
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
