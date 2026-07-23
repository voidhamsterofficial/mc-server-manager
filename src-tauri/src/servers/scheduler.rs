//! Scheduled tasks: cron-driven commands, restarts, and backups, persisted in
//! the app database and executed by a background loop.

use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Local};
use croner::Cron;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::process;
use crate::servers::service;
use crate::servers::state::AppState;
use crate::storage::db::Db;

/// Key in `kv_settings` holding the JSON array of all scheduled tasks.
const TASKS_KEY: &str = "scheduled_tasks";

/// How often the scheduler checks for due tasks. Cron resolution is one
/// minute, so this comfortably never skips an occurrence.
const CHECK_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTask {
    pub id: String,
    pub server_id: String,
    pub name: String,
    /// Standard 5-field cron expression (minute hour day month weekday).
    pub cron: String,
    pub action: TaskAction,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TaskAction {
    Command { command: String },
    Restart,
    Backup,
    Start,
    Stop,
}

impl TaskAction {
    /// Whether the action only makes sense against a live server process.
    /// Sending a command to, stopping, or restarting a stopped server has
    /// nothing to act on; a backup or a start is fine (or only useful) while
    /// it's down.
    fn requires_running_server(&self) -> bool {
        match self {
            TaskAction::Command { .. } | TaskAction::Stop | TaskAction::Restart => true,
            TaskAction::Backup | TaskAction::Start => false,
        }
    }
}

pub fn load_tasks(db: &Db) -> AppResult<Vec<ScheduledTask>> {
    match db.get_kv(TASKS_KEY)? {
        Some(json) => Ok(serde_json::from_str(&json)?),
        None => Ok(Vec::new()),
    }
}

pub fn save_tasks(db: &Db, tasks: &[ScheduledTask]) -> AppResult<()> {
    let serialized = serde_json::to_string(tasks)?;
    db.set_kv(TASKS_KEY, &serialized)
}

pub fn validate_cron(expression: &str) -> AppResult<()> {
    let parsed = Cron::from_str(expression);
    if let Err(parse_error) = parsed {
        return Err(AppError::InvalidCron(parse_error.to_string()));
    }
    Ok(())
}

/// Human-friendly "next run" preview for the UI.
pub fn next_occurrence_unix(expression: &str) -> Option<i64> {
    let cron = Cron::from_str(expression).ok()?;
    let next = cron.find_next_occurrence(&Local::now(), false).ok()?;
    Some(next.timestamp())
}

/// Spawns the background loop that fires due tasks for the app's lifetime.
/// Uses Tauri's runtime because it is called from `setup`, outside any
/// Tokio context.
pub fn spawn_scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_check = Local::now();
        let mut tick = tokio::time::interval(CHECK_INTERVAL);
        // The first tick completes immediately; consume it so tasks created
        // in the past don't all fire at startup.
        tick.tick().await;

        loop {
            tick.tick().await;
            let now = Local::now();
            run_due_tasks(&app, last_check, now).await;
            last_check = now;
        }
    });
}

async fn run_due_tasks(app: &AppHandle, since: DateTime<Local>, now: DateTime<Local>) {
    let tasks = {
        let state = app.state::<AppState>();
        let tasks_guard = state.tasks.lock().await;
        tasks_guard.clone()
    };

    for task in tasks {
        let should_run = task.enabled && is_due(&task.cron, since, now);
        if !should_run {
            continue;
        }
        tokio::spawn(execute_task(app.clone(), task));
    }
}

/// Whether the cron expression has an occurrence in `(since, now]`.
fn is_due(cron_expression: &str, since: DateTime<Local>, now: DateTime<Local>) -> bool {
    let Ok(cron) = Cron::from_str(cron_expression) else {
        return false;
    };
    let Ok(next) = cron.find_next_occurrence(&since, false) else {
        return false;
    };
    next <= now
}

pub async fn execute_task(app: AppHandle, task: ScheduledTask) {
    if task.action.requires_running_server() && !server_is_running(&app, &task.server_id).await {
        // Nothing to act on — skip quietly rather than failing. Backup and
        // Start tasks still fire while the server is stopped.
        log::info!(
            "skipping scheduled task '{}' — the server isn't running",
            task.name
        );
        return;
    }

    let outcome = run_action(&app, &task).await;
    if let Err(error) = outcome {
        log::warn!("scheduled task '{}' failed: {error}", task.name);
    }
}

async fn server_is_running(app: &AppHandle, server_id: &str) -> bool {
    let state = app.state::<AppState>();
    let is_running = process::is_running(&state.running, server_id).await;
    is_running
}

async fn run_action(app: &AppHandle, task: &ScheduledTask) -> AppResult<()> {
    match &task.action {
        TaskAction::Command { command } => {
            let state = app.state::<AppState>();
            process::send_command(&state.running, &task.server_id, command).await
        }
        TaskAction::Restart => service::restart_server(app, &task.server_id).await,
        TaskAction::Backup => {
            service::create_backup(app, &task.server_id).await?;
            Ok(())
        }
        TaskAction::Start => service::start_server(app, &task.server_id).await,
        TaskAction::Stop => service::stop_server(app, &task.server_id).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_command_stop_and_restart_need_a_running_server() {
        let command = TaskAction::Command {
            command: "list".to_string(),
        };
        assert!(command.requires_running_server());
        assert!(TaskAction::Stop.requires_running_server());
        assert!(TaskAction::Restart.requires_running_server());
        assert!(!TaskAction::Backup.requires_running_server());
        assert!(!TaskAction::Start.requires_running_server());
    }
}
