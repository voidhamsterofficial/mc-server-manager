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
use crate::servers::{address, Loader, ServerConfig};
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

    // Deliberately NOT start_server: an automatic restart must not clear the
    // crash streak, or the counter resets every attempt and the cap above is
    // never reached. Only a start the user (or a schedule) asks for resets it.
    if let Err(error) = launch_server(app, server_id).await {
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

/// A running server already listening on the port another one wants.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortConflict {
    pub server_id: String,
    pub server_name: String,
    pub port: String,
}

/// Finds a running server already listening on `server_id`'s port.
///
/// Without this the second server starts, then dies deep inside the JVM with
/// a bind error that surfaces as an opaque crash — so the clash is caught
/// before anything launches, while it can still be explained.
pub async fn find_port_conflict(
    app: &AppHandle,
    server_id: &str,
) -> AppResult<Option<PortConflict>> {
    let state = app.state::<AppState>();
    let config = find_config(app, server_id).await?;
    let wanted_port = address::configured_port(&state.server_dir(&config), config.loader);

    let others = {
        let registry = state.registry.lock().await;
        registry.servers.clone()
    };

    for other in others {
        if other.id == config.id {
            continue;
        }
        if !process::is_running(&state.running, &other.id).await {
            continue;
        }

        let other_port = address::configured_port(&state.server_dir(&other), other.loader);
        if other_port != wanted_port {
            continue;
        }

        let conflict = PortConflict {
            server_id: other.id,
            server_name: other.name,
            port: other_port,
        };
        return Ok(Some(conflict));
    }

    Ok(None)
}

pub async fn start_server(app: &AppHandle, server_id: &str) -> AppResult<()> {
    // A start the user asked for ends any crash streak, so a server fixed by
    // hand gets its full allowance again. The crash-restart path calls
    // launch_server directly to keep the streak intact.
    app.state::<AppState>()
        .crash_restarts
        .lock()
        .await
        .remove(server_id);

    launch_server(app, server_id).await
}

/// Launches the server process without touching the crash-restart streak.
///
/// Shared by the user-facing `start_server` (which clears the streak first)
/// and the automatic crash-restart path (which must not).
async fn launch_server(app: &AppHandle, server_id: &str) -> AppResult<()> {
    // Enforced here, not only in the UI: scheduled and bulk starts reach this
    // path too, and none of them should be able to launch a doomed process.
    if let Some(conflict) = find_port_conflict(app, server_id).await? {
        return Err(AppError::PortInUse {
            port: conflict.port,
            other_server: conflict.server_name,
        });
    }

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

/// Stops the server holding a port, waits for it to actually let go, then
/// starts the one that wanted it.
///
/// Sequenced in one call because `stop_server` returns once the shutdown has
/// been asked for, not once the process has exited — starting immediately
/// would race the very port clash this resolves.
pub async fn stop_other_and_start(
    app: &AppHandle,
    running_server_id: &str,
    server_id: &str,
) -> AppResult<()> {
    stop_server(app, running_server_id).await?;
    wait_until_stopped(app, running_server_id).await?;
    start_server(app, server_id).await
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

    let report_progress = backup_progress_reporter(app.clone(), server_id.to_string());
    let created = match backups::create(server_dir, backups_dir.clone(), report_progress).await {
        Ok(created) => created,
        Err(error) => {
            // The UI shows a progress bar from the first progress event; tell
            // it to stop, or a failed backup leaves the bar up for good.
            emit_backup_event(app, crate::events::BACKUP_FAILED, server_id);
            return Err(error);
        }
    };

    if let Some(keep_newest) = config.backup_retention {
        let pruned = backups::prune(&backups_dir, keep_newest.max(1))?;
        if pruned > 0 {
            log::info!("pruned {pruned} old backup(s) for {server_id}");
        }
    }

    // Let any open Backups tab refresh its (possibly stale) list.
    emit_backup_event(app, crate::events::BACKUP_CREATED, server_id);
    Ok(created)
}

/// Emits one of the backup lifecycle events, which all carry just a server id.
fn emit_backup_event(app: &AppHandle, event_name: &str, server_id: &str) {
    if let Err(error) = app.emit(event_name, server_id.to_string()) {
        log::warn!("failed to emit {event_name} event: {error}");
    }
}

/// Emit at most one progress event per whole percent, so zipping a world with
/// tens of thousands of files doesn't flood the UI with events it would only
/// render as the same bar width.
fn backup_progress_reporter(app: AppHandle, server_id: String) -> backups::ProgressCallback {
    use std::sync::atomic::{AtomicU64, Ordering};

    let last_reported_percent = AtomicU64::new(u64::MAX);

    let reporter = move |processed_files: u64, total_files: u64| {
        let percent = match total_files {
            0 => 100,
            total => processed_files * 100 / total,
        };

        let is_final_file = processed_files >= total_files;
        let advanced_enough = percent != last_reported_percent.load(Ordering::Relaxed);
        if !is_final_file && !advanced_enough {
            return;
        }
        last_reported_percent.store(percent, Ordering::Relaxed);

        let payload = backups::BackupProgressEvent {
            server_id: server_id.clone(),
            processed_files,
            total_files,
        };
        if let Err(error) = app.emit(crate::events::BACKUP_PROGRESS, payload) {
            log::warn!("failed to emit backup progress: {error}");
        }
    };
    Box::new(reporter)
}

pub(crate) async fn wait_until_stopped(app: &AppHandle, server_id: &str) -> AppResult<()> {
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
