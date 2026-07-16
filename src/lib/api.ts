// Typed wrappers around every Tauri command — the single source of truth for
// command names and payload shapes on the frontend.

import { invoke } from "@tauri-apps/api/core";

export type Loader = "vanilla" | "paper" | "fabric" | "forge" | "neoforge";

export type ServerStatus = "stopped" | "starting" | "running" | "stopping" | "crashed";

export interface ServerConfig {
  id: string;
  name: string;
  mcVersion: string;
  loader: Loader;
  memoryMb: number;
  javaPath: string | null;
  createdAtUnix: number;
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
  acceptEula: boolean;
  /** Parent folder for the server; null uses the configured default. */
  locationParent: string | null;
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

export interface UpdateServerRequest {
  name: string;
  memoryMb: number;
  javaPath: string | null;
}

export const api = {
  listServers: () => invoke<ServerConfig[]>("list_servers"),
  listMinecraftVersions: () => invoke<McVersion[]>("list_minecraft_versions"),
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
  getSettings: () => invoke<AppSettings>("get_settings"),
  setServersBaseDir: (path: string) =>
    invoke<AppSettings>("set_servers_base_dir", { path }),
  previewServerDir: (name: string, locationParent: string | null) =>
    invoke<string>("preview_server_dir", { name, locationParent }),
  restartServer: (serverId: string) => invoke<void>("restart_server", { serverId }),
  serverPlayers: () => invoke<Record<string, string[]>>("server_players"),
  updateServer: (serverId: string, request: UpdateServerRequest) =>
    invoke<ServerConfig>("update_server", { serverId, request }),
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
