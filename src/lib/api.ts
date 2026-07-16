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

export interface Property {
  key: string;
  value: string;
}

export interface BackupInfo {
  fileName: string;
  sizeBytes: number;
  createdAtUnix: number;
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
}

export const api = {
  listServers: () => invoke<ServerConfig[]>("list_servers"),
  listLoaderVersions: (loader: Loader) =>
    invoke<McVersion[]>("list_loader_versions", { loader }),
  createServer: (request: CreateServerRequest) =>
    invoke<ServerConfig>("create_server", { request }),
  deleteServer: (serverId: string) => invoke<void>("delete_server", { serverId }),
  startServer: (serverId: string) => invoke<void>("start_server", { serverId }),
  stopServer: (serverId: string) => invoke<void>("stop_server", { serverId }),
  killServer: (serverId: string) => invoke<void>("kill_server", { serverId }),
  sendServerCommand: (serverId: string, command: string) =>
    invoke<void>("send_server_command", { serverId, command }),
  serverStatuses: () => invoke<Record<string, ServerStatus>>("server_statuses"),
  detectJava: () => invoke<JavaInstall[]>("detect_java"),
  killAllJava: () => invoke<number>("kill_all_java"),
  getSettings: () => invoke<AppSettings>("get_settings"),
  setServersBaseDir: (path: string) =>
    invoke<AppSettings>("set_servers_base_dir", { path }),
  previewServerDir: (name: string, locationParent: string | null) =>
    invoke<string>("preview_server_dir", { name, locationParent }),
  restartServer: (serverId: string) => invoke<void>("restart_server", { serverId }),
  serverPlayers: () => invoke<Record<string, string[]>>("server_players"),
  getPlayerRoster: (serverId: string) =>
    invoke<RosterEntry[]>("get_player_roster", { serverId }),
  updateServer: (serverId: string, request: UpdateServerRequest) =>
    invoke<ServerConfig>("update_server", { serverId, request }),
  setServerIcon: (serverId: string, sourcePath: string) =>
    invoke<void>("set_server_icon", { serverId, sourcePath }),
  getServerIcon: (serverId: string) => invoke<string | null>("get_server_icon", { serverId }),
  removeServerIcon: (serverId: string) => invoke<void>("remove_server_icon", { serverId }),
  getServerProperties: (serverId: string) =>
    invoke<Property[]>("get_server_properties", { serverId }),
  saveServerProperties: (serverId: string, updates: Property[]) =>
    invoke<void>("save_server_properties", { serverId, updates }),
  createBackup: (serverId: string) => invoke<BackupInfo>("create_backup", { serverId }),
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
