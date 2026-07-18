//! Spigot installer. Spigot has no downloadable jar — it must be compiled
//! with BuildTools, which takes several minutes and produces spigot-<v>.jar.

use std::path::Path;

use crate::error::{AppError, AppResult};
use crate::installers::vanilla::{self, McVersion, SERVER_JAR_NAME};
use crate::installers::{download_file, run_java_tool, ExpectedChecksum, ProgressCallback};

const BUILDTOOLS_URL: &str =
    "https://hub.spigotmc.org/jenkins/job/BuildTools/lastSuccessfulBuild/artifact/target/BuildTools.jar";

/// The compile happens in a scratch folder so the hundreds of BuildTools
/// work files never pollute the server directory.
const BUILD_DIR_NAME: &str = "buildtools-work";

/// Spigot builds against official releases, so the Mojang release list is
/// the version list. BuildTools can't compile anything older than 1.8.
pub async fn list_versions(client: &reqwest::Client) -> AppResult<Vec<McVersion>> {
    let all = vanilla::list_versions(client).await?;
    let releases = all
        .into_iter()
        .filter(|version| version.kind == "release" && buildtools_supports(&version.id))
        .collect();
    Ok(releases)
}

fn buildtools_supports(mc_version: &str) -> bool {
    let mut parts = mc_version.split('.');
    let Some(era) = parts.next() else {
        return false;
    };
    if era != "1" {
        // Year-based versions (25.x+) are newer than 1.8 by definition.
        return true;
    }

    let minor: u32 = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    minor >= 8
}

pub async fn install(
    client: &reqwest::Client,
    mc_version: &str,
    server_dir: &Path,
    java_executable: &Path,
    report_progress: &ProgressCallback,
) -> AppResult<()> {
    let build_dir = server_dir.join(BUILD_DIR_NAME);
    std::fs::create_dir_all(&build_dir)?;

    let buildtools_path = build_dir.join("BuildTools.jar");
    download_file(
        client,
        BUILDTOOLS_URL,
        &buildtools_path,
        ExpectedChecksum::None,
        report_progress,
    )
    .await?;

    // Signal "still working, unknown length" to the UI: compiling takes
    // minutes and has no byte progress.
    report_progress(0, None);

    run_java_tool(
        java_executable,
        &build_dir,
        &[
            "-jar",
            "BuildTools.jar",
            "--rev",
            mc_version,
            "--compile",
            "spigot",
        ],
    )
    .await?;

    promote_built_jar(&build_dir, server_dir, mc_version)?;

    if let Err(error) = std::fs::remove_dir_all(&build_dir) {
        log::warn!("could not clean BuildTools workspace: {error}");
    }
    Ok(())
}

/// Moves the compiled spigot jar out of the workspace as `server.jar`.
fn promote_built_jar(build_dir: &Path, server_dir: &Path, mc_version: &str) -> AppResult<()> {
    let expected = build_dir.join(format!("spigot-{mc_version}.jar"));
    let built_jar = if expected.exists() {
        expected
    } else {
        find_any_spigot_jar(build_dir)?
    };

    std::fs::rename(built_jar, server_dir.join(SERVER_JAR_NAME))?;
    Ok(())
}

fn find_any_spigot_jar(build_dir: &Path) -> AppResult<std::path::PathBuf> {
    for entry in std::fs::read_dir(build_dir)?.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.starts_with("spigot-") && file_name.ends_with(".jar") {
            return Ok(entry.path());
        }
    }

    let message = "BuildTools finished but no spigot jar was produced".to_string();
    Err(AppError::Process(message))
}
