<script lang="ts">
  import { fade } from "svelte/transition";
  import { api, supportsPlugins, type ServerConfig } from "../api";
  import { serversStore } from "../stores/servers.svelte";
  import { toastsStore } from "../stores/toasts.svelte";
  import StatusBlob from "../components/StatusBlob.svelte";
  import Button from "../components/Button.svelte";
  import Chip from "../components/Chip.svelte";
  import DashboardTab from "./tabs/DashboardTab.svelte";
  import ConsoleTab from "./tabs/ConsoleTab.svelte";
  import PlayersTab from "./tabs/PlayersTab.svelte";
  import PluginsTab from "./tabs/PluginsTab.svelte";
  import SettingsTab from "./tabs/SettingsTab.svelte";
  import BackupsTab from "./tabs/BackupsTab.svelte";
  import SchedulerTab from "./tabs/SchedulerTab.svelte";
  import FilesTab from "./tabs/FilesTab.svelte";

  interface Props {
    server: ServerConfig;
    onback: () => void;
  }

  let { server, onback }: Props = $props();

  const ALL_TABS = [
    { id: "dashboard", label: "Dashboard", emoji: "🏡" },
    { id: "console", label: "Console", emoji: "📜" },
    { id: "players", label: "Players", emoji: "🧑‍🤝‍🧑" },
    { id: "plugins", label: "Plugins", emoji: "🧩" },
    { id: "files", label: "Files", emoji: "📁" },
    { id: "settings", label: "Settings", emoji: "🛠️" },
    { id: "backups", label: "Backups", emoji: "🎁" },
    { id: "scheduler", label: "Scheduler", emoji: "⏰" },
  ] as const;

  type TabId = (typeof ALL_TABS)[number]["id"];

  // The Plugins tab only appears for plugin-capable software.
  const tabs = $derived(
    ALL_TABS.filter((tab) => tab.id !== "plugins" || supportsPlugins(server.loader)),
  );

  let activeTab = $state<TabId>("dashboard");
  let busy = $state(false);
  let confirmingDelete = $state(false);

  $effect(() => {
    // If the visible tabs change (switching servers) and the active one is
    // gone, fall back to the dashboard.
    if (!tabs.some((tab) => tab.id === activeTab)) {
      activeTab = "dashboard";
    }
  });

  const status = $derived(serversStore.statusOf(server.id));
  const canStart = $derived(status === "stopped" || status === "crashed");
  const canStop = $derived(status === "running" || status === "starting");

  async function run(action: () => Promise<unknown>) {
    busy = true;
    try {
      await action();
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      busy = false;
    }
  }

  async function deleteServer() {
    const deletedName = server.name;
    await run(async () => {
      await api.deleteServer(server.id);
      // Leave this view before the refresh drops the server from the list,
      // or the derived `server` becomes null while we still render it.
      onback();
      await serversStore.refresh();
      toastsStore.show(`"${deletedName}" was deleted 🗑️`);
    });
  }
</script>

<section class="detail" in:fade={{ duration: 120 }}>
  <header>
    <div class="title">
      <h1>{server.name}</h1>
      <div class="meta">
        <StatusBlob {status} showLabel />
        <Chip>🧱 {server.mcVersion}</Chip>
        <Chip>{server.loader}</Chip>
        <Chip>💾 {server.memoryMb} MB</Chip>
      </div>
    </div>
    <div class="actions">
      {#if canStart}
        <Button disabled={busy} onclick={() => run(() => api.startServer(server.id))}>
          ▶ Start
        </Button>
      {/if}
      {#if canStop}
        <Button
          variant="soft"
          disabled={busy}
          onclick={() => run(() => api.restartServer(server.id))}
        >
          🔄 Restart
        </Button>
        <Button variant="danger" disabled={busy} onclick={() => run(() => api.stopServer(server.id))}>
          ⏹ Stop
        </Button>
        <Button
          variant="ghost"
          disabled={busy}
          title="Force-kill the process"
          onclick={() => run(() => api.killServer(server.id))}
        >
          ☠ Kill
        </Button>
      {/if}
      {#if canStart}
        {#if confirmingDelete}
          <Button variant="danger" disabled={busy} onclick={deleteServer}>Really delete?</Button>
          <Button variant="ghost" onclick={() => (confirmingDelete = false)}>Keep it</Button>
        {:else}
          <Button variant="ghost" onclick={() => (confirmingDelete = true)}>🗑 Delete</Button>
        {/if}
      {/if}
    </div>
  </header>

  <nav class="tabs">
    {#each tabs as tab (tab.id)}
      <button
        class="tab"
        class:active={activeTab === tab.id}
        onclick={() => (activeTab = tab.id)}
      >
        <span class="tab-emoji">{tab.emoji}</span>
        {tab.label}
      </button>
    {/each}
  </nav>

  <div class="tab-content">
    {#if activeTab === "dashboard"}
      <DashboardTab {server} />
    {:else if activeTab === "console"}
      <ConsoleTab {server} />
    {:else if activeTab === "players"}
      <PlayersTab {server} />
    {:else if activeTab === "plugins"}
      <PluginsTab {server} />
    {:else if activeTab === "files"}
      <FilesTab {server} />
    {:else if activeTab === "settings"}
      <SettingsTab {server} />
    {:else if activeTab === "backups"}
      <BackupsTab {server} />
    {:else if activeTab === "scheduler"}
      <SchedulerTab {server} />
    {/if}
  </div>
</section>

<style>
  .detail {
    max-width: 1100px;
    margin: 0 auto;
    padding: 1.25rem 1.75rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    height: 100%;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .title {
    min-width: 0;
  }

  h1 {
    margin: 0 0 0.4rem;
    font-size: 1.35rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .tabs {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .tab {
    display: inline-flex;
    align-items: center;
    gap: 0.45em;
    line-height: 1;
    border: none;
    background: transparent;
    color: var(--muted);
    font-family: inherit;
    font-size: 0.9rem;
    font-weight: 700;
    padding: 0.65em 1.1em;
    border-radius: var(--radius-md);
    cursor: pointer;
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out);
  }

  .tab:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .tab.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }

  .tab-emoji {
    font-size: 1rem;
    line-height: 1;
  }

  .tab-content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
</style>
