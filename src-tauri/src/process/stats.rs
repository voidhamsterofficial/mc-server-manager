//! Periodic resource sampling of running server processes, emitted to the
//! UI as `server:stats` events.

use std::time::Duration;

use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};
use tauri::AppHandle;

use crate::events;
use crate::process::{self, emit_event, RunningMap};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);

/// Payload of the `server:stats` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsEvent {
    pub server_id: String,
    /// Whole-machine-normalized CPU usage (0–100).
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub uptime_seconds: u64,
}

/// Spawns the background task that measures every running server forever.
/// Idle when nothing is running. Uses Tauri's runtime because it is called
/// from `setup`, outside any Tokio context.
pub fn spawn_sampler(app: AppHandle, running: RunningMap) {
    tauri::async_runtime::spawn(async move {
        let mut system = System::new();
        let core_count = available_core_count();
        let mut tick = tokio::time::interval(SAMPLE_INTERVAL);

        loop {
            tick.tick().await;
            sample_once(&app, &running, &mut system, core_count).await;
        }
    });
}

async fn sample_once(app: &AppHandle, running: &RunningMap, system: &mut System, core_count: u32) {
    let targets = process::sample_targets(running).await;
    if targets.is_empty() {
        return;
    }

    let pids: Vec<Pid> = targets
        .iter()
        .map(|target| Pid::from_u32(target.pid))
        .collect();
    system.refresh_processes(ProcessesToUpdate::Some(&pids), true);

    for target in targets {
        let Some(measured) = system.process(Pid::from_u32(target.pid)) else {
            continue;
        };

        let normalized_cpu_percent = measured.cpu_usage() / core_count as f32;
        let payload = StatsEvent {
            server_id: target.server_id,
            cpu_percent: normalized_cpu_percent,
            memory_bytes: measured.memory(),
            uptime_seconds: target.started_at.elapsed().as_secs(),
        };
        emit_event(app, events::SERVER_STATS, payload);
    }
}

fn available_core_count() -> u32 {
    let parallelism = std::thread::available_parallelism();
    match parallelism {
        Ok(count) => count.get() as u32,
        Err(_) => 1,
    }
}
