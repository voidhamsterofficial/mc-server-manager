<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
  import { api, PROXY_LOADERS, type Loader, type McVersion } from "../api";
  import {
    MEMORY_MAX_MB,
    MEMORY_MIN_MB,
    MEMORY_STEP_MB,
    SERVER_NAME_MAX_LENGTH,
  } from "../constants";
  import { onInstallProgress } from "../events";
  import { serversStore } from "../stores/servers.svelte";
  import { toastsStore } from "../stores/toasts.svelte";
  import Modal from "../components/Modal.svelte";
  import Button from "../components/Button.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";

  interface Props {
    open: boolean;
    onclose: () => void;
  }

  let { open, onclose }: Props = $props();

  interface CatalogEntry {
    value: Loader;
    label: string;
    emoji: string;
    hint: string;
    available: boolean;
  }

  const LOADER_CATALOG: { category: string; entries: CatalogEntry[] }[] = [
    {
      category: "Vanilla & official",
      entries: [
        {
          value: "vanilla",
          label: "Vanilla",
          emoji: "🍦",
          hint: "The official Mojang server",
          available: true,
        },
        {
          value: "bds",
          label: "Bedrock",
          emoji: "🪨",
          hint: "Official Bedrock server (Win/Linux)",
          available: true,
        },
      ],
    },
    {
      category: "Plugins (Bukkit ecosystem)",
      entries: [
        {
          value: "paper",
          label: "Paper",
          emoji: "📜",
          hint: "Fast — the plugin gold standard",
          available: true,
        },
        {
          value: "purpur",
          label: "Purpur",
          emoji: "🧪",
          hint: "Paper + extreme configurability",
          available: true,
        },
        {
          value: "folia",
          label: "Folia",
          emoji: "🧵",
          hint: "Multithreaded, huge player counts",
          available: true,
        },
        {
          value: "spigot",
          label: "Spigot",
          emoji: "🔩",
          hint: "Compiled by BuildTools — takes minutes",
          available: true,
        },
      ],
    },
    {
      category: "Mods",
      entries: [
        {
          value: "fabric",
          label: "Fabric",
          emoji: "🧶",
          hint: "Lightweight modern mod loader",
          available: true,
        },
        {
          value: "neoforge",
          label: "NeoForge",
          emoji: "🔥",
          hint: "The modern Forge successor",
          available: true,
        },
        {
          value: "forge",
          label: "Forge",
          emoji: "⚒️",
          hint: "The classic modding giant",
          available: true,
        },
        {
          value: "quilt",
          label: "Quilt",
          emoji: "🪡",
          hint: "Community fork of Fabric",
          available: true,
        },
      ],
    },
    {
      category: "Hybrid (mods + plugins)",
      entries: [
        {
          value: "arclight",
          label: "Arclight",
          emoji: "🌉",
          hint: "Plugins on Forge (Forge edition)",
          available: true,
        },
        {
          value: "mohist",
          label: "Mohist",
          emoji: "🧬",
          hint: "Forge modpacks + plugins",
          available: true,
        },
      ],
    },
    {
      category: "Network proxies",
      entries: [
        {
          value: "velocity",
          label: "Velocity",
          emoji: "⚡",
          hint: "Modern secure proxy",
          available: true,
        },
        {
          value: "bungeecord",
          label: "BungeeCord",
          emoji: "🌐",
          hint: "The original network proxy",
          available: true,
        },
      ],
    },
  ];

  const DEFAULT_GAME_PORT = 25565;
  const DEFAULT_PROXY_PORT = 25577;

  let step = $state<"software" | "details">("software");
  let name = $state("");
  let loader = $state<Loader>("vanilla");
  let versions = $state<McVersion[]>([]);
  let versionsForLoader = $state<Loader | null>(null);
  let selectedVersion = $state("");
  let memoryMb = $state(2048);
  let port = $state(DEFAULT_GAME_PORT);
  let portTouched = $state(false);
  let acceptEula = $state(false);
  let showSnapshots = $state(false);
  let loadingVersions = $state(false);
  let creating = $state(false);
  let progress = $state<number | null>(null);
  let locationParent = $state<string | null>(null);
  let locationPreview = $state("");
  let previewTimer: ReturnType<typeof setTimeout> | undefined;
  let advancedOpen = $state(false);
  let javaArgs = $state("");
  let startCommand = $state("");

  const isProxy = $derived(PROXY_LOADERS.includes(loader));
  const isBedrock = $derived(loader === "bds");
  const chosenEntry = $derived(
    LOADER_CATALOG.flatMap((group) => group.entries).find((entry) => entry.value === loader),
  );
  const hasSnapshots = $derived(versions.some((version) => version.type !== "release"));
  const visibleVersions = $derived(
    versions.filter((version) => showSnapshots || version.type === "release"),
  );
  // A cleared number input binds to null, and the min/max attributes are only
  // spinner hints — so validate the port before it can reach the backend.
  const portValid = $derived(
    typeof port === "number" && Number.isInteger(port) && port >= 1024 && port <= 65535,
  );
  const canSubmit = $derived(
    name.trim().length > 0 &&
      selectedVersion !== "" &&
      portValid &&
      (acceptEula || isProxy) &&
      !creating,
  );

  $effect(() => {
    // Keep the selection valid when the snapshot filter changes: if the
    // chosen version just got hidden, fall back to the newest visible one.
    const stillVisible = visibleVersions.some((version) => version.id === selectedVersion);
    if (!stillVisible) {
      selectedVersion = visibleVersions[0]?.id ?? "";
    }
  });

  $effect(() => {
    // Sensible port defaults per software family, until the user edits it.
    if (!portTouched) {
      port = isProxy ? DEFAULT_PROXY_PORT : DEFAULT_GAME_PORT;
    }
  });

  $effect(() => {
    // Re-preview the target folder whenever the name or parent changes,
    // debounced so we don't call the backend on every keystroke.
    if (!open || step !== "details") {
      return;
    }
    const currentName = name;
    const currentParent = locationParent;
    clearTimeout(previewTimer);
    previewTimer = setTimeout(async () => {
      try {
        locationPreview = await api.previewServerDir(currentName, currentParent);
      } catch {
        locationPreview = "";
      }
    }, 150);
  });

  onMount(() => {
    const unlistenPromise = onInstallProgress((event) => {
      if (!creating || event.step !== "download-server-jar") {
        return;
      }
      progress =
        event.totalBytes === null ? null : event.downloadedBytes / event.totalBytes;
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  });

  function chooseSoftware(entry: CatalogEntry) {
    if (!entry.available) {
      toastsStore.show(`${entry.label} support is coming soon! 🚧`);
      return;
    }
    loader = entry.value;
    step = "details";
    if (versionsForLoader !== loader) {
      loadVersions(loader);
    }
  }

  async function loadVersions(forLoader: Loader) {
    loadingVersions = true;
    try {
      versions = await api.listLoaderVersions(forLoader);
      versionsForLoader = forLoader;
      const newestRelease = versions.find((version) => version.type === "release");
      selectedVersion = newestRelease?.id ?? versions[0]?.id ?? "";
    } catch (error) {
      versions = [];
      selectedVersion = "";
      toastsStore.error(`Couldn't load versions: ${error}`);
    } finally {
      loadingVersions = false;
    }
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    creating = true;
    progress = null;
    try {
      const server = await api.createServer({
        name,
        mcVersion: selectedVersion,
        loader,
        memoryMb,
        port,
        acceptEula,
        locationParent,
        javaArgs: javaArgs.trim() === "" ? null : javaArgs.trim(),
        startCommand: startCommand.trim() === "" ? null : startCommand.trim(),
      });
      await serversStore.refresh();
      toastsStore.success(`"${server.name}" is ready! 🎂`);
      resetForm();
      onclose();
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      creating = false;
    }
  }

  function resetForm() {
    step = "software";
    name = "";
    acceptEula = false;
    memoryMb = 2048;
    portTouched = false;
    javaArgs = "";
    startCommand = "";
    advancedOpen = false;
    progress = null;
  }

  function handleClose() {
    resetForm();
    onclose();
  }

  function openEula(event: MouseEvent) {
    event.preventDefault();
    openUrl("https://aka.ms/MinecraftEULA").catch(() => {
      toastsStore.show("EULA: https://aka.ms/MinecraftEULA");
    });
  }

  async function browseLocation() {
    try {
      const picked = await openFolderDialog({
        directory: true,
        title: "Choose where your servers live",
      });
      if (typeof picked === "string") {
        locationParent = picked;
      }
    } catch (error) {
      toastsStore.error(String(error));
    }
  }
</script>

<Modal
  {open}
  wide
  title={step === "software" ? "Pick your server software 🧱" : "New server 🍰"}
  onclose={creating ? undefined : handleClose}
>
  {#if step === "software"}
    <div class="catalog" in:fade={{ duration: 120 }}>
      {#each LOADER_CATALOG as group (group.category)}
        <div class="category">
          <h4>{group.category}</h4>
          <div class="tiles">
            {#each group.entries as entry (entry.value)}
              <button
                type="button"
                class="tile"
                class:unavailable={!entry.available}
                onclick={() => chooseSoftware(entry)}
              >
                <span class="tile-emoji">{entry.emoji}</span>
                <span class="tile-name">
                  {entry.label}
                  {#if !entry.available}
                    <span class="soon">soon</span>
                  {/if}
                </span>
                <span class="tile-hint">{entry.hint}</span>
              </button>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <form onsubmit={submit} in:fade={{ duration: 120 }}>
      <button type="button" class="chosen" onclick={() => (step = "software")}>
        <span class="tile-emoji">{chosenEntry?.emoji}</span>
        <span class="chosen-name">{chosenEntry?.label}</span>
        <span class="chosen-change">change</span>
      </button>

      <section class="group">
        <h4>🧱 Basics</h4>
        <div class="group-grid">
          <label class="grow">
            <span>Name</span>
            <input
              type="text"
              bind:value={name}
              placeholder="My cozy world"
              maxlength={SERVER_NAME_MAX_LENGTH}
            />
          </label>
          <label class="grow">
            <span>Version</span>
            {#if loadingVersions}
              <div class="loading">Fetching versions… ⛏️</div>
            {:else}
              <select bind:value={selectedVersion}>
                {#each visibleVersions as version (version.id)}
                  <option value={version.id}>{version.id}</option>
                {/each}
              </select>
            {/if}
          </label>
          <label class="port-label">
            <span>Port</span>
            <input
              type="number"
              min="1024"
              max="65535"
              bind:value={port}
              oninput={() => (portTouched = true)}
              aria-invalid={portTouched && !portValid}
            />
            {#if portTouched && !portValid}
              <span class="field-error">Enter a port from 1024–65535</span>
            {/if}
          </label>
        </div>
        {#if hasSnapshots}
          <label class="checkbox">
            <input type="checkbox" bind:checked={showSnapshots} />
            <span>Show snapshots</span>
          </label>
        {/if}
        {#if isProxy}
          <p class="hint">
            Proxies keep their port in their own config (velocity.toml / config.yml) —
            set it there after the first start.
          </p>
        {/if}
      </section>

      <section class="group">
        <h4>💾 Resources & storage</h4>
        {#if !isBedrock}
          <label>
            <span>Memory — {memoryMb} MB</span>
            <input
              type="range"
              min={MEMORY_MIN_MB}
              max={MEMORY_MAX_MB}
              step={MEMORY_STEP_MB}
              bind:value={memoryMb}
            />
          </label>
        {/if}
        <div class="location">
          <span class="field-label">📁 Save location</span>
          <div class="location-row">
            <span class="location-path" title={locationPreview}>
              {locationPreview || "…"}
            </span>
            <Button variant="soft" onclick={browseLocation}>Browse…</Button>
          </div>
        </div>
      </section>

      {#if !isProxy}
        <label class="checkbox eula">
          <input type="checkbox" bind:checked={acceptEula} />
          <span>
            I accept the
            <a href="https://aka.ms/MinecraftEULA" onclick={openEula}>Minecraft EULA</a>
          </span>
        </label>
      {/if}

      <section class="group">
        <button
          type="button"
          class="advanced-toggle"
          onclick={() => (advancedOpen = !advancedOpen)}
        >
          <span class="chevron" class:open={advancedOpen}>▸</span>
          🛠️ Advanced
        </button>
        {#if advancedOpen}
          <div class="advanced-body">
            {#if !isBedrock}
              <label>
                <span>Extra JVM arguments</span>
                <input
                  type="text"
                  bind:value={javaArgs}
                  placeholder="-XX:+UseG1GC -XX:MaxGCPauseMillis=200"
                  spellcheck="false"
                />
              </label>
            {/if}
            <label>
              <span>Custom start command (overrides everything)</span>
              <input
                type="text"
                bind:value={startCommand}
                placeholder="java -Xmx4G -jar server.jar nogui"
                spellcheck="false"
              />
            </label>
            <p class="hint">
              The custom command runs from the server folder and replaces the normal
              launch entirely — memory and JVM args above are ignored.
            </p>
          </div>
        {/if}
      </section>

      {#if creating}
        <div class="progress">
          <ProgressBar value={progress} />
          <p class="hint centered">Downloading the server software… 📦</p>
        </div>
      {/if}

      <div class="actions">
        <Button variant="ghost" onclick={() => (step = "software")}>← Back</Button>
        <Button type="submit" disabled={!canSubmit}>
          {creating ? "Creating…" : "Create server 🚀"}
        </Button>
      </div>
    </form>
  {/if}
</Modal>

<style>
  /* --- Step 1: the software catalog ------------------------------------ */
  .catalog {
    display: flex;
    flex-direction: column;
    gap: 1.1rem;
  }

  .category h4 {
    margin: 0 0 0.5rem;
    font-size: 0.8rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 0.6rem;
  }

  /* Big blocky choice tiles with the classic bevel. */
  .tile {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.25rem;
    border: none;
    font-family: inherit;
    text-align: left;
    background: var(--surface-2);
    color: var(--text);
    border-radius: 8px;
    padding: 0.75rem 0.85rem;
    cursor: pointer;
    box-shadow:
      inset 0 2px 0 rgba(255, 255, 255, 0.08),
      inset 0 -3px 0 rgba(0, 0, 0, 0.2),
      0 0 0 2px rgba(15, 15, 18, 0.3);
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .tile:hover {
    background: var(--accent-soft);
  }

  .tile.unavailable {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .tile.unavailable:hover {
    background: var(--surface-2);
  }

  .tile-emoji {
    font-size: 1.5rem;
    line-height: 1;
  }

  .tile-name {
    font-family: var(--font-pixel);
    font-size: 0.95rem;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    gap: 0.4em;
  }

  .soon {
    font-family: var(--font-body);
    font-size: 0.65rem;
    font-weight: 700;
    text-transform: uppercase;
    color: var(--peach);
    background: var(--peach-soft);
    border-radius: var(--radius-sm);
    padding: 0.15em 0.5em;
  }

  .tile-hint {
    font-size: 0.75rem;
    color: var(--muted);
    line-height: 1.3;
  }

  /* --- Step 2: details, carded like the rest of the app ------------------ */
  form {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }

  .group {
    background: color-mix(in srgb, var(--surface-2) 45%, var(--surface));
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 0.9rem 1.1rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .group h4 {
    margin: 0;
    font-size: 0.85rem;
    font-weight: 700;
  }

  .group-grid {
    display: grid;
    grid-template-columns: 1fr 1fr 110px;
    gap: 0.75rem;
  }

  @media (max-width: 700px) {
    .group-grid {
      grid-template-columns: 1fr;
    }
  }

  .field-label {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--muted);
  }

  .chosen {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    border: none;
    font-family: inherit;
    background: var(--accent-soft);
    color: var(--text);
    border-radius: var(--radius-md);
    padding: 0.6rem 0.9rem;
    cursor: pointer;
  }

  .chosen-name {
    font-family: var(--font-pixel);
    font-weight: 700;
    flex: 1;
    text-align: left;
  }

  .chosen-change {
    font-size: 0.8rem;
    font-weight: 700;
    color: var(--accent-strong);
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--muted);
  }

  .grow {
    min-width: 0;
  }

  .port-label {
    min-width: 0;
  }

  .field-error {
    font-size: 0.78rem;
    font-weight: 600;
    color: var(--strawberry);
  }

  input[type="text"],
  input[type="number"],
  select {
    font-family: inherit;
    font-size: 1rem;
    color: var(--text);
    background: var(--surface-2);
    border: 2px solid transparent;
    border-radius: var(--radius-md);
    box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.12);
    padding: 0.6em 0.9em;
    outline: none;
    transition: border-color 0.18s ease;
    min-width: 0;
  }

  input[type="text"]:focus,
  input[type="number"]:focus,
  select:focus {
    border-color: var(--accent);
  }

  input[type="range"] {
    accent-color: var(--accent);
  }

  .checkbox {
    flex-direction: row;
    align-items: center;
    gap: 0.5rem;
  }

  .checkbox input {
    width: 1.1rem;
    height: 1.1rem;
    accent-color: var(--accent);
  }

  .eula a {
    color: var(--accent);
  }

  .loading {
    color: var(--muted);
    font-weight: 400;
    padding: 0.6em 0;
  }

  .location {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .location-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .location-path {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--text);
    background: var(--surface-2);
    border-radius: var(--radius-md);
    padding: 0.6em 0.9em;
    overflow-wrap: break-word;
    word-break: break-all;
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

  .progress {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
  }

  .hint.centered {
    text-align: center;
  }

  .actions {
    display: flex;
    justify-content: space-between;
    gap: 0.6rem;
  }
</style>
