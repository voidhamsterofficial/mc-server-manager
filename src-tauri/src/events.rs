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

/// A backup finished for a server (payload: the server id string).
pub const BACKUP_CREATED: &str = "server:backup-created";
