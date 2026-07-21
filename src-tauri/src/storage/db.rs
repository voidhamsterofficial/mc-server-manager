//! The app-wide SQLite database: global settings, the list of known servers,
//! player rosters, scheduled tasks, and installed-plugin/mod tracking.
//!
//! Per-server settings (`blockparty-server.yaml`, `server.properties`, worlds,
//! plugin jars, …) stay in each server's own folder exactly as before — only
//! cross-server, app-level state lives in this database. `rusqlite`'s
//! `bundled` feature statically compiles SQLite, so this works identically on
//! macOS, Windows, and Linux with no system dependency.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::{AppHandle, Manager};

use crate::error::AppResult;

/// The database file name, inside whichever directory it currently lives in.
pub const DB_FILE_NAME: &str = "blockparty.db";
/// Pointer file that always lives in the *default* app-data directory. When
/// present, its contents are the directory the database has been relocated
/// to; when absent, the database lives in the default directory itself.
pub const LOCATION_POINTER_FILE: &str = "blockparty.location";

pub struct Db {
    conn: Connection,
    path: PathBuf,
}

impl Db {
    /// Opens (creating if needed) the database at `path` and applies the
    /// schema.
    pub fn open(path: PathBuf) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        Self::migrate(&conn)?;
        Ok(Self { conn, path })
    }

    fn migrate(conn: &Connection) -> AppResult<()> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS kv_settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS known_servers (
                id  TEXT PRIMARY KEY,
                dir TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS rosters (
                server_id TEXT PRIMARY KEY,
                data      TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS plugin_installs (
                server_id         TEXT NOT NULL,
                file_name         TEXT NOT NULL,
                source            TEXT NOT NULL,
                project_id        TEXT,
                version_id        TEXT,
                version_number    TEXT,
                mc_version        TEXT,
                loader_facet      TEXT,
                installed_at_unix INTEGER NOT NULL,
                PRIMARY KEY (server_id, file_name)
            );
            ",
        )?;
        Ok(())
    }

    // --- generic key/value settings -------------------------------------

    pub fn get_kv(&self, key: &str) -> AppResult<Option<String>> {
        let mut statement = self
            .conn
            .prepare("SELECT value FROM kv_settings WHERE key = ?1")?;
        let value = statement
            .query_row(params![key], |row| row.get(0))
            .optional()?;
        Ok(value)
    }

    pub fn set_kv(&self, key: &str, value: &str) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO kv_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // --- known servers ----------------------------------------------------

    pub fn list_known_servers(&self) -> AppResult<Vec<(String, PathBuf)>> {
        let mut statement = self.conn.prepare("SELECT id, dir FROM known_servers")?;
        let rows = statement.query_map([], |row| {
            let id: String = row.get(0)?;
            let dir: String = row.get(1)?;
            Ok((id, PathBuf::from(dir)))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn add_known_server(&self, id: &str, dir: &Path) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO known_servers (id, dir) VALUES (?1, ?2)
             ON CONFLICT(id) DO UPDATE SET dir = excluded.dir",
            params![id, dir.to_string_lossy()],
        )?;
        Ok(())
    }

    pub fn remove_known_server(&self, id: &str) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM known_servers WHERE id = ?1", params![id])?;
        Ok(())
    }

    // --- rosters ------------------------------------------------------------

    pub fn load_roster_json(&self, server_id: &str) -> AppResult<Option<String>> {
        let mut statement = self
            .conn
            .prepare("SELECT data FROM rosters WHERE server_id = ?1")?;
        let value = statement
            .query_row(params![server_id], |row| row.get(0))
            .optional()?;
        Ok(value)
    }

    pub fn save_roster_json(&self, server_id: &str, json: &str) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO rosters (server_id, data) VALUES (?1, ?2)
             ON CONFLICT(server_id) DO UPDATE SET data = excluded.data",
            params![server_id, json],
        )?;
        Ok(())
    }

    pub fn delete_roster(&self, server_id: &str) -> AppResult<()> {
        self.conn.execute(
            "DELETE FROM rosters WHERE server_id = ?1",
            params![server_id],
        )?;
        Ok(())
    }

    // --- plugin/mod install tracking ---------------------------------------

    pub fn record_plugin_install(&self, record: &PluginInstallRecord) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO plugin_installs
                (server_id, file_name, source, project_id, version_id, version_number,
                 mc_version, loader_facet, installed_at_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(server_id, file_name) DO UPDATE SET
                source = excluded.source,
                project_id = excluded.project_id,
                version_id = excluded.version_id,
                version_number = excluded.version_number,
                mc_version = excluded.mc_version,
                loader_facet = excluded.loader_facet,
                installed_at_unix = excluded.installed_at_unix",
            params![
                record.server_id,
                record.file_name,
                record.source,
                record.project_id,
                record.version_id,
                record.version_number,
                record.mc_version,
                record.loader_facet,
                record.installed_at_unix as i64,
            ],
        )?;
        Ok(())
    }

    pub fn list_plugin_installs(&self, server_id: &str) -> AppResult<Vec<PluginInstallRecord>> {
        let mut statement = self.conn.prepare(
            "SELECT server_id, file_name, source, project_id, version_id, version_number,
                    mc_version, loader_facet, installed_at_unix
             FROM plugin_installs WHERE server_id = ?1",
        )?;
        let rows = statement.query_map(params![server_id], |row| {
            Ok(PluginInstallRecord {
                server_id: row.get(0)?,
                file_name: row.get(1)?,
                source: row.get(2)?,
                project_id: row.get(3)?,
                version_id: row.get(4)?,
                version_number: row.get(5)?,
                mc_version: row.get(6)?,
                loader_facet: row.get(7)?,
                installed_at_unix: row.get::<_, i64>(8)? as u64,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn remove_plugin_install(&self, server_id: &str, file_name: &str) -> AppResult<()> {
        self.conn.execute(
            "DELETE FROM plugin_installs WHERE server_id = ?1 AND file_name = ?2",
            params![server_id, file_name],
        )?;
        Ok(())
    }

    /// Follows an addon's install record to its new file name. Enabling and
    /// disabling renames the jar (`foo.jar` <-> `foo.jar.disabled`), and the
    /// record is keyed by file name — without this the provenance is orphaned
    /// and update checks quietly stop covering the addon.
    pub fn rename_plugin_install(
        &self,
        server_id: &str,
        old_file_name: &str,
        new_file_name: &str,
    ) -> AppResult<()> {
        self.conn.execute(
            "UPDATE plugin_installs SET file_name = ?3
             WHERE server_id = ?1 AND file_name = ?2",
            params![server_id, old_file_name, new_file_name],
        )?;
        Ok(())
    }

    pub fn clear_plugin_installs(&self, server_id: &str) -> AppResult<()> {
        self.conn.execute(
            "DELETE FROM plugin_installs WHERE server_id = ?1",
            params![server_id],
        )?;
        Ok(())
    }

    /// Moves the database file to `new_path` (a full file path, not just a
    /// directory), reopening the connection there. No-op if already there.
    pub fn relocate(&mut self, new_path: PathBuf) -> AppResult<()> {
        if new_path == self.path {
            return Ok(());
        }
        if let Some(parent) = new_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Swap in a throwaway connection so the real one can be closed and its
        // file moved out from under `self`.
        let placeholder = Connection::open_in_memory()?;
        let old_conn = std::mem::replace(&mut self.conn, placeholder);
        old_conn.close().map_err(|(_, error)| error)?;

        if let Err(move_error) = move_db_file(&self.path, &new_path) {
            // The move failed (disk full, permissions, a read-only target, …).
            // Reopen the database at its original location so `self` stays a
            // working handle instead of silently becoming the empty in-memory
            // placeholder, which would drop every subsequent read and write.
            self.conn = Connection::open(&self.path)?;
            return Err(move_error);
        }

        let new_conn = Connection::open(&new_path)?;
        Self::migrate(&new_conn)?;
        self.conn = new_conn;
        self.path = new_path;
        Ok(())
    }
}

/// Moves the database file to `destination`. A missing source is not an error
/// — a database that has never been written yet simply has nothing to move.
fn move_db_file(source: &Path, destination: &Path) -> AppResult<()> {
    if !source.exists() {
        return Ok(());
    }

    // `rename` fails with "cross-device link" when the destination is on a
    // different drive/volume (common for a user-chosen folder), so fall back
    // to copy + remove in that case.
    if std::fs::rename(source, destination).is_ok() {
        return Ok(());
    }

    std::fs::copy(source, destination)?;
    std::fs::remove_file(source)?;
    Ok(())
}

/// One installed plugin/mod's provenance, used to check for updates later.
#[derive(Debug, Clone)]
pub struct PluginInstallRecord {
    pub server_id: String,
    pub file_name: String,
    /// "modrinth" | "spigot" | "curseforge".
    pub source: String,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub version_number: Option<String>,
    pub mc_version: Option<String>,
    pub loader_facet: Option<String>,
    pub installed_at_unix: u64,
}

// --- storage location resolution -----------------------------------------

/// The default database directory: the OS's per-app data directory (e.g.
/// `~/Library/Application Support/<id>` on macOS, `%APPDATA%\<id>` on
/// Windows, `~/.local/share/<id>` on Linux).
pub fn default_db_dir(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(app.path().app_data_dir()?)
}

/// Reads the `.location` pointer file (which always lives in the *default*
/// dir) for a custom directory the database was relocated to.
fn read_location_pointer(default_dir: &Path) -> Option<PathBuf> {
    let contents = std::fs::read_to_string(default_dir.join(LOCATION_POINTER_FILE)).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

/// The directory the database currently lives in: the pointer's target if a
/// `.location` file exists, otherwise the default directory.
pub fn resolve_db_dir(app: &AppHandle) -> AppResult<PathBuf> {
    let default_dir = default_db_dir(app)?;
    Ok(read_location_pointer(&default_dir).unwrap_or(default_dir))
}

/// The full path to the database file, wherever it currently lives.
pub fn resolve_db_path(app: &AppHandle) -> AppResult<PathBuf> {
    Ok(resolve_db_dir(app)?.join(DB_FILE_NAME))
}
