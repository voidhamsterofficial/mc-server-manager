//! Running-server process management: spawning the Java child, streaming its
//! console output to the UI in batches, and orchestrating shutdown.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::console::{self, ConsoleLine, ConsoleSignal};
use crate::error::{AppError, AppResult};
use crate::events;
use crate::installers::vanilla::SERVER_JAR_NAME;
use crate::platform;
use crate::servers::{ServerConfig, ServerStatus};

/// How long buffered console lines wait before being flushed to the UI.
/// Batching keeps event traffic low when the server logs in bursts.
const CONSOLE_BATCH_INTERVAL: Duration = Duration::from_millis(50);

/// Grace period between sending `stop` and force-killing the process.
const GRACEFUL_STOP_TIMEOUT: Duration = Duration::from_secs(30);

const LINE_CHANNEL_CAPACITY: usize = 1024;
const COMMAND_CHANNEL_CAPACITY: usize = 64;

/// All currently running server processes, keyed by server id.
pub type RunningMap = Arc<Mutex<HashMap<String, ProcessHandle>>>;

/// Live handles to one running server process. The child itself is owned by
/// its supervisor task; we only keep channels to talk to it.
pub struct ProcessHandle {
    command_tx: mpsc::Sender<String>,
    kill_tx: Option<oneshot::Sender<()>>,
    status: ServerStatus,
    stop_requested: bool,
    pid: Option<u32>,
    started_at: std::time::Instant,
    players: Vec<String>,
}

/// One running server the stats sampler should measure.
pub struct SampleTarget {
    pub server_id: String,
    pub pid: u32,
    pub started_at: std::time::Instant,
}

/// Payload of the `server:players` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayersEvent {
    pub server_id: String,
    pub players: Vec<String>,
}

/// Payload of the `server:status` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEvent {
    pub server_id: String,
    pub status: ServerStatus,
}

/// Payload of the `server:console` event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleBatchEvent {
    pub server_id: String,
    pub lines: Vec<ConsoleLine>,
}

/// Spawns the server process and the tasks that service it.
pub async fn start(
    app: &AppHandle,
    running: &RunningMap,
    config: &ServerConfig,
    server_dir: &Path,
    java_executable: &Path,
) -> AppResult<()> {
    let mut running_guard = running.lock().await;
    if running_guard.contains_key(&config.id) {
        return Err(AppError::ServerAlreadyRunning);
    }

    let mut child = spawn_java_process(config, server_dir, java_executable)?;
    let stdin = take_pipe(child.stdin.take())?;
    let stdout = take_pipe(child.stdout.take())?;
    let stderr = take_pipe(child.stderr.take())?;

    let (command_tx, command_rx) = mpsc::channel::<String>(COMMAND_CHANNEL_CAPACITY);
    let (line_tx, line_rx) = mpsc::channel::<String>(LINE_CHANNEL_CAPACITY);
    let (kill_tx, kill_rx) = oneshot::channel::<()>();

    let handle = ProcessHandle {
        command_tx,
        kill_tx: Some(kill_tx),
        status: ServerStatus::Starting,
        stop_requested: false,
        pid: child.id(),
        started_at: std::time::Instant::now(),
        players: Vec::new(),
    };
    running_guard.insert(config.id.clone(), handle);
    drop(running_guard);
    emit_status(app, &config.id, ServerStatus::Starting);

    tokio::spawn(forward_lines(stdout, line_tx.clone()));
    tokio::spawn(forward_lines(stderr, line_tx));
    tokio::spawn(write_commands(stdin, command_rx));
    tokio::spawn(stream_console(
        app.clone(),
        Arc::clone(running),
        config.id.clone(),
        line_rx,
    ));
    tokio::spawn(supervise(
        app.clone(),
        Arc::clone(running),
        config.id.clone(),
        child,
        kill_rx,
    ));

    Ok(())
}

/// Requests a graceful stop (`stop` command) and schedules a force-kill in
/// case the server hangs on the way down.
pub async fn stop(app: &AppHandle, running: &RunningMap, server_id: &str) -> AppResult<()> {
    let command_tx = {
        let mut running_guard = running.lock().await;
        let handle = running_guard
            .get_mut(server_id)
            .ok_or(AppError::ServerNotRunning)?;
        handle.stop_requested = true;
        handle.status = ServerStatus::Stopping;
        handle.command_tx.clone()
    };
    emit_status(app, server_id, ServerStatus::Stopping);

    let send_result = command_tx.send("stop".to_string()).await;
    if send_result.is_err() {
        // The stdin writer already exited, which means the process is
        // already going down; the scheduled force-kill still applies.
    }

    schedule_force_kill(running, server_id);
    Ok(())
}

/// Immediately terminates the server process.
pub async fn kill(running: &RunningMap, server_id: &str) -> AppResult<()> {
    let was_running = send_kill_signal(running, server_id).await;
    if !was_running {
        return Err(AppError::ServerNotRunning);
    }
    Ok(())
}

/// Sends one console command line to the server.
pub async fn send_command(running: &RunningMap, server_id: &str, command: &str) -> AppResult<()> {
    let trimmed_command = command.trim();
    if trimmed_command.is_empty() {
        return Err(AppError::InvalidInput("command is empty".to_string()));
    }

    let command_tx = {
        let running_guard = running.lock().await;
        let handle = running_guard
            .get(server_id)
            .ok_or(AppError::ServerNotRunning)?;
        handle.command_tx.clone()
    };

    let send_result = command_tx.send(trimmed_command.to_string()).await;
    send_result.map_err(|_| AppError::ServerNotRunning)
}

/// Snapshot of every running server's status, for initial UI state.
pub async fn statuses(running: &RunningMap) -> HashMap<String, ServerStatus> {
    let running_guard = running.lock().await;

    let mut snapshot = HashMap::new();
    for (server_id, handle) in running_guard.iter() {
        snapshot.insert(server_id.clone(), handle.status);
    }
    snapshot
}

pub async fn is_running(running: &RunningMap, server_id: &str) -> bool {
    let running_guard = running.lock().await;
    running_guard.contains_key(server_id)
}

/// Snapshot of online players per running server, for initial UI state.
pub async fn players(running: &RunningMap) -> HashMap<String, Vec<String>> {
    let running_guard = running.lock().await;

    let mut snapshot = HashMap::new();
    for (server_id, handle) in running_guard.iter() {
        snapshot.insert(server_id.clone(), handle.players.clone());
    }
    snapshot
}

/// The processes the stats sampler should currently measure.
pub async fn sample_targets(running: &RunningMap) -> Vec<SampleTarget> {
    let running_guard = running.lock().await;

    let mut targets = Vec::new();
    for (server_id, handle) in running_guard.iter() {
        let Some(pid) = handle.pid else {
            continue;
        };
        targets.push(SampleTarget {
            server_id: server_id.clone(),
            pid,
            started_at: handle.started_at,
        });
    }
    targets
}

fn spawn_java_process(
    config: &ServerConfig,
    server_dir: &Path,
    java_executable: &Path,
) -> AppResult<Child> {
    let max_heap_flag = format!("-Xmx{}M", config.memory_mb);
    let min_heap_flag = format!("-Xms{}M", config.memory_mb);

    let mut command = Command::new(java_executable);
    command
        .args([&max_heap_flag, &min_heap_flag])
        .args(["-jar", SERVER_JAR_NAME, "nogui"])
        .current_dir(server_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    platform::hide_console_window(&mut command);

    let child = command.spawn()?;
    Ok(child)
}

fn take_pipe<T>(pipe: Option<T>) -> AppResult<T> {
    let message = "child process pipe was not captured".to_string();
    pipe.ok_or(AppError::Process(message))
}

/// Reads one output stream line-by-line into the shared line channel.
async fn forward_lines<R: AsyncRead + Unpin>(stream: R, line_tx: mpsc::Sender<String>) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let receiver_alive = line_tx.send(line).await.is_ok();
        if !receiver_alive {
            return;
        }
    }
}

/// Owns stdin: forwards queued console commands into the process.
async fn write_commands(mut stdin: ChildStdin, mut command_rx: mpsc::Receiver<String>) {
    while let Some(command) = command_rx.recv().await {
        let line = format!("{command}\n");
        let write_result = stdin.write_all(line.as_bytes()).await;
        if write_result.is_err() {
            return;
        }
        let flush_result = stdin.flush().await;
        if flush_result.is_err() {
            return;
        }
    }
}

/// Collects raw log lines, watches them for state changes, and flushes them
/// to the UI in small batches.
async fn stream_console(
    app: AppHandle,
    running: RunningMap,
    server_id: String,
    mut line_rx: mpsc::Receiver<String>,
) {
    let mut flush_interval = tokio::time::interval(CONSOLE_BATCH_INTERVAL);
    let mut pending_lines: Vec<ConsoleLine> = Vec::new();

    loop {
        tokio::select! {
            maybe_line = line_rx.recv() => {
                let Some(raw_line) = maybe_line else {
                    break;
                };
                ingest_line(&app, &running, &server_id, raw_line, &mut pending_lines).await;
            }
            _ = flush_interval.tick() => {
                flush_console(&app, &server_id, &mut pending_lines);
            }
        }
    }

    flush_console(&app, &server_id, &mut pending_lines);
}

async fn ingest_line(
    app: &AppHandle,
    running: &RunningMap,
    server_id: &str,
    raw_line: String,
    pending_lines: &mut Vec<ConsoleLine>,
) {
    let (line, signal) = console::analyze(&raw_line);

    match signal {
        Some(ConsoleSignal::ServerReady) => {
            set_status(app, running, server_id, ServerStatus::Running).await;
        }
        Some(ConsoleSignal::PlayerJoined(player_name)) => {
            record_player_change(app, running, server_id, PlayerChange::Joined(player_name)).await;
        }
        Some(ConsoleSignal::PlayerLeft(player_name)) => {
            record_player_change(app, running, server_id, PlayerChange::Left(player_name)).await;
        }
        Some(ConsoleSignal::PlayerKicked(player_name)) => {
            let state = app.state::<crate::state::AppState>();
            state.rosters.record_kick(server_id, &player_name).await;
        }
        None => {}
    }

    pending_lines.push(line);
}

fn flush_console(app: &AppHandle, server_id: &str, pending_lines: &mut Vec<ConsoleLine>) {
    if pending_lines.is_empty() {
        return;
    }

    let batch = ConsoleBatchEvent {
        server_id: server_id.to_string(),
        lines: std::mem::take(pending_lines),
    };
    emit_event(app, events::SERVER_CONSOLE, batch);
}

/// Owns the child process: waits for it to exit (or kills it on signal),
/// then removes the handle and reports the final status.
async fn supervise(
    app: AppHandle,
    running: RunningMap,
    server_id: String,
    mut child: Child,
    kill_rx: oneshot::Receiver<()>,
) {
    let exit_was_clean = tokio::select! {
        wait_result = child.wait() => {
            match wait_result {
                Ok(exit_status) => exit_status.success(),
                Err(_) => false,
            }
        }
        _ = kill_rx => {
            force_kill(&mut child).await
        }
    };

    let stop_was_requested = remove_handle(&running, &server_id).await;
    let final_status = if stop_was_requested || exit_was_clean {
        ServerStatus::Stopped
    } else {
        ServerStatus::Crashed
    };
    emit_status(&app, &server_id, final_status);

    let state = app.state::<crate::state::AppState>();
    state.rosters.close_all_sessions(&server_id).await;

    let empty_player_list = PlayersEvent {
        server_id: server_id.clone(),
        players: Vec::new(),
    };
    emit_event(&app, events::SERVER_PLAYERS, empty_player_list);
}

enum PlayerChange {
    Joined(String),
    Left(String),
}

async fn record_player_change(
    app: &AppHandle,
    running: &RunningMap,
    server_id: &str,
    change: PlayerChange,
) {
    let state = app.state::<crate::state::AppState>();
    match &change {
        PlayerChange::Joined(name) => state.rosters.record_join(server_id, name).await,
        PlayerChange::Left(name) => state.rosters.record_leave(server_id, name).await,
    }

    let players = {
        let mut running_guard = running.lock().await;
        let Some(handle) = running_guard.get_mut(server_id) else {
            return;
        };
        apply_player_change(&mut handle.players, change);
        handle.players.clone()
    };

    let payload = PlayersEvent {
        server_id: server_id.to_string(),
        players,
    };
    emit_event(app, events::SERVER_PLAYERS, payload);
}

fn apply_player_change(players: &mut Vec<String>, change: PlayerChange) {
    match change {
        PlayerChange::Joined(name) => {
            if !players.contains(&name) {
                players.push(name);
            }
        }
        PlayerChange::Left(name) => {
            players.retain(|player| player != &name);
        }
    }
}

/// Kills the child. A kill is never a "clean" exit.
async fn force_kill(child: &mut Child) -> bool {
    if let Err(error) = child.kill().await {
        eprintln!("failed to kill server process: {error}");
    }
    false
}

/// Removes the handle from the running map, reporting whether the stop was
/// user-requested (as opposed to a crash).
async fn remove_handle(running: &RunningMap, server_id: &str) -> bool {
    let mut running_guard = running.lock().await;
    let removed = running_guard.remove(server_id);

    match removed {
        Some(handle) => handle.stop_requested,
        None => false,
    }
}

fn schedule_force_kill(running: &RunningMap, server_id: &str) {
    let running = Arc::clone(running);
    let server_id = server_id.to_string();

    tokio::spawn(async move {
        tokio::time::sleep(GRACEFUL_STOP_TIMEOUT).await;
        send_kill_signal(&running, &server_id).await;
    });
}

/// Signals the supervisor to kill the process. Returns whether the server
/// was still tracked as running.
async fn send_kill_signal(running: &RunningMap, server_id: &str) -> bool {
    let mut running_guard = running.lock().await;
    let Some(handle) = running_guard.get_mut(server_id) else {
        return false;
    };

    handle.stop_requested = true;
    if let Some(kill_tx) = handle.kill_tx.take() {
        // The supervisor may be exiting concurrently; a lost signal is fine.
        let _delivered = kill_tx.send(());
    }
    true
}

async fn set_status(
    app: &AppHandle,
    running: &RunningMap,
    server_id: &str,
    new_status: ServerStatus,
) {
    let mut running_guard = running.lock().await;
    let Some(handle) = running_guard.get_mut(server_id) else {
        return;
    };
    handle.status = new_status;
    drop(running_guard);

    emit_status(app, server_id, new_status);
}

fn emit_status(app: &AppHandle, server_id: &str, status: ServerStatus) {
    let payload = StatusEvent {
        server_id: server_id.to_string(),
        status,
    };
    emit_event(app, events::SERVER_STATUS, payload);
}

pub(crate) fn emit_event<P: Serialize + Clone>(app: &AppHandle, event_name: &str, payload: P) {
    if let Err(error) = app.emit(event_name, payload) {
        eprintln!("failed to emit {event_name}: {error}");
    }
}
