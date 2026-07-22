// A nudge for views that show how a server is reached.
//
// The listen port lives in the server's own config file (server.properties,
// velocity.toml, …) rather than in ServerConfig, so changing it alters none
// of the state those views already watch — the dashboard went on showing the
// old port until something else happened to remount it. This gives them one
// explicit thing to depend on.

class ServerAddressStore {
  private revisions = $state<Record<string, number>>({});

  /** Read this inside an effect to re-run it whenever the address changes. */
  revisionOf(serverId: string): number {
    const revision = this.revisions[serverId] ?? 0;
    return revision;
  }

  /** Call after changing anything that affects how a server is reached. */
  markChanged(serverId: string): void {
    this.revisions[serverId] = this.revisionOf(serverId) + 1;
  }
}

export const serverAddressStore = new ServerAddressStore();
