//! Scheduled tasks: cron-driven commands, restarts, and backups, persisted
//! to `schedules.json` and executed by a background loop.

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Local};
use croner::Cron;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::process;
use crate::service;
use crate::state::AppState;

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

pub fn load_tasks(path: &Path) -> AppResult<Vec<ScheduledTask>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let contents = std::fs::read_to_string(path)?;
    let tasks = serde_json::from_str(&contents)?;
    Ok(tasks)
}

pub fn save_tasks(path: &Path, tasks: &[ScheduledTask]) -> AppResult<()> {
    let serialized = serde_json::to_string_pretty(tasks)?;
    std::fs::write(path, serialized)?;
    Ok(())
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
    let outcome = run_action(&app, &task).await;
    if let Err(error) = outcome {
        eprintln!("scheduled task '{}' failed: {error}", task.name);
    }
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
