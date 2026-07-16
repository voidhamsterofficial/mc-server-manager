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

use crate::console::{self, ConsoleLine, ConsoleSignal, ConsoleSpan, LogLevel};
use crate::error::{AppError, AppResult};
use crate::events;
use crate::installers::vanilla::SERVER_JAR_NAME;
use crate::platform;
use crate::servers::{Loader, ServerConfig, ServerStatus};

/// How long buffered console lines wait before being flushed to the UI.
/// Batching keeps event traffic low when the server logs in bursts.
const CONSOLE_BATCH_INTERVAL: Duration = Duration::from_millis(50);

/// Grace period between sending `stop` and force-killing the process.
const GRACEFUL_STOP_TIMEOUT: Duration = Duration::from_secs(30);

const LINE_CHANNEL_CAPACITY: usize = 1024;
const COMMAND_CHANNEL_CAPACITY: usize = 64;

/// Records the running server's PID inside its folder, so a later app
/// session can reclaim an orphan that survived an app crash or force-kill.
const PID_FILE_NAME: &str = "blockparty.pid";

/// How long to wait for a killed orphan to release its file locks.
const ORPHAN_EXIT_WAIT: Duration = Duration::from_secs(5);

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
    /// The software's graceful shutdown command ("stop", "end", …).
    stop_command: &'static str,
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

    reclaim_orphaned_server(app, &config.id, server_dir).await;

    let mut child = spawn_java_process(config, server_dir, java_executable)?;
    platform::tie_child_to_app_lifetime(&child);
    write_pid_file(server_dir, child.id());
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
        stop_command: config.loader.stop_command(),
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
        server_dir.to_path_buf(),
        child,
        kill_rx,
    ));

    Ok(())
}

/// Kills a previous app session's server process if it is still alive and
/// holding this world (its PID survives in the server folder). Prevents
/// "another process has locked a portion of the file" on start.
async fn reclaim_orphaned_server(app: &AppHandle, server_id: &str, server_dir: &Path) {
    let Some(orphan_pid) = read_pid_file(server_dir) else {
        return;
    };

    let mut system = sysinfo::System::new();
    let pid = sysinfo::Pid::from_u32(orphan_pid);
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);

    let is_java_process = system
        .process(pid)
        .map(|process| {
            let name = process.name().to_string_lossy().to_lowercase();
            name.contains("java")
        })
        .unwrap_or(false);
    if !is_java_process {
        // Process is gone (or the OS reused the PID) — just clean up.
        remove_pid_file(server_dir);
        return;
    }

    if let Some(orphan) = system.process(pid) {
        orphan.kill();
    }
    wait_for_process_exit(&mut system, pid).await;
    remove_pid_file(server_dir);

    let notice = ConsoleLine {
        spans: vec![ConsoleSpan {
            text: format!(
                "Recovered: terminated an orphaned server process (pid {orphan_pid}) that was still holding this world."
            ),
            color: Some("#ffaa00".to_string()),
            bold: false,
        }],
        level: LogLevel::Warn,
    };
    let batch = ConsoleBatchEvent {
        server_id: server_id.to_string(),
        lines: vec![notice],
    };
    emit_event(app, events::SERVER_CONSOLE, batch);
}

async fn wait_for_process_exit(system: &mut sysinfo::System, pid: sysinfo::Pid) {
    let poll_interval = Duration::from_millis(250);
    let attempts = ORPHAN_EXIT_WAIT.as_millis() / poll_interval.as_millis();

    for _ in 0..attempts {
        tokio::time::sleep(poll_interval).await;
        system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
        if system.process(pid).is_none() {
            return;
        }
    }
}

fn write_pid_file(server_dir: &Path, pid: Option<u32>) {
    let Some(pid) = pid else {
        return;
    };
    if let Err(error) = std::fs::write(server_dir.join(PID_FILE_NAME), pid.to_string()) {
        eprintln!("could not write pid file: {error}");
    }
}

fn read_pid_file(server_dir: &Path) -> Option<u32> {
    let contents = std::fs::read_to_string(server_dir.join(PID_FILE_NAME)).ok()?;
    contents.trim().parse().ok()
}

fn remove_pid_file(server_dir: &Path) {
    let pid_path = server_dir.join(PID_FILE_NAME);
    if !pid_path.exists() {
        return;
    }
    if let Err(error) = std::fs::remove_file(&pid_path) {
        eprintln!("could not remove pid file: {error}");
    }
}

/// Emergency recovery: force-kills every Java process that belongs to
/// Blockparty — running from our managed Java folder, or working inside one
/// of our server folders. Unrelated Javas (game launchers, IDEs) survive.
/// Returns how many processes were killed.
pub async fn kill_all_blockparty_java(
    managed_java_dir: std::path::PathBuf,
    server_dirs: Vec<std::path::PathBuf>,
) -> u32 {
    let sweep = tokio::task::spawn_blocking(move || {
        let system = sysinfo::System::new_all();

        let mut killed_count = 0;
        for process in system.processes().values() {
            if !is_blockparty_java(process, &managed_java_dir, &server_dirs) {
                continue;
            }
            if process.kill() {
                killed_count += 1;
            }
        }
        killed_count
    })
    .await;

    match sweep {
        Ok(killed_count) => killed_count,
        Err(join_error) => {
            eprintln!("java sweep failed: {join_error}");
            0
        }
    }
}

fn is_blockparty_java(
    process: &sysinfo::Process,
    managed_java_dir: &Path,
    server_dirs: &[std::path::PathBuf],
) -> bool {
    let name = process.name().to_string_lossy().to_lowercase();
    if !name.contains("java") {
        return false;
    }

    let runs_our_java = process
        .exe()
        .map(|exe_path| exe_path.starts_with(managed_java_dir))
        .unwrap_or(false);
    let works_in_server_dir = process
        .cwd()
        .map(|cwd| server_dirs.iter().any(|dir| cwd.starts_with(dir)))
        .unwrap_or(false);

    runs_our_java || works_in_server_dir
}

/// Requests a graceful stop (`stop` command) and schedules a force-kill in
/// case the server hangs on the way down.
pub async fn stop(app: &AppHandle, running: &RunningMap, server_id: &str) -> AppResult<()> {
    let (command_tx, stop_command) = {
        let mut running_guard = running.lock().await;
        let handle = running_guard
            .get_mut(server_id)
            .ok_or(AppError::ServerNotRunning)?;
        handle.stop_requested = true;
        handle.status = ServerStatus::Stopping;
        (handle.command_tx.clone(), handle.stop_command)
    };
    emit_status(app, server_id, ServerStatus::Stopping);

    let send_result = command_tx.send(stop_command.to_string()).await;
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
    let mut command = build_launch_command(config, server_dir, java_executable)?;
    command
        .current_dir(server_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    platform::hide_console_window(&mut command);

    let child = command.spawn()?;
    Ok(child)
}

/// How a loader's installed files are launched.
enum LaunchTarget {
    /// `java ... -jar <name>`
    Jar(String),
    /// `java ... @<argsfile>` — modern Forge/NeoForge installs.
    ArgsFile(std::path::PathBuf),
}

/// The process to launch: the user's custom start command, the native
/// Bedrock binary, or a java invocation shaped for the loader.
fn build_launch_command(
    config: &ServerConfig,
    server_dir: &Path,
    java_executable: &Path,
) -> AppResult<Command> {
    if let Some(custom_command) = &config.start_command {
        return parse_custom_command(custom_command);
    }

    if config.loader == Loader::Bds {
        let binary_name = if cfg!(windows) {
            "bedrock_server.exe"
        } else {
            "bedrock_server"
        };
        let command = Command::new(server_dir.join(binary_name));
        return Ok(command);
    }

    let max_heap_flag = format!("-Xmx{}M", config.memory_mb);
    let min_heap_flag = format!("-Xms{}M", config.memory_mb);

    let mut command = Command::new(java_executable);
    command.args([&max_heap_flag, &min_heap_flag]);
    if let Some(java_args) = &config.java_args {
        command.args(java_args.split_whitespace());
    }

    match launch_target(config.loader, server_dir) {
        LaunchTarget::Jar(jar_name) => {
            command.args(["-jar", &jar_name]);
        }
        LaunchTarget::ArgsFile(args_path) => {
            command.arg(format!("@{}", args_path.display()));
        }
    }
    if !config.loader.is_proxy() {
        command.arg("nogui");
    }
    Ok(command)
}

fn launch_target(loader: Loader, server_dir: &Path) -> LaunchTarget {
    match loader {
        Loader::Forge => forge_family_target(server_dir, "net/minecraftforge/forge"),
        Loader::NeoForge => forge_family_target(server_dir, "net/neoforged/neoforge"),
        Loader::Quilt => {
            let quilt_launcher = "quilt-server-launch.jar";
            if server_dir.join(quilt_launcher).exists() {
                return LaunchTarget::Jar(quilt_launcher.to_string());
            }
            LaunchTarget::Jar(SERVER_JAR_NAME.to_string())
        }
        _ => LaunchTarget::Jar(SERVER_JAR_NAME.to_string()),
    }
}

/// Modern Forge/NeoForge installs launch via an args file under
/// `libraries/<vendor>/<version>/`; legacy Forge produced a plain jar.
fn forge_family_target(server_dir: &Path, vendor_subpath: &str) -> LaunchTarget {
    if let Some(args_file) = find_args_file(server_dir, vendor_subpath) {
        return LaunchTarget::ArgsFile(args_file);
    }
    if let Some(legacy_jar) = find_jar_with_prefix(server_dir, "forge-") {
        return LaunchTarget::Jar(legacy_jar);
    }
    LaunchTarget::Jar(SERVER_JAR_NAME.to_string())
}

fn find_args_file(server_dir: &Path, vendor_subpath: &str) -> Option<std::path::PathBuf> {
    let args_file_name = if cfg!(windows) {
        "win_args.txt"
    } else {
        "unix_args.txt"
    };
    let vendor_dir = server_dir.join("libraries").join(vendor_subpath);

    for entry in std::fs::read_dir(vendor_dir).ok()?.flatten() {
        let candidate = entry.path().join(args_file_name);
        if candidate.exists() {
            // Relative to the server dir (the working directory), matching
            // the relative paths inside the args file itself.
            let relative = candidate.strip_prefix(server_dir).ok()?.to_path_buf();
            return Some(relative);
        }
    }
    None
}

fn find_jar_with_prefix(server_dir: &Path, prefix: &str) -> Option<String> {
    for entry in std::fs::read_dir(server_dir).ok()?.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.starts_with(prefix) && file_name.ends_with(".jar") {
            return Some(file_name);
        }
    }
    None
}

/// Splits a custom start command on whitespace (no quoting support yet).
fn parse_custom_command(custom_command: &str) -> AppResult<Command> {
    let mut parts = custom_command.split_whitespace();
    let Some(program) = parts.next() else {
        let message = "the custom start command is empty".to_string();
        return Err(AppError::InvalidInput(message));
    };

    let mut command = Command::new(program);
    command.args(parts);
    Ok(command)
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
    server_dir: std::path::PathBuf,
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

    remove_pid_file(&server_dir);

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
