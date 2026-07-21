//! Shared file management for jar-based addons: plugins in a server's
//! `plugins/` folder and mods in its `mods/` folder. Listing, enabling and
//! disabling, and deleting all work the same way regardless of which folder
//! or which marketplace the jar came from.
//!
//! Enable/disable is done by renaming `foo.jar` <-> `foo.jar.disabled`, the
//! convention Bukkit- and Forge-family loaders already understand.
//!
//! Sibling modules cover where addons come from: [`sources`] (the Modrinth /
//! SpigotMC / CurseForge marketplaces), [`cache`] (which keeps us from asking
//! them the same question twice), and the [`plugins`] / [`mods`] folder
//! wrappers built on top of it.

pub mod cache;
pub mod mods;
pub mod plugins;
pub mod sources;

use std::path::Path;

use serde::Serialize;

use crate::error::{AppError, AppResult};

const DISABLED_SUFFIX: &str = ".disabled";

/// One addon jar already present in a server's addon folder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledAddon {
    /// The on-disk file name, e.g. `EssentialsX.jar` or `EssentialsX.jar.disabled`.
    pub file_name: String,
    /// A friendlier name for display (extension and `.disabled` stripped).
    pub display_name: String,
    pub enabled: bool,
    pub size_bytes: u64,
}

/// Rejects anything that isn't a plain file name, so callers can never escape
/// the addon folder with `..` or a path separator.
pub fn safe_file_name(file_name: &str) -> AppResult<&str> {
    let is_plain = !file_name.is_empty()
        && !file_name.contains('/')
        && !file_name.contains('\\')
        && file_name != "."
        && file_name != "..";
    if !is_plain {
        return Err(AppError::InvalidInput(
            "invalid addon file name".to_string(),
        ));
    }
    Ok(file_name)
}

/// A friendlier label from a jar file name: drops `.disabled`, the `.jar`
/// extension, and leaves any trailing `-<version>` in place.
pub fn display_name(file_name: &str) -> String {
    let without_disabled = file_name.strip_suffix(DISABLED_SUFFIX).unwrap_or(file_name);
    let stem = without_disabled
        .strip_suffix(".jar")
        .unwrap_or(without_disabled);
    stem.to_string()
}

/// Lists the addons in `dir` (enabled and disabled), alphabetically. A
/// missing folder simply means no addons yet.
pub fn list_installed(dir: &Path) -> AppResult<Vec<InstalledAddon>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut addons = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let enabled = file_name.ends_with(".jar");
        let disabled = file_name.ends_with(".jar.disabled");
        if !enabled && !disabled {
            continue;
        }

        addons.push(InstalledAddon {
            display_name: display_name(&file_name),
            enabled,
            size_bytes: metadata.len(),
            file_name,
        });
    }

    addons.sort_by_key(|addon| addon.display_name.to_lowercase());
    Ok(addons)
}

/// Copies a jar from anywhere on disk into an addon folder, replacing any
/// jar already there under that name. Only `.jar` files are accepted — the
/// addon folders are loaded blindly by the server at startup, so letting
/// anything else in just plants a file that will never load.
///
/// No install record is written: a hand-dropped jar has no marketplace
/// provenance, so it's listed and toggleable like any other addon but sits
/// out of update checks.
pub fn import_jar(dir: &Path, source_path: &Path) -> AppResult<InstalledAddon> {
    if !source_path.is_file() {
        return Err(AppError::InvalidInput(
            "that isn't a file — drop a .jar".to_string(),
        ));
    }

    let raw_name = source_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| AppError::InvalidInput("that file has no name".to_string()))?;
    if !raw_name.to_lowercase().ends_with(".jar") {
        return Err(AppError::InvalidInput(format!(
            "{raw_name} isn't a .jar file"
        )));
    }
    let file_name = safe_file_name(&raw_name)?.to_string();

    std::fs::create_dir_all(dir)?;
    let destination = dir.join(&file_name);
    std::fs::copy(source_path, &destination)?;

    let size_bytes = std::fs::metadata(&destination).map(|meta| meta.len())?;
    Ok(InstalledAddon {
        display_name: display_name(&file_name),
        enabled: true,
        size_bytes,
        file_name,
    })
}

/// Enables or disables an addon by renaming its jar. Returns the new file name.
pub fn set_enabled(dir: &Path, file_name: &str, enabled: bool) -> AppResult<String> {
    let file_name = safe_file_name(file_name)?;
    let current = dir.join(file_name);
    if !current.is_file() {
        return Err(AppError::InvalidInput("addon not found".to_string()));
    }

    let target_name = if enabled {
        match file_name.strip_suffix(DISABLED_SUFFIX) {
            Some(base) => base.to_string(),
            None => return Ok(file_name.to_string()), // already enabled
        }
    } else if file_name.ends_with(DISABLED_SUFFIX) {
        return Ok(file_name.to_string()); // already disabled
    } else {
        format!("{file_name}{DISABLED_SUFFIX}")
    };

    let target = dir.join(&target_name);
    std::fs::rename(&current, &target)?;
    Ok(target_name)
}

/// Deletes an addon jar from its folder.
pub fn delete(dir: &Path, file_name: &str) -> AppResult<()> {
    let file_name = safe_file_name(file_name)?;
    let path = dir.join(file_name);
    if !path.is_file() {
        return Err(AppError::InvalidInput("addon not found".to_string()));
    }
    std::fs::remove_file(path)?;
    Ok(())
}

/// Turns an arbitrary marketplace title into a filesystem-safe `.jar` name,
/// for sources (like SpigotMC) that don't hand back a ready file name.
pub fn sanitize_jar_name(title: &str, id: &str) -> String {
    let safe_title: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = safe_title.trim_matches('-');
    if trimmed.is_empty() {
        format!("{id}.jar")
    } else {
        format!("{trimmed}-{id}.jar")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_name_strips_extension_and_disabled() {
        assert_eq!(display_name("EssentialsX.jar"), "EssentialsX");
        assert_eq!(display_name("EssentialsX.jar.disabled"), "EssentialsX");
        assert_eq!(display_name("LuckPerms-5.4.jar"), "LuckPerms-5.4");
    }

    #[test]
    fn safe_file_name_rejects_paths() {
        assert!(safe_file_name("ok.jar").is_ok());
        assert!(safe_file_name("../evil.jar").is_err());
        assert!(safe_file_name("sub/evil.jar").is_err());
        assert!(safe_file_name("..").is_err());
        assert!(safe_file_name("").is_err());
    }

    #[test]
    fn import_jar_rejects_non_jars() {
        let dir = std::env::temp_dir().join("serverforge-import-jar-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let source = dir.join("notes.txt");
        std::fs::write(&source, b"not a plugin").expect("write source");

        let outcome = import_jar(&dir.join("plugins"), &source);
        assert!(outcome.is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_jar_copies_into_the_addon_folder() {
        let dir = std::env::temp_dir().join("serverforge-import-jar-ok-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let source = dir.join("CoolPlugin.jar");
        std::fs::write(&source, b"jar bytes").expect("write source");
        let plugins = dir.join("plugins");

        let imported = import_jar(&plugins, &source).expect("import");
        assert_eq!(imported.file_name, "CoolPlugin.jar");
        assert_eq!(imported.display_name, "CoolPlugin");
        assert!(imported.enabled);
        assert!(plugins.join("CoolPlugin.jar").is_file());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sanitize_jar_name_keeps_it_safe() {
        assert_eq!(
            sanitize_jar_name("Essentials X!", "123"),
            "Essentials-X-123.jar"
        );
        assert_eq!(sanitize_jar_name("../../evil", "9"), "evil-9.jar");
    }
}
