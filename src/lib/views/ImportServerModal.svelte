<script lang="ts">
  import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
  import { api, type Loader } from "../ipc/api";
  import { toastsStore } from "../stores/toasts.svelte";
  import { MEMORY_MAX_MB, MEMORY_MIN_MB, MEMORY_STEP_MB } from "../util/constants";
  import Modal from "../components/Modal.svelte";
  import Button from "../components/Button.svelte";

  interface Props {
    open: boolean;
    onclose: () => void;
    onimported: () => void;
  }

  let { open, onclose, onimported }: Props = $props();

  const LOADER_OPTIONS: { value: Loader; label: string }[] = [
    { value: "vanilla", label: "Vanilla" },
    { value: "bds", label: "Bedrock" },
    { value: "paper", label: "Paper" },
    { value: "purpur", label: "Purpur" },
    { value: "folia", label: "Folia" },
    { value: "spigot", label: "Spigot" },
    { value: "fabric", label: "Fabric" },
    { value: "neoforge", label: "NeoForge" },
    { value: "forge", label: "Forge" },
    { value: "quilt", label: "Quilt" },
    { value: "arclight", label: "Arclight" },
    { value: "mohist", label: "Mohist" },
    { value: "velocity", label: "Velocity" },
    { value: "bungeecord", label: "BungeeCord" },
  ];

  let dir = $state<string | null>(null);
  let name = $state("");
  let loader = $state<Loader>("vanilla");
  let mcVersion = $state("");
  let memoryMb = $state(2048);
  let importing = $state(false);

  const canSubmit = $derived(dir !== null && name.trim().length > 0 && !importing);

  function reset() {
    dir = null;
    name = "";
    loader = "vanilla";
    mcVersion = "";
    memoryMb = 2048;
    importing = false;
  }

  function close() {
    reset();
    onclose();
  }

  /** The folder's own name, as a starting guess for the server's name. */
  function baseName(path: string): string {
    const trimmed = path.replace(/[/\\]+$/, "");
    const lastSlash = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
    return lastSlash === -1 ? trimmed : trimmed.slice(lastSlash + 1);
  }

  async function browseDir() {
    try {
      const picked = await openFolderDialog({
        directory: true,
        title: "Choose the existing server folder",
      });
      if (typeof picked !== "string") {
        return;
      }
      dir = picked;
      if (name.trim() === "") {
        name = baseName(picked);
      }
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (dir === null) {
      return;
    }
    importing = true;
    try {
      const config = await api.importServer({
        dir,
        name: name.trim(),
        loader,
        mcVersion: mcVersion.trim(),
        memoryMb,
      });
      toastsStore.success(`Imported "${config.name}"`);
      onimported();
      close();
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      importing = false;
    }
  }
</script>

<Modal {open} title="Import an existing server" onclose={close}>
  <form class="import-form" onsubmit={submit}>
    <p class="hint">
      Point Blockparty at a server folder you already have — one it lost track of, or one
      you set up another way. Its files stay exactly where they are.
    </p>

    <label>
      <span>Server folder</span>
      <div class="dir-row">
        <code class="dir-path">{dir ?? "No folder chosen"}</code>
        <Button type="button" variant="soft" onclick={browseDir}>Choose…</Button>
      </div>
    </label>

    <label>
      <span>Name</span>
      <input type="text" bind:value={name} placeholder="My imported world" />
    </label>

    <div class="grid">
      <label>
        <span>Software</span>
        <select bind:value={loader}>
          {#each LOADER_OPTIONS as option (option.value)}
            <option value={option.value}>{option.label}</option>
          {/each}
        </select>
      </label>
      <label>
        <span>Minecraft version (optional)</span>
        <input type="text" bind:value={mcVersion} placeholder="e.g. 1.21.1" />
      </label>
    </div>

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

    <p class="hint">
      If the folder already has Blockparty's own settings file, its saved software,
      version, and memory are used instead of what's picked here.
    </p>

    <div class="actions">
      <Button type="button" variant="ghost" onclick={close}>Cancel</Button>
      <Button type="submit" disabled={!canSubmit}>
        {importing ? "Importing…" : "Import server"}
      </Button>
    </div>
  </form>
</Modal>

<style>
  .import-form {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--muted);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 0.9rem;
  }

  .dir-row {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .dir-path {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--text);
    background: var(--surface-2);
    border-radius: var(--radius-md);
    padding: 0.55em 0.8em;
    overflow-wrap: break-word;
    word-break: break-all;
  }

  /* Text and select controls inherit the app-wide blocky style from theme.css. */
  input[type="range"] {
    accent-color: var(--accent);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
    margin-top: 0.4rem;
  }
</style>
