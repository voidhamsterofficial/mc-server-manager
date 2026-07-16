// Rolling resource-usage history per server, fed by `server:stats` events.

import type { StatsEvent } from "../events";

/** ~3 minutes of history at the 2s sample interval. */
const MAX_SAMPLES = 90;

export interface ServerStats {
  cpuHistory: number[];
  memoryHistory: number[];
  latest: StatsEvent | null;
}

const EMPTY_STATS: ServerStats = { cpuHistory: [], memoryHistory: [], latest: null };

class StatsStore {
  byServer = $state<Record<string, ServerStats>>({});

  of(serverId: string): ServerStats {
    return this.byServer[serverId] ?? EMPTY_STATS;
  }

  record(event: StatsEvent): void {
    const current = this.byServer[event.serverId] ?? EMPTY_STATS;
    const next: ServerStats = {
      cpuHistory: pushCapped(current.cpuHistory, event.cpuPercent),
      memoryHistory: pushCapped(current.memoryHistory, event.memoryBytes),
      latest: event,
    };
    this.byServer = { ...this.byServer, [event.serverId]: next };
  }

  clear(serverId: string): void {
    const { [serverId]: _removed, ...rest } = this.byServer;
    this.byServer = rest;
  }
}

function pushCapped(history: number[], value: number): number[] {
  const appended = [...history, value];
  if (appended.length > MAX_SAMPLES) {
    appended.splice(0, appended.length - MAX_SAMPLES);
  }
  return appended;
}

export const statsStore = new StatsStore();
