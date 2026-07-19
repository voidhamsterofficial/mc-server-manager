//! Persistence and on-disk state: the app-wide SQLite database, server
//! backups, `server.properties` handling, the first-run location default, the
//! server-scoped file browser, and shared atomic-write helpers.

pub mod backups;
pub mod db;
pub mod files;
pub mod fsutil;
pub mod properties;
pub mod settings;
