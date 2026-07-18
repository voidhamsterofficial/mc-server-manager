//! Application state managed by Tauri. Global settings live in a YAML next
//! to the binary (or the user config dir when that's read-only); each
//! server's settings live in its own folder. App data keeps the satellite
//! stores: scheduled tasks, player rosters, and managed Java runtimes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::error::AppResult;
use crate::process::RunningMap;
use crate::roster::RosterStore;
use crate::scheduler::{self, ScheduledTask};
use crate::servers::{self, ServerConfig, ServerRegistry};
use crate::settings::{self, GlobalSettings};

const TASKS_FILE_NAME: &str = "schedules.json";
const SERVERS_DIR_NAME: &str = "servers";
const BACKUPS_DIR_NAME: &str = "backups";
const MANAGED_JAVA_DIR_NAME: &str = "java";
const ROSTERS_DIR_NAME: &str = "rosters";

pub struct AppState {
    data_dir: PathBuf,
    settings_path: PathBuf,
    pub http: reqwest::Client,
    pub registry: Mutex<ServerRegistry>,
    pub settings: Mutex<GlobalSettings>,
    pub tasks: Mutex<Vec<ScheduledTask>>,
    pub running: RunningMap,
    /// Serializes automatic Java downloads so two servers starting at once
    /// don't fetch the same runtime twice.
    pub java_download_lock: Mutex<()>,
    pub rosters: RosterStore,
}

impl AppState {
    /// Creates the on-disk layout and loads persisted state. Called once at
    /// app startup.
    pub fn initialize(app: &AppHandle) -> AppResult<Self> {
        let data_dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(data_dir.join(SERVERS_DIR_NAME))?;
        std::fs::create_dir_all(data_dir.join(MANAGED_JAVA_DIR_NAME))?;

        let settings_path = settings::global_settings_path(app)?;
        let global = settings::load_or_migrate(app, &settings_path, &data_dir)?;
        let registry = ServerRegistry::load_from_dirs(&global.server_dirs);

        let tasks = scheduler::load_tasks(&data_dir.join(TASKS_FILE_NAME))?;

        let state = Self {
            http: build_http_client()?,
            registry: Mutex::new(registry),
            settings: Mutex::new(global),
            tasks: Mutex::new(tasks),
            running: Arc::new(Mutex::new(HashMap::new())),
            java_download_lock: Mutex::new(()),
            rosters: RosterStore::new(data_dir.join(ROSTERS_DIR_NAME)),
            settings_path,
            data_dir,
        };
        Ok(state)
    }

    /// Where the global settings YAML lives.
    pub fn settings_path(&self) -> PathBuf {
        self.settings_path.clone()
    }

    pub fn tasks_path(&self) -> PathBuf {
        self.data_dir.join(TASKS_FILE_NAME)
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

    /// Persists a newly created server: its own YAML plus the global list
    /// of server folders.
    pub async fn persist_new_server(&self, config: &ServerConfig) -> AppResult<()> {
        servers::save_server_settings(config)?;

        let mut settings = self.settings.lock().await;
        if !settings.server_dirs.contains(&config.dir) {
            settings.server_dirs.push(config.dir.clone());
            settings.save(&self.settings_path)?;
        }
        Ok(())
    }

    /// Forgets a deleted server's folder in the global list.
    pub async fn persist_removed_server(&self, server_dir: &Path) -> AppResult<()> {
        let mut settings = self.settings.lock().await;
        settings.server_dirs.retain(|dir| dir != server_dir);
        settings.save(&self.settings_path)
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
