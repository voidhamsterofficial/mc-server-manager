//! Reading a player's persisted game mode straight from the world save.
//!
//! Minecraft never logs a player's game mode on join, nor when it's changed via
//! the in-game menu or on many non-vanilla servers — so console parsing alone
//! leaves most players "unknown". The world save is the ground truth:
//! `usercache.json` maps names to UUIDs and `<level>/playerdata/<uuid>.dat`
//! (gzipped NBT) holds `playerGameType`.

use std::io::Read;
use std::path::Path;

use flate2::read::GzDecoder;
use serde::Deserialize;

use crate::storage::properties;

/// One entry of the server's `usercache.json`.
#[derive(Debug, Deserialize)]
struct UserCacheEntry {
    name: String,
    uuid: String,
}

/// The single field we need out of a player's `.dat` NBT; every other tag is
/// ignored by serde.
#[derive(Debug, Deserialize)]
struct PlayerDat {
    #[serde(rename = "playerGameType")]
    player_game_type: Option<i32>,
}

/// Resolves a player's game mode from the world save, falling back to the
/// world's default `gamemode` when their personal data can't be read. Returns a
/// capitalized short word ("Survival", "Creative", …) to match the console
/// signal's format, or `None` if nothing is known yet.
pub fn game_mode(server_dir: &Path, player_name: &str) -> Option<String> {
    player_game_mode(server_dir, player_name).or_else(|| default_game_mode(server_dir))
}

/// The player's own persisted mode from `<level>/playerdata/<uuid>.dat`.
fn player_game_mode(server_dir: &Path, player_name: &str) -> Option<String> {
    let uuid = lookup_uuid(server_dir, player_name)?;
    let dat_path = server_dir
        .join(level_name(server_dir))
        .join("playerdata")
        .join(format!("{uuid}.dat"));

    let compressed = std::fs::read(&dat_path).ok()?;
    let mut decoder = GzDecoder::new(&compressed[..]);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).ok()?;

    let data: PlayerDat = fastnbt::from_bytes(&decompressed).ok()?;
    mode_word(data.player_game_type?)
}

/// Case-insensitive name -> UUID from `usercache.json`.
fn lookup_uuid(server_dir: &Path, player_name: &str) -> Option<String> {
    let raw = std::fs::read_to_string(server_dir.join("usercache.json")).ok()?;
    let entries: Vec<UserCacheEntry> = serde_json::from_str(&raw).ok()?;
    entries
        .into_iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(player_name))
        .map(|entry| entry.uuid)
}

/// The world folder name (`level-name`), defaulting to "world".
fn level_name(server_dir: &Path) -> String {
    properties::read(server_dir)
        .ok()
        .and_then(|props| props.into_iter().find(|p| p.key == "level-name"))
        .map(|p| p.value)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "world".to_string())
}

/// The server's default game mode (`gamemode` property), capitalized.
fn default_game_mode(server_dir: &Path) -> Option<String> {
    let props = properties::read(server_dir).ok()?;
    let value = props.into_iter().find(|p| p.key == "gamemode")?.value;
    capitalize(&value)
}

/// Maps the numeric `playerGameType` to a capitalized short word.
fn mode_word(game_type: i32) -> Option<String> {
    let word = match game_type {
        0 => "Survival",
        1 => "Creative",
        2 => "Adventure",
        3 => "Spectator",
        _ => return None,
    };
    Some(word.to_string())
}

/// Capitalizes a lowercase mode word: "survival" -> "Survival".
fn capitalize(value: &str) -> Option<String> {
    let mut chars = value.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().collect::<String>() + chars.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_numeric_game_types() {
        assert_eq!(mode_word(0).as_deref(), Some("Survival"));
        assert_eq!(mode_word(1).as_deref(), Some("Creative"));
        assert_eq!(mode_word(2).as_deref(), Some("Adventure"));
        assert_eq!(mode_word(3).as_deref(), Some("Spectator"));
        assert_eq!(mode_word(7), None);
    }

    #[test]
    fn capitalizes_default_mode_words() {
        assert_eq!(capitalize("survival").as_deref(), Some("Survival"));
        assert_eq!(capitalize("creative").as_deref(), Some("Creative"));
        assert_eq!(capitalize(""), None);
    }
}
