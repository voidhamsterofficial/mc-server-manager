<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
  import { api, type McVersion } from "../api";
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

  let name = $state("");
  let versions = $state<McVersion[]>([]);
  let selectedVersion = $state("");
  let memoryMb = $state(2048);
  let acceptEula = $state(false);
  let showSnapshots = $state(false);
  let loadingVersions = $state(false);
  let creating = $state(false);
  let progress = $state<number | null>(null);
  let locationParent = $state<string | null>(null);
  let locationPreview = $state("");
  let previewTimer: ReturnType<typeof setTimeout> | undefined;

  const visibleVersions = $derived(
    versions.filter((version) => showSnapshots || version.type === "release"),
  );
  const canSubmit = $derived(
    name.trim().length > 0 && selectedVersion !== "" && acceptEula && !creating,
  );

  $effect(() => {
    if (open && versions.length === 0 && !loadingVersions) {
      loadVersions();
    }
  });

  $effect(() => {
    // Re-preview the target folder whenever the name or parent changes,
    // debounced so we don't call the backend on every keystroke.
    if (!open) {
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

  async function loadVersions() {
    loadingVersions = true;
    try {
      versions = await api.listMinecraftVersions();
      const latestRelease = versions.find((version) => version.type === "release");
      selectedVersion = latestRelease?.id ?? "";
    } catch (error) {
      toastsStore.error(`Couldn't load Minecraft versions: ${error}`);
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
        loader: "vanilla",
        memoryMb,
        acceptEula,
        locationParent,
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
    name = "";
    acceptEula = false;
    memoryMb = 2048;
    progress = null;
  }

  function openEula(event: MouseEvent) {
    event.preventDefault();
    openUrl("https://aka.ms/MinecraftEULA").catch(() => {
      toastsStore.show("EULA: https://aka.ms/MinecraftEULA");
    });
  }

  async function browseLocation() {
    const picked = await openFolderDialog({
      directory: true,
      title: "Choose where your servers live",
    });
    if (typeof picked === "string") {
      locationParent = picked;
    }
  }
</script>

<Modal {open} title="New server 🍰" onclose={creating ? undefined : onclose}>
  <form onsubmit={submit}>
    <label>
      <span>Name</span>
      <input
        type="text"
        bind:value={name}
        placeholder="My cozy world"
        maxlength={SERVER_NAME_MAX_LENGTH}
      />
    </label>

    <label>
      <span>Minecraft version</span>
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

    <label class="checkbox">
      <input type="checkbox" bind:checked={showSnapshots} />
      <span>Show snapshots</span>
    </label>

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

    <div class="location">
      <span class="location-label">📁 Save location</span>
      <div class="location-row">
        <span class="location-path" title={locationPreview}>
          {locationPreview || "…"}
        </span>
        <Button variant="soft" onclick={browseLocation}>Browse…</Button>
      </div>
    </div>

    <label class="checkbox eula">
      <input type="checkbox" bind:checked={acceptEula} />
      <span>
        I accept the
        <a href="https://aka.ms/MinecraftEULA" onclick={openEula}>Minecraft EULA</a>
      </span>
    </label>

    {#if creating}
      <div class="progress">
        <ProgressBar value={progress} />
        <p class="hint">Downloading the server jar… 📦</p>
      </div>
    {/if}

    <div class="actions">
      <Button type="submit" disabled={!canSubmit}>
        {creating ? "Creating…" : "Create server 🚀"}
      </Button>
    </div>
  </form>
</Modal>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--muted);
  }

  input[type="text"],
  select {
    font-family: inherit;
    font-size: 1rem;
    color: var(--text);
    background: var(--surface-2);
    border: 2px solid transparent;
    border-radius: var(--radius-md);
    padding: 0.6em 0.9em;
    outline: none;
    transition: border-color 0.18s ease;
  }

  input[type="text"]:focus,
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

  .location-label {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--muted);
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

  .progress {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
    text-align: center;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
