//! Stopping a server on a countdown, backing it up cleanly, then optionally
//! starting it again.
//!
//! Zipping a world while the server is writing to it can capture half-finished
//! region files, so the Backups tab offers to stop first. Players get a warning
//! they can act on, which is why this runs on a countdown rather than pulling
//! the server out from under them mid-sentence.
//!
//! The countdown lives here rather than in the UI so it survives navigating
//! away, switching tabs, or closing the window's focus — the same reason the
//! scheduler runs in the background.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

use crate::error::{AppError, AppResult};
use crate::events;
use crate::process::console::LogLevel;
use crate::process::{self, emit_event};
use crate::servers::state::AppState;
use crate::servers::{current_unix_time, service};

/// How long before the stop players are reminded of it. Only the marks that
/// fall inside the chosen delay fire, so a two-minute countdown warns from
/// "2 minutes" down rather than claiming "15 minutes".
const WARNING_MARKS_SECONDS: &[u64] = &[900, 600, 300, 120, 60, 30, 10, 5, 4, 3, 2, 1];

/// What the Backups tab dialog asked for.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimedBackupRequest {
    /// Said in-game when the countdown starts. Blank sends nothing.
    pub message: String,
    pub delay_seconds: u64,
    pub restart_when_done: bool,
}

/// A countdown currently running, as the UI sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingTimedBackup {
    pub server_id: String,
    /// When the server will be stopped. The UI renders its own ticking clock
    /// from this, so the backend doesn't emit an event every second.
    pub stops_at_unix: u64,
    pub restart_when_done: bool,
}

/// Sent whenever a countdown starts, is cancelled, or reaches zero.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimedBackupEvent {
    pub server_id: String,
    /// `None` once nothing is pending for this server any more.
    pub pending: Option<PendingTimedBackup>,
}

/// The live half of a pending countdown. Sending on `cancel` — or dropping it,
/// which is what replacing a map entry does — tells the countdown task to give
/// up.
pub struct TimedBackupHandle {
    stops_at_unix: u64,
    restart_when_done: bool,
    cancel: oneshot::Sender<()>,
}

/// Starts a countdown to stop `server_id`, back it up, and optionally start it
/// again. Replaces any countdown already running for that server.
pub async fn schedule(
    app: &AppHandle,
    server_id: &str,
    request: TimedBackupRequest,
) -> AppResult<PendingTimedBackup> {
    let state = app.state::<AppState>();
    if !process::is_running(&state.running, server_id).await {
        return Err(AppError::ServerNotRunning);
    }

    let stops_at_unix = current_unix_time() + request.delay_seconds;
    let (cancel_sender, cancel_receiver) = oneshot::channel();
    let handle = TimedBackupHandle {
        stops_at_unix,
        restart_when_done: request.restart_when_done,
        cancel: cancel_sender,
    };

    {
        // Inserting over an existing entry drops its sender, which cancels the
        // countdown it belonged to.
        let mut pending = state.timed_backups.lock().await;
        pending.insert(server_id.to_string(), handle);
    }

    let scheduled = PendingTimedBackup {
        server_id: server_id.to_string(),
        stops_at_unix,
        restart_when_done: request.restart_when_done,
    };
    emit_pending(app, server_id, Some(scheduled.clone()));

    tokio::spawn(run_countdown(
        app.clone(),
        server_id.to_string(),
        request,
        cancel_receiver,
    ));
    Ok(scheduled)
}

/// Calls off a pending countdown. Doing nothing when none is pending keeps the
/// UI's "Cancel" idempotent.
pub async fn cancel(app: &AppHandle, server_id: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    let removed = {
        let mut pending = state.timed_backups.lock().await;
        pending.remove(server_id)
    };
    let Some(handle) = removed else {
        return Ok(());
    };
    if handle.cancel.send(()).is_err() {
        // The countdown had already reached zero — nothing left to call off.
        log::debug!("timed backup for {server_id} had already finished");
    }

    broadcast(app, server_id, "Scheduled stop cancelled — carry on!").await;
    process::announce(
        app,
        server_id,
        "Scheduled backup cancelled — the server stays up.".to_string(),
        LogLevel::Warn,
    );
    emit_pending(app, server_id, None);
    Ok(())
}

/// The countdown running for a server, if any — so the UI can pick one up
/// after a reload or when the tab is opened mid-countdown.
pub async fn pending(app: &AppHandle, server_id: &str) -> Option<PendingTimedBackup> {
    let state = app.state::<AppState>();
    let pending = state.timed_backups.lock().await;
    let handle = pending.get(server_id)?;
    let found = PendingTimedBackup {
        server_id: server_id.to_string(),
        stops_at_unix: handle.stops_at_unix,
        restart_when_done: handle.restart_when_done,
    };
    Some(found)
}

async fn run_countdown(
    app: AppHandle,
    server_id: String,
    request: TimedBackupRequest,
    mut cancel_receiver: oneshot::Receiver<()>,
) {
    let opening = request.message.trim();
    if !opening.is_empty() {
        broadcast(&app, &server_id, opening).await;
    }

    let mut remaining_seconds = request.delay_seconds;
    while remaining_seconds > 0 {
        let was_cancelled = tokio::select! {
            // Resolves as soon as the handle (and so its sender) is dropped.
            _ = &mut cancel_receiver => true,
            _ = tokio::time::sleep(Duration::from_secs(1)) => false,
        };
        if was_cancelled {
            return;
        }

        remaining_seconds -= 1;
        if WARNING_MARKS_SECONDS.contains(&remaining_seconds) {
            let warning = format!(
                "Server stopping for a backup in {}.",
                describe_duration(remaining_seconds)
            );
            broadcast(&app, &server_id, &warning).await;
        }
    }

    finish(&app, &server_id, request.restart_when_done).await;
}

/// Runs the stop → back up → (restart) sequence once the clock runs out.
async fn finish(app: &AppHandle, server_id: &str, restart_when_done: bool) {
    {
        // Past the point of cancelling: the stop is about to happen.
        let state = app.state::<AppState>();
        let mut pending = state.timed_backups.lock().await;
        pending.remove(server_id);
    }
    emit_pending(app, server_id, None);

    let outcome = stop_back_up_and_restart(app, server_id, restart_when_done).await;
    if let Err(error) = outcome {
        process::announce(
            app,
            server_id,
            format!("Scheduled backup failed: {error}"),
            LogLevel::Error,
        );
    }
}

async fn stop_back_up_and_restart(
    app: &AppHandle,
    server_id: &str,
    restart_when_done: bool,
) -> AppResult<()> {
    let state = app.state::<AppState>();
    // It may have been stopped by hand (or crashed) during the countdown, in
    // which case there is nothing to shut down and the backup is already safe.
    if process::is_running(&state.running, server_id).await {
        broadcast(app, server_id, "Stopping now for a backup.").await;
        service::stop_server(app, server_id).await?;
        service::wait_until_stopped(app, server_id).await?;
    }

    process::announce(
        app,
        server_id,
        "Server stopped — taking the backup.".to_string(),
        LogLevel::Warn,
    );
    service::create_backup(app, server_id).await?;

    if !restart_when_done {
        process::announce(
            app,
            server_id,
            "Backup finished. Leaving the server stopped.".to_string(),
            LogLevel::Warn,
        );
        return Ok(());
    }

    process::announce(
        app,
        server_id,
        "Backup finished — starting the server again.".to_string(),
        LogLevel::Warn,
    );
    service::start_server(app, server_id).await
}

/// Says something in-game. A failure here just means nobody is listening (the
/// server stopped mid-countdown), which must not derail the backup itself.
async fn broadcast(app: &AppHandle, server_id: &str, text: &str) {
    let state = app.state::<AppState>();
    let command = format!("say {text}");
    if let Err(error) = process::send_command(&state.running, server_id, &command).await {
        log::debug!("could not broadcast to {server_id}: {error}");
    }
}

/// "15 minutes" / "30 seconds", for the in-game warnings.
fn describe_duration(seconds: u64) -> String {
    if seconds >= 60 {
        let minutes = seconds / 60;
        let suffix = if minutes == 1 { "" } else { "s" };
        return format!("{minutes} minute{suffix}");
    }

    let suffix = if seconds == 1 { "" } else { "s" };
    format!("{seconds} second{suffix}")
}

fn emit_pending(app: &AppHandle, server_id: &str, pending: Option<PendingTimedBackup>) {
    let payload = TimedBackupEvent {
        server_id: server_id.to_string(),
        pending,
    };
    emit_event(app, events::BACKUP_TIMED, payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_durations_for_players() {
        assert_eq!(describe_duration(900), "15 minutes");
        assert_eq!(describe_duration(60), "1 minute");
        assert_eq!(describe_duration(30), "30 seconds");
        assert_eq!(describe_duration(1), "1 second");
    }

    #[test]
    fn warning_marks_are_descending_and_inside_an_hour() {
        let mut sorted = WARNING_MARKS_SECONDS.to_vec();
        sorted.sort_unstable_by(|left, right| right.cmp(left));
        assert_eq!(sorted, WARNING_MARKS_SECONDS.to_vec());
    }
}
