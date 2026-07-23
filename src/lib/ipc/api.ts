// Typed wrappers around every Tauri command — the single source of truth for
// command names and payload shapes on the frontend.

import { invoke } from "@tauri-apps/api/core";

export type Loader =
  | "vanilla"
  | "bds"
  | "paper"
  | "purpur"
  | "spigot"
  | "folia"
  | "fabric"
  | "neoforge"
  | "forge"
  | "quilt"
  | "arclight"
  | "mohist"
  | "velocity"
  | "bungeecord";

export const PROXY_LOADERS: Loader[] = ["velocity", "bungeecord"];

/** Loaders that load `.jar` plugins (Bukkit family, hybrids, proxies). Mirrors
 *  `Loader::plugin_facet` on the backend. */
export const PLUGIN_LOADERS: Loader[] = [
  "paper",
  "purpur",
  "spigot",
  "folia",
  "mohist",
  "arclight",
  "velocity",
  "bungeecord",
];

export function supportsPlugins(loader: Loader): boolean {
  return PLUGIN_LOADERS.includes(loader);
}

/** Loaders that load `.jar` mods (Forge family and mod-capable hybrids).
 *  Mirrors `Loader::mod_facet` on the backend. */
export const MOD_LOADERS: Loader[] = ["forge", "neoforge", "fabric", "quilt", "mohist", "arclight"];

export function supportsMods(loader: Loader): boolean {
  return MOD_LOADERS.includes(loader);
}

export type ServerStatus = "stopped" | "starting" | "running" | "stopping" | "crashed";

export interface ServerConfig {
  id: string;
  name: string;
  mcVersion: string;
  loader: Loader;
  memoryMb: number;
  javaPath: string | null;
  dir: string;
  backupsDir: string | null;
  javaArgs: string | null;
  startCommand: string | null;
  backupRetention: number | null;
  /** Restart automatically after a crash, at most this many times in a row;
   *  null disables it. */
  crashRestartLimit: number | null;
  createdAtUnix: number;
}

/** Where a server's backups land: the override, or `backups` in its dir. */
export function resolveBackupsDir(server: ServerConfig): string {
  if (server.backupsDir !== null) {
    return server.backupsDir;
  }
  const separator = server.dir.includes("/") ? "/" : "\\";
  return `${server.dir}${separator}backups`;
}

/** The absolute path of a file inside a server's folder, from the
 *  forward-slashed relative path the file browser works in. */
export function resolveServerFilePath(server: ServerConfig, relPath: string): string {
  const separator = server.dir.includes("/") ? "/" : "\\";
  const nativeRelPath = relPath.split("/").join(separator);
  return `${server.dir}${separator}${nativeRelPath}`;
}

export interface McVersion {
  id: string;
  type: string;
  releaseTime: string;
}

export interface JavaInstall {
  path: string;
  majorVersion: number;
}

export interface CreateServerRequest {
  name: string;
  mcVersion: string;
  loader: Loader;
  memoryMb: number;
  port: number;
  acceptEula: boolean;
  /** Parent folder for the server; null uses the configured default. */
  locationParent: string | null;
  javaArgs: string | null;
  startCommand: string | null;
}

export interface AppSettings {
  serversBaseDir: string;
}

export interface StorageLocation {
  dir: string;
  isDefault: boolean;
}

export interface ImportServerRequest {
  dir: string;
  name: string;
  loader: Loader;
  mcVersion: string;
  memoryMb: number;
}

export interface Property {
  key: string;
  value: string;
}

/** A running server already listening on the port another one wants. */
export interface PortConflict {
  serverId: string;
  serverName: string;
  port: string;
}

export interface BackupInfo {
  fileName: string;
  sizeBytes: number;
  createdAtUnix: number;
}

/** Asks the backend to stop a running server after a delay, back it up, and
 *  optionally start it again. */
export interface TimedBackupRequest {
  /** Said in-game when the countdown starts. Blank sends nothing. */
  message: string;
  delaySeconds: number;
  restartWhenDone: boolean;
}

/** A stop-and-backup countdown currently running for a server. */
export interface PendingTimedBackup {
  serverId: string;
  /** The UI ticks its own clock from this rather than polling the backend. */
  stopsAtUnix: number;
  restartWhenDone: boolean;
}

export type TaskAction =
  | { type: "command"; command: string }
  | { type: "restart" }
  | { type: "backup" }
  | { type: "start" }
  | { type: "stop" };

export interface ScheduledTask {
  id: string;
  serverId: string;
  name: string;
  cron: string;
  action: TaskAction;
  enabled: boolean;
}

export interface RosterEntry {
  name: string;
  online: boolean;
  banned: boolean;
  firstJoinedUnix: number;
  lastSeenUnix: number;
  joinCount: number;
  kickCount: number;
  totalPlaySeconds: number;
}

export interface ChatEntry {
  atUnix: number;
  message: string;
}

export interface PlayerDetail {
  name: string;
  online: boolean;
  banned: boolean;
  banReason: string | null;
  firstJoinedUnix: number;
  lastSeenUnix: number;
  joinCount: number;
  kickCount: number;
  chatCount: number;
  totalPlaySeconds: number;
  lastGameMode: string | null;
  recentChat: ChatEntry[];
}

export interface ServerAddress {
  lanIp: string;
  port: string;
}

export interface ForwardResult {
  /** Whether a mapping was successfully added on the router. */
  success: boolean;
  /** The address to share with friends, when there is one. */
  publicAddress: string | null;
  /** Forwarded, but the ISP's CGNAT means friends likely still can't connect. */
  cgnat: boolean;
  /** A human-friendly explanation to surface in the UI. */
  message: string;
}

export interface InstalledPlugin {
  fileName: string;
  displayName: string;
  enabled: boolean;
  sizeBytes: number;
}

/** Mods share the same on-disk shape as plugins (a `.jar` in a folder). */
export type InstalledMod = InstalledPlugin;

/** Marketplace an addon (plugin or mod) is browsed from or installed via. */
export type AddonSource = "modrinth" | "spigot" | "curseforge";

export interface AddonSearchResult {
  source: AddonSource;
  projectId: string;
  slug: string;
  title: string;
  description: string;
  downloads: number;
  iconUrl: string | null;
  author: string;
}

export interface AddonUpdateStatus {
  fileName: string;
  displayName: string;
  source: AddonSource;
  projectId: string;
  currentVersion: string | null;
  latestVersion: string | null;
  hasUpdate: boolean;
}

export interface DirEntry {
  name: string;
  relPath: string;
  isDir: boolean;
  sizeBytes: number;
}

export interface UpdateServerRequest {
  name: string;
  memoryMb: number;
  /** null means auto-resolve (or download) a suitable Java. */
  javaPath: string | null;
  /** null resets to the default `backups` folder in the server dir. */
  backupsDir: string | null;
  javaArgs: string | null;
  startCommand: string | null;
  /** Keep only this many newest backups; null keeps everything. */
  backupRetention: number | null;
  /** Restart after a crash up to this many times in a row; null disables it. */
  crashRestartLimit: number | null;
}

export const api = {
  listServers: () => invoke<ServerConfig[]>("list_servers"),
  listLoaderVersions: (loader: Loader) =>
    invoke<McVersion[]>("list_loader_versions", { loader }),
  createServer: (request: CreateServerRequest) =>
    invoke<ServerConfig>("create_server", { request }),
  importServer: (request: ImportServerRequest) =>
    invoke<ServerConfig>("import_server", { request }),
  deleteServer: (serverId: string) => invoke<void>("delete_server", { serverId }),
  startServer: (serverId: string) => invoke<void>("start_server", { serverId }),
  portConflict: (serverId: string) =>
    invoke<PortConflict | null>("port_conflict", { serverId }),
  stopOtherAndStart: (runningServerId: string, serverId: string) =>
    invoke<void>("stop_other_and_start", { runningServerId, serverId }),
  stopServer: (serverId: string) => invoke<void>("stop_server", { serverId }),
  killServer: (serverId: string) => invoke<void>("kill_server", { serverId }),
  sendServerCommand: (serverId: string, command: string) =>
    invoke<void>("send_server_command", { serverId, command }),
  serverStatuses: () => invoke<Record<string, ServerStatus>>("server_statuses"),
  detectJava: () => invoke<JavaInstall[]>("detect_java"),
  killAllJava: () => invoke<number>("kill_all_java"),
  openLogsDir: () => invoke<void>("open_logs_dir"),
  getSettings: () => invoke<AppSettings>("get_settings"),
  setServersBaseDir: (path: string) =>
    invoke<AppSettings>("set_servers_base_dir", { path }),
  previewServerDir: (name: string, locationParent: string | null) =>
    invoke<string>("preview_server_dir", { name, locationParent }),
  getStorageLocation: () => invoke<StorageLocation>("get_storage_location"),
  setStorageLocation: (dir: string) =>
    invoke<StorageLocation>("set_storage_location", { dir }),
  resetStorageLocation: () => invoke<StorageLocation>("reset_storage_location"),
  restartServer: (serverId: string) => invoke<void>("restart_server", { serverId }),
  serverPlayers: () => invoke<Record<string, string[]>>("server_players"),
  getPlayerRoster: (serverId: string) =>
    invoke<RosterEntry[]>("get_player_roster", { serverId }),
  getPlayerDetail: (serverId: string, playerName: string) =>
    invoke<PlayerDetail | null>("get_player_detail", { serverId, playerName }),
  getServerAddress: (serverId: string) =>
    invoke<ServerAddress>("get_server_address", { serverId }),
  openPortForward: (serverId: string) =>
    invoke<ForwardResult>("open_port_forward", { serverId }),
  closePortForward: (serverId: string) => invoke<void>("close_port_forward", { serverId }),
  /** Null when the port isn't currently forwarded on the router. */
  portForwardStatus: (serverId: string) =>
    invoke<ForwardResult | null>("port_forward_status", { serverId }),
  updateServer: (serverId: string, request: UpdateServerRequest) =>
    invoke<ServerConfig>("update_server", { serverId, request }),
  setServerIcon: (serverId: string, sourcePath: string) =>
    invoke<void>("set_server_icon", { serverId, sourcePath }),
  getServerIcon: (serverId: string) => invoke<string | null>("get_server_icon", { serverId }),
  removeServerIcon: (serverId: string) => invoke<void>("remove_server_icon", { serverId }),
  listServerFiles: (serverId: string, relPath: string) =>
    invoke<DirEntry[]>("list_server_files", { serverId, relPath }),
  readServerFile: (serverId: string, relPath: string) =>
    invoke<string>("read_server_file", { serverId, relPath }),
  writeServerFile: (serverId: string, relPath: string, contents: string) =>
    invoke<void>("write_server_file", { serverId, relPath, contents }),
  deleteServerFile: (serverId: string, relPath: string) =>
    invoke<void>("delete_server_file", { serverId, relPath }),
  /** Creates an empty file in `relDir`; resolves with its relative path. */
  createServerFile: (serverId: string, relDir: string, name: string) =>
    invoke<string>("create_server_file", { serverId, relDir, name }),
  /** Creates a folder in `relDir`; resolves with its relative path. */
  createServerDir: (serverId: string, relDir: string, name: string) =>
    invoke<string>("create_server_dir", { serverId, relDir, name }),
  /** Renames a file or folder in place; resolves with its new relative path. */
  renameServerFile: (serverId: string, relPath: string, newName: string) =>
    invoke<string>("rename_server_file", { serverId, relPath, newName }),
  /** Copies a file from disk into `relDir`; resolves with the name it landed under. */
  importServerFile: (serverId: string, relDir: string, sourcePath: string) =>
    invoke<string>("import_server_file", { serverId, relDir, sourcePath }),
  listPlugins: (serverId: string) =>
    invoke<InstalledPlugin[]>("list_plugins", { serverId }),
  setPluginEnabled: (serverId: string, fileName: string, enabled: boolean) =>
    invoke<string>("set_plugin_enabled", { serverId, fileName, enabled }),
  /** The command this server launches with by default, ignoring any custom
   *  start command override. */
  previewStartCommand: (serverId: string) =>
    invoke<string>("preview_start_command", { serverId }),
  /** Installs a `.jar` from disk into the server's `plugins/` folder. */
  importPluginJar: (serverId: string, sourcePath: string) =>
    invoke<InstalledPlugin>("import_plugin_jar", { serverId, sourcePath }),
  deletePlugin: (serverId: string, fileName: string) =>
    invoke<void>("delete_plugin", { serverId, fileName }),
  searchPlugins: (serverId: string, source: AddonSource, query: string) =>
    invoke<AddonSearchResult[]>("search_plugins", { serverId, source, query }),
  installPlugin: (serverId: string, source: AddonSource, projectId: string) =>
    invoke<InstalledPlugin>("install_plugin", { serverId, source, projectId }),
  checkPluginUpdates: (serverId: string) =>
    invoke<AddonUpdateStatus[]>("check_plugin_updates", { serverId }),
  updatePlugin: (serverId: string, fileName: string) =>
    invoke<InstalledPlugin>("update_plugin", { serverId, fileName }),
  listMods: (serverId: string) => invoke<InstalledMod[]>("list_mods", { serverId }),
  setModEnabled: (serverId: string, fileName: string, enabled: boolean) =>
    invoke<string>("set_mod_enabled", { serverId, fileName, enabled }),
  /** Installs a `.jar` from disk into the server's `mods/` folder. */
  importModJar: (serverId: string, sourcePath: string) =>
    invoke<InstalledMod>("import_mod_jar", { serverId, sourcePath }),
  deleteMod: (serverId: string, fileName: string) =>
    invoke<void>("delete_mod", { serverId, fileName }),
  searchMods: (serverId: string, source: AddonSource, query: string) =>
    invoke<AddonSearchResult[]>("search_mods", { serverId, source, query }),
  installMod: (serverId: string, source: AddonSource, projectId: string) =>
    invoke<InstalledMod>("install_mod", { serverId, source, projectId }),
  checkModUpdates: (serverId: string) =>
    invoke<AddonUpdateStatus[]>("check_mod_updates", { serverId }),
  updateMod: (serverId: string, fileName: string) =>
    invoke<InstalledMod>("update_mod", { serverId, fileName }),
  getCurseforgeApiKey: () => invoke<string | null>("get_curseforge_api_key"),
  setCurseforgeApiKey: (apiKey: string) =>
    invoke<void>("set_curseforge_api_key", { apiKey }),
  getServerProperties: (serverId: string) =>
    invoke<Property[]>("get_server_properties", { serverId }),
  saveServerProperties: (serverId: string, updates: Property[]) =>
    invoke<void>("save_server_properties", { serverId, updates }),
  createBackup: (serverId: string) => invoke<BackupInfo>("create_backup", { serverId }),
  scheduleTimedBackup: (serverId: string, request: TimedBackupRequest) =>
    invoke<PendingTimedBackup>("schedule_timed_backup", { serverId, request }),
  cancelTimedBackup: (serverId: string) => invoke<void>("cancel_timed_backup", { serverId }),
  timedBackupStatus: (serverId: string) =>
    invoke<PendingTimedBackup | null>("timed_backup_status", { serverId }),
  listBackups: (serverId: string) => invoke<BackupInfo[]>("list_backups", { serverId }),
  restoreBackup: (serverId: string, fileName: string) =>
    invoke<void>("restore_backup", { serverId, fileName }),
  deleteBackup: (serverId: string, fileName: string) =>
    invoke<void>("delete_backup", { serverId, fileName }),
  listTasks: () => invoke<ScheduledTask[]>("list_tasks"),
  upsertTask: (task: ScheduledTask) => invoke<ScheduledTask>("upsert_task", { task }),
  deleteTask: (taskId: string) => invoke<void>("delete_task", { taskId }),
  runTaskNow: (taskId: string) => invoke<void>("run_task_now", { taskId }),
  previewNextRun: (cron: string) => invoke<number | null>("preview_next_run", { cron }),
};
