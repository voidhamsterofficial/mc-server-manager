// Typed wrappers around the events the backend emits.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PendingTimedBackup, ServerStatus } from "./api";

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

export interface BackupProgressEvent {
  serverId: string;
  processedFiles: number;
  totalFiles: number;
}

/** Fires with the server id when a backup finishes. */
export function onBackupCreated(handler: (serverId: string) => void): Promise<UnlistenFn> {
  return listen<string>("server:backup-created", (event) => handler(event.payload));
}

/** Fires repeatedly while a backup is being zipped. */
export function onBackupProgress(
  handler: (event: BackupProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<BackupProgressEvent>("server:backup-progress", (event) =>
    handler(event.payload),
  );
}

/** Fires with the server id when a backup gives up, so the UI can stop
 *  showing it as in progress. */
export function onBackupFailed(handler: (serverId: string) => void): Promise<UnlistenFn> {
  return listen<string>("server:backup-failed", (event) => handler(event.payload));
}

/** A stop-and-backup countdown started, was cancelled, or reached zero.
 *  `pending` is null once nothing is scheduled for that server any more. */
export interface TimedBackupEvent {
  serverId: string;
  pending: PendingTimedBackup | null;
}

export function onTimedBackup(
  handler: (event: TimedBackupEvent) => void,
): Promise<UnlistenFn> {
  return listen<TimedBackupEvent>("server:backup-timed", (event) => handler(event.payload));
}
