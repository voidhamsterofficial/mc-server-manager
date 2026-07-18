//! Per-server player history: everyone who has ever joined, how often, how
//! long they've played, and how many times they were kicked. Fed by console
//! signals and persisted as one JSON file per server in app data.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::error::AppResult;
use crate::servers::current_unix_time;

/// The most recent chat lines kept per player.
const MAX_CHAT_HISTORY: usize = 200;

/// One remembered chat line.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatEntry {
    pub at_unix: u64,
    pub message: String,
}

/// Everything remembered about one player on one server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerRecord {
    pub first_joined_unix: u64,
    pub last_seen_unix: u64,
    pub join_count: u32,
    pub kick_count: u32,
    pub total_play_seconds: u64,
    #[serde(default)]
    pub chat_count: u32,
    #[serde(default)]
    pub chat_log: Vec<ChatEntry>,
    /// Last game mode we saw this player set to, if ever.
    #[serde(default)]
    pub last_game_mode: Option<String>,
}

/// Full detail for one player, for the player page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerDetail {
    pub name: String,
    pub online: bool,
    pub banned: bool,
    /// The reason recorded when the player was banned, if any.
    pub ban_reason: Option<String>,
    pub first_joined_unix: u64,
    pub last_seen_unix: u64,
    pub join_count: u32,
    pub kick_count: u32,
    pub chat_count: u32,
    pub total_play_seconds: u64,
    /// Last game mode we saw this player set to, if ever.
    pub last_game_mode: Option<String>,
    /// Newest chat lines first.
    pub recent_chat: Vec<ChatEntry>,
}

/// One server's player history plus the sessions currently in progress.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Roster {
    players: HashMap<String, PlayerRecord>,
    /// Player -> session start time. Not persisted: sessions end when the
    /// server (or the app) stops.
    #[serde(skip)]
    active_sessions: HashMap<String, u64>,
}

/// A roster row as shown in the UI, enriched with live state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterEntry {
    pub name: String,
    pub online: bool,
    pub banned: bool,
    pub first_joined_unix: u64,
    pub last_seen_unix: u64,
    pub join_count: u32,
    pub kick_count: u32,
    /// Includes the in-progress session for online players.
    pub total_play_seconds: u64,
}

/// All rosters, loaded lazily per server and saved after every change.
pub struct RosterStore {
    rosters_dir: PathBuf,
    by_server: Mutex<HashMap<String, Roster>>,
}

impl RosterStore {
    pub fn new(rosters_dir: PathBuf) -> Self {
        Self {
            rosters_dir,
            by_server: Mutex::new(HashMap::new()),
        }
    }

    pub async fn record_join(&self, server_id: &str, player_name: &str) {
        let now = current_unix_time();
        let mut rosters = self.by_server.lock().await;
        let roster = loaded_roster(&mut rosters, &self.rosters_dir, server_id);

        let record = roster.players.entry(player_name.to_string()).or_default();
        if record.first_joined_unix == 0 {
            record.first_joined_unix = now;
        }
        record.last_seen_unix = now;
        record.join_count += 1;
        roster.active_sessions.insert(player_name.to_string(), now);

        self.save(server_id, roster);
    }

    pub async fn record_leave(&self, server_id: &str, player_name: &str) {
        let now = current_unix_time();
        let mut rosters = self.by_server.lock().await;
        let roster = loaded_roster(&mut rosters, &self.rosters_dir, server_id);

        close_session(roster, player_name, now);
        self.save(server_id, roster);
    }

    pub async fn record_kick(&self, server_id: &str, player_name: &str) {
        let mut rosters = self.by_server.lock().await;
        let roster = loaded_roster(&mut rosters, &self.rosters_dir, server_id);

        let record = roster.players.entry(player_name.to_string()).or_default();
        record.kick_count += 1;

        self.save(server_id, roster);
    }

    pub async fn record_game_mode(&self, server_id: &str, player_name: &str, mode: &str) {
        let mut rosters = self.by_server.lock().await;
        let roster = loaded_roster(&mut rosters, &self.rosters_dir, server_id);

        let record = roster.players.entry(player_name.to_string()).or_default();
        record.last_game_mode = Some(mode.to_string());

        self.save(server_id, roster);
    }

    pub async fn record_chat(&self, server_id: &str, player_name: &str, message: &str) {
        let now = current_unix_time();
        let mut rosters = self.by_server.lock().await;
        let roster = loaded_roster(&mut rosters, &self.rosters_dir, server_id);

        let record = roster.players.entry(player_name.to_string()).or_default();
        record.chat_count += 1;
        record.chat_log.push(ChatEntry {
            at_unix: now,
            message: message.to_string(),
        });
        if record.chat_log.len() > MAX_CHAT_HISTORY {
            let overflow = record.chat_log.len() - MAX_CHAT_HISTORY;
            record.chat_log.drain(0..overflow);
        }

        self.save(server_id, roster);
    }

    /// Full detail for one player, for the player page.
    pub async fn detail(
        &self,
        server_id: &str,
        player_name: &str,
        online_players: &[String],
        banned: bool,
        ban_reason: Option<String>,
    ) -> Option<PlayerDetail> {
        let now = current_unix_time();
        let mut rosters = self.by_server.lock().await;
        let roster = loaded_roster(&mut rosters, &self.rosters_dir, server_id);

        let record = roster.players.get(player_name)?;
        let live_session_seconds = roster
            .active_sessions
            .get(player_name)
            .map(|start| now.saturating_sub(*start))
            .unwrap_or(0);

        let mut recent_chat = record.chat_log.clone();
        recent_chat.reverse();

        Some(PlayerDetail {
            name: player_name.to_string(),
            online: online_players.iter().any(|p| p == player_name),
            banned,
            ban_reason,
            first_joined_unix: record.first_joined_unix,
            last_seen_unix: record.last_seen_unix,
            join_count: record.join_count,
            kick_count: record.kick_count,
            chat_count: record.chat_count,
            total_play_seconds: record.total_play_seconds + live_session_seconds,
            last_game_mode: record.last_game_mode.clone(),
            recent_chat,
        })
    }

    /// Ends every open session — called when the server process exits.
    pub async fn close_all_sessions(&self, server_id: &str) {
        let now = current_unix_time();
        let mut rosters = self.by_server.lock().await;
        let roster = loaded_roster(&mut rosters, &self.rosters_dir, server_id);

        let open_players: Vec<String> = roster.active_sessions.keys().cloned().collect();
        for player_name in open_players {
            close_session(roster, &player_name, now);
        }
        self.save(server_id, roster);
    }

    /// The full history for one server, enriched with online/banned state,
    /// sorted by total playtime (busiest players first).
    pub async fn entries(
        &self,
        server_id: &str,
        online_players: &[String],
        banned_names: &[String],
    ) -> Vec<RosterEntry> {
        let now = current_unix_time();
        let mut rosters = self.by_server.lock().await;
        let roster = loaded_roster(&mut rosters, &self.rosters_dir, server_id);

        let mut entries: Vec<RosterEntry> = roster
            .players
            .iter()
            .map(|(name, record)| {
                let live_session_seconds = roster
                    .active_sessions
                    .get(name)
                    .map(|start| now.saturating_sub(*start))
                    .unwrap_or(0);

                RosterEntry {
                    name: name.clone(),
                    online: online_players.contains(name),
                    banned: banned_names.iter().any(|banned| banned == name),
                    first_joined_unix: record.first_joined_unix,
                    last_seen_unix: record.last_seen_unix,
                    join_count: record.join_count,
                    kick_count: record.kick_count,
                    total_play_seconds: record.total_play_seconds + live_session_seconds,
                }
            })
            .collect();

        entries.sort_by_key(|entry| std::cmp::Reverse(entry.total_play_seconds));
        entries
    }

    /// Drops a deleted server's history from cache and disk.
    pub async fn forget(&self, server_id: &str) {
        let mut rosters = self.by_server.lock().await;
        rosters.remove(server_id);

        let path = roster_path(&self.rosters_dir, server_id);
        if !path.exists() {
            return;
        }
        if let Err(error) = std::fs::remove_file(&path) {
            log::warn!("could not remove roster for {server_id}: {error}");
        }
    }

    fn save(&self, server_id: &str, roster: &Roster) {
        let result = save_roster(&self.rosters_dir, server_id, roster);
        if let Err(error) = result {
            log::warn!("failed to save player roster for {server_id}: {error}");
        }
    }
}

fn close_session(roster: &mut Roster, player_name: &str, now: u64) {
    let Some(session_start) = roster.active_sessions.remove(player_name) else {
        return;
    };
    let Some(record) = roster.players.get_mut(player_name) else {
        return;
    };
    record.total_play_seconds += now.saturating_sub(session_start);
    record.last_seen_unix = now;
}

/// Gets the cached roster for a server, loading it from disk on first use.
fn loaded_roster<'a>(
    rosters: &'a mut HashMap<String, Roster>,
    rosters_dir: &Path,
    server_id: &str,
) -> &'a mut Roster {
    rosters
        .entry(server_id.to_string())
        .or_insert_with(|| load_roster(rosters_dir, server_id))
}

fn load_roster(rosters_dir: &Path, server_id: &str) -> Roster {
    let path = roster_path(rosters_dir, server_id);
    if !path.exists() {
        return Roster::default();
    }

    let loaded = std::fs::read_to_string(&path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok());
    loaded.unwrap_or_default()
}

fn save_roster(rosters_dir: &Path, server_id: &str, roster: &Roster) -> AppResult<()> {
    std::fs::create_dir_all(rosters_dir)?;
    let serialized = serde_json::to_string_pretty(roster)?;
    std::fs::write(roster_path(rosters_dir, server_id), serialized)?;
    Ok(())
}

fn roster_path(rosters_dir: &Path, server_id: &str) -> PathBuf {
    rosters_dir.join(format!("{server_id}.json"))
}

/// One entry from the server's `banned-players.json`.
#[derive(Debug, Clone)]
pub struct BanRecord {
    pub name: String,
    /// The reason recorded with the ban (Minecraft defaults to
    /// "Banned by an operator." when none is given).
    pub reason: Option<String>,
}

/// The server's own `banned-players.json` — the authoritative ban list,
/// whether the ban came from our UI or in-game, with each ban's reason.
pub fn read_bans(server_dir: &Path) -> Vec<BanRecord> {
    #[derive(Deserialize)]
    struct BannedPlayer {
        name: String,
        #[serde(default)]
        reason: Option<String>,
    }

    let path = server_dir.join("banned-players.json");
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let entries: Vec<BannedPlayer> = serde_json::from_str(&contents).unwrap_or_default();
    entries
        .into_iter()
        .map(|entry| BanRecord {
            name: entry.name,
            reason: entry.reason,
        })
        .collect()
}

/// Just the names from `banned-players.json`, for the roster list.
pub fn read_banned_names(server_dir: &Path) -> Vec<String> {
    read_bans(server_dir)
        .into_iter()
        .map(|record| record.name)
        .collect()
}
