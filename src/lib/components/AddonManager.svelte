<script lang="ts">
  import { fade } from "svelte/transition";
  import { untrack } from "svelte";
  import { Puzzle, Trash2, Moon, Sun, Search, Download, ArrowUpCircle, CircleCheck } from "@lucide/svelte";
  import type { AddonSearchResult, AddonSource, AddonUpdateStatus, InstalledPlugin } from "../ipc/api";
  import { toastsStore } from "../stores/toasts.svelte";
  import { formatBytes } from "../util/format";
  import Button from "./Button.svelte";

  interface SourceOption {
    value: AddonSource;
    label: string;
  }

  interface Props {
    serverId: string;
    /** "plugin" or "mod" — only used for copy. */
    kind: string;
    /** Feature accent (a CSS color) for this addon type's icons. */
    accentColor: string;
    sources: SourceOption[];
    /** True when the currently selected source needs setup (e.g. a missing
     *  CurseForge API key) before it can be browsed. */
    sourceBlocked?: (source: AddonSource) => string | null;
    list: (serverId: string) => Promise<InstalledPlugin[]>;
    setEnabled: (serverId: string, fileName: string, enabled: boolean) => Promise<string>;
    remove: (serverId: string, fileName: string) => Promise<void>;
    search: (serverId: string, source: AddonSource, query: string) => Promise<AddonSearchResult[]>;
    install: (serverId: string, source: AddonSource, projectId: string) => Promise<InstalledPlugin>;
    checkUpdates: (serverId: string) => Promise<AddonUpdateStatus[]>;
    update: (serverId: string, fileName: string) => Promise<InstalledPlugin>;
  }

  let {
    serverId,
    kind,
    accentColor,
    sources,
    sourceBlocked,
    list,
    setEnabled,
    remove,
    search,
    install,
    checkUpdates,
    update,
  }: Props = $props();

  let installed = $state<InstalledPlugin[]>([]);
  let updates = $state<Map<string, AddonUpdateStatus>>(new Map());
  let loadingInstalled = $state(false);
  let checkingUpdates = $state(false);
  let busyFile = $state<string | null>(null);
  let confirmingDelete = $state<string | null>(null);

  let source = $state<AddonSource>(untrack(() => sources[0].value));
  let searchQuery = $state("");
  let results = $state<AddonSearchResult[]>([]);
  let searching = $state(false);
  let installingId = $state<string | null>(null);
  let searched = $state(false);

  $effect(() => {
    // Reload when the server changes.
    void serverId;
    loadInstalled();
  });

  $effect(() => {
    // Re-browse when the server or the selected marketplace changes; a
    // leftover query from before either switch would otherwise keep showing
    // results for the wrong server or source.
    void serverId;
    void source;
    searchQuery = "";
    browse("");
  });

  async function loadInstalled() {
    loadingInstalled = true;
    try {
      installed = await list(serverId);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      loadingInstalled = false;
    }
    refreshUpdates();
  }

  async function refreshUpdates() {
    checkingUpdates = true;
    try {
      const statuses = await checkUpdates(serverId);
      updates = new Map(statuses.map((status) => [status.fileName, status]));
    } catch {
      // Update checks are best-effort — a marketplace hiccup shouldn't block the tab.
    } finally {
      checkingUpdates = false;
    }
  }

  async function browse(query: string) {
    if (sourceBlocked?.(source)) {
      results = [];
      searched = false;
      return;
    }
    searching = true;
    try {
      results = await search(serverId, source, query);
      searched = true;
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      searching = false;
    }
  }

  function submitSearch(event: SubmitEvent) {
    event.preventDefault();
    browse(searchQuery.trim());
  }

  async function toggle(plugin: InstalledPlugin) {
    busyFile = plugin.fileName;
    try {
      await setEnabled(serverId, plugin.fileName, !plugin.enabled);
      await loadInstalled();
      toastsStore.success(plugin.enabled ? `${kind} disabled` : `${kind} enabled`);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      busyFile = null;
    }
  }

  async function doRemove(plugin: InstalledPlugin) {
    confirmingDelete = null;
    busyFile = plugin.fileName;
    try {
      await remove(serverId, plugin.fileName);
      await loadInstalled();
      toastsStore.show(`Removed ${plugin.displayName}`);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      busyFile = null;
    }
  }

  async function doInstall(result: AddonSearchResult) {
    installingId = result.projectId;
    try {
      const plugin = await install(serverId, result.source, result.projectId);
      await loadInstalled();
      toastsStore.success(`Installed ${plugin.displayName}`);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      installingId = null;
    }
  }

  async function doUpdate(plugin: InstalledPlugin) {
    busyFile = plugin.fileName;
    try {
      const updated = await update(serverId, plugin.fileName);
      await loadInstalled();
      toastsStore.success(`Updated ${updated.displayName}`);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      busyFile = null;
    }
  }

  function hideBrokenIcon(event: Event) {
    (event.currentTarget as HTMLImageElement).style.display = "none";
  }

  const blockedReason = $derived(sourceBlocked?.(source) ?? null);

  /** "source:projectId" keys for every installed addon Blockparty has
   *  provenance for, so already-installed search results can be recognized. */
  const installedKeys = $derived(
    new Set([...updates.values()].map((status) => `${status.source}:${status.projectId}`)),
  );

  function isInstalled(result: AddonSearchResult): boolean {
    return installedKeys.has(`${result.source}:${result.projectId}`);
  }
</script>

<div class="addon-manager">
  <section class="installed">
    <div class="section-head">
      <h3>Installed {kind}s</h3>
      {#if checkingUpdates}<span class="muted small">Checking for updates…</span>{/if}
    </div>
    <p class="hint">Changes take effect the next time the server starts.</p>

    {#if loadingInstalled && installed.length === 0}
      <p class="muted">Loading…</p>
    {:else if installed.length === 0}
      <div class="empty">
        <span class="face"><Puzzle size={38} color={accentColor} /></span>
        <p>No {kind}s yet — find some below!</p>
      </div>
    {:else}
      <ul class="installed-list">
        {#each installed as addon (addon.fileName)}
          {@const updateStatus = updates.get(addon.fileName)}
          <li class:disabled={!addon.enabled} in:fade={{ duration: 120 }}>
            <div class="plugin-info">
              <span class="plugin-name">{addon.displayName}</span>
              <span class="plugin-meta">
                {formatBytes(addon.sizeBytes)}{addon.enabled ? "" : " · disabled"}
                {#if updateStatus?.hasUpdate}
                  · <span class="update-badge">update available{updateStatus.latestVersion ? ` (${updateStatus.latestVersion})` : ""}</span>
                {/if}
              </span>
            </div>
            <div class="plugin-actions">
              {#if updateStatus?.hasUpdate}
                <Button
                  variant="soft"
                  disabled={busyFile === addon.fileName}
                  onclick={() => doUpdate(addon)}
                >
                  <ArrowUpCircle size={14} /> Update
                </Button>
              {/if}
              <Button
                variant="ghost"
                disabled={busyFile === addon.fileName}
                onclick={() => toggle(addon)}
              >
                {#if addon.enabled}<Moon size={14} /> Disable{:else}<Sun size={14} /> Enable{/if}
              </Button>
              {#if confirmingDelete === addon.fileName}
                <Button
                  variant="danger"
                  disabled={busyFile === addon.fileName}
                  onclick={() => doRemove(addon)}
                >
                  Sure?
                </Button>
                <Button variant="ghost" onclick={() => (confirmingDelete = null)}>No</Button>
              {:else}
                <Button variant="ghost" square onclick={() => (confirmingDelete = addon.fileName)}>
                  <Trash2 size={15} />
                </Button>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="browse">
    <h3>Browse {kind}s</h3>

    {#if sources.length > 1}
      <div class="source-tabs" role="tablist">
        {#each sources as option (option.value)}
          <button
            type="button"
            role="tab"
            class="source-tab"
            class:active={source === option.value}
            aria-selected={source === option.value}
            onclick={() => (source = option.value)}
          >
            {option.label}
          </button>
        {/each}
      </div>
    {/if}

    {#if blockedReason}
      <p class="muted blocked">{blockedReason}</p>
    {:else}
      <form class="search-row" onsubmit={submitSearch}>
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Search for {kind}s…"
          spellcheck="false"
        />
        <Button type="submit" disabled={searching}><Search size={15} /> Search</Button>
      </form>

      {#if searching && results.length === 0}
        <p class="muted">Searching…</p>
      {:else if searched && results.length === 0}
        <p class="muted">No {kind}s found — try a different search.</p>
      {:else}
        <ul class="results">
          {#each results as result (result.projectId)}
            <li in:fade={{ duration: 120 }}>
              {#if result.iconUrl}
                <img
                  class="result-icon"
                  src={result.iconUrl}
                  alt=""
                  width="44"
                  height="44"
                  loading="lazy"
                  onerror={hideBrokenIcon}
                />
              {:else}
                <span class="result-icon placeholder"><Puzzle size={20} color={accentColor} /></span>
              {/if}
              <div class="result-info">
                <div class="result-head">
                  <span class="result-title">{result.title}</span>
                  <span class="result-downloads"><Download size={12} /> {result.downloads.toLocaleString()}</span>
                </div>
                <p class="result-desc">{result.description}</p>
                {#if result.author}<span class="result-author">by {result.author}</span>{/if}
              </div>
              {#if isInstalled(result)}
                <span class="installed-badge"><CircleCheck size={15} /> Installed</span>
              {:else}
                <Button
                  variant="soft"
                  disabled={installingId !== null}
                  onclick={() => doInstall(result)}
                >
                  {#if installingId !== result.projectId}<Download size={15} />{/if} {installingId === result.projectId ? "Installing…" : "Install"}
                </Button>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </section>
</div>

<style>
  .addon-manager {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    padding-bottom: 1rem;
  }

  .section-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.6rem;
  }

  h3 {
    margin: 0 0 0.2rem;
    font-size: 1rem;
  }

  .hint {
    margin: 0 0 0.75rem;
    font-size: 0.85rem;
    color: var(--muted);
  }

  .muted {
    color: var(--muted);
    font-size: 0.9rem;
  }

  .muted.small {
    font-size: 0.78rem;
  }

  .blocked {
    background: var(--surface-2);
    border-radius: var(--radius-md);
    padding: 0.75rem 0.9rem;
  }

  .empty {
    text-align: center;
    color: var(--muted);
    padding: 1.5rem 0;
  }

  .face {
    display: flex;
    justify-content: center;
    margin-bottom: 0.3rem;
  }

  .installed-list,
  .results {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .installed-list li {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-soft);
    padding: 0.6rem 0.9rem;
  }

  .installed-list li.disabled {
    opacity: 0.6;
  }

  .plugin-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  .plugin-name {
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .plugin-meta {
    font-size: 0.78rem;
    color: var(--muted);
  }

  .update-badge {
    color: var(--accent-strong);
    font-weight: 600;
  }

  .installed-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.35em;
    flex-shrink: 0;
    font-size: 0.85rem;
    font-weight: 700;
    color: var(--mint);
    background: var(--mint-soft);
    border-radius: var(--radius-md);
    padding: 0.5em 0.9em;
    white-space: nowrap;
  }

  .plugin-actions {
    display: flex;
    gap: 0.35rem;
    flex-shrink: 0;
  }

  .source-tabs {
    display: flex;
    gap: 0.4rem;
    margin-bottom: 0.75rem;
  }

  .source-tab {
    font-family: inherit;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
    background: var(--surface-2);
    border: 2px solid transparent;
    border-radius: var(--radius-md);
    padding: 0.4em 0.9em;
    cursor: pointer;
    transition:
      background-color var(--duration-fast) var(--ease-out),
      color var(--duration-fast) var(--ease-out);
  }

  .source-tab:hover {
    color: var(--text);
  }

  .source-tab.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
    border-color: var(--accent-soft);
  }

  .search-row {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.9rem;
  }

  .search-row input {
    flex: 1;
    min-width: 0;
    font-family: inherit;
    font-size: 0.95rem;
    color: var(--text);
    background: var(--surface-2);
    border: 2px solid transparent;
    border-radius: var(--radius-md);
    padding: 0.5em 0.9em;
    outline: none;
    transition: border-color 0.18s ease;
  }

  .search-row input:focus {
    border-color: var(--accent);
  }

  .results li {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-soft);
    padding: 0.6rem 0.9rem;
  }

  .result-icon {
    flex-shrink: 0;
    width: 44px;
    height: 44px;
    border-radius: var(--radius-sm);
    image-rendering: pixelated;
    object-fit: cover;
  }

  .result-icon.placeholder {
    display: grid;
    place-items: center;
    font-size: 1.6rem;
    background: var(--surface-2);
  }

  .result-info {
    flex: 1;
    min-width: 0;
  }

  .result-head {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
  }

  .result-title {
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .result-downloads {
    display: inline-flex;
    align-items: center;
    gap: 0.25em;
    font-size: 0.75rem;
    color: var(--muted);
    white-space: nowrap;
    flex-shrink: 0;
    margin-left: auto;
  }

  .result-desc {
    margin: 0.15rem 0;
    font-size: 0.83rem;
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .result-author {
    font-size: 0.75rem;
    color: var(--muted);
  }
</style>
