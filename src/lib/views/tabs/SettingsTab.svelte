<script lang="ts">
  import { untrack } from "svelte";
  import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
  import { Server, Wrench, Gift, Image, MessageSquare, NotebookText, Copy } from "@lucide/svelte";
  import {
    api,
    resolveBackupsDir,
    type JavaInstall,
    type Property,
    type ServerConfig,
  } from "../../ipc/api";
  import { decodeMotdProperty, encodeMotdProperty } from "../../util/motd";
  import MotdEditor from "../../components/MotdEditor.svelte";
  import {
    MEMORY_MAX_MB,
    MEMORY_MIN_MB,
    MEMORY_STEP_MB,
    SERVER_NAME_MAX_LENGTH,
  } from "../../util/constants";
  import { serversStore } from "../../stores/servers.svelte";
  import { toastsStore } from "../../stores/toasts.svelte";
  import Button from "../../components/Button.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  // --- Server config (name / memory / java / launch / backups) ---
  let editedName = $state("");
  let editedMemoryMb = $state(0);
  let editedBackupsDir = $state<string | null>(null);
  let editedJavaPath = $state<string>("");
  let editedJavaArgs = $state("");
  let editedStartCommand = $state("");
  let editedRetention = $state("");
  let javaInstalls = $state<JavaInstall[]>([]);
  let savingConfig = $state(false);
  let advancedOpen = $state(false);

  const isBedrock = $derived(server.loader === "bds");

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

  // --- Server icon ---
  let iconDataUrl = $state<string | null>(null);

  const motdValue = $derived(
    decodeMotdProperty(
      edited["motd"] ?? properties.find((property) => property.key === "motd")?.value ?? "",
    ),
  );

  // Re-seed the form only when the selected server actually changes — not on
  // every serversStore.refresh(), which hands us a fresh object with the same
  // id after a save. Re-seeding on each refresh would reset the fields, reload
  // properties, and jerk the scroll position out from under the user.
  /** The command ServerForge would build for this server, shown so the custom
   *  override has something concrete to copy and edit. */
  let defaultStartCommand = $state<string | null>(null);

  let seededForId: string | null = null;
  $effect(() => {
    const id = server.id;
    if (id === seededForId) {
      return;
    }
    seededForId = id;
    untrack(() => {
      editedName = server.name;
      editedMemoryMb = server.memoryMb;
      editedBackupsDir = server.backupsDir;
      editedJavaPath = server.javaPath ?? "";
      editedJavaArgs = server.javaArgs ?? "";
      editedStartCommand = server.startCommand ?? "";
      editedRetention = server.backupRetention === null ? "" : String(server.backupRetention);
      loadProperties(id);
      loadIcon(id);
      loadDefaultStartCommand(id);
    });
  });

  $effect(() => {
    api
      .detectJava()
      .then((installs) => (javaInstalls = installs))
      .catch(() => (javaInstalls = []));
  });

  async function loadIcon(serverId: string) {
    try {
      iconDataUrl = await api.getServerIcon(serverId);
    } catch {
      iconDataUrl = null;
    }
  }

  async function browseIcon() {
    try {
      const picked = await openFolderDialog({
        title: "Choose a server icon image",
        filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp", "bmp"] }],
      });
      if (typeof picked !== "string") {
        return;
      }
      await api.setServerIcon(server.id, picked);
      await loadIcon(server.id);
      toastsStore.success("Server icon updated — applies on the next start");
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function removeIcon() {
    try {
      await api.removeServerIcon(server.id);
      iconDataUrl = null;
      toastsStore.show("Server icon removed");
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function browseBackupsDir() {
    try {
      const picked = await openFolderDialog({
        directory: true,
        title: "Choose where this server's backups go",
      });
      if (typeof picked === "string") {
        editedBackupsDir = picked;
      }
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  /** Asks the backend what it would actually run, rather than describing it
   *  here — a second description in the UI would drift from the real one. */
  async function loadDefaultStartCommand(serverId: string) {
    defaultStartCommand = null;
    try {
      defaultStartCommand = await api.previewStartCommand(serverId);
    } catch {
      // Only a reference display; a server whose jar isn't installed yet
      // simply doesn't get one.
    }
  }

  async function copyDefaultStartCommand() {
    if (defaultStartCommand === null) {
      return;
    }
    try {
      await navigator.clipboard.writeText(defaultStartCommand);
      toastsStore.success("Copied the default start command");
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function loadProperties(serverId: string) {
    // Only show the "Loading…" placeholder on the very first load; on a reload
    // (e.g. after saving) keep the current rows in place so the list never
    // collapses to one line and yanks the scroll position.
    loadingProperties = properties.length === 0;
    try {
      const loaded = await api.getServerProperties(serverId);
      properties = loaded;
      edited = {};
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
      const retentionNumber = Number.parseInt(editedRetention, 10);
      await api.updateServer(server.id, {
        name: editedName,
        memoryMb: editedMemoryMb,
        javaPath: editedJavaPath === "" ? null : editedJavaPath,
        backupsDir: editedBackupsDir,
        javaArgs: editedJavaArgs.trim() === "" ? null : editedJavaArgs.trim(),
        startCommand: editedStartCommand.trim() === "" ? null : editedStartCommand.trim(),
        backupRetention:
          Number.isNaN(retentionNumber) || retentionNumber < 1 ? null : retentionNumber,
      });
      await serversStore.refresh();
      toastsStore.success("Server settings saved");
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
      toastsStore.success("server.properties saved — restart to apply");
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      savingProperties = false;
    }
  }
</script>

<div class="settings-tab">
  <section class="card">
    <h3><Server size={18} color="var(--accent)" /> Server</h3>
    <div class="config-grid">
      <label>
        <span>Name</span>
        <input type="text" bind:value={editedName} maxlength={SERVER_NAME_MAX_LENGTH} />
      </label>
      {#if !isBedrock}
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
        <label>
          <span>Java runtime</span>
          <select bind:value={editedJavaPath}>
            <option value="">Auto (best match, downloads if needed)</option>
            {#each javaInstalls as install (install.path)}
              <option value={install.path}>Java {install.majorVersion} — {install.path}</option>
            {/each}
          </select>
        </label>
      {/if}
      <label>
        <span>Backup retention (empty = keep all)</span>
        <input type="number" min="1" bind:value={editedRetention} placeholder="keep all" />
      </label>
    </div>

    <div class="advanced">
      <button type="button" class="advanced-toggle" onclick={() => (advancedOpen = !advancedOpen)}>
        <span class="chevron" class:open={advancedOpen}>▸</span>
        <Wrench size={15} /> Advanced launch
      </button>
      {#if advancedOpen}
        <div class="advanced-body">
          {#if !isBedrock}
            <label>
              <span>Extra JVM arguments</span>
              <input
                type="text"
                bind:value={editedJavaArgs}
                placeholder="-XX:+UseG1GC"
                spellcheck="false"
              />
            </label>
          {/if}
          <label>
            <span>Custom start command (overrides everything)</span>
            <input
              type="text"
              bind:value={editedStartCommand}
              placeholder="java -Xmx4G -jar server.jar nogui"
              spellcheck="false"
            />
          </label>
          {#if defaultStartCommand !== null}
            <div class="default-command">
              <span class="default-command-label">
                Default (used when the box above is empty):
              </span>
              <code>{defaultStartCommand}</code>
              <Button variant="ghost" onclick={copyDefaultStartCommand}>
                <Copy size={14} /> Copy
              </Button>
            </div>
          {/if}
        </div>
      {/if}
    </div>
    <div class="backups-row">
      <span class="backups-label"><Gift size={14} /> Backups folder</span>
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

  {#if !isBedrock}
    <section class="card">
      <div class="icon-motd-grid">
        <div class="icon-block">
          <h3><Image size={18} color="var(--accent)" /> Server icon</h3>
        <div class="icon-row">
          {#if iconDataUrl}
            <img class="icon-preview" src={iconDataUrl} alt="Server icon" width="64" height="64" />
          {:else}
            <div class="icon-placeholder">no icon</div>
          {/if}
          <div class="icon-actions">
            <Button variant="soft" onclick={browseIcon}>Choose image…</Button>
            {#if iconDataUrl}
              <Button variant="ghost" onclick={removeIcon}>Remove</Button>
            {/if}
          </div>
        </div>
        <p class="hint">
          Any image works — it's resized to the 64×64 the game needs. Shows in the
          multiplayer list after the next start.
        </p>
      </div>

        <div class="motd-block">
          <h3><MessageSquare size={18} color="var(--accent)" /> MOTD</h3>
          <MotdEditor
            value={motdValue}
            onchange={(text) => setValue("motd", encodeMotdProperty(text))}
          />
          <p class="hint">Saves together with the server.properties changes below.</p>
        </div>
      </div>
    </section>
  {/if}

  <section class="card props-card">
    <div class="props-head">
      <h3><NotebookText size={18} color="var(--accent)" /> server.properties</h3>
      <input class="filter" type="text" bind:value={filterText} placeholder="Filter keys…" />
    </div>

    {#if loadingProperties}
      <p class="hint">Loading…</p>
    {:else if properties.length === 0}
      <p class="hint">
        No server.properties yet — it appears after the server's first start
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
  /* One scroll context: the whole tab scrolls inside tab-content, so there
     is no nested-scroll dead zone and the Save buttons are always
     reachable. */
  .settings-tab {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding-bottom: 1.5rem;
  }

  .icon-motd-grid {
    display: grid;
    grid-template-columns: minmax(200px, 240px) 1fr;
    gap: 1.25rem;
  }

  .icon-block,
  .motd-block {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    min-width: 0;
  }

  .icon-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .icon-preview {
    border-radius: var(--radius-sm);
    image-rendering: pixelated;
    box-shadow: 0 0 0 2px rgba(15, 15, 18, 0.35);
  }

  .default-command {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-top: -0.2rem;
  }

  .default-command-label {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .default-command code {
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 0.35rem 0.5rem;
  }

  .icon-placeholder {
    width: 64px;
    height: 64px;
    display: grid;
    place-items: center;
    font-size: 0.7rem;
    color: var(--muted);
    background: var(--surface-2);
    border-radius: var(--radius-sm);
    box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.15);
  }

  .icon-actions {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    align-items: flex-start;
  }

  @media (max-width: 900px) {
    .icon-motd-grid {
      grid-template-columns: 1fr;
    }
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

  .advanced {
    margin-top: 0.9rem;
  }

  .advanced-toggle {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    border: none;
    background: transparent;
    color: var(--muted);
    font-family: inherit;
    font-size: 0.9rem;
    font-weight: 700;
    padding: 0;
    cursor: pointer;
  }

  .advanced-toggle:hover {
    color: var(--text);
  }

  .chevron {
    display: inline-block;
    transition: transform var(--duration-fast) var(--ease-out);
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .advanced-body {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 0.75rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
  }

  /* Text, number, and select controls inherit the app-wide blocky style from
     theme.css. */
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
