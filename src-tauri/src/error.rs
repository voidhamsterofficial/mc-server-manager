use serde::Serialize;

/// Application-wide error type. Every fallible service returns `AppResult`,
/// and Tauri commands serialize the error as its display message.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Tauri error: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("server not found: {0}")]
    ServerNotFound(String),

    #[error("server is already running")]
    ServerAlreadyRunning,

    #[error("port {port} is already in use by the running server \"{other_server}\"")]
    PortInUse { port: String, other_server: String },

    #[error("server is not running")]
    ServerNotRunning,

    #[error("no suitable Java installation found (need Java {required_major} or newer)")]
    NoSuitableJava { required_major: u32 },

    #[error("downloaded file failed checksum verification: {file_name}")]
    ChecksumMismatch { file_name: String },

    #[error("unknown Minecraft version: {0}")]
    UnknownMinecraftVersion(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("process error: {0}")]
    Process(String),

    #[error("archive error: {0}")]
    Archive(#[from] zip::result::ZipError),

    #[error("invalid cron expression: {0}")]
    InvalidCron(String),

    #[error("scheduled task not found: {0}")]
    TaskNotFound(String),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let message = self.to_string();
        serializer.serialize_str(&message)
    }
}

pub type AppResult<T> = Result<T, AppError>;
