<script lang="ts">
  import { api, type Property, type ServerConfig } from "../../api";
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

  // --- Server config (name / memory / java) ---
  let editedName = $state("");
  let editedMemoryMb = $state(0);
  let savingConfig = $state(false);

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
    loadProperties(server.id);
  });

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
    <div class="card-actions">
      <span class="hint">Changes apply on the next start.</span>
      <Button disabled={savingConfig} onclick={saveConfig}>Save</Button>
    </div>
  </section>

  <section class="card">
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
  .settings-tab {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding-bottom: 1rem;
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    padding: 1rem 1.25rem;
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
    max-height: 420px;
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
