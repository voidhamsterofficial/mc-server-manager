//! Per-server player history: everyone who has ever joined, how often, how
//! long they've played, and how many times they were kicked. Fed by console
//! signals and persisted as one JSON blob per server in the app database.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::servers::current_unix_time;
use crate::storage::db::Db;

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

/// All rosters, loaded lazily per server and saved after every change. The
/// `Db` is shared (`Arc`) with `AppState`, which holds the same instance
/// behind its own lock for settings/known-servers/plugin-install access.
pub struct RosterStore {
    db: Arc<Mutex<Db>>,
    by_server: Mutex<HashMap<String, Roster>>,
}

impl RosterStore {
    pub fn new(db: Arc<Mutex<Db>>) -> Self {
        Self {
            db,
            by_server: Mutex::new(HashMap::new()),
        }
    }

    pub async fn record_join(&self, server_id: &str, player_name: &str) {
        let now = current_unix_time();
        let mut rosters = self.by_server.lock().await;
        let roster = self.loaded_roster(&mut rosters, server_id).await;

        let record = roster.players.entry(player_name.to_string()).or_default();
        if record.first_joined_unix == 0 {
            record.first_joined_unix = now;
        }
        record.last_seen_unix = now;
        record.join_count += 1;
        roster.active_sessions.insert(player_name.to_string(), now);

        self.save(server_id, roster).await;
    }

    pub async fn record_leave(&self, server_id: &str, player_name: &str) {
        let now = current_unix_time();
        let mut rosters = self.by_server.lock().await;
        let roster = self.loaded_roster(&mut rosters, server_id).await;

        close_session(roster, player_name, now);
        self.save(server_id, roster).await;
    }

    pub async fn record_kick(&self, server_id: &str, player_name: &str) {
        let mut rosters = self.by_server.lock().await;
        let roster = self.loaded_roster(&mut rosters, server_id).await;

        let record = roster.players.entry(player_name.to_string()).or_default();
        record.kick_count += 1;

        self.save(server_id, roster).await;
    }

    pub async fn record_game_mode(&self, server_id: &str, player_name: &str, mode: &str) {
        let mut rosters = self.by_server.lock().await;
        let roster = self.loaded_roster(&mut rosters, server_id).await;

        let record = roster.players.entry(player_name.to_string()).or_default();
        record.last_game_mode = Some(mode.to_string());

        self.save(server_id, roster).await;
    }

    pub async fn record_chat(&self, server_id: &str, player_name: &str, message: &str) {
        let now = current_unix_time();
        let mut rosters = self.by_server.lock().await;
        let roster = self.loaded_roster(&mut rosters, server_id).await;

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

        self.save(server_id, roster).await;
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
        let roster = self.loaded_roster(&mut rosters, server_id).await;

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
        let roster = self.loaded_roster(&mut rosters, server_id).await;

        let open_players: Vec<String> = roster.active_sessions.keys().cloned().collect();
        for player_name in open_players {
            close_session(roster, &player_name, now);
        }
        self.save(server_id, roster).await;
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
        let roster = self.loaded_roster(&mut rosters, server_id).await;

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

    /// Drops a deleted server's history from cache and the database.
    pub async fn forget(&self, server_id: &str) {
        let mut rosters = self.by_server.lock().await;
        rosters.remove(server_id);
        drop(rosters);

        let db = self.db.lock().await;
        if let Err(error) = db.delete_roster(server_id) {
            log::warn!("could not remove roster for {server_id}: {error}");
        }
    }

    /// Gets the cached roster for a server, loading it from the database on
    /// first use.
    async fn loaded_roster<'a>(
        &self,
        rosters: &'a mut HashMap<String, Roster>,
        server_id: &str,
    ) -> &'a mut Roster {
        if !rosters.contains_key(server_id) {
            let loaded = self.load_from_db(server_id).await;
            rosters.insert(server_id.to_string(), loaded);
        }
        rosters.get_mut(server_id).expect("just inserted")
    }

    async fn load_from_db(&self, server_id: &str) -> Roster {
        let db = self.db.lock().await;
        match db.load_roster_json(server_id) {
            Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_default(),
            Ok(None) => Roster::default(),
            Err(error) => {
                log::warn!("could not load roster for {server_id}: {error}");
                Roster::default()
            }
        }
    }

    async fn save(&self, server_id: &str, roster: &Roster) {
        let serialized = match serde_json::to_string(roster) {
            Ok(serialized) => serialized,
            Err(error) => {
                log::warn!("failed to serialize roster for {server_id}: {error}");
                return;
            }
        };
        let db = self.db.lock().await;
        if let Err(error) = db.save_roster_json(server_id, &serialized) {
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
