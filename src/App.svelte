<script lang="ts">
  import { onMount } from "svelte";
  import Dashboard from "./lib/views/Dashboard.svelte";
  import ServerDetail from "./lib/views/ServerDetail.svelte";
  import AppSettings from "./lib/views/AppSettings.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import Confetti from "./lib/components/Confetti.svelte";
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

  type Route = { view: "home" } | { view: "server"; serverId: string } | { view: "settings" };

  /** Hide the Java pill after this long without progress (e.g. a failure). */
  const JAVA_PILL_STALE_MS = 10_000;
  const JAVA_PILL_DONE_LINGER_MS = 1_600;

  let route = $state<Route>({ view: "home" });
  let confettiBurst = $state(0);
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
          confettiBurst += 1;
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
          title={server.name}
        >
          <StatusBlob status={serversStore.statusOf(server.id)} />
          <span class="server-name">{server.name}</span>
        </button>
      {/each}
    </nav>

    <button
      class="settings-item"
      class:active={route.view === "settings"}
      onclick={() => (route = { view: "settings" })}
    >
      ⚙️ <span>Settings</span>
    </button>
  </aside>

  <main>
    {#if selectedServer}
      <ServerDetail server={selectedServer} onback={() => (route = { view: "home" })} />
    {:else if route.view === "settings"}
      <AppSettings />
    {:else}
      <Dashboard onopen={(serverId) => (route = { view: "server", serverId })} />
    {/if}
  </main>
</div>

{#if javaDownload}
  <div class="java-pill" transition:fade={{ duration: 120 }}>
    <span class="java-cup">☕</span>
    Downloading Java… {javaDownloadText}
  </div>
{/if}

<Toasts />
<Confetti trigger={confettiBurst} />

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

  main {
    flex: 1;
    min-width: 0;
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
    border-radius: 999px;
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
