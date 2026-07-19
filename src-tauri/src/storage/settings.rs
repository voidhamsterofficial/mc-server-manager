//! Small helper for picking a sensible first-run default for where servers
//! are created. The setting itself now lives in the SQLite database (see
//! `db.rs`); this module just has the "no value yet" fallback.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// Name of the folder created inside the chosen base location.
const DEFAULT_SERVERS_FOLDER_NAME: &str = "Blockparty Servers";

/// Somewhere the user can actually find their worlds: the Documents folder,
/// falling back to the home directory, then app data as a last resort.
pub fn default_servers_base_dir(app: &AppHandle, app_data_dir: &std::path::Path) -> PathBuf {
    if let Ok(documents_dir) = app.path().document_dir() {
        return documents_dir.join(DEFAULT_SERVERS_FOLDER_NAME);
    }
    if let Ok(home_dir) = app.path().home_dir() {
        return home_dir.join(DEFAULT_SERVERS_FOLDER_NAME);
    }
    app_data_dir.join("servers")
}
