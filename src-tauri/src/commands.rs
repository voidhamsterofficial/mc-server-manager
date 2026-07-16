//! Tauri command layer: thin wrappers that validate input and delegate to
//! the domain services. The future web panel reuses the same services.

use std::collections::HashMap;
use std::path::PathBuf;

use tauri::{AppHandle, State};

use serde::Deserialize;

use crate::backups::{self, BackupInfo};
use crate::error::{AppError, AppResult};
use crate::installers::{self, vanilla};
use crate::java::{self, JavaInstall};
use crate::process;
use crate::properties::{self, Property};
use crate::roster::{self, RosterEntry};
use crate::scheduler::{self, ScheduledTask};
use crate::servers::{self, CreateServerRequest, Loader, ServerConfig, ServerStatus};
use crate::service;
use crate::settings::AppSettings;
use crate::state::AppState;

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

    if !request.loader.is_proxy() {
        servers::write_eula_acceptance(&server_dir)?;
        let port_property = vec![Property {
            key: "server-port".to_string(),
            value: request.port.to_string(),
        }];
        properties::write(&server_dir, &port_property)?;
    }

    let mut registry = state.registry.lock().await;
    registry.add(config.clone());
    registry.save(&state.registry_path())?;
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
        let removed = registry
            .remove(&server_id)
            .ok_or_else(|| AppError::ServerNotFound(server_id.clone()))?;
        registry.save(&state.registry_path())?;
        removed
    };

    let server_dir = state.server_dir(&removed_config);
    if server_dir.exists() {
        std::fs::remove_dir_all(&server_dir)?;
    }

    // A deleted server takes its satellite data with it: scheduled tasks
    // and player history.
    {
        let mut tasks = state.tasks.lock().await;
        let before = tasks.len();
        tasks.retain(|task| task.server_id != server_id);
        if tasks.len() != before {
            scheduler::save_tasks(&state.tasks_path(), &tasks)?;
        }
    }
    state.rosters.forget(&server_id).await;
    Ok(())
}

#[tauri::command]
pub async fn start_server(app: AppHandle, server_id: String) -> AppResult<()> {
    service::start_server(&app, &server_id).await
}

#[tauri::command]
pub async fn stop_server(app: AppHandle, server_id: String) -> AppResult<()> {
    service::stop_server(&app, &server_id).await
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
    let settings = state.settings.lock().await;
    Ok(settings.clone())
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
        None => {
            let settings = state.settings.lock().await;
            settings.servers_base_dir.clone()
        }
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

    let mut settings = state.settings.lock().await;
    settings.servers_base_dir = path;
    settings.save(&state.settings_path())?;
    Ok(settings.clone())
}

/// Resolves the parent directory for a new server. A freshly chosen folder
/// becomes the new default for next time.
async fn resolve_location_parent(
    state: &State<'_, AppState>,
    chosen_parent: Option<PathBuf>,
) -> AppResult<PathBuf> {
    let mut settings = state.settings.lock().await;

    let Some(chosen) = chosen_parent else {
        return Ok(settings.servers_base_dir.clone());
    };

    if chosen != settings.servers_base_dir {
        settings.servers_base_dir = chosen.clone();
        settings.save(&state.settings_path())?;
    }
    Ok(chosen)
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

    registry.save(&state.registry_path())?;
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

    scheduler::save_tasks(&state.tasks_path(), &tasks)?;
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

    scheduler::save_tasks(&state.tasks_path(), &tasks)?;
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
        eprintln!("failed to clean up {}: {error}", dir.display());
    }
}
