<script lang="ts">
  import { fade } from "svelte/transition";
  import { api, type InstalledPlugin, type PluginSearchResult, type ServerConfig } from "../../api";
  import { toastsStore } from "../../stores/toasts.svelte";
  import { formatBytes } from "../../format";
  import Button from "../../components/Button.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  let installed = $state<InstalledPlugin[]>([]);
  let loadingInstalled = $state(false);
  let busyFile = $state<string | null>(null);
  let confirmingDelete = $state<string | null>(null);

  let searchQuery = $state("");
  let results = $state<PluginSearchResult[]>([]);
  let searching = $state(false);
  let installingId = $state<string | null>(null);
  let searched = $state(false);

  $effect(() => {
    // Reload when the server changes; also browse popular plugins to start.
    void server.id;
    loadInstalled();
    browse("");
  });

  async function loadInstalled() {
    loadingInstalled = true;
    try {
      installed = await api.listPlugins(server.id);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      loadingInstalled = false;
    }
  }

  async function browse(query: string) {
    searching = true;
    try {
      results = await api.searchPlugins(server.id, query);
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
      await api.setPluginEnabled(server.id, plugin.fileName, !plugin.enabled);
      await loadInstalled();
      toastsStore.success(plugin.enabled ? "Plugin disabled" : "Plugin enabled ✨");
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      busyFile = null;
    }
  }

  async function remove(plugin: InstalledPlugin) {
    confirmingDelete = null;
    busyFile = plugin.fileName;
    try {
      await api.deletePlugin(server.id, plugin.fileName);
      await loadInstalled();
      toastsStore.show(`Removed ${plugin.displayName} 🗑️`);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      busyFile = null;
    }
  }

  async function install(result: PluginSearchResult) {
    installingId = result.projectId;
    try {
      const plugin = await api.installPlugin(server.id, result.projectId);
      await loadInstalled();
      toastsStore.success(`Installed ${plugin.displayName} 🧩`);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      installingId = null;
    }
  }

  function hideBrokenIcon(event: Event) {
    (event.currentTarget as HTMLImageElement).style.display = "none";
  }
</script>

<div class="plugins-tab">
  <section class="installed">
    <h3>Installed plugins</h3>
    <p class="hint">Changes take effect the next time the server starts. 🔄</p>

    {#if loadingInstalled && installed.length === 0}
      <p class="muted">Loading…</p>
    {:else if installed.length === 0}
      <div class="empty">
        <span class="face">🧩</span>
        <p>No plugins yet — find some below!</p>
      </div>
    {:else}
      <ul class="installed-list">
        {#each installed as plugin (plugin.fileName)}
          <li class:disabled={!plugin.enabled} in:fade={{ duration: 120 }}>
            <div class="plugin-info">
              <span class="plugin-name">{plugin.displayName}</span>
              <span class="plugin-meta">
                {formatBytes(plugin.sizeBytes)}{plugin.enabled ? "" : " · disabled"}
              </span>
            </div>
            <div class="plugin-actions">
              <Button
                variant="ghost"
                disabled={busyFile === plugin.fileName}
                onclick={() => toggle(plugin)}
              >
                {plugin.enabled ? "🌙 Disable" : "☀️ Enable"}
              </Button>
              {#if confirmingDelete === plugin.fileName}
                <Button
                  variant="danger"
                  disabled={busyFile === plugin.fileName}
                  onclick={() => remove(plugin)}
                >
                  Sure?
                </Button>
                <Button variant="ghost" onclick={() => (confirmingDelete = null)}>No</Button>
              {:else}
                <Button variant="ghost" onclick={() => (confirmingDelete = plugin.fileName)}>🗑</Button>
              {/if}
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <section class="browse">
    <h3>Browse plugins</h3>
    <p class="hint">Powered by Modrinth — filtered to {server.loader} and {server.mcVersion}.</p>

    <form class="search-row" onsubmit={submitSearch}>
      <input
        type="text"
        bind:value={searchQuery}
        placeholder="Search for plugins… (e.g. EssentialsX)"
        spellcheck="false"
      />
      <Button type="submit" disabled={searching}>🔍 Search</Button>
    </form>

    {#if searching && results.length === 0}
      <p class="muted">Searching…</p>
    {:else if searched && results.length === 0}
      <p class="muted">No plugins found — try a different search.</p>
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
              <span class="result-icon placeholder">🧩</span>
            {/if}
            <div class="result-info">
              <div class="result-head">
                <span class="result-title">{result.title}</span>
                <span class="result-downloads">⬇ {result.downloads.toLocaleString()}</span>
              </div>
              <p class="result-desc">{result.description}</p>
              <span class="result-author">by {result.author}</span>
            </div>
            <Button
              variant="soft"
              disabled={installingId !== null}
              onclick={() => install(result)}
            >
              {installingId === result.projectId ? "Installing…" : "⬇ Install"}
            </Button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>

<style>
  .plugins-tab {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    padding-bottom: 1rem;
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

  .empty {
    text-align: center;
    color: var(--muted);
    padding: 1.5rem 0;
  }

  .face {
    font-size: 2.4rem;
    display: block;
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

  .plugin-actions {
    display: flex;
    gap: 0.35rem;
    flex-shrink: 0;
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
