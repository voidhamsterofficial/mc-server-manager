// Typed wrappers around the events the backend emits.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ServerStatus } from "./api";

export type LogLevel = "info" | "warn" | "error";

export interface ConsoleSpan {
  text: string;
  /** CSS hex color; absent means the line-level default. */
  color?: string;
  bold: boolean;
}

export interface ConsoleLine {
  spans: ConsoleSpan[];
  level: LogLevel;
}

export interface ConsoleBatchEvent {
  serverId: string;
  lines: ConsoleLine[];
}

export interface StatusEvent {
  serverId: string;
  status: ServerStatus;
}

export interface InstallProgressEvent {
  serverId: string;
  step: string;
  downloadedBytes: number;
  totalBytes: number | null;
}

export function onConsoleBatch(
  handler: (event: ConsoleBatchEvent) => void,
): Promise<UnlistenFn> {
  return listen<ConsoleBatchEvent>("server:console", (event) => handler(event.payload));
}

export function onStatusChange(handler: (event: StatusEvent) => void): Promise<UnlistenFn> {
  return listen<StatusEvent>("server:status", (event) => handler(event.payload));
}

export function onInstallProgress(
  handler: (event: InstallProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<InstallProgressEvent>("install:progress", (event) => handler(event.payload));
}

export interface PlayersEvent {
  serverId: string;
  players: string[];
}

export interface StatsEvent {
  serverId: string;
  cpuPercent: number;
  memoryBytes: number;
  uptimeSeconds: number;
}

export function onPlayersChange(handler: (event: PlayersEvent) => void): Promise<UnlistenFn> {
  return listen<PlayersEvent>("server:players", (event) => handler(event.payload));
}

export function onStats(handler: (event: StatsEvent) => void): Promise<UnlistenFn> {
  return listen<StatsEvent>("server:stats", (event) => handler(event.payload));
}

/** Fires with the server id when a backup finishes. */
export function onBackupCreated(handler: (serverId: string) => void): Promise<UnlistenFn> {
  return listen<string>("server:backup-created", (event) => handler(event.payload));
}
