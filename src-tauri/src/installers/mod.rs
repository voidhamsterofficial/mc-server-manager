//! Server software installers. Each submodule knows how to obtain the server
//! jar for one loader; shared download plumbing lives here.

pub mod arclight;
pub mod bds;
pub mod bungee;
pub mod fabric;
pub mod forgelike;
pub mod mohist;
pub mod paper;
pub mod purpur;
pub mod quilt;
pub mod spigot;
pub mod vanilla;

use std::path::Path;

use crate::error::{AppError, AppResult};
use crate::servers::Loader;

/// Installs the chosen server software into `server_dir`.
/// `java_executable` is required for installers that run a Java tool
/// (Forge, NeoForge, Quilt, Spigot's BuildTools).
pub async fn install(
    client: &reqwest::Client,
    loader: Loader,
    mc_version: &str,
    server_dir: &Path,
    java_executable: Option<&Path>,
    report_progress: &ProgressCallback,
) -> AppResult<()> {
    let java = |loader_name: &str| {
        java_executable.ok_or_else(|| {
            AppError::Process(format!("{loader_name} installation needs a Java runtime"))
        })
    };

    match loader {
        Loader::Vanilla => vanilla::install(client, mc_version, server_dir, report_progress).await,
        Loader::Paper | Loader::Folia | Loader::Velocity => {
            paper::install(client, loader, mc_version, server_dir, report_progress).await
        }
        Loader::Purpur => purpur::install(client, mc_version, server_dir, report_progress).await,
        Loader::Fabric => fabric::install(client, mc_version, server_dir, report_progress).await,
        Loader::BungeeCord => bungee::install(client, server_dir, report_progress).await,
        Loader::Forge | Loader::NeoForge => {
            forgelike::install(
                client,
                loader,
                mc_version,
                server_dir,
                java("Forge")?,
                report_progress,
            )
            .await
        }
        Loader::Quilt => {
            quilt::install(
                client,
                mc_version,
                server_dir,
                java("Quilt")?,
                report_progress,
            )
            .await
        }
        Loader::Spigot => {
            spigot::install(
                client,
                mc_version,
                server_dir,
                java("Spigot")?,
                report_progress,
            )
            .await
        }
        Loader::Mohist => mohist::install(client, mc_version, server_dir, report_progress).await,
        Loader::Arclight => {
            arclight::install(client, mc_version, server_dir, report_progress).await
        }
        Loader::Bds => bds::install(client, mc_version, server_dir, report_progress).await,
    }
}

/// How many characters of installer output to keep in error messages.
const TOOL_OUTPUT_TAIL: usize = 600;

/// Runs a Java tool (an installer jar) inside the server folder and waits
/// for it to finish, surfacing the tail of its output on failure.
pub(crate) async fn run_java_tool(
    java_executable: &Path,
    server_dir: &Path,
    args: &[&str],
) -> AppResult<()> {
    let mut command = tokio::process::Command::new(java_executable);
    command
        .args(args)
        .current_dir(server_dir)
        .stdin(std::process::Stdio::null());
    crate::platform::hide_console_window(&mut command);

    let output = command.output().await?;
    if output.status.success() {
        return Ok(());
    }

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let mut tail_start = combined.len().saturating_sub(TOOL_OUTPUT_TAIL);
    while !combined.is_char_boundary(tail_start) {
        tail_start += 1;
    }
    let message = format!("installer failed:\n{}", &combined[tail_start..]);
    Err(AppError::Process(message))
}

use futures_util::StreamExt;
use serde::Serialize;
use sha1::Digest as Sha1Digest;
use sha1::Sha1;
use sha2::digest::Digest as Sha256Digest;
use sha2::Sha256;
use tokio::io::AsyncWriteExt;

/// Which checksum a download is verified against. `None` is for sources
/// that publish no hash (HTTPS is the only integrity check there).
pub enum ExpectedChecksum<'a> {
    None,
    Sha1(&'a str),
    Sha256(&'a str),
}

/// Progress of an ongoing installation, emitted as `install:progress`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgressEvent {
    pub server_id: String,
    pub step: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

/// Called with (downloaded_bytes, total_bytes) as a download advances.
pub type ProgressCallback = Box<dyn Fn(u64, Option<u64>) + Send + Sync>;

/// Emit progress at most every this many bytes, so a fast download doesn't
/// flood the UI with events.
const PROGRESS_EMIT_STEP_BYTES: u64 = 512 * 1024;

/// Builds a progress callback that forwards throttled `install:progress`
/// events to the UI.
pub fn progress_event_reporter(
    app: tauri::AppHandle,
    server_id: String,
    step: &'static str,
) -> ProgressCallback {
    use std::sync::atomic::{AtomicU64, Ordering};
    use tauri::Emitter;

    let last_reported_bytes = AtomicU64::new(0);

    let reporter = move |downloaded_bytes: u64, total_bytes: Option<u64>| {
        let previously_reported = last_reported_bytes.load(Ordering::Relaxed);
        let is_final_chunk = total_bytes == Some(downloaded_bytes);
        let advanced_enough =
            downloaded_bytes.saturating_sub(previously_reported) >= PROGRESS_EMIT_STEP_BYTES;
        if !is_final_chunk && !advanced_enough {
            return;
        }
        last_reported_bytes.store(downloaded_bytes, Ordering::Relaxed);

        let payload = InstallProgressEvent {
            server_id: server_id.clone(),
            step: step.to_string(),
            downloaded_bytes,
            total_bytes,
        };
        if let Err(error) = app.emit(crate::events::INSTALL_PROGRESS, payload) {
            eprintln!("failed to emit install progress: {error}");
        }
    };
    Box::new(reporter)
}

/// Streams `url` to `destination`, verifying the expected checksum and
/// reporting progress along the way.
pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    destination: &Path,
    expected: ExpectedChecksum<'_>,
    report_progress: &ProgressCallback,
) -> AppResult<()> {
    let response = client.get(url).send().await?.error_for_status()?;
    let total_bytes = response.content_length();

    let mut file = tokio::fs::File::create(destination).await?;
    let mut sha1_hasher = Sha1::new();
    let mut sha256_hasher = Sha256::new();
    let mut downloaded_bytes: u64 = 0;
    let mut body_stream = response.bytes_stream();

    while let Some(chunk_result) = body_stream.next().await {
        let chunk = chunk_result?;
        match expected {
            ExpectedChecksum::Sha1(_) => sha1_hasher.update(&chunk),
            ExpectedChecksum::Sha256(_) => sha256_hasher.update(&chunk),
            ExpectedChecksum::None => {}
        }
        file.write_all(&chunk).await?;
        downloaded_bytes += chunk.len() as u64;
        report_progress(downloaded_bytes, total_bytes);
    }
    file.flush().await?;

    let (expected_hex, actual_hex) = match expected {
        ExpectedChecksum::None => {
            return Ok(());
        }
        ExpectedChecksum::Sha1(expected_hex) => (expected_hex, hex::encode(sha1_hasher.finalize())),
        ExpectedChecksum::Sha256(expected_hex) => {
            (expected_hex, hex::encode(sha256_hasher.finalize()))
        }
    };
    verify_checksum(destination, expected_hex, &actual_hex)
}

fn verify_checksum(destination: &Path, expected_hex: &str, actual_hex: &str) -> AppResult<()> {
    if actual_hex.eq_ignore_ascii_case(expected_hex) {
        return Ok(());
    }

    let file_name = destination
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| destination.display().to_string());
    Err(AppError::ChecksumMismatch { file_name })
}
