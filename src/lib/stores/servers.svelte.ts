// Reactive store for servers, their statuses, and console buffers.

import { api, type ServerConfig, type ServerStatus } from "../ipc/api";
import type { ConsoleLine } from "../ipc/events";

const MAX_CONSOLE_LINES = 5000;

class ServersStore {
  servers = $state<ServerConfig[]>([]);
  statuses = $state<Record<string, ServerStatus>>({});
  consoles = $state<Record<string, ConsoleLine[]>>({});
  players = $state<Record<string, string[]>>({});
  // Distinguishes "still loading" from "confirmed empty" so the dashboard
  // doesn't flash a first-run welcome screen before the initial fetch lands.
  loaded = $state(false);
  loadError = $state<string | null>(null);

  statusOf(serverId: string): ServerStatus {
    return this.statuses[serverId] ?? "stopped";
  }

  consoleOf(serverId: string): ConsoleLine[] {
    return this.consoles[serverId] ?? [];
  }

  playersOf(serverId: string): string[] {
    return this.players[serverId] ?? [];
  }

  async refresh(): Promise<void> {
    try {
      const [servers, statuses, players] = await Promise.all([
        api.listServers(),
        api.serverStatuses(),
        api.serverPlayers(),
      ]);
      this.servers = servers;
      this.statuses = statuses;
      this.players = players;
      this.loadError = null;
    } catch (error) {
      this.loadError = String(error);
      throw error;
    } finally {
      this.loaded = true;
    }
  }

  setStatus(serverId: string, status: ServerStatus): void {
    this.statuses = { ...this.statuses, [serverId]: status };
  }

  setPlayers(serverId: string, players: string[]): void {
    this.players = { ...this.players, [serverId]: players };
  }

  appendConsole(serverId: string, lines: ConsoleLine[]): void {
    const existing = this.consoles[serverId] ?? [];
    const merged = [...existing, ...lines];
    if (merged.length > MAX_CONSOLE_LINES) {
      merged.splice(0, merged.length - MAX_CONSOLE_LINES);
    }
    this.consoles = { ...this.consoles, [serverId]: merged };
  }
}

export const serversStore = new ServersStore();
