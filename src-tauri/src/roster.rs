//! Per-server player history: everyone who has ever joined, how often, how
//! long they've played, and how many times they were kicked. Fed by console
//! signals and persisted as one JSON file per server in app data.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::error::AppResult;
use crate::servers::current_unix_time;

/// Everything remembered about one player on one server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerRecord {
    pub first_joined_unix: u64,
    pub last_seen_unix: u64,
    pub join_count: u32,
    pub kick_count: u32,
    pub total_play_seconds: u64,
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

    fn save(&self, server_id: &str, roster: &Roster) {
        let result = save_roster(&self.rosters_dir, server_id, roster);
        if let Err(error) = result {
            eprintln!("failed to save player roster for {server_id}: {error}");
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

/// Player names from the server's own `banned-players.json` — the
/// authoritative ban list, whether banned from our UI or in-game.
pub fn read_banned_names(server_dir: &Path) -> Vec<String> {
    #[derive(Deserialize)]
    struct BannedPlayer {
        name: String,
    }

    let path = server_dir.join("banned-players.json");
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let entries: Vec<BannedPlayer> = serde_json::from_str(&contents).unwrap_or_default();
    entries.into_iter().map(|entry| entry.name).collect()
}
