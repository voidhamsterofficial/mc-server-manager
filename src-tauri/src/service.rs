//! Orchestration shared by the Tauri command layer and the scheduler (and,
//! later, the remote web panel): start/stop/restart flows and backups.

use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::backups::{self, BackupInfo};
use crate::error::{AppError, AppResult};
use crate::installers;
use crate::java;
use crate::process;
use crate::servers::ServerConfig;
use crate::state::AppState;

/// How long a restart waits for the old process to exit before giving up.
const RESTART_WAIT: Duration = Duration::from_secs(60);

/// Delay between `save-all` and zipping when backing up a live server, so
/// the world flush can finish.
const LIVE_BACKUP_FLUSH_DELAY: Duration = Duration::from_secs(2);

pub async fn find_config(app: &AppHandle, server_id: &str) -> AppResult<ServerConfig> {
    let state = app.state::<AppState>();
    let registry = state.registry.lock().await;
    let config = registry.find(server_id)?.clone();
    Ok(config)
}

pub async fn start_server(app: &AppHandle, server_id: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    let config = find_config(app, server_id).await?;

    let java_executable = match &config.java_path {
        Some(explicit_path) => explicit_path.clone(),
        None => resolve_or_download_java(app, &config).await?,
    };

    let server_dir = state.server_dir(&config);
    process::start(app, &state.running, &config, &server_dir, &java_executable).await
}

/// Finds a suitable installed Java, or automatically downloads the required
/// Temurin JRE when none exists.
async fn resolve_or_download_java(app: &AppHandle, config: &ServerConfig) -> AppResult<PathBuf> {
    let state = app.state::<AppState>();
    let managed_java_dir = state.managed_java_dir();
    let required_major = required_java_major(&state.http, &config.mc_version).await;

    match java::resolve_for(required_major, &managed_java_dir).await {
        Ok(install) => {
            return Ok(install.path);
        }
        Err(AppError::NoSuitableJava { .. }) => {}
        Err(other_error) => {
            return Err(other_error);
        }
    }

    let _download_guard = state.java_download_lock.lock().await;

    // A concurrent start may have finished the download while we waited.
    if let Ok(install) = java::resolve_for(required_major, &managed_java_dir).await {
        return Ok(install.path);
    }

    let report_progress =
        installers::progress_event_reporter(app.clone(), config.id.clone(), "download-java");
    let downloaded = java::download::install_temurin(
        &state.http,
        required_major,
        &managed_java_dir,
        &report_progress,
    )
    .await?;
    Ok(downloaded.path)
}

/// The Java major a Minecraft version needs: the static mapping for classic
/// `1.x` versions, otherwise the newest LTS Adoptium ships (so brand-new
/// Minecraft versions get a new enough runtime automatically).
async fn required_java_major(client: &reqwest::Client, mc_version: &str) -> u32 {
    if let Some(mapped_major) = java::mapped_java_major(mc_version) {
        return mapped_major;
    }

    match java::download::most_recent_lts(client).await {
        Ok(latest_lts) => latest_lts,
        Err(lookup_error) => {
            eprintln!("could not look up the latest Java LTS: {lookup_error}");
            java::FALLBACK_JAVA_MAJOR
        }
    }
}

pub async fn stop_server(app: &AppHandle, server_id: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    process::stop(app, &state.running, server_id).await
}

pub async fn restart_server(app: &AppHandle, server_id: &str) -> AppResult<()> {
    stop_server(app, server_id).await?;
    wait_until_stopped(app, server_id).await?;
    start_server(app, server_id).await
}

/// Backs up a server. A running server gets a `save-all` first so the world
/// on disk is fresh.
pub async fn create_backup(app: &AppHandle, server_id: &str) -> AppResult<BackupInfo> {
    let state = app.state::<AppState>();
    let config = find_config(app, server_id).await?;

    if process::is_running(&state.running, server_id).await {
        process::send_command(&state.running, server_id, "save-all").await?;
        tokio::time::sleep(LIVE_BACKUP_FLUSH_DELAY).await;
    }

    let server_dir = state.server_dir(&config);
    let backups_dir = state.backups_dir(&config);
    backups::create(server_dir, backups_dir).await
}

async fn wait_until_stopped(app: &AppHandle, server_id: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    let poll_interval = Duration::from_secs(1);
    let attempts = RESTART_WAIT.as_secs();

    for _ in 0..attempts {
        if !process::is_running(&state.running, server_id).await {
            return Ok(());
        }
        tokio::time::sleep(poll_interval).await;
    }

    let message = "server did not stop within the restart window".to_string();
    Err(AppError::Process(message))
}
