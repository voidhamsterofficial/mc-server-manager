//! Server software installers. Each submodule knows how to obtain the server
//! jar for one loader; shared download plumbing lives here.

pub mod vanilla;

use std::path::Path;

use futures_util::StreamExt;
use serde::Serialize;
use sha1::Digest as Sha1Digest;
use sha1::Sha1;
use sha2::digest::Digest as Sha256Digest;
use sha2::Sha256;
use tokio::io::AsyncWriteExt;

use crate::error::{AppError, AppResult};

/// Which checksum a download is verified against.
pub enum ExpectedChecksum<'a> {
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
        }
        file.write_all(&chunk).await?;
        downloaded_bytes += chunk.len() as u64;
        report_progress(downloaded_bytes, total_bytes);
    }
    file.flush().await?;

    let (expected_hex, actual_hex) = match expected {
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
