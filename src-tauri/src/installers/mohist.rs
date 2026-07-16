//! Mohist installer (mohistmc.com API v2). The downloaded jar self-installs
//! its Forge libraries on first start.

use std::path::Path;

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::installers::vanilla::{McVersion, SERVER_JAR_NAME};
use crate::installers::{download_file, ExpectedChecksum, ProgressCallback};

const MOHIST_API_BASE: &str = "https://mohistmc.com/api/v2/projects/mohist";

#[derive(Debug, Deserialize)]
struct MohistProject {
    versions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MohistBuilds {
    builds: Vec<MohistBuild>,
}

#[derive(Debug, Deserialize)]
struct MohistBuild {
    number: u32,
}

pub async fn list_versions(client: &reqwest::Client) -> AppResult<Vec<McVersion>> {
    let project: MohistProject = client
        .get(MOHIST_API_BASE)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let versions = project
        .versions
        .into_iter()
        .rev()
        .map(|id| McVersion {
            id,
            kind: "release".to_string(),
            release_time: String::new(),
        })
        .collect();
    Ok(versions)
}

pub async fn install(
    client: &reqwest::Client,
    mc_version: &str,
    server_dir: &Path,
    report_progress: &ProgressCallback,
) -> AppResult<()> {
    let builds: MohistBuilds = client
        .get(format!("{MOHIST_API_BASE}/{mc_version}/builds"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let latest_build = builds
        .builds
        .last()
        .ok_or_else(|| AppError::UnknownMinecraftVersion(mc_version.to_string()))?;

    let download_url = format!(
        "{MOHIST_API_BASE}/{mc_version}/builds/{}/download",
        latest_build.number
    );
    let jar_path = server_dir.join(SERVER_JAR_NAME);
    download_file(
        client,
        &download_url,
        &jar_path,
        ExpectedChecksum::None,
        report_progress,
    )
    .await
}
