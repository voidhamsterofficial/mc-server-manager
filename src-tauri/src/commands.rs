//! Tauri command layer: thin wrappers that validate input and delegate to
//! the domain services. The future web panel reuses the same services.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, State};

use serde::Deserialize;

use crate::addons::mods;
use crate::addons::plugins;
use crate::addons::sources;
use crate::error::{AppError, AppResult};
use crate::installers::{self, vanilla};
use crate::java::{self, JavaInstall};
use crate::players::roster::{self, RosterEntry};
use crate::portforward;
use crate::process;
use crate::servers::address;
use crate::servers::scheduler::{self, ScheduledTask};
use crate::servers::service;
use crate::servers::state::AppState;
use crate::servers::{self, CreateServerRequest, Loader, ServerConfig, ServerStatus};
use crate::storage::backups::{self, BackupInfo};
use crate::storage::db;
use crate::storage::files;
use crate::storage::properties::{self, Property};

/// The subset of app-wide state exposed to the frontend as "Settings".
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub servers_base_dir: PathBuf,
}

#[tauri::command]
pub async fn list_servers(state: State<'_, AppState>) -> AppResult<Vec<ServerConfig>> {
    let registry = state.registry.lock().await;
    let servers = registry.servers.clone();
    Ok(servers)
}

#[tauri::command]
pub async fn create_server(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CreateServerRequest,
) -> AppResult<ServerConfig> {
    request.validate()?;

    let location_parent = resolve_location_parent(&state, request.location_parent.clone()).await?;
    let slug = servers::folder_slug(&request.name);
    let server_dir = servers::unique_server_dir(&location_parent, &slug);

    let config = servers::new_server_config(&request, server_dir.clone());
    std::fs::create_dir_all(&server_dir)?;

    // Some installers run a Java tool (Forge/NeoForge installers, Quilt's
    // installer, Spigot's BuildTools) — resolve or download Java up front.
    let needs_java_tool = matches!(
        request.loader,
        Loader::Forge | Loader::NeoForge | Loader::Quilt | Loader::Spigot
    );
    let install_java = if needs_java_tool {
        Some(service::resolve_or_download_java(&app, &config).await?)
    } else {
        None
    };

    let report_progress =
        installers::progress_event_reporter(app, config.id.clone(), "download-server-jar");
    let install_result = installers::install(
        &state.http,
        request.loader,
        &config.mc_version,
        &server_dir,
        install_java.as_deref(),
        &report_progress,
    )
    .await;
    if let Err(error) = install_result {
        remove_dir_best_effort(&server_dir);
        return Err(error);
    }

    if !request.loader.is_proxy() && request.loader != Loader::Bds {
        servers::write_eula_acceptance(&server_dir)?;
        // Pre-generate a full server.properties so the file is complete
        // before first start — otherwise the server generates it and can
        // discard edits made in between.
        properties::ensure_defaults(&server_dir)?;
        let port_properties = vec![
            Property {
                key: "server-port".to_string(),
                value: request.port.to_string(),
            },
            Property {
                key: "query.port".to_string(),
                value: request.port.to_string(),
            },
        ];
        properties::write(&server_dir, &port_properties)?;
    }

    {
        let mut registry = state.registry.lock().await;
        registry.add(config.clone());
    }
    state.persist_new_server(&config).await?;
    Ok(config)
}

/// An existing server folder to add to Blockparty's known-servers list.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportServerRequest {
    pub dir: PathBuf,
    pub name: String,
    pub loader: Loader,
    pub mc_version: String,
    pub memory_mb: u32,
}

/// Adds an existing server folder to the known-servers list — for a server
/// created outside Blockparty, or one whose entry was lost (e.g. the database
/// was reset). If the folder already carries Blockparty's own settings file,
/// that file's data wins over the supplied fields; nothing on disk is
/// otherwise touched.
#[tauri::command]
pub async fn import_server(
    state: State<'_, AppState>,
    request: ImportServerRequest,
) -> AppResult<ServerConfig> {
    if !request.dir.is_dir() {
        return Err(AppError::InvalidInput(
            "that folder doesn't exist".to_string(),
        ));
    }
    if request.name.trim().is_empty() {
        return Err(AppError::InvalidInput("server name is empty".to_string()));
    }

    {
        let registry = state.registry.lock().await;
        let already_known = registry
            .servers
            .iter()
            .find(|server| server.dir == request.dir);
        if let Some(existing) = already_known {
            return Ok(existing.clone());
        }
    }

    let memory_mb = if request.memory_mb == 0 {
        2048
    } else {
        request.memory_mb
    };
    let config = servers::config_for_import(
        &request.dir,
        request.name.trim().to_string(),
        request.loader,
        request.mc_version.trim().to_string(),
        memory_mb,
    );

    {
        let mut registry = state.registry.lock().await;
        registry.add(config.clone());
    }
    state.persist_new_server(&config).await?;
    Ok(config)
}

/// Available versions for one server software, newest first.
#[tauri::command]
pub async fn list_loader_versions(
    state: State<'_, AppState>,
    loader: Loader,
) -> AppResult<Vec<vanilla::McVersion>> {
    match loader {
        Loader::Vanilla => vanilla::list_versions(&state.http).await,
        Loader::Paper | Loader::Folia | Loader::Velocity => {
            installers::paper::list_versions(&state.http, loader).await
        }
        Loader::Purpur => installers::purpur::list_versions(&state.http).await,
        Loader::Fabric => installers::fabric::list_versions(&state.http).await,
        Loader::BungeeCord => Ok(installers::bungee::list_versions()),
        Loader::Forge | Loader::NeoForge => {
            installers::forgelike::list_versions(&state.http, loader).await
        }
        Loader::Quilt => installers::quilt::list_versions(&state.http).await,
        Loader::Spigot => installers::spigot::list_versions(&state.http).await,
        Loader::Mohist => installers::mohist::list_versions(&state.http).await,
        Loader::Arclight => installers::arclight::list_versions(&state.http).await,
        Loader::Bds => installers::bds::list_versions(&state.http).await,
    }
}

#[tauri::command]
pub async fn delete_server(state: State<'_, AppState>, server_id: String) -> AppResult<()> {
    if process::is_running(&state.running, &server_id).await {
        let message = "stop the server before deleting it".to_string();
        return Err(AppError::InvalidInput(message));
    }

    let removed_config = {
        let mut registry = state.registry.lock().await;
        registry
            .remove(&server_id)
            .ok_or_else(|| AppError::ServerNotFound(server_id.clone()))?
    };

    let server_dir = state.server_dir(&removed_config);
    state.forget_known_server(&server_id).await?;
    if server_dir.exists() {
        std::fs::remove_dir_all(&server_dir)?;
    }

    // A deleted server takes its satellite data with it: scheduled tasks,
    // player history, and any tracked plugin/mod installs.
    {
        let mut tasks = state.tasks.lock().await;
        let before = tasks.len();
        tasks.retain(|task| task.server_id != server_id);
        if tasks.len() != before {
            let db = state.db.lock().await;
            scheduler::save_tasks(&db, &tasks)?;
        }
    }
    state.rosters.forget(&server_id).await;
    {
        let db = state.db.lock().await;
        db.clear_plugin_installs(&server_id)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn start_server(app: AppHandle, server_id: String) -> AppResult<()> {
    service::start_server(&app, &server_id).await
}

#[tauri::command]
pub async fn stop_server(app: AppHandle, server_id: String) -> AppResult<()> {
    service::stop_server(&app, &server_id).await?;
    // An explicit stop should also close any port we opened to the internet.
    // (Restart goes through service::restart_server, which doesn't call this, so
    // a restart keeps the mapping.)
    close_forward_after_stop(&app, server_id);
    Ok(())
}

/// Best-effort, off the request path: if this server's port was UPnP-forwarded
/// this session, close the mapping so a stopped server isn't left reachable.
fn close_forward_after_stop(app: &AppHandle, server_id: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let Some(external_port) = state.forwarded.lock().await.remove(&server_id) else {
            return;
        };
        let Ok(config) = service::find_config(&app, &server_id).await else {
            return;
        };
        let Ok(std::net::IpAddr::V4(lan_ip)) = address::local_lan_ip().parse::<std::net::IpAddr>()
        else {
            return;
        };
        let _ =
            portforward::close(address::forward_protocol(config.loader), external_port, lan_ip)
                .await;
    });
}

/// Closes every UPnP mapping this session opened, so quitting the app doesn't
/// silently leave servers reachable from the internet. Called on app exit;
/// best-effort and bounded by an overall timeout so an unresponsive router
/// can't hang shutdown.
///
/// This only covers a graceful quit — a crash or force-kill skips it, same as
/// any other in-memory state. [`port_forward_status`] is what recovers from
/// that case, by rediscovering the leftover mapping on the next launch.
pub async fn close_all_port_forwards(app: &AppHandle) {
    let state = app.state::<AppState>();
    let to_close: Vec<(String, u16)> = state.forwarded.lock().await.drain().collect();
    if to_close.is_empty() {
        return;
    }

    let Ok(std::net::IpAddr::V4(lan_ip)) = address::local_lan_ip().parse::<std::net::IpAddr>()
    else {
        return;
    };

    let closes = to_close.into_iter().map(|(server_id, external_port)| {
        let app = app.clone();
        async move {
            let Ok(config) = service::find_config(&app, &server_id).await else {
                return;
            };
            let _ = portforward::close(
                address::forward_protocol(config.loader),
                external_port,
                lan_ip,
            )
            .await;
        }
    });

    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        futures_util::future::join_all(closes),
    )
    .await;
}

#[tauri::command]
pub async fn restart_server(app: AppHandle, server_id: String) -> AppResult<()> {
    service::restart_server(&app, &server_id).await
}

#[tauri::command]
pub async fn kill_server(state: State<'_, AppState>, server_id: String) -> AppResult<()> {
    process::kill(&state.running, &server_id).await
}

#[tauri::command]
pub async fn send_server_command(
    state: State<'_, AppState>,
    server_id: String,
    command: String,
) -> AppResult<()> {
    process::send_command(&state.running, &server_id, &command).await
}

#[tauri::command]
pub async fn server_statuses(
    state: State<'_, AppState>,
) -> AppResult<HashMap<String, ServerStatus>> {
    let snapshot = process::statuses(&state.running).await;
    Ok(snapshot)
}

#[tauri::command]
pub async fn detect_java(state: State<'_, AppState>) -> AppResult<Vec<JavaInstall>> {
    let installs = java::detect_installs(&state.managed_java_dir()).await;
    Ok(installs)
}

/// Opens the folder holding Blockparty's log files in the OS file manager, so a
/// user can grab them when reporting a problem.
#[tauri::command]
pub async fn open_logs_dir(app: AppHandle) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;

    let dir = app
        .path()
        .app_log_dir()
        .map_err(|error| AppError::Process(format!("could not find the log folder: {error}")))?;
    std::fs::create_dir_all(&dir).ok();
    app.opener()
        .open_path(dir.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|error| AppError::Process(format!("could not open the log folder: {error}")))?;
    Ok(())
}

/// The pixel size Minecraft requires for `server-icon.png`.
const SERVER_ICON_SIZE: u32 = 64;
const SERVER_ICON_FILE: &str = "server-icon.png";

/// Sets the server's list icon from any image file: resized to the required
/// 64x64 and saved as `server-icon.png`. Applies on the next start.
#[tauri::command]
pub async fn set_server_icon(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    source_path: PathBuf,
) -> AppResult<()> {
    let config = service::find_config(&app, &server_id).await?;
    let icon_path = state.server_dir(&config).join(SERVER_ICON_FILE);

    let conversion = tokio::task::spawn_blocking(move || {
        let source = image::open(&source_path)
            .map_err(|image_error| AppError::InvalidInput(image_error.to_string()))?;
        let resized = source.resize_exact(
            SERVER_ICON_SIZE,
            SERVER_ICON_SIZE,
            image::imageops::FilterType::Lanczos3,
        );
        resized
            .save(&icon_path)
            .map_err(|image_error| AppError::Process(image_error.to_string()))
    })
    .await
    .map_err(|join_error| AppError::Process(join_error.to_string()))?;
    conversion
}

/// The current server icon as a data URL, if one is set.
#[tauri::command]
pub async fn get_server_icon(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Option<String>> {
    use base64::Engine;

    let config = service::find_config(&app, &server_id).await?;
    let icon_path = state.server_dir(&config).join(SERVER_ICON_FILE);
    if !icon_path.exists() {
        return Ok(None);
    }

    let bytes = std::fs::read(&icon_path)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(Some(format!("data:image/png;base64,{encoded}")))
}

#[tauri::command]
pub async fn remove_server_icon(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<()> {
    let config = service::find_config(&app, &server_id).await?;
    let icon_path = state.server_dir(&config).join(SERVER_ICON_FILE);
    if icon_path.exists() {
        std::fs::remove_file(&icon_path)?;
    }
    Ok(())
}

/// Recovery hammer: kills every Java process Blockparty is responsible for
/// (tracked or orphaned). Returns how many were terminated.
#[tauri::command]
pub async fn kill_all_java(state: State<'_, AppState>) -> AppResult<u32> {
    let server_dirs: Vec<PathBuf> = {
        let registry = state.registry.lock().await;
        registry
            .servers
            .iter()
            .map(|config| state.server_dir(config))
            .collect()
    };

    let killed_count =
        process::kill_all_blockparty_java(state.managed_java_dir(), server_dirs).await;
    Ok(killed_count)
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<AppSettings> {
    Ok(AppSettings {
        servers_base_dir: state.servers_base_dir().await?,
    })
}

/// Shows the wizard where a server's files would land for a given name and
/// (optional) parent folder, before anything is created.
#[tauri::command]
pub async fn preview_server_dir(
    state: State<'_, AppState>,
    name: String,
    location_parent: Option<PathBuf>,
) -> AppResult<PathBuf> {
    let parent = match location_parent {
        Some(chosen) => chosen,
        None => state.servers_base_dir().await?,
    };

    let slug = servers::folder_slug(&name);
    let preview = servers::unique_server_dir(&parent, &slug);
    Ok(preview)
}

/// Changes where new servers are created by default. Existing servers keep
/// their current folders.
#[tauri::command]
pub async fn set_servers_base_dir(
    state: State<'_, AppState>,
    path: PathBuf,
) -> AppResult<AppSettings> {
    if path.as_os_str().is_empty() {
        return Err(AppError::InvalidInput("path is empty".to_string()));
    }
    std::fs::create_dir_all(&path)?;
    state.set_servers_base_dir(&path).await?;
    Ok(AppSettings {
        servers_base_dir: path,
    })
}

/// Resolves the parent directory for a new server. A freshly chosen folder
/// becomes the new default for next time.
async fn resolve_location_parent(
    state: &State<'_, AppState>,
    chosen_parent: Option<PathBuf>,
) -> AppResult<PathBuf> {
    let Some(chosen) = chosen_parent else {
        return state.servers_base_dir().await;
    };

    if chosen != state.servers_base_dir().await? {
        state.set_servers_base_dir(&chosen).await?;
    }
    Ok(chosen)
}

/// Where the app database currently lives, for the Settings UI.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageLocation {
    pub dir: String,
    pub is_default: bool,
}

#[tauri::command]
pub async fn get_storage_location(app: AppHandle) -> AppResult<StorageLocation> {
    let default_dir = db::default_db_dir(&app)?;
    let current_dir = db::resolve_db_dir(&app)?;
    Ok(StorageLocation {
        is_default: paths_match(&current_dir, &default_dir),
        dir: current_dir.to_string_lossy().to_string(),
    })
}

/// Moves the database to a new directory: relocates the file, then writes
/// (or, if choosing the default location, removes) the `.location` pointer
/// file that always lives in the default app-data directory.
#[tauri::command]
pub async fn set_storage_location(
    app: AppHandle,
    state: State<'_, AppState>,
    dir: PathBuf,
) -> AppResult<StorageLocation> {
    relocate_storage(&app, &state, dir).await
}

/// Moves the database back to the default location and removes the pointer
/// file, undoing `set_storage_location`.
#[tauri::command]
pub async fn reset_storage_location(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<StorageLocation> {
    let default_dir = db::default_db_dir(&app)?;
    relocate_storage(&app, &state, default_dir).await
}

async fn relocate_storage(
    app: &AppHandle,
    state: &AppState,
    dir: PathBuf,
) -> AppResult<StorageLocation> {
    if dir.as_os_str().is_empty() {
        return Err(AppError::InvalidInput("path is empty".to_string()));
    }
    std::fs::create_dir_all(&dir)?;

    let default_dir = db::default_db_dir(app)?;
    let is_default = paths_match(&dir, &default_dir);
    let new_db_path = dir.join(db::DB_FILE_NAME);

    {
        let mut db = state.db.lock().await;
        db.relocate(new_db_path)?;
    }

    std::fs::create_dir_all(&default_dir)?;
    let pointer_path = default_dir.join(db::LOCATION_POINTER_FILE);
    if is_default {
        if pointer_path.exists() {
            std::fs::remove_file(&pointer_path)?;
        }
    } else {
        crate::storage::fsutil::atomic_write(&pointer_path, dir.to_string_lossy().as_bytes())?;
    }

    Ok(StorageLocation {
        dir: dir.to_string_lossy().to_string(),
        is_default,
    })
}

/// Whether two directories are "the same location" for the purpose of
/// deciding whether the pointer file should exist. Canonicalizes first so
/// trivial differences (trailing slash, case on Windows) don't cause a
/// spurious "not default" reading; falls back to plain equality when either
/// side can't be resolved (e.g. doesn't exist yet).
fn paths_match(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Fields of a server a user may edit after creation.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateServerRequest {
    pub name: String,
    pub memory_mb: u32,
    /// `None` means auto-resolve (or download) a suitable Java.
    pub java_path: Option<PathBuf>,
    /// `None` resets to the default `backups` folder in the server dir.
    pub backups_dir: Option<PathBuf>,
    pub java_args: Option<String>,
    pub start_command: Option<String>,
    /// Keep only this many newest backups; `None` keeps everything.
    pub backup_retention: Option<u32>,
}

#[tauri::command]
pub async fn update_server(
    state: State<'_, AppState>,
    server_id: String,
    request: UpdateServerRequest,
) -> AppResult<ServerConfig> {
    let trimmed_name = request.name.trim();
    if trimmed_name.is_empty() {
        return Err(AppError::InvalidInput("server name is empty".to_string()));
    }

    let mut registry = state.registry.lock().await;
    let position = registry
        .servers
        .iter()
        .position(|server| server.id == server_id)
        .ok_or_else(|| AppError::ServerNotFound(server_id.clone()))?;

    let config = &mut registry.servers[position];
    config.name = trimmed_name.to_string();
    config.memory_mb = request.memory_mb;
    config.java_path = request.java_path;
    config.backups_dir = request.backups_dir;
    config.java_args = servers::normalized_option(&request.java_args);
    config.start_command = servers::normalized_option(&request.start_command);
    config.backup_retention = request.backup_retention;
    let updated = config.clone();
    drop(registry);

    servers::save_server_settings(&updated)?;
    Ok(updated)
}

#[tauri::command]
pub async fn server_players(state: State<'_, AppState>) -> AppResult<HashMap<String, Vec<String>>> {
    let snapshot = process::players(&state.running).await;
    Ok(snapshot)
}

/// Everyone who has ever joined this server, with live online/banned state.
#[tauri::command]
pub async fn get_player_roster(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Vec<RosterEntry>> {
    let config = service::find_config(&app, &server_id).await?;

    let online_by_server = process::players(&state.running).await;
    let online_players = online_by_server
        .get(&server_id)
        .cloned()
        .unwrap_or_default();
    let banned_names = roster::read_banned_names(&state.server_dir(&config));

    let entries = state
        .rosters
        .entries(&server_id, &online_players, &banned_names)
        .await;
    Ok(entries)
}

/// The address players connect to: this machine's LAN IP and the server's
/// configured port.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerAddress {
    pub lan_ip: String,
    pub port: String,
}

#[tauri::command]
pub async fn get_server_address(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<ServerAddress> {
    let config = service::find_config(&app, &server_id).await?;
    let server_dir = state.server_dir(&config);

    let address = ServerAddress {
        lan_ip: address::local_lan_ip(),
        port: address::configured_port(&server_dir, config.loader),
    };
    Ok(address)
}

/// The result of trying to open a server's port to the internet over UPnP.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardResult {
    /// Whether a mapping was successfully added on the router.
    pub success: bool,
    /// The address to share with friends (public IP + port), when there is one.
    pub public_address: Option<String>,
    /// The router forwarded the port, but the ISP's shared (CGNAT) addressing
    /// means friends still likely can't connect.
    pub cgnat: bool,
    /// A human-friendly explanation for the UI.
    pub message: String,
}

impl ForwardResult {
    fn failure(message: impl Into<String>) -> Self {
        ForwardResult {
            success: false,
            public_address: None,
            cgnat: false,
            message: message.into(),
        }
    }
}

/// Builds the "it's forwarded" outcome for a mapping on `wan_ip`, including the
/// CGNAT check — shared by opening a mapping and by finding an existing one.
async fn forward_success(wan_ip: std::net::IpAddr, external_port: u16) -> ForwardResult {
    let public = portforward::public_ip().await;
    // A WAN address that isn't what the internet sees means another NAT sits
    // in front of the router.
    let cgnat = portforward::is_behind_carrier_nat(wan_ip)
        || public
            .as_deref()
            .map(|ip| ip != wan_ip.to_string())
            .unwrap_or(false);
    let host = public.unwrap_or_else(|| wan_ip.to_string());

    let message = if cgnat {
        "Your router forwarded the port, but your ISP uses shared (CGNAT) \
         addressing, so friends likely still can't connect. See the \
         \"Playing over the internet\" docs for options."
            .to_string()
    } else {
        "Port forwarded! Share the address below with your friends.".to_string()
    };

    ForwardResult {
        success: true,
        public_address: Some(format!("{host}:{external_port}")),
        cgnat,
        message,
    }
}

/// Opens (UPnP-forwards) a server's port so players can connect over the
/// internet. Expected failures — no UPnP, or CGNAT — come back as a descriptive
/// [`ForwardResult`] rather than an error, so the UI can guide the user.
#[tauri::command]
pub async fn open_port_forward(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<ForwardResult> {
    let config = service::find_config(&app, &server_id).await?;
    let server_dir = state.server_dir(&config);
    let port_text = address::configured_port(&server_dir, config.loader);

    let port: u16 = match port_text.parse() {
        Ok(port) => port,
        Err(_) => {
            return Ok(ForwardResult::failure(format!(
                "The server port \"{port_text}\" isn't a valid number."
            )));
        }
    };

    let lan_ip = match address::local_lan_ip().parse() {
        Ok(std::net::IpAddr::V4(ip)) => ip,
        _ => {
            return Ok(ForwardResult::failure(
                "Couldn't determine this PC's LAN IP address.",
            ));
        }
    };

    match portforward::open(address::forward_protocol(config.loader), port, lan_ip).await {
        Ok((wan_ip, external_port)) => {
            // Remember the external port so an explicit stop closes the right
            // mapping instead of leaving the port open to the internet — the
            // router may have handed back a different port than we asked for
            // if it wouldn't give up a stale conflicting mapping.
            state
                .forwarded
                .lock()
                .await
                .insert(server_id.clone(), external_port);
            Ok(forward_success(wan_ip, external_port).await)
        }
        Err(portforward::PortForwardError::NoGateway) => Ok(ForwardResult::failure(
            "No UPnP router found. Check that UPnP is enabled on your router, and that \
             this PC isn't on a VPN or a guest network — either one hides the router \
             from Blockparty. Otherwise you can forward the port manually; see the \
             \"Playing over the internet\" docs.",
        )),
        Err(error) => Ok(ForwardResult::failure(format!(
            "Couldn't set up forwarding automatically ({error}). You can forward the \
             port manually — see the \"Playing over the internet\" docs."
        ))),
    }
}

/// Whether this server's port is *already* forwarded on the router.
///
/// Router mappings outlive the app, so on startup the UI asks this rather than
/// assuming nothing is forwarded. Returns null when it isn't forwarded, or the
/// router doesn't speak UPnP — neither is an error worth bothering the user
/// with here.
#[tauri::command]
pub async fn port_forward_status(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Option<ForwardResult>> {
    let config = service::find_config(&app, &server_id).await?;
    let server_dir = state.server_dir(&config);

    let Ok(port) = address::configured_port(&server_dir, config.loader).parse::<u16>() else {
        return Ok(None);
    };
    let Ok(std::net::IpAddr::V4(lan_ip)) = address::local_lan_ip().parse::<std::net::IpAddr>()
    else {
        return Ok(None);
    };

    match portforward::status(address::forward_protocol(config.loader), port, lan_ip).await {
        Ok(Some((wan_ip, external_port))) => {
            // A mapping left over from a previous run — track it so stopping the
            // server still closes it this session.
            state
                .forwarded
                .lock()
                .await
                .insert(server_id.clone(), external_port);
            Ok(Some(forward_success(wan_ip, external_port).await))
        }
        _ => Ok(None),
    }
}

/// Removes a server's UPnP port mapping. Best-effort: a missing mapping or an
/// unreachable router is not treated as an error.
#[tauri::command]
pub async fn close_port_forward(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<()> {
    let tracked_port = state.forwarded.lock().await.remove(&server_id);

    let config = service::find_config(&app, &server_id).await?;
    let server_dir = state.server_dir(&config);
    // Fall back to the configured (internal) port if we don't have a tracked
    // external port — e.g. the mapping was found on a previous external port
    // via `status` but the app hasn't recorded it this session.
    let port = match tracked_port {
        Some(port) => Ok(port),
        None => address::configured_port(&server_dir, config.loader).parse::<u16>(),
    };
    let lan_ip = address::local_lan_ip().parse::<std::net::IpAddr>();

    if let (Ok(port), Ok(std::net::IpAddr::V4(lan_ip))) = (port, lan_ip) {
        let _ = portforward::close(address::forward_protocol(config.loader), port, lan_ip).await;
    }
    Ok(())
}

/// Full detail for one player, for the player page.
#[tauri::command]
pub async fn get_player_detail(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    player_name: String,
) -> AppResult<Option<roster::PlayerDetail>> {
    let config = service::find_config(&app, &server_id).await?;

    let online_by_server = process::players(&state.running).await;
    let online_players = online_by_server
        .get(&server_id)
        .cloned()
        .unwrap_or_default();

    let bans = roster::read_bans(&state.server_dir(&config));
    let ban = bans.iter().find(|record| record.name == player_name);
    let banned = ban.is_some();
    let ban_reason = ban.and_then(|record| record.reason.clone());

    let mut detail = state
        .rosters
        .detail(
            &server_id,
            &player_name,
            &online_players,
            banned,
            ban_reason,
        )
        .await;

    // The console only reports a game mode when someone runs a logged
    // `/gamemode` command, so most players have none recorded. Fall back to the
    // ground truth in the world save (their playerdata, then the world default).
    if let Some(detail) = detail.as_mut() {
        if detail.last_game_mode.is_none() {
            detail.last_game_mode =
                crate::players::playerdata::game_mode(&state.server_dir(&config), &player_name);
        }
    }

    Ok(detail)
}

#[tauri::command]
pub async fn list_server_files(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    rel_path: String,
) -> AppResult<Vec<files::DirEntry>> {
    let config = service::find_config(&app, &server_id).await?;
    files::list_dir(&state.server_dir(&config), &rel_path)
}

#[tauri::command]
pub async fn read_server_file(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    rel_path: String,
) -> AppResult<String> {
    let config = service::find_config(&app, &server_id).await?;
    files::read_text(&state.server_dir(&config), &rel_path)
}

#[tauri::command]
pub async fn write_server_file(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    rel_path: String,
    contents: String,
) -> AppResult<()> {
    let config = service::find_config(&app, &server_id).await?;
    files::write_text(&state.server_dir(&config), &rel_path, &contents)
}

#[tauri::command]
pub async fn delete_server_file(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    rel_path: String,
) -> AppResult<()> {
    let config = service::find_config(&app, &server_id).await?;
    files::delete_entry(&state.server_dir(&config), &rel_path)
}

/// The Modrinth loader facet for a plugin-capable server, or an error for
/// software that doesn't take plugins.
fn plugin_facet(config: &ServerConfig) -> AppResult<&'static str> {
    config.loader.plugin_facet().ok_or_else(|| {
        AppError::InvalidInput("this server type does not support plugins".to_string())
    })
}

/// The Modrinth loader facet for a mod-capable server, or an error for
/// software that doesn't take mods.
fn mod_facet(config: &ServerConfig) -> AppResult<&'static str> {
    config
        .loader
        .mod_facet()
        .ok_or_else(|| AppError::InvalidInput("this server type does not support mods".to_string()))
}

/// Empty for proxies (whose plugins aren't tagged by Minecraft version).
fn plugin_mc_version(config: &ServerConfig) -> &str {
    if config.loader.is_proxy() {
        ""
    } else {
        &config.mc_version
    }
}

/// Key in `kv_settings` for the user-supplied CurseForge API key.
const CURSEFORGE_API_KEY_KV_KEY: &str = "curseforge_api_key";

async fn curseforge_api_key(state: &AppState) -> AppResult<Option<String>> {
    let db = state.db.lock().await;
    db.get_kv(CURSEFORGE_API_KEY_KV_KEY)
}

#[tauri::command]
pub async fn get_curseforge_api_key(state: State<'_, AppState>) -> AppResult<Option<String>> {
    curseforge_api_key(&state).await
}

#[tauri::command]
pub async fn set_curseforge_api_key(state: State<'_, AppState>, api_key: String) -> AppResult<()> {
    let db = state.db.lock().await;
    db.set_kv(CURSEFORGE_API_KEY_KV_KEY, api_key.trim())
}

/// Records provenance for a freshly installed/updated addon, so a later
/// update check knows what to compare against.
async fn record_addon_install(
    state: &AppState,
    server_id: &str,
    file_name: &str,
    project_id: &str,
    ctx: sources::MarketplaceContext<'_>,
    version: &sources::AddonVersion,
) -> AppResult<()> {
    let record = db::PluginInstallRecord {
        server_id: server_id.to_string(),
        file_name: file_name.to_string(),
        source: ctx.source.as_db_str().to_string(),
        project_id: Some(project_id.to_string()),
        version_id: Some(version.version_id.clone()),
        version_number: Some(version.version_number.clone()),
        mc_version: Some(ctx.mc_version.to_string()),
        loader_facet: Some(ctx.loader_facet.to_string()),
        installed_at_unix: servers::current_unix_time(),
    };
    let db = state.db.lock().await;
    db.record_plugin_install(&record)
}

#[tauri::command]
pub async fn list_plugins(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Vec<plugins::InstalledPlugin>> {
    let config = service::find_config(&app, &server_id).await?;
    plugins::list_installed(&state.server_dir(&config))
}

#[tauri::command]
pub async fn set_plugin_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    file_name: String,
    enabled: bool,
) -> AppResult<String> {
    let config = service::find_config(&app, &server_id).await?;
    plugins::set_enabled(&state.server_dir(&config), &file_name, enabled)
}

#[tauri::command]
pub async fn delete_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    file_name: String,
) -> AppResult<()> {
    let config = service::find_config(&app, &server_id).await?;
    plugins::delete(&state.server_dir(&config), &file_name)?;
    let db = state.db.lock().await;
    db.remove_plugin_install(&server_id, &file_name)
}

#[tauri::command]
pub async fn search_plugins(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    source: sources::AddonSource,
    query: String,
) -> AppResult<Vec<sources::AddonSearchResult>> {
    let config = service::find_config(&app, &server_id).await?;
    let ctx = sources::MarketplaceContext {
        source,
        loader_facet: plugin_facet(&config)?,
        mc_version: plugin_mc_version(&config),
        curseforge_api_key: None,
    };
    plugins::search(&state.http, ctx, &query).await
}

#[tauri::command]
pub async fn install_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    source: sources::AddonSource,
    project_id: String,
) -> AppResult<plugins::InstalledPlugin> {
    let config = service::find_config(&app, &server_id).await?;
    let ctx = sources::MarketplaceContext {
        source,
        loader_facet: plugin_facet(&config)?,
        mc_version: plugin_mc_version(&config),
        curseforge_api_key: None,
    };
    let server_dir = state.server_dir(&config);
    let report_progress =
        installers::progress_event_reporter(app, config.id.clone(), "download-plugin");
    let outcome =
        plugins::install(&state.http, &server_dir, ctx, &project_id, &report_progress).await?;
    record_addon_install(
        &state,
        &server_id,
        &outcome.addon.file_name,
        &project_id,
        ctx,
        &outcome.version,
    )
    .await?;
    Ok(outcome.addon)
}

#[tauri::command]
pub async fn check_plugin_updates(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Vec<plugins::PluginUpdateStatus>> {
    let config = service::find_config(&app, &server_id).await?;
    let server_dir = state.server_dir(&config);
    let records = {
        let db = state.db.lock().await;
        db.list_plugin_installs(&server_id)?
    };
    plugins::check_for_updates(&state.http, &server_dir, &records).await
}

#[tauri::command]
pub async fn update_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    file_name: String,
) -> AppResult<plugins::InstalledPlugin> {
    let record = {
        let db = state.db.lock().await;
        db.list_plugin_installs(&server_id)?
            .into_iter()
            .find(|record| record.file_name == file_name)
            .ok_or_else(|| {
                AppError::InvalidInput("no update provenance for this plugin".to_string())
            })?
    };
    let source = sources::AddonSource::from_db_str(&record.source)
        .ok_or_else(|| AppError::InvalidInput("unknown plugin source".to_string()))?;
    let project_id = record.project_id.clone().ok_or_else(|| {
        AppError::InvalidInput("no update provenance for this plugin".to_string())
    })?;

    let config = service::find_config(&app, &server_id).await?;
    let ctx = sources::MarketplaceContext {
        source,
        loader_facet: plugin_facet(&config)?,
        mc_version: plugin_mc_version(&config),
        curseforge_api_key: None,
    };
    let server_dir = state.server_dir(&config);
    let report_progress =
        installers::progress_event_reporter(app, config.id.clone(), "download-plugin");
    let outcome =
        plugins::install(&state.http, &server_dir, ctx, &project_id, &report_progress).await?;

    // The updated jar may have a different file name than the one it
    // replaces (e.g. a version bump baked into the name) — drop the old one.
    if outcome.addon.file_name != file_name {
        plugins::delete(&server_dir, &file_name).ok();
        let db = state.db.lock().await;
        db.remove_plugin_install(&server_id, &file_name)?;
    }
    record_addon_install(
        &state,
        &server_id,
        &outcome.addon.file_name,
        &project_id,
        ctx,
        &outcome.version,
    )
    .await?;
    Ok(outcome.addon)
}

#[tauri::command]
pub async fn list_mods(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Vec<mods::InstalledMod>> {
    let config = service::find_config(&app, &server_id).await?;
    mods::list_installed(&state.server_dir(&config))
}

#[tauri::command]
pub async fn set_mod_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    file_name: String,
    enabled: bool,
) -> AppResult<String> {
    let config = service::find_config(&app, &server_id).await?;
    mods::set_enabled(&state.server_dir(&config), &file_name, enabled)
}

#[tauri::command]
pub async fn delete_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    file_name: String,
) -> AppResult<()> {
    let config = service::find_config(&app, &server_id).await?;
    mods::delete(&state.server_dir(&config), &file_name)?;
    let db = state.db.lock().await;
    db.remove_plugin_install(&server_id, &file_name)
}

#[tauri::command]
pub async fn search_mods(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    source: sources::AddonSource,
    query: String,
) -> AppResult<Vec<sources::AddonSearchResult>> {
    let config = service::find_config(&app, &server_id).await?;
    let api_key = curseforge_api_key(&state).await?;
    let ctx = sources::MarketplaceContext {
        source,
        loader_facet: mod_facet(&config)?,
        mc_version: &config.mc_version,
        curseforge_api_key: api_key.as_deref(),
    };
    mods::search(&state.http, ctx, &query).await
}

#[tauri::command]
pub async fn install_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    source: sources::AddonSource,
    project_id: String,
) -> AppResult<mods::InstalledMod> {
    let config = service::find_config(&app, &server_id).await?;
    let api_key = curseforge_api_key(&state).await?;
    let ctx = sources::MarketplaceContext {
        source,
        loader_facet: mod_facet(&config)?,
        mc_version: &config.mc_version,
        curseforge_api_key: api_key.as_deref(),
    };
    let server_dir = state.server_dir(&config);
    let report_progress =
        installers::progress_event_reporter(app, config.id.clone(), "download-mod");
    let outcome =
        mods::install(&state.http, &server_dir, ctx, &project_id, &report_progress).await?;
    record_addon_install(
        &state,
        &server_id,
        &outcome.addon.file_name,
        &project_id,
        ctx,
        &outcome.version,
    )
    .await?;
    Ok(outcome.addon)
}

#[tauri::command]
pub async fn check_mod_updates(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Vec<mods::ModUpdateStatus>> {
    let config = service::find_config(&app, &server_id).await?;
    let server_dir = state.server_dir(&config);
    let api_key = curseforge_api_key(&state).await?;
    let records = {
        let db = state.db.lock().await;
        db.list_plugin_installs(&server_id)?
    };
    mods::check_for_updates(&state.http, &server_dir, &records, api_key.as_deref()).await
}

#[tauri::command]
pub async fn update_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    file_name: String,
) -> AppResult<mods::InstalledMod> {
    let record = {
        let db = state.db.lock().await;
        db.list_plugin_installs(&server_id)?
            .into_iter()
            .find(|record| record.file_name == file_name)
            .ok_or_else(|| {
                AppError::InvalidInput("no update provenance for this mod".to_string())
            })?
    };
    let source = sources::AddonSource::from_db_str(&record.source)
        .ok_or_else(|| AppError::InvalidInput("unknown mod source".to_string()))?;
    let project_id = record
        .project_id
        .clone()
        .ok_or_else(|| AppError::InvalidInput("no update provenance for this mod".to_string()))?;

    let config = service::find_config(&app, &server_id).await?;
    let api_key = curseforge_api_key(&state).await?;
    let ctx = sources::MarketplaceContext {
        source,
        loader_facet: mod_facet(&config)?,
        mc_version: &config.mc_version,
        curseforge_api_key: api_key.as_deref(),
    };
    let server_dir = state.server_dir(&config);
    let report_progress =
        installers::progress_event_reporter(app, config.id.clone(), "download-mod");
    let outcome =
        mods::install(&state.http, &server_dir, ctx, &project_id, &report_progress).await?;

    if outcome.addon.file_name != file_name {
        mods::delete(&server_dir, &file_name).ok();
        let db = state.db.lock().await;
        db.remove_plugin_install(&server_id, &file_name)?;
    }
    record_addon_install(
        &state,
        &server_id,
        &outcome.addon.file_name,
        &project_id,
        ctx,
        &outcome.version,
    )
    .await?;
    Ok(outcome.addon)
}

#[tauri::command]
pub async fn get_server_properties(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Vec<Property>> {
    let config = service::find_config(&app, &server_id).await?;
    let server_dir = state.server_dir(&config);
    properties::read(&server_dir)
}

#[tauri::command]
pub async fn save_server_properties(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    updates: Vec<Property>,
) -> AppResult<()> {
    let config = service::find_config(&app, &server_id).await?;
    let server_dir = state.server_dir(&config);
    properties::write(&server_dir, &updates)
}

#[tauri::command]
pub async fn create_backup(app: AppHandle, server_id: String) -> AppResult<BackupInfo> {
    service::create_backup(&app, &server_id).await
}

#[tauri::command]
pub async fn list_backups(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
) -> AppResult<Vec<BackupInfo>> {
    let config = service::find_config(&app, &server_id).await?;
    backups::list(&state.backups_dir(&config))
}

#[tauri::command]
pub async fn restore_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    file_name: String,
) -> AppResult<()> {
    if process::is_running(&state.running, &server_id).await {
        let message = "stop the server before restoring a backup".to_string();
        return Err(AppError::InvalidInput(message));
    }

    let config = service::find_config(&app, &server_id).await?;
    let server_dir = state.server_dir(&config);
    let backups_dir = state.backups_dir(&config);
    let archive_path = backups::safe_archive_path(&backups_dir, &file_name)?;
    backups::restore(server_dir, backups_dir, archive_path).await
}

#[tauri::command]
pub async fn delete_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    server_id: String,
    file_name: String,
) -> AppResult<()> {
    let config = service::find_config(&app, &server_id).await?;
    backups::delete(&state.backups_dir(&config), &file_name)
}

#[tauri::command]
pub async fn list_tasks(state: State<'_, AppState>) -> AppResult<Vec<ScheduledTask>> {
    let tasks = state.tasks.lock().await;
    Ok(tasks.clone())
}

/// Creates a task (empty id) or updates an existing one (matching id).
#[tauri::command]
pub async fn upsert_task(
    state: State<'_, AppState>,
    mut task: ScheduledTask,
) -> AppResult<ScheduledTask> {
    scheduler::validate_cron(&task.cron)?;
    if task.name.trim().is_empty() {
        return Err(AppError::InvalidInput("task name is empty".to_string()));
    }
    if task.id.is_empty() {
        task.id = uuid::Uuid::new_v4().to_string();
    }

    let mut tasks = state.tasks.lock().await;
    let existing_position = tasks.iter().position(|known| known.id == task.id);
    match existing_position {
        Some(position) => tasks[position] = task.clone(),
        None => tasks.push(task.clone()),
    }

    {
        let db = state.db.lock().await;
        scheduler::save_tasks(&db, &tasks)?;
    }
    Ok(task)
}

#[tauri::command]
pub async fn delete_task(state: State<'_, AppState>, task_id: String) -> AppResult<()> {
    let mut tasks = state.tasks.lock().await;
    let existing_position = tasks
        .iter()
        .position(|known| known.id == task_id)
        .ok_or_else(|| AppError::TaskNotFound(task_id.clone()))?;
    tasks.remove(existing_position);

    {
        let db = state.db.lock().await;
        scheduler::save_tasks(&db, &tasks)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn run_task_now(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: String,
) -> AppResult<()> {
    let task = {
        let tasks = state.tasks.lock().await;
        let found = tasks.iter().find(|known| known.id == task_id);
        found
            .cloned()
            .ok_or_else(|| AppError::TaskNotFound(task_id.clone()))?
    };

    tokio::spawn(scheduler::execute_task(app, task));
    Ok(())
}

/// Unix timestamp of a cron expression's next firing, for UI previews.
#[tauri::command]
pub async fn preview_next_run(cron: String) -> AppResult<Option<i64>> {
    scheduler::validate_cron(&cron)?;
    let next = scheduler::next_occurrence_unix(&cron);
    Ok(next)
}

/// Best-effort cleanup of a half-created server directory; the original
/// installation error is what the user needs to see, not a cleanup failure.
fn remove_dir_best_effort(dir: &std::path::Path) {
    if let Err(error) = std::fs::remove_dir_all(dir) {
        log::warn!("failed to clean up {}: {error}", dir.display());
    }
}
