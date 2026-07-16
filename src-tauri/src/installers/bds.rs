//! Bedrock Dedicated Server installer. Mojang publishes BDS as a zip linked
//! from the download page (no API), so the link is scraped from the page.
//! BDS is a native executable — no Java involved.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::installers::vanilla::McVersion;
use crate::installers::{download_file, ExpectedChecksum, ProgressCallback};

const BDS_PAGE_URL: &str = "https://www.minecraft.net/en-us/download/server/bedrock";

/// The page rejects non-browser clients, so this request masquerades.
const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";

const ARCHIVE_FILE: &str = "bds.zip";

fn platform_key() -> AppResult<&'static str> {
    if cfg!(windows) {
        return Ok("bin-win");
    }
    if cfg!(target_os = "linux") {
        return Ok("bin-linux");
    }
    let message = "Bedrock Dedicated Server is only available for Windows and Linux".to_string();
    Err(AppError::InvalidInput(message))
}

async fn find_download_url(client: &reqwest::Client) -> AppResult<String> {
    let html = client
        .get(BDS_PAGE_URL)
        .header("User-Agent", BROWSER_USER_AGENT)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let marker = format!("{}/bedrock-server-", platform_key()?);
    let marker_position = html.find(&marker).ok_or_else(|| {
        AppError::Process("could not find the Bedrock download link on minecraft.net".to_string())
    })?;

    let before_marker = &html[..marker_position];
    let url_start = before_marker
        .rfind("https://")
        .ok_or_else(|| AppError::Process("unexpected Bedrock download page format".to_string()))?;

    let from_url = &html[url_start..];
    let zip_end = from_url
        .find(".zip")
        .ok_or_else(|| AppError::Process("unexpected Bedrock download link".to_string()))?;

    Ok(from_url[..zip_end + 4].to_string())
}

fn version_from_url(url: &str) -> Option<String> {
    let after_prefix = url.rsplit("bedrock-server-").next()?;
    let version = after_prefix.strip_suffix(".zip")?;
    Some(version.to_string())
}

/// Mojang only serves the current build, so the list is one entry.
pub async fn list_versions(client: &reqwest::Client) -> AppResult<Vec<McVersion>> {
    let url = find_download_url(client).await?;
    let id = version_from_url(&url).unwrap_or_else(|| "latest".to_string());

    let entry = McVersion {
        id,
        kind: "release".to_string(),
        release_time: String::new(),
    };
    Ok(vec![entry])
}

pub async fn install(
    client: &reqwest::Client,
    _mc_version: &str,
    server_dir: &Path,
    report_progress: &ProgressCallback,
) -> AppResult<()> {
    let url = find_download_url(client).await?;

    let archive_path = server_dir.join(ARCHIVE_FILE);
    download_file(
        client,
        &url,
        &archive_path,
        ExpectedChecksum::None,
        report_progress,
    )
    .await?;

    extract_archive(archive_path.clone(), server_dir.to_path_buf()).await?;
    if let Err(error) = std::fs::remove_file(&archive_path) {
        eprintln!("could not remove {}: {error}", archive_path.display());
    }

    mark_executable(server_dir);
    Ok(())
}

async fn extract_archive(archive_path: PathBuf, destination: PathBuf) -> AppResult<()> {
    let result = tokio::task::spawn_blocking(move || {
        let archive_file = std::fs::File::open(&archive_path)?;
        let mut archive = zip::ZipArchive::new(archive_file)?;
        archive.extract(&destination)?;
        Ok(())
    })
    .await
    .map_err(|join_error| AppError::Process(join_error.to_string()))?;
    result
}

#[cfg(unix)]
fn mark_executable(server_dir: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let binary = server_dir.join("bedrock_server");
    let permissions = std::fs::Permissions::from_mode(0o755);
    if let Err(error) = std::fs::set_permissions(&binary, permissions) {
        eprintln!("could not mark bedrock_server executable: {error}");
    }
}

#[cfg(not(unix))]
fn mark_executable(_server_dir: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_version_from_download_url() {
        let url = "https://www.minecraft.net/bedrockdedicatedserver/bin-win/bedrock-server-1.21.51.02.zip";
        assert_eq!(version_from_url(url), Some("1.21.51.02".to_string()));
    }
}
