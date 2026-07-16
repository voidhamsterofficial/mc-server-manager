//! Quilt installer — downloads the official quilt-installer tool and runs
//! its headless server install, which produces `quilt-server-launch.jar`.

use std::path::Path;

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::installers::vanilla::McVersion;
use crate::installers::{download_file, run_java_tool, ExpectedChecksum, ProgressCallback};

const QUILT_META_BASE: &str = "https://meta.quiltmc.org/v3/versions";

const INSTALLER_FILE: &str = "quilt-installer.jar";

#[derive(Debug, Deserialize)]
struct GameVersion {
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct InstallerVersion {
    url: String,
}

/// Game versions Quilt supports, newest first.
pub async fn list_versions(client: &reqwest::Client) -> AppResult<Vec<McVersion>> {
    let game_versions: Vec<GameVersion> = client
        .get(format!("{QUILT_META_BASE}/game"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let versions = game_versions
        .into_iter()
        .map(|game| McVersion {
            id: game.version,
            kind: if game.stable { "release" } else { "snapshot" }.to_string(),
            release_time: String::new(),
        })
        .collect();
    Ok(versions)
}

pub async fn install(
    client: &reqwest::Client,
    mc_version: &str,
    server_dir: &Path,
    java_executable: &Path,
    report_progress: &ProgressCallback,
) -> AppResult<()> {
    let installers: Vec<InstallerVersion> = client
        .get(format!("{QUILT_META_BASE}/installer"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let newest_installer = installers
        .first()
        .ok_or_else(|| AppError::Process("no Quilt installer available".to_string()))?;

    let installer_path = server_dir.join(INSTALLER_FILE);
    download_file(
        client,
        &newest_installer.url,
        &installer_path,
        ExpectedChecksum::None,
        report_progress,
    )
    .await?;

    run_java_tool(
        java_executable,
        server_dir,
        &[
            "-jar",
            INSTALLER_FILE,
            "install",
            "server",
            mc_version,
            "--install-dir=.",
            "--download-server",
        ],
    )
    .await?;

    if let Err(error) = std::fs::remove_file(&installer_path) {
        eprintln!("could not remove quilt installer: {error}");
    }
    Ok(())
}
