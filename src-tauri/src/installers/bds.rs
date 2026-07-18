//! Bedrock Dedicated Server installer. Mojang publishes the current BDS build
//! through its download-links API, which names a zip per platform. BDS is a
//! native executable — no Java involved.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::installers::vanilla::McVersion;
use crate::installers::{download_file, ExpectedChecksum, ProgressCallback};

/// Mojang's official list of current downloads. This replaced scraping the
/// download page, which stopped carrying the link in its HTML.
const BDS_LINKS_API: &str =
    "https://net-secondary.web.minecraft-services.net/api/v1.0/download/links";

const ARCHIVE_FILE: &str = "bds.zip";

/// minecraft.net's CDN silently drops requests that don't look like a browser,
/// so the zip download needs this. (The links API above is happy with any
/// User-Agent — only the download is fussy.)
const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";

/// A client that presents as a browser, for the CDN that insists on one.
fn browser_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(BROWSER_USER_AGENT)
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|error| AppError::Process(error.to_string()))
}

#[derive(Debug, Deserialize)]
struct DownloadLinks {
    result: LinksResult,
}

#[derive(Debug, Deserialize)]
struct LinksResult {
    links: Vec<DownloadLink>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadLink {
    download_type: String,
    download_url: String,
}

/// The API's download type for this platform's Bedrock server.
fn download_type() -> AppResult<&'static str> {
    if cfg!(windows) {
        return Ok("serverBedrockWindows");
    }
    if cfg!(target_os = "linux") {
        return Ok("serverBedrockLinux");
    }
    let message = "Bedrock Dedicated Server is only available for Windows and Linux".to_string();
    Err(AppError::InvalidInput(message))
}

async fn find_download_url(client: &reqwest::Client) -> AppResult<String> {
    let wanted = download_type()?;
    let listing: DownloadLinks = client
        .get(BDS_LINKS_API)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    listing
        .result
        .links
        .into_iter()
        .find(|link| link.download_type == wanted)
        .map(|link| link.download_url)
        .ok_or_else(|| {
            AppError::Process(format!(
                "Mojang's download list has no {wanted} entry right now"
            ))
        })
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

    // The CDN serving the zip requires a browser-looking client, unlike the
    // links API that named it.
    let downloader = browser_client()?;
    let archive_path = server_dir.join(ARCHIVE_FILE);
    download_file(
        &downloader,
        &url,
        &archive_path,
        ExpectedChecksum::None,
        report_progress,
    )
    .await?;

    extract_archive(archive_path.clone(), server_dir.to_path_buf()).await?;
    if let Err(error) = std::fs::remove_file(&archive_path) {
        log::warn!("could not remove {}: {error}", archive_path.display());
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
        log::warn!("could not mark bedrock_server executable: {error}");
    }
}

#[cfg(not(unix))]
fn mark_executable(_server_dir: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trimmed copy of a real response from the links API.
    const SAMPLE_RESPONSE: &str = r#"{"result":{"links":[
        {"downloadType":"serverBedrockWindows","downloadUrl":"https://www.minecraft.net/bedrockdedicatedserver/bin-win/bedrock-server-1.26.33.2.zip"},
        {"downloadType":"serverBedrockLinux","downloadUrl":"https://www.minecraft.net/bedrockdedicatedserver/bin-linux/bedrock-server-1.26.33.2.zip"},
        {"downloadType":"serverBedrockPreviewWindows","downloadUrl":"https://www.minecraft.net/bedrockdedicatedserver/bin-win-preview/bedrock-server-1.26.40.31.zip"},
        {"downloadType":"serverJar","downloadUrl":"https://piston-data.mojang.com/v1/objects/abc/server.jar"}
    ]}}"#;

    #[test]
    fn extracts_version_from_download_url() {
        let url =
            "https://www.minecraft.net/bedrockdedicatedserver/bin-win/bedrock-server-1.21.51.02.zip";
        assert_eq!(version_from_url(url), Some("1.21.51.02".to_string()));
    }

    #[test]
    fn parses_the_links_api_response() {
        let listing: DownloadLinks = serde_json::from_str(SAMPLE_RESPONSE).expect("parse");
        assert_eq!(listing.result.links.len(), 4);
    }

    /// The stable server must be picked — never the preview, and never the
    /// Java jar that shares the same listing. BDS only ships for Windows and
    /// Linux, so `download_type()` has no answer on other platforms (e.g. a
    /// macOS dev machine) — gate the test to where it's meaningful.
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn picks_the_stable_server_for_this_platform() {
        let listing: DownloadLinks = serde_json::from_str(SAMPLE_RESPONSE).expect("parse");
        let wanted = download_type().expect("supported platform in tests");

        let chosen = listing
            .result
            .links
            .into_iter()
            .find(|link| link.download_type == wanted)
            .map(|link| link.download_url)
            .expect("a link for this platform");

        assert!(chosen.contains("bedrock-server-1.26.33.2.zip"));
        assert!(!chosen.contains("preview"));
        assert_eq!(version_from_url(&chosen), Some("1.26.33.2".to_string()));
    }
}
