//! Names of the events the backend emits to the UI. Payload types live with
//! the domain that produces them.

/// Batched console lines: [`crate::process::ConsoleBatchEvent`].
pub const SERVER_CONSOLE: &str = "server:console";

/// A server changed status: [`crate::process::StatusEvent`].
pub const SERVER_STATUS: &str = "server:status";

/// Progress of a server installation: [`crate::installers::InstallProgressEvent`].
pub const INSTALL_PROGRESS: &str = "install:progress";

/// Online player list changed: [`crate::process::PlayersEvent`].
pub const SERVER_PLAYERS: &str = "server:players";

/// Periodic resource usage sample: [`crate::process::stats::StatsEvent`].
pub const SERVER_STATS: &str = "server:stats";

/// A server's process ended unexpectedly: [`crate::process::CrashedEvent`].
/// Emitted by the process supervisor; whether to restart is *policy* and
/// lives in `servers::service`, which listens for this — the low-level
/// process layer deliberately doesn't reach back up into the start path.
pub const SERVER_CRASHED: &str = "server:crashed";

/// A backup finished for a server (payload: the server id string).
pub const BACKUP_CREATED: &str = "server:backup-created";

/// Progress of an ongoing backup: [`crate::storage::backups::BackupProgressEvent`].
pub const BACKUP_PROGRESS: &str = "server:backup-progress";

/// A backup failed for a server (payload: the server id string). The UI needs
/// this to clear its in-progress state — without it a failed backup would
/// leave the progress bar stuck on screen forever.
pub const BACKUP_FAILED: &str = "server:backup-failed";
