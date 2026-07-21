//! Orchestration shared by the Tauri command layer and the scheduler (and,
//! later, the remote web panel): start/stop/restart flows and backups.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::installers;
use crate::java;
use crate::process;
use crate::servers::state::AppState;
use crate::servers::{Loader, ServerConfig};
use crate::storage::backups::{self, BackupInfo};

/// How long a restart waits for the old process to exit before giving up.
const RESTART_WAIT: Duration = Duration::from_secs(60);

/// Delay between `save-all` and zipping when backing up a live server, so
/// the world flush can finish.
const LIVE_BACKUP_FLUSH_DELAY: Duration = Duration::from_secs(2);

/// How long to wait before bringing a crashed server back, so one that dies
/// during startup can't spin restarting many times a second.
const CRASH_RESTART_DELAY: Duration = Duration::from_secs(5);

/// Decides what to do about a server that crashed — the listener for
/// `server:crashed`. Kept here rather than in the process supervisor so the
/// low-level process layer never has to know about the start path.
///
/// Restarts are capped, so a server that dies on every boot eventually stays
/// down instead of retrying forever, and everything it decides is announced
/// in the server's own console: a server that silently resurrects itself is
/// harder to reason about than one that stays down.
pub async fn restart_after_crash(app: &AppHandle, server_id: &str) {
    let state = app.state::<AppState>();

    // On the way out of the app, leave it down — restarting here would spawn
    // a server process the app is no longer around to own.
    if state.shutting_down.load(Ordering::SeqCst) {
        return;
    }

    let Ok(config) = find_config(app, server_id).await else {
        // Deleted while it was going down — nothing to bring back.
        return;
    };
    let Some(limit) = config.crash_restart_limit else {
        return;
    };

    let attempt = {
        let mut restarts = state.crash_restarts.lock().await;
        let attempt = restarts.entry(server_id.to_string()).or_insert(0);
        *attempt += 1;
        *attempt
    };

    if attempt > limit {
        process::announce(
            app,
            server_id,
            format!(
                "Crashed again after {limit} automatic restart(s) — leaving it stopped. Start it by hand once the cause is fixed."
            ),
            process::console::LogLevel::Error,
        );
        return;
    }

    process::announce(
        app,
        server_id,
        format!(
            "Server crashed (attempt {attempt} of {limit}) — restarting automatically in {}s.",
            CRASH_RESTART_DELAY.as_secs()
        ),
        process::console::LogLevel::Warn,
    );
    tokio::time::sleep(CRASH_RESTART_DELAY).await;

    // The user may have started it again by hand while we were waiting, or
    // closed the app.
    if state.shutting_down.load(Ordering::SeqCst) {
        return;
    }
    if process::is_running(&state.running, server_id).await {
        return;
    }

    if let Err(error) = start_server(app, server_id).await {
        process::announce(
            app,
            server_id,
            format!("Automatic restart failed: {error}"),
            process::console::LogLevel::Error,
        );
    }
}

pub async fn find_config(app: &AppHandle, server_id: &str) -> AppResult<ServerConfig> {
    let state = app.state::<AppState>();
    let registry = state.registry.lock().await;
    let config = registry.find(server_id)?.clone();
    Ok(config)
}

pub async fn start_server(app: &AppHandle, server_id: &str) -> AppResult<()> {
    // A start the user asked for ends any crash streak, so a server fixed by
    // hand gets its full allowance again.
    app.state::<AppState>()
        .crash_restarts
        .lock()
        .await
        .remove(server_id);

    let state = app.state::<AppState>();
    let config = find_config(app, server_id).await?;

    let java_executable = if config.loader == Loader::Bds {
        // Bedrock is a native binary — no Java involved.
        PathBuf::new()
    } else {
        match &config.java_path {
            Some(explicit_path) => explicit_path.clone(),
            None => resolve_or_download_java(app, &config).await?,
        }
    };

    let server_dir = state.server_dir(&config);
    process::start(app, &state.running, &config, &server_dir, &java_executable).await
}

/// Finds a suitable installed Java, or automatically downloads the required
/// Temurin JRE when none exists.
pub(crate) async fn resolve_or_download_java(
    app: &AppHandle,
    config: &ServerConfig,
) -> AppResult<PathBuf> {
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
            log::warn!("could not look up the latest Java LTS: {lookup_error}");
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
    let created = backups::create(server_dir, backups_dir.clone()).await?;

    if let Some(keep_newest) = config.backup_retention {
        let pruned = backups::prune(&backups_dir, keep_newest.max(1))?;
        if pruned > 0 {
            log::info!("pruned {pruned} old backup(s) for {server_id}");
        }
    }

    // Let any open Backups tab refresh its (possibly stale) list.
    if let Err(error) = app.emit(crate::events::BACKUP_CREATED, server_id.to_string()) {
        log::warn!("failed to emit backup-created event: {error}");
    }
    Ok(created)
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
