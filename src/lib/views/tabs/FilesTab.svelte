<script lang="ts">
  import { fade } from "svelte/transition";
  import { api, type DirEntry, type ServerConfig } from "../../api";
  import { toastsStore } from "../../stores/toasts.svelte";
  import { contextMenuStore, type MenuEntry } from "../../stores/contextMenu.svelte";
  import { formatBytes } from "../../format";
  import Button from "../../components/Button.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  const TEXT_EXTENSIONS = [
    "txt", "properties", "yml", "yaml", "json", "json5", "toml", "conf", "cfg",
    "log", "sh", "bat", "md", "ini", "mcmeta", "csv", "xml", "html",
  ];

  let currentPath = $state("");
  let entries = $state<DirEntry[]>([]);
  let loading = $state(false);

  // Editor state for the currently open file.
  let openFile = $state<string | null>(null);
  let fileContents = $state("");
  let savingFile = $state(false);
  let confirmingDelete = $state<string | null>(null);

  const breadcrumbs = $derived(buildBreadcrumbs(currentPath));

  $effect(() => {
    // Reload when the server changes.
    void server.id;
    currentPath = "";
    openFile = null;
    loadDir("");
  });

  function buildBreadcrumbs(path: string): { label: string; path: string }[] {
    const crumbs = [{ label: "📁 root", path: "" }];
    if (path === "") {
      return crumbs;
    }
    const parts = path.split("/");
    let accumulated = "";
    for (const part of parts) {
      accumulated = accumulated === "" ? part : `${accumulated}/${part}`;
      crumbs.push({ label: part, path: accumulated });
    }
    return crumbs;
  }

  async function loadDir(path: string) {
    loading = true;
    try {
      entries = await api.listServerFiles(server.id, path);
      currentPath = path;
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      loading = false;
    }
  }

  function isTextFile(name: string): boolean {
    const ext = name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
    return TEXT_EXTENSIONS.includes(ext);
  }

  async function openEntry(entry: DirEntry) {
    if (entry.isDir) {
      await loadDir(entry.relPath);
      return;
    }
    if (!isTextFile(entry.name)) {
      toastsStore.show("That's not a text file — open the folder externally to edit it");
      return;
    }
    try {
      fileContents = await api.readServerFile(server.id, entry.relPath);
      openFile = entry.relPath;
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function saveFile() {
    if (openFile === null) {
      return;
    }
    savingFile = true;
    try {
      await api.writeServerFile(server.id, openFile, fileContents);
      toastsStore.success("File saved 💾");
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      savingFile = false;
    }
  }

  async function deleteEntry(relPath: string) {
    confirmingDelete = null;
    try {
      await api.deleteServerFile(server.id, relPath);
      toastsStore.show("Deleted");
      await loadDir(currentPath);
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function copyToClipboard(text: string, successMessage: string) {
    try {
      await navigator.clipboard.writeText(text);
      toastsStore.success(successMessage);
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  /** Right-click actions for one file or folder. */
  function openEntryMenu(event: MouseEvent, entry: DirEntry) {
    const items: MenuEntry[] = [];
    if (entry.isDir) {
      items.push({ label: "Open folder", emoji: "📂", action: () => openEntry(entry) });
    } else {
      items.push({
        label: "Edit file",
        emoji: "📝",
        disabled: !isTextFile(entry.name),
        action: () => openEntry(entry),
      });
    }
    items.push(
      { label: "Copy name", emoji: "📋", action: () => copyToClipboard(entry.name, "Copied name 📋") },
      "separator",
      {
        label: "Delete",
        emoji: "🗑",
        danger: true,
        action: () => (confirmingDelete = entry.relPath),
      },
    );
    contextMenuStore.show(event, items);
  }
</script>

<div class="files-tab">
  {#if openFile !== null}
    <div class="editor-head">
      <Button variant="ghost" onclick={() => (openFile = null)}>← Back to files</Button>
      <code class="editing">{openFile}</code>
      <Button disabled={savingFile} onclick={saveFile}>Save 💾</Button>
    </div>
    <textarea class="editor" bind:value={fileContents} spellcheck="false"></textarea>
  {:else}
    <nav class="breadcrumbs">
      {#each breadcrumbs as crumb, index (crumb.path)}
        {#if index > 0}<span class="sep">/</span>{/if}
        <button class="crumb" onclick={() => loadDir(crumb.path)}>{crumb.label}</button>
      {/each}
    </nav>

    {#if loading && entries.length === 0}
      <p class="hint">Loading…</p>
    {:else if entries.length === 0}
      <p class="hint">This folder is empty.</p>
    {:else}
      <!-- Keyed on the path so each folder's rows fade in as a smooth swap;
           entries persist through the (fast, local) load so the list never
           collapses to a "Loading…" line and snaps back. -->
      {#key currentPath}
        <ul class="entries" class:stale={loading} in:fade={{ duration: 120 }}>
          {#each entries as entry (entry.relPath)}
          <li>
            <button
              class="entry"
              onclick={() => openEntry(entry)}
              oncontextmenu={(event) => openEntryMenu(event, entry)}
            >
              <span class="entry-icon">{entry.isDir ? "📁" : isTextFile(entry.name) ? "📄" : "📦"}</span>
              <span class="entry-name">{entry.name}</span>
              <span class="entry-size">{entry.isDir ? "" : formatBytes(entry.sizeBytes)}</span>
            </button>
            {#if confirmingDelete === entry.relPath}
              <Button variant="danger" onclick={() => deleteEntry(entry.relPath)}>Sure?</Button>
              <Button variant="ghost" onclick={() => (confirmingDelete = null)}>No</Button>
            {:else}
              <Button variant="ghost" onclick={() => (confirmingDelete = entry.relPath)}>🗑</Button>
            {/if}
          </li>
        {/each}
        </ul>
      {/key}
    {/if}
  {/if}
</div>

<style>
  .files-tab {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    height: 100%;
    padding-bottom: 1rem;
  }

  .breadcrumbs {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    flex-wrap: wrap;
  }

  .crumb {
    border: none;
    background: transparent;
    color: var(--accent-strong);
    font-family: inherit;
    font-size: 0.9rem;
    font-weight: 700;
    cursor: pointer;
    padding: 0.2rem 0.4rem;
    border-radius: var(--radius-sm);
  }

  .crumb:hover {
    background: var(--surface-2);
  }

  .sep {
    color: var(--muted);
  }

  .entries {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    transition: opacity var(--duration-fast) var(--ease-out);
  }

  /* While the next folder loads, the current rows stay put and just dim,
     so navigation never jumps — no collapse to a "Loading…" line. */
  .entries.stale {
    opacity: 0.5;
    pointer-events: none;
  }

  .entries li {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 0.3rem 0.5rem;
  }

  .entry {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.6rem;
    border: none;
    background: transparent;
    font-family: inherit;
    text-align: left;
    color: var(--text);
    cursor: pointer;
    padding: 0.35rem 0.4rem;
    border-radius: var(--radius-sm);
  }

  .entry:hover {
    background: var(--surface-2);
  }

  .entry-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  .entry-size {
    font-size: 0.78rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .editor-head {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  .editing {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* The editor is always terminal-dark, in both app themes, so the text
     colour must stay light — themed --text would go near-black-on-black. */
  .editor {
    flex: 1;
    min-height: 0;
    resize: none;
    font-family: var(--font-mono);
    font-size: 0.85rem;
    line-height: 1.5;
    color: #d8d8dc;
    background: #1a1b1e;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 0.75rem 0.9rem;
    outline: none;
  }

  .editor:focus {
    border-color: var(--accent);
  }

  .hint {
    color: var(--muted);
    font-size: 0.9rem;
  }
</style>
