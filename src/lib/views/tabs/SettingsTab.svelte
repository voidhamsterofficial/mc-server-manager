<script lang="ts">
  import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
  import { api, resolveBackupsDir, type Property, type ServerConfig } from "../../api";
  import {
    MEMORY_MAX_MB,
    MEMORY_MIN_MB,
    MEMORY_STEP_MB,
    SERVER_NAME_MAX_LENGTH,
  } from "../../constants";
  import { serversStore } from "../../stores/servers.svelte";
  import { toastsStore } from "../../stores/toasts.svelte";
  import Button from "../../components/Button.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  // --- Server config (name / memory / backups location) ---
  let editedName = $state("");
  let editedMemoryMb = $state(0);
  let editedBackupsDir = $state<string | null>(null);
  let savingConfig = $state(false);

  const backupsDirPreview = $derived(
    editedBackupsDir ?? resolveBackupsDir({ ...server, backupsDir: null }),
  );

  // --- server.properties ---
  let properties = $state<Property[]>([]);
  let edited = $state<Record<string, string>>({});
  let filterText = $state("");
  let loadingProperties = $state(false);
  let savingProperties = $state(false);

  /** Options for keys whose values are a fixed vanilla set. */
  const ENUM_OPTIONS: Record<string, string[]> = {
    gamemode: ["survival", "creative", "adventure", "spectator"],
    difficulty: ["peaceful", "easy", "normal", "hard"],
  };

  const dirtyCount = $derived(Object.keys(edited).length);
  const filteredProperties = $derived(
    properties.filter((property) =>
      property.key.toLowerCase().includes(filterText.trim().toLowerCase()),
    ),
  );

  $effect(() => {
    editedName = server.name;
    editedMemoryMb = server.memoryMb;
    editedBackupsDir = server.backupsDir;
    loadProperties(server.id);
  });

  async function browseBackupsDir() {
    const picked = await openFolderDialog({
      directory: true,
      title: "Choose where this server's backups go",
    });
    if (typeof picked === "string") {
      editedBackupsDir = picked;
    }
  }

  async function loadProperties(serverId: string) {
    loadingProperties = true;
    edited = {};
    try {
      properties = await api.getServerProperties(serverId);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      loadingProperties = false;
    }
  }

  function valueOf(property: Property): string {
    return edited[property.key] ?? property.value;
  }

  function setValue(key: string, value: string) {
    edited = { ...edited, [key]: value };
  }

  function controlKind(property: Property): "toggle" | "number" | "enum" | "text" {
    if (ENUM_OPTIONS[property.key]) {
      return "enum";
    }
    const value = property.value;
    if (value === "true" || value === "false") {
      return "toggle";
    }
    if (value !== "" && /^-?\d+$/.test(value)) {
      return "number";
    }
    return "text";
  }

  async function saveConfig() {
    savingConfig = true;
    try {
      await api.updateServer(server.id, {
        name: editedName,
        memoryMb: editedMemoryMb,
        javaPath: server.javaPath,
        backupsDir: editedBackupsDir,
      });
      await serversStore.refresh();
      toastsStore.success("Server settings saved 💾");
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      savingConfig = false;
    }
  }

  async function saveProperties() {
    savingProperties = true;
    try {
      const updates: Property[] = Object.entries(edited).map(([key, value]) => ({ key, value }));
      await api.saveServerProperties(server.id, updates);
      await loadProperties(server.id);
      toastsStore.success("server.properties saved — restart to apply 🔁");
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      savingProperties = false;
    }
  }
</script>

<div class="settings-tab">
  <section class="card">
    <h3>🧸 Server</h3>
    <div class="config-grid">
      <label>
        <span>Name</span>
        <input type="text" bind:value={editedName} maxlength={SERVER_NAME_MAX_LENGTH} />
      </label>
      <label>
        <span>Memory — {editedMemoryMb} MB</span>
        <input
          type="range"
          min={MEMORY_MIN_MB}
          max={MEMORY_MAX_MB}
          step={MEMORY_STEP_MB}
          bind:value={editedMemoryMb}
        />
      </label>
    </div>
    <div class="backups-row">
      <span class="backups-label">🎁 Backups folder</span>
      <div class="backups-controls">
        <code class="backups-path" title={backupsDirPreview}>{backupsDirPreview}</code>
        <Button variant="soft" onclick={browseBackupsDir}>Browse…</Button>
        {#if editedBackupsDir !== null}
          <Button variant="ghost" onclick={() => (editedBackupsDir = null)}>
            Reset to default
          </Button>
        {/if}
      </div>
    </div>
    <div class="card-actions">
      <span class="hint">Changes apply on the next start.</span>
      <Button disabled={savingConfig} onclick={saveConfig}>Save</Button>
    </div>
  </section>

  <section class="card props-card">
    <div class="props-head">
      <h3>🗒️ server.properties</h3>
      <input class="filter" type="text" bind:value={filterText} placeholder="Filter keys… 🔍" />
    </div>

    {#if loadingProperties}
      <p class="hint">Loading…</p>
    {:else if properties.length === 0}
      <p class="hint">
        No server.properties yet — it appears after the server's first start 🌱
      </p>
    {:else}
      <div class="props-list">
        {#each filteredProperties as property (property.key)}
          <div class="prop-row" class:dirty={edited[property.key] !== undefined}>
            <span class="prop-key">{property.key}</span>
            {#if controlKind(property) === "toggle"}
              <input
                type="checkbox"
                checked={valueOf(property) === "true"}
                onchange={(event) =>
                  setValue(property.key, event.currentTarget.checked ? "true" : "false")}
              />
            {:else if controlKind(property) === "enum"}
              <select
                value={valueOf(property)}
                onchange={(event) => setValue(property.key, event.currentTarget.value)}
              >
                {#each ENUM_OPTIONS[property.key] as option (option)}
                  <option value={option}>{option}</option>
                {/each}
              </select>
            {:else if controlKind(property) === "number"}
              <input
                type="number"
                value={valueOf(property)}
                onchange={(event) => setValue(property.key, event.currentTarget.value)}
              />
            {:else}
              <input
                type="text"
                value={valueOf(property)}
                onchange={(event) => setValue(property.key, event.currentTarget.value)}
              />
            {/if}
          </div>
        {/each}
      </div>

      <div class="card-actions">
        <span class="hint">
          {dirtyCount === 0
            ? "Edit any value — comments and order are preserved."
            : `${dirtyCount} change${dirtyCount === 1 ? "" : "s"} pending · restart to apply`}
        </span>
        <Button disabled={savingProperties || dirtyCount === 0} onclick={saveProperties}>
          Save changes
        </Button>
      </div>
    {/if}
  </section>
</div>

<style>
  /* Fills the tab exactly: the server card keeps its natural height and the
     properties card flexes to the remaining space, scrolling its key list
     internally — no box ever renders taller than the window. */
  .settings-tab {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding-bottom: 0.25rem;
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    padding: 1rem 1.25rem;
    flex-shrink: 0;
  }

  .props-card {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  h3 {
    margin: 0 0 0.75rem;
    font-size: 1rem;
  }

  .config-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 0.9rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
  }

  input[type="text"],
  input[type="number"],
  select {
    font-family: inherit;
    font-size: 0.95rem;
    color: var(--text);
    background: var(--surface-2);
    border: 2px solid transparent;
    border-radius: var(--radius-md);
    padding: 0.5em 0.8em;
    outline: none;
    transition: border-color 0.18s ease;
  }

  input[type="text"]:focus,
  input[type="number"]:focus,
  select:focus {
    border-color: var(--accent);
  }

  input[type="range"] {
    accent-color: var(--accent);
  }

  input[type="checkbox"] {
    width: 1.15rem;
    height: 1.15rem;
    accent-color: var(--accent);
    justify-self: start;
  }

  .backups-row {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-top: 0.9rem;
  }

  .backups-label {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
  }

  .backups-controls {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  .backups-path {
    flex: 1;
    min-width: 200px;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--text);
    background: var(--surface-2);
    border-radius: var(--radius-md);
    padding: 0.55em 0.9em;
    overflow-wrap: break-word;
    word-break: break-all;
    user-select: text;
  }

  .card-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    margin-top: 0.9rem;
  }

  .hint {
    font-size: 0.82rem;
    color: var(--muted);
  }

  .props-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    margin-bottom: 0.6rem;
  }

  .props-head h3 {
    margin: 0;
  }

  .filter {
    width: 220px;
  }

  .props-list {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }

  .prop-row {
    display: grid;
    grid-template-columns: 1fr 220px;
    align-items: center;
    gap: 0.75rem;
    padding: 0.45rem 0.8rem;
    border-bottom: 1px solid var(--border);
    transition: background-color 0.15s ease;
  }

  .prop-row:last-child {
    border-bottom: none;
  }

  .prop-row:hover {
    background: var(--surface-2);
  }

  .prop-row.dirty {
    background: var(--accent-soft);
  }

  .prop-key {
    font-family: var(--font-mono);
    font-size: 0.82rem;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
