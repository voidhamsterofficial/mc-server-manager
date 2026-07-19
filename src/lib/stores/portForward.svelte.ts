// Per-server UPnP port-forward state.
//
// Keyed by server id on purpose: the Dashboard tab is reused across servers
// (only its `server` prop changes), so component-local state would leak one
// server's forwarding result — and its public address — onto every other one.

import type { ForwardResult } from "../ipc/api";

class PortForwardStore {
  private results = $state<Record<string, ForwardResult>>({});
  private busyIds = $state<Record<string, boolean>>({});
  // Which servers we've already asked the router about this session.
  // Deliberately a plain Set, not $state: it's a cache, and making it reactive
  // would re-trigger the very effect that fills it.
  private checked = new Set<string>();

  /**
   * True the first time it's called for a server, false after — so the router
   * is queried once per server per session rather than on every tab visit.
   */
  claimStatusCheck(serverId: string): boolean {
    if (this.checked.has(serverId)) {
      return false;
    }
    this.checked.add(serverId);
    return true;
  }

  /** The last forward attempt for this server, or null if none this session. */
  resultOf(serverId: string): ForwardResult | null {
    return this.results[serverId] ?? null;
  }

  isBusy(serverId: string): boolean {
    return this.busyIds[serverId] ?? false;
  }

  setBusy(serverId: string, busy: boolean): void {
    this.busyIds[serverId] = busy;
  }

  record(serverId: string, result: ForwardResult): void {
    this.results[serverId] = result;
  }

  /** Forgets a server's forwarding state — after closing it, or on delete. */
  clear(serverId: string): void {
    delete this.results[serverId];
    delete this.busyIds[serverId];
    this.checked.delete(serverId);
  }
}

export const portForwardStore = new PortForwardStore();
