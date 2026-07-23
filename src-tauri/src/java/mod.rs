//! Java runtime handling: mapping Minecraft versions to required Java
//! versions, and discovering installed JVMs on this machine.

pub mod download;

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::error::{AppError, AppResult};

/// A usable Java executable found on this machine.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaInstall {
    pub path: PathBuf,
    pub major_version: u32,
}

/// Last-resort Java major when the version is unmapped and the Adoptium
/// LTS lookup is unreachable.
pub const FALLBACK_JAVA_MAJOR: u32 = 21;

const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// Maps a classic `1.x` Minecraft version to the Java major its server
/// requires. Returns `None` for snapshots and newer version schemes — those
/// need the newest available LTS, resolved dynamically by the caller.
pub fn mapped_java_major(mc_version: &str) -> Option<u32> {
    let (minor, patch) = parse_release_version(mc_version)?;

    if minor <= 16 {
        return Some(8);
    }
    if minor == 17 {
        return Some(16);
    }
    if minor < 20 || (minor == 20 && patch <= 4) {
        return Some(17);
    }
    if minor <= 21 {
        return Some(21);
    }
    None
}

/// Parses `1.<minor>.<patch>` (patch optional) into `(minor, patch)`.
fn parse_release_version(mc_version: &str) -> Option<(u32, u32)> {
    let mut parts = mc_version.split('.');
    let era = parts.next()?;
    if era != "1" {
        return None;
    }

    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = parts.next().unwrap_or("0").parse().ok()?;
    Some((minor, patch))
}

/// Finds all working Java installations: `PATH`, `JAVA_HOME`, well-known
/// vendor directories, and any runtimes we downloaded ourselves.
pub async fn detect_installs(managed_java_dir: &Path) -> Vec<JavaInstall> {
    let candidates = candidate_executables(managed_java_dir);

    let mut installs: Vec<JavaInstall> = Vec::new();
    for candidate in candidates {
        let Some(install) = probe_java(&candidate).await else {
            continue;
        };
        let already_found = installs.iter().any(|known| known.path == install.path);
        if !already_found {
            installs.push(install);
        }
    }

    installs.sort_by_key(|install| std::cmp::Reverse(install.major_version));
    installs
}

/// Picks the best installed Java for a required major version: the exact
/// major if available, otherwise the oldest install that is new enough.
pub async fn resolve_for(required_major: u32, managed_java_dir: &Path) -> AppResult<JavaInstall> {
    let installs = detect_installs(managed_java_dir).await;

    let exact_match = installs
        .iter()
        .find(|install| install.major_version == required_major);
    if let Some(install) = exact_match {
        return Ok(install.clone());
    }

    let newer_installs = installs
        .iter()
        .filter(|install| install.major_version > required_major);
    let oldest_sufficient = newer_installs.min_by_key(|install| install.major_version);
    if let Some(install) = oldest_sufficient {
        return Ok(install.clone());
    }

    Err(AppError::NoSuitableJava { required_major })
}

/// Whether this executable really is a working Java.
///
/// Worth the ~100ms before a launch that takes tens of seconds: it is the only
/// way to tell a runtime that starts from one that dies inside the OS loader,
/// which otherwise reaches the user as a server that crashed with an empty
/// console and no explanation.
pub async fn is_usable(executable: &Path) -> bool {
    let probed = probe_java(executable).await;
    probed.is_some()
}

fn java_executable_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn candidate_executables(managed_java_dir: &Path) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Whatever `java` resolves to on PATH.
    candidates.push(PathBuf::from(java_executable_name()));

    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        candidates.push(
            Path::new(&java_home)
                .join("bin")
                .join(java_executable_name()),
        );
    }

    for vendor_dir in vendor_directories() {
        candidates.extend(javas_under(&vendor_dir));
    }
    candidates.extend(javas_under(managed_java_dir));

    candidates
}

/// Directories where JVM vendors conventionally install runtimes.
fn vendor_directories() -> Vec<PathBuf> {
    if cfg!(windows) {
        return vec![
            PathBuf::from(r"C:\Program Files\Java"),
            PathBuf::from(r"C:\Program Files\Eclipse Adoptium"),
            PathBuf::from(r"C:\Program Files\Microsoft"),
            PathBuf::from(r"C:\Program Files\Zulu"),
            PathBuf::from(r"C:\Program Files (x86)\Java"),
        ];
    }
    if cfg!(target_os = "macos") {
        return vec![PathBuf::from("/Library/Java/JavaVirtualMachines")];
    }
    vec![PathBuf::from("/usr/lib/jvm")]
}

/// Lists `<dir>/<entry>/bin/java` (or the macOS `Contents/Home` variant) for
/// every entry in a vendor directory.
fn javas_under(vendor_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(vendor_dir) else {
        return Vec::new();
    };

    let mut executables = Vec::new();
    for entry in entries.flatten() {
        let install_root = entry.path();
        executables.push(install_root.join("bin").join(java_executable_name()));
        if cfg!(target_os = "macos") {
            let macos_bin = install_root
                .join("Contents/Home/bin")
                .join(java_executable_name());
            executables.push(macos_bin);
        }
    }
    executables
}

/// Runs `java -version` and parses the reported major version. Returns
/// `None` for candidates that don't exist or don't behave like a JVM.
async fn probe_java(executable: &Path) -> Option<JavaInstall> {
    // Most candidates are guesses at vendor layouts that simply aren't there.
    // Only the bare `java`/`java.exe` is relative, and that one needs a PATH
    // lookup to be resolved at all.
    if executable.is_absolute() && !executable.is_file() {
        return None;
    }

    let mut command = tokio::process::Command::new(executable);
    command.arg("-version");
    crate::platform::hide_console_window(&mut command);

    let output_future = command.output();
    let Ok(run_result) = tokio::time::timeout(VERSION_PROBE_TIMEOUT, output_future).await else {
        log::warn!("timed out probing Java at {}", executable.display());
        return None;
    };
    let output = run_result.ok()?;

    // `java -version` prints to stderr.
    let version_text = String::from_utf8_lossy(&output.stderr);
    let Some(major_version) = parse_major_version(&version_text) else {
        // A runtime that is present but won't run — a half-unpacked or
        // partly-uninstalled JDK whose `java.exe` can't load its own
        // libraries. Skipping it lets a working one be found or downloaded.
        log::warn!(
            "ignoring unusable Java at {}: `java -version` {}",
            executable.display(),
            output.status
        );
        return None;
    };
    let resolved_path = resolve_executable_path(executable);

    let install = JavaInstall {
        path: resolved_path,
        major_version,
    };
    Some(install)
}

/// Canonicalizes so that duplicate discoveries (PATH + vendor dir) dedupe.
fn resolve_executable_path(executable: &Path) -> PathBuf {
    let canonical = dunce_canonicalize(executable);
    canonical.unwrap_or_else(|| executable.to_path_buf())
}

/// `std::fs::canonicalize` on Windows returns `\\?\` paths that Java rejects
/// as a working directory; strip the prefix when present.
fn dunce_canonicalize(path: &Path) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(path).ok()?;
    let display = canonical.to_string_lossy();
    let Some(stripped) = display.strip_prefix(r"\\?\") else {
        return Some(canonical);
    };
    Some(PathBuf::from(stripped))
}

/// Extracts the major version from `java -version` output, handling both the
/// legacy `1.8.0_392` and modern `21.0.3` formats.
fn parse_major_version(version_output: &str) -> Option<u32> {
    let quoted_start = version_output.find('"')?;
    let after_quote = &version_output[quoted_start + 1..];
    let quoted_end = after_quote.find('"')?;
    let version_string = &after_quote[..quoted_end];

    let mut numbers = version_string.split(['.', '_', '-', '+']);
    let first: u32 = numbers.next()?.parse().ok()?;
    if first != 1 {
        return Some(first);
    }

    let legacy_major: u32 = numbers.next()?.parse().ok()?;
    Some(legacy_major)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_minecraft_versions_to_java_majors() {
        assert_eq!(mapped_java_major("1.12.2"), Some(8));
        assert_eq!(mapped_java_major("1.16.5"), Some(8));
        assert_eq!(mapped_java_major("1.17.1"), Some(16));
        assert_eq!(mapped_java_major("1.18"), Some(17));
        assert_eq!(mapped_java_major("1.20.4"), Some(17));
        assert_eq!(mapped_java_major("1.20.5"), Some(21));
        assert_eq!(mapped_java_major("1.21"), Some(21));
        // Snapshots and post-1.x version schemes resolve dynamically.
        assert_eq!(mapped_java_major("24w14a"), None);
        assert_eq!(mapped_java_major("26.2"), None);
        assert_eq!(mapped_java_major("1.22"), None);
    }

    #[test]
    fn parses_modern_and_legacy_java_versions() {
        let modern = "openjdk version \"21.0.3\" 2024-04-16";
        assert_eq!(parse_major_version(modern), Some(21));

        let legacy = "java version \"1.8.0_392\"";
        assert_eq!(parse_major_version(legacy), Some(8));
    }
}
