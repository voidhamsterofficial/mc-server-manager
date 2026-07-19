//! Application state managed by Tauri. Cross-server / app-level state (the
//! known-servers list, the default server location, scheduled tasks, player
//! rosters, and installed-plugin tracking) lives in a SQLite database — see
//! `db.rs`. Each server's own settings stay in its own folder as YAML.
//! App data also keeps a couple of satellite directories: managed Java
//! runtimes and the (deprecated but still used as the default) servers dir.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::error::AppResult;
use crate::players::roster::RosterStore;
use crate::process::RunningMap;
use crate::servers::scheduler::{self, ScheduledTask};
use crate::servers::{self, ServerConfig, ServerRegistry};
use crate::storage::db::Db;
use crate::storage::settings;

const SERVERS_DIR_NAME: &str = "servers";
const BACKUPS_DIR_NAME: &str = "backups";
const MANAGED_JAVA_DIR_NAME: &str = "java";

/// Key in `kv_settings` for the default parent directory new servers are
/// created under.
const SERVERS_BASE_DIR_KEY: &str = "servers_base_dir";

pub struct AppState {
    data_dir: PathBuf,
    pub http: reqwest::Client,
    pub registry: Mutex<ServerRegistry>,
    /// The app-wide SQLite database. Shared with `RosterStore` (an `Arc`) so
    /// both can reach it without state.rs mediating every roster write.
    pub db: Arc<Mutex<Db>>,
    pub tasks: Mutex<Vec<ScheduledTask>>,
    pub running: RunningMap,
    /// Serializes automatic Java downloads so two servers starting at once
    /// don't fetch the same runtime twice.
    pub java_download_lock: Mutex<()>,
    pub rosters: RosterStore,
    /// server_id -> external port, for servers whose port we forwarded via
    /// UPnP this session. Lets us close the mapping when the server stops
    /// (on the external port the router actually used, which can differ from
    /// the server's internal port — see `portforward::open`), and skip a
    /// pointless UPnP round-trip for servers that were never forwarded.
    pub forwarded: Mutex<HashMap<String, u16>>,
}

impl AppState {
    /// Creates the on-disk layout and loads persisted state. Called once at
    /// app startup.
    pub fn initialize(app: &AppHandle) -> AppResult<Self> {
        let data_dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(data_dir.join(SERVERS_DIR_NAME))?;
        std::fs::create_dir_all(data_dir.join(MANAGED_JAVA_DIR_NAME))?;

        let db_path = crate::storage::db::resolve_db_path(app)?;
        let db = Db::open(db_path)?;

        // First run: seed a sensible default server location.
        if db.get_kv(SERVERS_BASE_DIR_KEY)?.is_none() {
            let default_dir = settings::default_servers_base_dir(app, &data_dir);
            db.set_kv(SERVERS_BASE_DIR_KEY, &default_dir.to_string_lossy())?;
        }

        // Known servers whose folder or settings file can no longer be found
        // are dropped from the list here, rather than resurrected forever.
        let known_servers = db.list_known_servers()?;
        let (registry, missing_ids) = ServerRegistry::load_known(&known_servers);
        for id in &missing_ids {
            if let Err(error) = db.remove_known_server(id) {
                log::warn!("could not prune unreachable server {id}: {error}");
            }
        }

        let tasks = scheduler::load_tasks(&db)?;

        let db = Arc::new(Mutex::new(db));

        let state = Self {
            http: build_http_client()?,
            registry: Mutex::new(registry),
            tasks: Mutex::new(tasks),
            running: Arc::new(Mutex::new(HashMap::new())),
            java_download_lock: Mutex::new(()),
            rosters: RosterStore::new(Arc::clone(&db)),
            forwarded: Mutex::new(HashMap::new()),
            db,
            data_dir,
        };
        Ok(state)
    }

    pub fn servers_dir(&self) -> PathBuf {
        self.data_dir.join(SERVERS_DIR_NAME)
    }

    /// A server's files directory. Pre-YAML configs stored an empty dir and
    /// lived in app data; migration fills it, but stay defensive.
    pub fn server_dir(&self, config: &ServerConfig) -> PathBuf {
        if config.dir.as_os_str().is_empty() {
            return self.servers_dir().join(&config.id);
        }
        config.dir.clone()
    }

    /// Where one server's backup archives live: the per-server override, or
    /// a `backups` folder inside the server directory by default.
    pub fn backups_dir(&self, config: &ServerConfig) -> PathBuf {
        if let Some(chosen_dir) = &config.backups_dir {
            return chosen_dir.clone();
        }
        self.server_dir(config).join(BACKUPS_DIR_NAME)
    }

    /// Where auto-downloaded Java runtimes live.
    pub fn managed_java_dir(&self) -> PathBuf {
        self.data_dir.join(MANAGED_JAVA_DIR_NAME)
    }

    /// The default parent directory new servers are created under.
    pub async fn servers_base_dir(&self) -> AppResult<PathBuf> {
        let db = self.db.lock().await;
        let stored = db.get_kv(SERVERS_BASE_DIR_KEY)?;
        Ok(stored
            .map(PathBuf::from)
            .unwrap_or_else(|| self.servers_dir()))
    }

    pub async fn set_servers_base_dir(&self, dir: &Path) -> AppResult<()> {
        let db = self.db.lock().await;
        db.set_kv(SERVERS_BASE_DIR_KEY, &dir.to_string_lossy())
    }

    /// Persists a newly created (or imported) server: its own YAML plus an
    /// entry in the database's known-servers list.
    pub async fn persist_new_server(&self, config: &ServerConfig) -> AppResult<()> {
        servers::save_server_settings(config)?;
        let db = self.db.lock().await;
        db.add_known_server(&config.id, &config.dir)
    }

    /// Drops a server from the known-servers list (used on delete, and when a
    /// server's folder can no longer be found).
    pub async fn forget_known_server(&self, server_id: &str) -> AppResult<()> {
        let db = self.db.lock().await;
        db.remove_known_server(server_id)
    }
}

fn build_http_client() -> AppResult<reqwest::Client> {
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            "Blockparty/",
            env!("CARGO_PKG_VERSION"),
            " (github.com/Squ1ggly/mc-server-manager)"
        ))
        // Without these, a hung CDN or half-open socket blocks the awaiting
        // task forever (version lists spin, downloads never fail). read_timeout
        // is an idle/between-chunks timeout, so it won't cut a slow-but-
        // progressing download — only a genuinely stalled one.
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(60))
        .build()?;
    Ok(client)
}
