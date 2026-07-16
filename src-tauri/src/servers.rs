//! Server domain: configuration model, the persisted registry, and the
//! create/delete services that orchestrate installers and disk layout.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Which server software a server runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Paper,
    Fabric,
    Forge,
    NeoForge,
}

/// Lifecycle state of a server process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Crashed,
}

/// Persisted configuration for one managed server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    pub loader: Loader,
    pub memory_mb: u32,
    /// Explicit Java executable override; `None` means auto-resolve.
    pub java_path: Option<PathBuf>,
    /// Where this server's files live. Empty for servers created before
    /// custom locations existed (resolved to the legacy app-data path).
    #[serde(default)]
    pub dir: PathBuf,
    /// Where this server's backups go; `None` means the default
    /// `backups` folder inside the server directory.
    #[serde(default)]
    pub backups_dir: Option<PathBuf>,
    pub created_at_unix: u64,
}

/// The full set of managed servers, persisted as one JSON file.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ServerRegistry {
    pub servers: Vec<ServerConfig>,
}

impl ServerRegistry {
    /// Loads the registry from disk, returning an empty registry when the
    /// file does not exist yet (first launch).
    pub fn load(path: &Path) -> AppResult<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let file_contents = std::fs::read_to_string(path)?;
        let registry = serde_json::from_str(&file_contents)?;
        Ok(registry)
    }

    pub fn save(&self, path: &Path) -> AppResult<()> {
        let serialized = serde_json::to_string_pretty(self)?;
        std::fs::write(path, serialized)?;
        Ok(())
    }

    pub fn find(&self, server_id: &str) -> AppResult<&ServerConfig> {
        let found = self.servers.iter().find(|server| server.id == server_id);
        found.ok_or_else(|| AppError::ServerNotFound(server_id.to_string()))
    }

    pub fn add(&mut self, config: ServerConfig) {
        self.servers.push(config);
    }

    /// Removes a server by id, returning its config if it existed.
    pub fn remove(&mut self, server_id: &str) -> Option<ServerConfig> {
        let position = self
            .servers
            .iter()
            .position(|server| server.id == server_id)?;
        let removed = self.servers.remove(position);
        Some(removed)
    }
}

/// Validated request to create a new server.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateServerRequest {
    pub name: String,
    pub mc_version: String,
    pub loader: Loader,
    pub memory_mb: u32,
    /// The user must explicitly accept the Minecraft EULA
    /// (https://aka.ms/MinecraftEULA) before we write `eula=true`.
    pub accept_eula: bool,
    /// Parent directory to create the server folder in; `None` uses the
    /// configured default location.
    pub location_parent: Option<PathBuf>,
}

impl CreateServerRequest {
    pub fn validate(&self) -> AppResult<()> {
        let trimmed_name = self.name.trim();
        if trimmed_name.is_empty() {
            return Err(AppError::InvalidInput("server name is empty".to_string()));
        }
        if !self.accept_eula {
            let message = "the Minecraft EULA must be accepted to create a server".to_string();
            return Err(AppError::InvalidInput(message));
        }
        if self.memory_mb < MINIMUM_MEMORY_MB {
            let message = format!("memory must be at least {MINIMUM_MEMORY_MB} MB");
            return Err(AppError::InvalidInput(message));
        }
        Ok(())
    }
}

const MINIMUM_MEMORY_MB: u32 = 512;

/// Builds a fresh config (id, timestamp) from a validated create request.
pub fn new_server_config(request: &CreateServerRequest, dir: PathBuf) -> ServerConfig {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at_unix = current_unix_time();

    ServerConfig {
        id,
        name: request.name.trim().to_string(),
        mc_version: request.mc_version.clone(),
        loader: request.loader,
        memory_mb: request.memory_mb,
        java_path: None,
        dir,
        backups_dir: None,
        created_at_unix,
    }
}

/// Folder names Windows refuses regardless of extension.
const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

const FALLBACK_FOLDER_SLUG: &str = "server";

/// Turns a server name into a folder name that is valid on every platform:
/// illegal characters become dashes, trailing dots/spaces are trimmed
/// (Windows), and reserved device names are avoided.
pub fn folder_slug(server_name: &str) -> String {
    let cleaned: String = server_name
        .trim()
        .chars()
        .map(sanitize_folder_char)
        .collect();
    let trimmed = cleaned.trim_matches([' ', '.', '-']).to_string();

    let is_reserved = WINDOWS_RESERVED_NAMES
        .iter()
        .any(|reserved| trimmed.eq_ignore_ascii_case(reserved));
    if trimmed.is_empty() || is_reserved {
        return FALLBACK_FOLDER_SLUG.to_string();
    }
    trimmed
}

fn sanitize_folder_char(character: char) -> char {
    let is_safe = character.is_alphanumeric()
        || character == ' '
        || character == '-'
        || character == '_'
        || character == '.';
    if is_safe {
        return character;
    }
    '-'
}

/// Picks the first free directory under `parent`: `slug`, then `slug-2`,
/// `slug-3`, … so two servers with the same name never collide.
pub fn unique_server_dir(parent: &Path, slug: &str) -> PathBuf {
    let first_candidate = parent.join(slug);
    if !first_candidate.exists() {
        return first_candidate;
    }

    let mut suffix: u32 = 2;
    loop {
        let candidate = parent.join(format!("{slug}-{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
        suffix += 1;
    }
}

/// Writes `eula.txt` recording the user's acceptance of the Minecraft EULA.
pub fn write_eula_acceptance(server_dir: &Path) -> AppResult<()> {
    let eula_path = server_dir.join("eula.txt");
    let contents = "# Accepted via mc-server-manager on behalf of the user.\neula=true\n";
    std::fs::write(eula_path, contents)?;
    Ok(())
}

pub(crate) fn current_unix_time() -> u64 {
    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH);
    let seconds = since_epoch.map(|duration| duration.as_secs());
    seconds.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_keep_friendly_names() {
        assert_eq!(folder_slug("My cozy world"), "My cozy world");
        assert_eq!(folder_slug("skyblock_2-electric"), "skyblock_2-electric");
    }

    #[test]
    fn slugs_replace_illegal_characters() {
        assert_eq!(folder_slug("what/about\\this:one?"), "what-about-this-one");
        assert_eq!(folder_slug("  <\"quoted\">  "), "quoted");
    }

    #[test]
    fn slugs_avoid_windows_traps() {
        assert_eq!(folder_slug("CON"), "server");
        assert_eq!(folder_slug("ends with dots..."), "ends with dots");
        assert_eq!(folder_slug(""), "server");
        assert_eq!(folder_slug("///"), "server");
    }
}
