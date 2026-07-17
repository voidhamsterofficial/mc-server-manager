<script lang="ts">
  import { onMount } from "svelte";
  import Dashboard from "./lib/views/Dashboard.svelte";
  import ServerDetail from "./lib/views/ServerDetail.svelte";
  import AppSettings from "./lib/views/AppSettings.svelte";
  import Docs from "./lib/views/Docs.svelte";
  import Button from "./lib/components/Button.svelte";
  import ContextMenu from "./lib/components/ContextMenu.svelte";
  import CreateServerWizard from "./lib/views/CreateServerWizard.svelte";
  import { contextMenuStore, type MenuEntry } from "./lib/stores/contextMenu.svelte";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { api, type ServerConfig } from "./lib/api";
  import Toasts from "./lib/components/Toasts.svelte";
  import ReasonPrompt from "./lib/components/ReasonPrompt.svelte";
  import StatusBlob from "./lib/components/StatusBlob.svelte";
  import GrassBlock from "./lib/components/GrassBlock.svelte";
  import { serversStore } from "./lib/stores/servers.svelte";
  import { statsStore } from "./lib/stores/stats.svelte";
  import { toastsStore } from "./lib/stores/toasts.svelte";
  import { formatBytes } from "./lib/format";
  import {
    onConsoleBatch,
    onInstallProgress,
    onPlayersChange,
    onStats,
    onStatusChange,
    type InstallProgressEvent,
  } from "./lib/events";
  import { fade } from "svelte/transition";

  type Route =
    | { view: "home" }
    | { view: "server"; serverId: string }
    | { view: "settings" }
    | { view: "docs" };

  /** Hide the Java pill after this long without progress (e.g. a failure). */
  const JAVA_PILL_STALE_MS = 10_000;
  const JAVA_PILL_DONE_LINGER_MS = 1_600;

  let route = $state<Route>({ view: "home" });
  let wizardOpen = $state(false);
  let javaDownload = $state<InstallProgressEvent | null>(null);
  let javaPillTimer: ReturnType<typeof setTimeout> | undefined;

  const javaDownloadText = $derived.by(() => {
    if (javaDownload === null) {
      return "";
    }
    if (javaDownload.totalBytes === null) {
      return formatBytes(javaDownload.downloadedBytes);
    }
    const percent = Math.round((javaDownload.downloadedBytes / javaDownload.totalBytes) * 100);
    return `${percent}%`;
  });

  function trackJavaDownload(event: InstallProgressEvent) {
    if (event.step !== "download-java") {
      return;
    }
    javaDownload = event;

    const isDone = event.totalBytes !== null && event.downloadedBytes >= event.totalBytes;
    const hidePillDelay = isDone ? JAVA_PILL_DONE_LINGER_MS : JAVA_PILL_STALE_MS;
    clearTimeout(javaPillTimer);
    javaPillTimer = setTimeout(() => {
      javaDownload = null;
    }, hidePillDelay);
  }

  const selectedServer = $derived.by(() => {
    const currentRoute = route;
    if (currentRoute.view !== "server") {
      return null;
    }
    const found = serversStore.servers.find((server) => server.id === currentRoute.serverId);
    return found ?? null;
  });

  // --- Bulk actions -------------------------------------------------------
  let bulkBusy = $state(false);

  const runningCount = $derived(
    serversStore.servers.filter((server) => serversStore.statusOf(server.id) === "running")
      .length,
  );

  async function bulkRun(
    verb: string,
    targets: ServerConfig[],
    action: (server: ServerConfig) => Promise<unknown>,
  ) {
    if (targets.length === 0) {
      toastsStore.show(`Nothing to ${verb} — no matching servers`);
      return;
    }
    bulkBusy = true;
    let succeeded = 0;
    for (const server of targets) {
      try {
        await action(server);
        succeeded++;
      } catch (error) {
        toastsStore.error(`${server.name}: ${error}`);
      }
    }
    if (succeeded > 0) {
      toastsStore.success(
        `${verb} ${succeeded} server${succeeded === 1 ? "" : "s"} ✔`,
      );
    }
    bulkBusy = false;
  }

  const startAll = () =>
    bulkRun(
      "Started",
      serversStore.servers.filter((server) =>
        ["stopped", "crashed"].includes(serversStore.statusOf(server.id)),
      ),
      (server) => api.startServer(server.id),
    );

  const stopAll = () =>
    bulkRun(
      "Stopped",
      serversStore.servers.filter((server) =>
        ["running", "starting"].includes(serversStore.statusOf(server.id)),
      ),
      (server) => api.stopServer(server.id),
    );

  const backupAll = () =>
    bulkRun("Backed up", serversStore.servers, (server) => api.createBackup(server.id));

  // --- Context menus ------------------------------------------------------

  async function runWithToast(action: () => Promise<unknown>, successMessage?: string) {
    try {
      await action();
      if (successMessage) {
        toastsStore.success(successMessage);
      }
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  function serverMenuEntries(server: ServerConfig): MenuEntry[] {
    const status = serversStore.statusOf(server.id);
    const canStart = status === "stopped" || status === "crashed";

    const entries: MenuEntry[] = [
      {
        label: "Open",
        emoji: "🖥",
        action: () => (route = { view: "server", serverId: server.id }),
      },
      "separator",
    ];

    if (canStart) {
      entries.push({
        label: "Start",
        emoji: "▶",
        action: () => runWithToast(() => api.startServer(server.id)),
      });
    } else {
      entries.push(
        {
          label: "Restart",
          emoji: "🔄",
          action: () => runWithToast(() => api.restartServer(server.id)),
        },
        {
          label: "Stop",
          emoji: "⏹",
          action: () => runWithToast(() => api.stopServer(server.id)),
        },
        {
          label: "Kill",
          emoji: "☠",
          danger: true,
          action: () => runWithToast(() => api.killServer(server.id)),
        },
      );
    }

    entries.push(
      {
        label: "Back up now",
        emoji: "🎁",
        action: () =>
          runWithToast(() => api.createBackup(server.id), `Backed up "${server.name}" 🎁`),
      },
      {
        label: "Open folder",
        emoji: "📂",
        disabled: server.dir === "",
        action: () => runWithToast(() => openPath(server.dir)),
      },
    );
    return entries;
  }

  function appMenuEntries(): MenuEntry[] {
    return [
      { label: "New server", emoji: "➕", action: () => (wizardOpen = true) },
      {
        label: "Refresh",
        emoji: "🔄",
        action: () => runWithToast(() => serversStore.refresh()),
      },
      "separator",
      { label: "Start all", emoji: "▶", action: startAll },
      { label: "Stop all", emoji: "⏹", action: stopAll },
      { label: "Back up all", emoji: "🎁", action: backupAll },
      "separator",
      { label: "Docs", emoji: "📖", action: () => (route = { view: "docs" }) },
      { label: "Settings", emoji: "⚙", action: () => (route = { view: "settings" }) },
    ];
  }

  function showServerMenu(event: MouseEvent, server: ServerConfig) {
    contextMenuStore.show(event, serverMenuEntries(server));
  }

  // Everywhere else: the app menu — except text fields, which keep the
  // native copy/paste menu.
  function handleGlobalContextMenu(event: MouseEvent) {
    const target = event.target as HTMLElement | null;
    if (target?.closest("input, textarea, select, [contenteditable]")) {
      return;
    }
    contextMenuStore.show(event, appMenuEntries());
  }

  onMount(() => {
    serversStore.refresh().catch((error) => toastsStore.error(String(error)));

    const unlistenPromises = [
      onConsoleBatch((batch) => serversStore.appendConsole(batch.serverId, batch.lines)),
      onPlayersChange((event) => serversStore.setPlayers(event.serverId, event.players)),
      onStats((event) => statsStore.record(event)),
      onInstallProgress(trackJavaDownload),
      onStatusChange((event) => {
        const previousStatus = serversStore.statusOf(event.serverId);
        serversStore.setStatus(event.serverId, event.status);

        if (previousStatus === "starting" && event.status === "running") {
          toastsStore.success("Server is up — happy crafting! 🎉");
        }
        if (event.status === "crashed") {
          toastsStore.error("Server crashed 💔 Check the console for details.");
        }
        if (event.status === "stopped" || event.status === "crashed") {
          statsStore.clear(event.serverId);
        }
      }),
    ];

    return () => {
      for (const promise of unlistenPromises) {
        promise.then((unlisten) => unlisten());
      }
    };
  });
</script>

<div class="shell">
  <aside class="sidebar">
    <button
      class="brand"
      class:active={route.view === "home"}
      onclick={() => (route = { view: "home" })}
      title="All servers"
    >
      <span class="mark"><GrassBlock size={22} /></span>
      <span class="name">Blockparty</span>
    </button>

    <nav class="server-nav">
      {#each serversStore.servers as server (server.id)}
        <button
          class="server-item"
          class:active={route.view === "server" && route.serverId === server.id}
          onclick={() => (route = { view: "server", serverId: server.id })}
          oncontextmenu={(event) => showServerMenu(event, server)}
          title={server.name}
        >
          <StatusBlob status={serversStore.statusOf(server.id)} />
          <span class="server-name">{server.name}</span>
        </button>
      {/each}
    </nav>

    <button
      class="settings-item"
      class:active={route.view === "docs"}
      onclick={() => (route = { view: "docs" })}
    >
      📖 <span>Docs</span>
    </button>
    <button
      class="settings-item"
      class:active={route.view === "settings"}
      onclick={() => (route = { view: "settings" })}
    >
      ⚙️ <span>Settings</span>
    </button>
  </aside>

  <div class="content">
    <header class="bulkbar">
      <Button onclick={() => (wizardOpen = true)}>＋ New server</Button>
      <span class="bulk-divider"></span>
      <Button variant="soft" disabled={bulkBusy} onclick={startAll}>▶ Start all</Button>
      <Button variant="danger" disabled={bulkBusy} onclick={stopAll}>⏹ Stop all</Button>
      <Button variant="ghost" disabled={bulkBusy} onclick={backupAll}>🎁 Backup all</Button>
      <span class="bulk-status">
        {runningCount}/{serversStore.servers.length} running
      </span>
    </header>

    <main>
      {#if selectedServer}
        <ServerDetail server={selectedServer} onback={() => (route = { view: "home" })} />
      {:else if route.view === "settings"}
        <AppSettings />
      {:else if route.view === "docs"}
        <Docs onopenview={(view) => (route = { view })} />
      {:else}
        <Dashboard
          onopen={(serverId) => (route = { view: "server", serverId })}
          onnew={() => (wizardOpen = true)}
          onservermenu={showServerMenu}
        />
      {/if}
    </main>
  </div>
</div>

{#if javaDownload}
  <div class="java-pill" transition:fade={{ duration: 120 }}>
    <span class="java-cup">☕</span>
    Downloading Java… {javaDownloadText}
  </div>
{/if}

<svelte:window oncontextmenu={handleGlobalContextMenu} />

<CreateServerWizard open={wizardOpen} onclose={() => (wizardOpen = false)} />
<ContextMenu />
<ReasonPrompt />
<Toasts />

<style>
  .shell {
    height: 100vh;
    display: flex;
  }

  .sidebar {
    width: 216px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1rem 0.75rem;
    border-right: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface) 55%, transparent);
    backdrop-filter: blur(10px);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    border: none;
    background: transparent;
    cursor: pointer;
    font-family: inherit;
    padding: 0.5rem 0.6rem;
    border-radius: var(--radius-md);
    transition: background-color 0.15s ease;
  }

  .brand:hover,
  .brand.active {
    background: var(--accent-soft);
  }

  .mark {
    display: inline-flex;
  }

  .name {
    font-family: var(--font-pixel);
    font-size: 1.05rem;
    font-weight: 700;
    background: linear-gradient(90deg, var(--accent), var(--peach));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }

  .server-nav {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .server-item,
  .settings-item {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    border: none;
    background: transparent;
    color: var(--text);
    font-family: inherit;
    font-size: 0.92rem;
    font-weight: 600;
    text-align: left;
    padding: 0.55rem 0.7rem;
    border-radius: var(--radius-md);
    cursor: pointer;
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .server-item:hover,
  .settings-item:hover {
    background: var(--surface-2);
  }

  .server-item.active,
  .settings-item.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }

  .server-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .bulkbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 1.5rem;
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface) 55%, transparent);
    flex-shrink: 0;
  }

  .bulk-divider {
    width: 1px;
    align-self: stretch;
    background: var(--border);
    margin: 0.15rem 0.35rem;
  }

  .bulk-status {
    margin-left: auto;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  main {
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
  }

  .java-pill {
    position: fixed;
    bottom: 1.25rem;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 0.5em;
    font-size: 0.9rem;
    font-weight: 700;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-pop);
    padding: 0.6em 1.2em;
    z-index: 90;
    font-variant-numeric: tabular-nums;
  }

  .java-cup {
    animation: steam 1.2s ease-in-out infinite;
  }

  @keyframes steam {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-3px) rotate(-6deg);
    }
  }
</style>
