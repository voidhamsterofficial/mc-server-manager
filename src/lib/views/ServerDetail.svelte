<script lang="ts">
  import { fade } from "svelte/transition";
  import {
    House,
    Terminal,
    Users,
    Puzzle,
    Folder,
    Settings,
    Archive,
    Clock,
    Play,
    RefreshCw,
    Square,
    Skull,
    Trash2,
    Blocks,
    Save,
  } from "@lucide/svelte";
  import { api, supportsMods, supportsPlugins, type ServerConfig } from "../ipc/api";
  import { FEATURE_COLOR } from "../util/features";
  import { serversStore } from "../stores/servers.svelte";
  import { toastsStore } from "../stores/toasts.svelte";
  import StatusBlob from "../components/StatusBlob.svelte";
  import Button from "../components/Button.svelte";
  import Chip from "../components/Chip.svelte";
  import DashboardTab from "./tabs/DashboardTab.svelte";
  import ConsoleTab from "./tabs/ConsoleTab.svelte";
  import PlayersTab from "./tabs/PlayersTab.svelte";
  import PluginsTab from "./tabs/PluginsTab.svelte";
  import ModsTab from "./tabs/ModsTab.svelte";
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
    { id: "dashboard", label: "Dashboard", icon: House },
    { id: "console", label: "Console", icon: Terminal },
    { id: "players", label: "Players", icon: Users },
    { id: "plugins", label: "Plugins", icon: Puzzle },
    { id: "mods", label: "Mods", icon: Blocks },
    { id: "files", label: "Files", icon: Folder },
    { id: "settings", label: "Settings", icon: Settings },
    { id: "backups", label: "Backups", icon: Archive },
    { id: "scheduler", label: "Scheduler", icon: Clock },
  ] as const;

  type TabId = (typeof ALL_TABS)[number]["id"];

  // The Plugins/Mods tabs only appear for software that supports them.
  const tabs = $derived(
    ALL_TABS.filter((tab) => {
      if (tab.id === "plugins") return supportsPlugins(server.loader);
      if (tab.id === "mods") return supportsMods(server.loader);
      return true;
    }),
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
      toastsStore.show(`"${deletedName}" was deleted`);
    });
  }
</script>

<section class="detail" in:fade={{ duration: 120 }}>
  <header>
    <div class="title">
      <h1>{server.name}</h1>
      <div class="meta">
        <StatusBlob {status} showLabel />
        <Chip tone="info"><Blocks size={13} /> {server.mcVersion}</Chip>
        <Chip>{server.loader}</Chip>
        <Chip tone="warning"><Save size={13} /> {server.memoryMb} MB</Chip>
      </div>
    </div>
    <div class="actions">
      {#if canStart}
        <Button disabled={busy} onclick={() => run(() => api.startServer(server.id))}>
          <Play size={15} /> Start
        </Button>
      {/if}
      {#if canStop}
        <Button
          variant="soft"
          disabled={busy}
          onclick={() => run(() => api.restartServer(server.id))}
        >
          <RefreshCw size={15} /> Restart
        </Button>
        <Button variant="danger" disabled={busy} onclick={() => run(() => api.stopServer(server.id))}>
          <Square size={15} /> Stop
        </Button>
        <Button
          variant="ghost"
          disabled={busy}
          title="Force-kill the process"
          onclick={() => run(() => api.killServer(server.id))}
        >
          <Skull size={15} /> Kill
        </Button>
      {/if}
      {#if canStart}
        {#if confirmingDelete}
          <Button variant="danger" disabled={busy} onclick={deleteServer}>Really delete?</Button>
          <Button variant="ghost" onclick={() => (confirmingDelete = false)}>Keep it</Button>
        {:else}
          <Button variant="ghost" onclick={() => (confirmingDelete = true)}>
            <Trash2 size={15} /> Delete
          </Button>
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
        <tab.icon size={16} color={FEATURE_COLOR[tab.id]} />
        {tab.label}
      </button>
    {/each}
  </nav>

  <!-- Keyed on the server so switching servers fully remounts the active tab.
       This gives every tab a clean per-server state and prevents a slow load
       for the previous server from resolving into the new one's view. -->
  <div class="tab-content">
    {#key server.id}
      {#if activeTab === "dashboard"}
        <DashboardTab {server} />
      {:else if activeTab === "console"}
        <ConsoleTab {server} />
      {:else if activeTab === "players"}
        <PlayersTab {server} />
      {:else if activeTab === "plugins"}
        <PluginsTab {server} />
      {:else if activeTab === "mods"}
        <ModsTab {server} />
      {:else if activeTab === "files"}
        <FilesTab {server} />
      {:else if activeTab === "settings"}
        <SettingsTab {server} />
      {:else if activeTab === "backups"}
        <BackupsTab {server} />
      {:else if activeTab === "scheduler"}
        <SchedulerTab {server} />
      {/if}
    {/key}
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

  .tab-content {
    flex: 1;
    min-height: 0;
    /* `scroll` (not `auto`) always reserves the scrollbar's space, so content
       that grows past the viewport (e.g. the dashboard's port-forward panel
       expanding after "Open to internet") brings the scrollbar in without
       shifting the whole tab sideways. Matches the app shell's main scroller.
       The custom scrollbar track is transparent, so it's invisible until it
       actually scrolls. */
    overflow-y: scroll;
    /* A non-visible overflow-y makes overflow-x compute to `auto` too, which
       clips anything sitting flush against the edges — including the 2px outset
       outline on buttons at the very left (e.g. the console's quick-commands
       button). A few px of padding gives those outlines room to render. */
    padding: 3px;
  }
</style>
