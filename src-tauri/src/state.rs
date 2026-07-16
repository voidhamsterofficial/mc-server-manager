//! Application state managed by Tauri: data directory layout, the persisted
//! server registry, the running-process map, and a shared HTTP client.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::error::AppResult;
use crate::process::RunningMap;
use crate::scheduler::{self, ScheduledTask};
use crate::servers::{ServerConfig, ServerRegistry};
use crate::settings::AppSettings;

const REGISTRY_FILE_NAME: &str = "servers.json";
const SETTINGS_FILE_NAME: &str = "settings.json";
const TASKS_FILE_NAME: &str = "schedules.json";
const SERVERS_DIR_NAME: &str = "servers";
const BACKUPS_DIR_NAME: &str = "backups";
const MANAGED_JAVA_DIR_NAME: &str = "java";

pub struct AppState {
    data_dir: PathBuf,
    pub http: reqwest::Client,
    pub registry: Mutex<ServerRegistry>,
    pub settings: Mutex<AppSettings>,
    pub tasks: Mutex<Vec<ScheduledTask>>,
    pub running: RunningMap,
    /// Serializes automatic Java downloads so two servers starting at once
    /// don't fetch the same runtime twice.
    pub java_download_lock: Mutex<()>,
}

impl AppState {
    /// Creates the on-disk layout and loads persisted state. Called once at
    /// app startup.
    pub fn initialize(app: &AppHandle) -> AppResult<Self> {
        let data_dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(data_dir.join(SERVERS_DIR_NAME))?;
        std::fs::create_dir_all(data_dir.join(MANAGED_JAVA_DIR_NAME))?;

        let registry_path = data_dir.join(REGISTRY_FILE_NAME);
        let registry = ServerRegistry::load(&registry_path)?;

        let settings_path = data_dir.join(SETTINGS_FILE_NAME);
        let settings = AppSettings::load_or_default(&settings_path, app, &data_dir);

        let tasks = scheduler::load_tasks(&data_dir.join(TASKS_FILE_NAME))?;

        let state = Self {
            http: build_http_client()?,
            registry: Mutex::new(registry),
            settings: Mutex::new(settings),
            tasks: Mutex::new(tasks),
            running: Arc::new(Mutex::new(HashMap::new())),
            java_download_lock: Mutex::new(()),
            data_dir,
        };
        Ok(state)
    }

    pub fn registry_path(&self) -> PathBuf {
        self.data_dir.join(REGISTRY_FILE_NAME)
    }

    pub fn servers_dir(&self) -> PathBuf {
        self.data_dir.join(SERVERS_DIR_NAME)
    }

    pub fn settings_path(&self) -> PathBuf {
        self.data_dir.join(SETTINGS_FILE_NAME)
    }

    pub fn tasks_path(&self) -> PathBuf {
        self.data_dir.join(TASKS_FILE_NAME)
    }

    /// Where one server's backup archives live: the per-server override, or
    /// a `backups` folder inside the server directory by default.
    pub fn backups_dir(&self, config: &ServerConfig) -> PathBuf {
        if let Some(chosen_dir) = &config.backups_dir {
            return chosen_dir.clone();
        }
        self.server_dir(config).join(BACKUPS_DIR_NAME)
    }

    /// A server's files directory. Servers created before custom locations
    /// existed have an empty `dir` and resolve to the legacy app-data path.
    pub fn server_dir(&self, config: &ServerConfig) -> PathBuf {
        if config.dir.as_os_str().is_empty() {
            return self.servers_dir().join(&config.id);
        }
        config.dir.clone()
    }

    /// Where auto-downloaded Java runtimes live (populated in a later phase;
    /// already scanned during Java detection).
    pub fn managed_java_dir(&self) -> PathBuf {
        self.data_dir.join(MANAGED_JAVA_DIR_NAME)
    }
}

fn build_http_client() -> AppResult<reqwest::Client> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("mc-server-manager/", env!("CARGO_PKG_VERSION")))
        .build()?;
    Ok(client)
}
