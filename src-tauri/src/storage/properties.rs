//! Reading and writing `server.properties` while preserving comments and
//! line order, so hand-edited files stay recognisable.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

const PROPERTIES_FILE_NAME: &str = "server.properties";

/// One `key=value` pair from `server.properties`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Property {
    pub key: String,
    pub value: String,
}

/// Reads all properties from a server directory. Returns an empty list when
/// the file doesn't exist yet (the server writes it on first start).
pub fn read(server_dir: &Path) -> AppResult<Vec<Property>> {
    let file_path = server_dir.join(PROPERTIES_FILE_NAME);
    if !file_path.exists() {
        return Ok(Vec::new());
    }

    let contents = std::fs::read_to_string(&file_path)?;
    let properties = parse(&contents);
    Ok(properties)
}

/// A complete default `server.properties`, so the file is fully populated
/// the moment a server is created. Without this the file is generated on
/// first start, which can clobber edits the user made in between.
const DEFAULT_PROPERTIES: &str = "\
allow-flight=false
allow-nether=true
difficulty=easy
enable-command-block=false
enforce-secure-profile=true
enforce-whitelist=false
force-gamemode=false
gamemode=survival
generate-structures=true
hardcore=false
hide-online-players=false
level-name=world
level-seed=
level-type=minecraft\\:normal
max-players=20
max-world-size=29999984
motd=A Minecraft Server
online-mode=true
pvp=true
simulation-distance=10
spawn-monsters=true
spawn-protection=16
view-distance=10
white-list=false
";

/// Writes the default `server.properties` if none exists yet. Called at
/// creation for game servers.
pub fn ensure_defaults(server_dir: &Path) -> AppResult<()> {
    let file_path = server_dir.join(PROPERTIES_FILE_NAME);
    if file_path.exists() {
        return Ok(());
    }
    std::fs::write(file_path, DEFAULT_PROPERTIES)?;
    Ok(())
}

/// Applies updated values to the file, keeping comments, ordering, and any
/// keys the update doesn't mention. Unknown new keys are appended.
pub fn write(server_dir: &Path, updates: &[Property]) -> AppResult<()> {
    let file_path = server_dir.join(PROPERTIES_FILE_NAME);
    let existing_contents = if file_path.exists() {
        std::fs::read_to_string(&file_path)?
    } else {
        String::new()
    };

    let merged = merge(&existing_contents, updates);
    std::fs::write(&file_path, merged)?;
    Ok(())
}

fn parse(contents: &str) -> Vec<Property> {
    let mut properties = Vec::new();
    for line in contents.lines() {
        let Some(property) = parse_line(line) else {
            continue;
        };
        properties.push(property);
    }
    properties
}

fn parse_line(line: &str) -> Option<Property> {
    let trimmed = line.trim();
    let is_comment_or_blank =
        trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!');
    if is_comment_or_blank {
        return None;
    }

    let (key, value) = trimmed.split_once('=')?;
    let property = Property {
        key: key.trim().to_string(),
        value: value.trim().to_string(),
    };
    Some(property)
}

fn merge(existing_contents: &str, updates: &[Property]) -> String {
    let mut remaining_updates: Vec<&Property> = updates.iter().collect();
    let mut output_lines: Vec<String> = Vec::new();

    for line in existing_contents.lines() {
        let rewritten = rewrite_line(line, &mut remaining_updates);
        output_lines.push(rewritten);
    }

    for new_property in remaining_updates {
        output_lines.push(format!("{}={}", new_property.key, new_property.value));
    }

    let mut merged = output_lines.join("\n");
    merged.push('\n');
    merged
}

/// Replaces a line's value if an update targets its key; removes the used
/// update from `remaining_updates`. Non-property lines pass through as-is.
fn rewrite_line(line: &str, remaining_updates: &mut Vec<&Property>) -> String {
    let Some(existing) = parse_line(line) else {
        return line.to_string();
    };

    let update_position = remaining_updates
        .iter()
        .position(|update| update.key == existing.key);
    let Some(position) = update_position else {
        return line.to_string();
    };

    let update = remaining_updates.remove(position);
    format!("{}={}", update.key, update.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "#Minecraft server properties\n#Wed Jul 15 2026\nmax-players=20\npvp=true\nmotd=A Minecraft Server\n";

    #[test]
    fn parses_keys_and_skips_comments() {
        let properties = parse(SAMPLE);
        assert_eq!(properties.len(), 3);
        assert_eq!(properties[0].key, "max-players");
        assert_eq!(properties[0].value, "20");
    }

    #[test]
    fn merge_preserves_comments_and_order() {
        let updates = vec![Property {
            key: "pvp".to_string(),
            value: "false".to_string(),
        }];
        let merged = merge(SAMPLE, &updates);

        assert!(merged.starts_with("#Minecraft server properties\n"));
        assert!(merged.contains("max-players=20"));
        assert!(merged.contains("pvp=false"));
        assert!(merged.contains("motd=A Minecraft Server"));
    }

    #[test]
    fn merge_appends_new_keys() {
        let updates = vec![Property {
            key: "view-distance".to_string(),
            value: "12".to_string(),
        }];
        let merged = merge(SAMPLE, &updates);
        assert!(merged.ends_with("view-distance=12\n"));
    }
}
