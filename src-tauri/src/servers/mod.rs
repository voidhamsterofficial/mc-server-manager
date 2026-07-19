//! Server domain: configuration model, the persisted registry, and the
//! create/delete services that orchestrate installers and disk layout.
//!
//! Sibling modules cover the rest of the server lifecycle: [`service`]
//! (start/stop/restart orchestration), [`state`] (Tauri-managed app state),
//! and [`scheduler`] (cron-driven tasks).

pub mod address;
pub mod scheduler;
pub mod service;
pub mod state;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Which server software a server runs. Not every variant has an automatic
/// installer yet — see [`Loader::is_installable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Loader {
    Vanilla,
    Bds,
    Paper,
    Purpur,
    Spigot,
    Folia,
    Fabric,
    NeoForge,
    Forge,
    Quilt,
    Arclight,
    Mohist,
    Velocity,
    BungeeCord,
}

impl Loader {
    /// Network proxies front game servers: no world, no EULA, no
    /// `server.properties`, and no `nogui` argument.
    pub fn is_proxy(self) -> bool {
        matches!(self, Loader::Velocity | Loader::BungeeCord)
    }

    /// The console command that shuts this software down gracefully.
    pub fn stop_command(self) -> &'static str {
        match self {
            Loader::BungeeCord => "end",
            Loader::Velocity => "shutdown",
            _ => "stop",
        }
    }

    /// The Modrinth loader name used to filter compatible plugins, or `None`
    /// for software that doesn't take plugins (mod loaders, vanilla, Bedrock).
    /// A `Some` value also means this server supports plugins at all.
    pub fn plugin_facet(self) -> Option<&'static str> {
        let facet = match self {
            Loader::Paper => "paper",
            Loader::Purpur => "purpur",
            Loader::Spigot => "spigot",
            Loader::Folia => "folia",
            // Hybrids load Bukkit/Spigot plugins.
            Loader::Mohist | Loader::Arclight => "spigot",
            Loader::Velocity => "velocity",
            Loader::BungeeCord => "bungeecord",
            _ => return None,
        };
        Some(facet)
    }

    /// The Modrinth loader name used to filter compatible mods, or `None` for
    /// software that doesn't take mod-loader mods (Bukkit-family, vanilla,
    /// Bedrock, proxies). A `Some` value also means this server supports
    /// installing mods at all.
    pub fn mod_facet(self) -> Option<&'static str> {
        let facet = match self {
            Loader::Forge => "forge",
            Loader::NeoForge => "neoforge",
            Loader::Fabric => "fabric",
            Loader::Quilt => "quilt",
            // Hybrids also load Forge mods.
            Loader::Mohist | Loader::Arclight => "forge",
            _ => return None,
        };
        Some(facet)
    }
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
    /// Extra JVM flags inserted before `-jar` (whitespace-separated).
    #[serde(default)]
    pub java_args: Option<String>,
    /// Full launch command override; replaces the java invocation entirely.
    #[serde(default)]
    pub start_command: Option<String>,
    /// Keep only this many newest backups; `None` keeps everything.
    #[serde(default)]
    pub backup_retention: Option<u32>,
    pub created_at_unix: u64,
}

/// Each server's settings live in its own folder as YAML — a fixed file
/// name, so renaming the server never orphans it (and it travels inside
/// backups automatically).
pub const SERVER_SETTINGS_FILE: &str = "blockparty-server.yaml";

/// The in-memory set of managed servers, assembled from each server
/// folder's own settings YAML at startup.
#[derive(Debug, Default)]
pub struct ServerRegistry {
    pub servers: Vec<ServerConfig>,
}

impl ServerRegistry {
    /// Builds the registry from the database's known-server list (id, dir
    /// pairs), reading each folder's own YAML. Folders that vanished or lost
    /// their settings file are reported back as `missing_ids` so the caller
    /// can prune them from the known-servers list instead of resurrecting a
    /// dead entry every time the app starts.
    pub fn load_known(known_servers: &[(String, PathBuf)]) -> (Self, Vec<String>) {
        let mut servers = Vec::new();
        let mut missing_ids = Vec::new();
        for (id, dir) in known_servers {
            match load_server_settings(dir) {
                Ok(mut config) => {
                    // The database's id is authoritative (a folder could in
                    // principle be a stale copy with a different id inside).
                    config.id = id.clone();
                    servers.push(config);
                }
                Err(error) => {
                    log::warn!(
                        "removing unreachable server {id} ({}): {error}",
                        dir.display()
                    );
                    missing_ids.push(id.clone());
                }
            }
        }
        (Self { servers }, missing_ids)
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

/// Writes a server's settings YAML into its own folder.
pub fn save_server_settings(config: &ServerConfig) -> AppResult<()> {
    let serialized = serde_yaml::to_string(config)
        .map_err(|yaml_error| AppError::Process(yaml_error.to_string()))?;
    std::fs::write(config.dir.join(SERVER_SETTINGS_FILE), serialized)?;
    Ok(())
}

/// Reads a server's settings YAML, healing the stored `dir` to wherever the
/// folder actually is now (folders can be moved between sessions).
pub fn load_server_settings(server_dir: &Path) -> AppResult<ServerConfig> {
    let contents = std::fs::read_to_string(server_dir.join(SERVER_SETTINGS_FILE))?;
    let mut config: ServerConfig = serde_yaml::from_str(&contents)
        .map_err(|yaml_error| AppError::Process(yaml_error.to_string()))?;
    config.dir = server_dir.to_path_buf();
    Ok(config)
}

/// Validated request to create a new server.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateServerRequest {
    pub name: String,
    pub mc_version: String,
    pub loader: Loader,
    pub memory_mb: u32,
    /// The port the server listens on, written to `server.properties`
    /// before the first start. Ignored for proxies (their configs differ).
    pub port: u16,
    /// The user must explicitly accept the Minecraft EULA
    /// (https://aka.ms/MinecraftEULA) before we write `eula=true`.
    pub accept_eula: bool,
    /// Parent directory to create the server folder in; `None` uses the
    /// configured default location.
    pub location_parent: Option<PathBuf>,
    pub java_args: Option<String>,
    pub start_command: Option<String>,
}

impl CreateServerRequest {
    pub fn validate(&self) -> AppResult<()> {
        let trimmed_name = self.name.trim();
        if trimmed_name.is_empty() {
            return Err(AppError::InvalidInput("server name is empty".to_string()));
        }
        if !self.accept_eula && !self.loader.is_proxy() {
            let message = "the Minecraft EULA must be accepted to create a server".to_string();
            return Err(AppError::InvalidInput(message));
        }
        if self.memory_mb < MINIMUM_MEMORY_MB {
            let message = format!("memory must be at least {MINIMUM_MEMORY_MB} MB");
            return Err(AppError::InvalidInput(message));
        }
        if self.port < 1024 {
            let message = "pick a port of 1024 or higher".to_string();
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
        java_args: normalized_option(&request.java_args),
        start_command: normalized_option(&request.start_command),
        backup_retention: None,
        created_at_unix,
    }
}

/// Builds (or recovers) a config for an existing folder being imported into
/// Blockparty. If the folder already has our settings YAML — e.g. it was
/// managed before and the app just lost track of it — that file wins and the
/// supplied fields are ignored; otherwise a fresh config is written for it.
pub fn config_for_import(
    dir: &Path,
    name: String,
    loader: Loader,
    mc_version: String,
    memory_mb: u32,
) -> ServerConfig {
    if let Ok(mut existing) = load_server_settings(dir) {
        existing.dir = dir.to_path_buf();
        return existing;
    }

    ServerConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        mc_version,
        loader,
        memory_mb,
        java_path: None,
        dir: dir.to_path_buf(),
        backups_dir: None,
        java_args: None,
        start_command: None,
        backup_retention: None,
        created_at_unix: current_unix_time(),
    }
}

/// Trims a free-text option; whitespace-only input means "not set".
pub(crate) fn normalized_option(value: &Option<String>) -> Option<String> {
    let trimmed = value.as_deref().map(str::trim).unwrap_or("");
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
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
