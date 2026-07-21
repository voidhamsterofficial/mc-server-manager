<script lang="ts">
  import { fade } from "svelte/transition";
  import {
    FolderOpen,
    Pencil,
    Copy,
    Trash2,
    ArrowLeft,
    Save,
    Folder,
    FileText,
    Package,
    Upload,
    ExternalLink,
  } from "@lucide/svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { watchFileDrops } from "../../util/dragDrop";
  import {
    api,
    resolveServerFilePath,
    type DirEntry,
    type ServerConfig,
  } from "../../ipc/api";
  import { toastsStore } from "../../stores/toasts.svelte";
  import { contextMenuStore, type MenuEntry } from "../../stores/contextMenu.svelte";
  import { formatFileSize } from "../../util/format";
  import { highlight, lineCount, syntaxFor } from "../../util/highlight";
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

  // Drag-and-drop import state: files dragged in from the OS land in the
  // folder currently on screen (drop jars straight into plugins/).
  let isDropTargeted = $state(false);
  let importing = $state(false);

  const breadcrumbs = $derived(buildBreadcrumbs(currentPath));

  // Editor rendering: a highlighted <pre> sits directly under a transparent
  // <textarea>, and a gutter beside them. All three share one font metric, so
  // the painted text lines up with the real caret.
  let editorTextarea = $state<HTMLTextAreaElement | null>(null);
  let highlightLayer = $state<HTMLPreElement | null>(null);
  let gutter = $state<HTMLDivElement | null>(null);

  const editorSyntax = $derived(openFile === null ? "plain" : syntaxFor(openFile));
  const highlightedContents = $derived(highlight(fileContents, editorSyntax));
  const editorLineNumbers = $derived(
    Array.from({ length: lineCount(fileContents) }, (_, index) => index + 1),
  );

  /** Keeps the painted layer and the gutter under the textarea's scroll. */
  function syncEditorScroll() {
    if (!editorTextarea) {
      return;
    }
    if (highlightLayer) {
      highlightLayer.scrollTop = editorTextarea.scrollTop;
      highlightLayer.scrollLeft = editorTextarea.scrollLeft;
    }
    if (gutter) {
      gutter.scrollTop = editorTextarea.scrollTop;
    }
  }

  $effect(() => {
    // Reload when the server changes.
    void server.id;
    currentPath = "";
    openFile = null;
    loadDir("");
  });

  $effect(() =>
    // Drops are ignored while the editor is open — the file list, and so the
    // destination folder, isn't on screen then.
    watchFileDrops({
      isAccepting: () => openFile === null && !importing,
      onHoverChange: (isOver) => (isDropTargeted = isOver),
      onDrop: (paths) => void importFiles(paths),
    }),
  );

  /** Copies dropped OS files into the folder currently being browsed. */
  async function importFiles(sourcePaths: string[]) {
    importing = true;
    const failures: string[] = [];
    let importedCount = 0;

    for (const sourcePath of sourcePaths) {
      try {
        await api.importServerFile(server.id, currentPath, sourcePath);
        importedCount += 1;
      } catch (error) {
        failures.push(String(error));
      }
    }

    importing = false;
    if (importedCount > 0) {
      const folderLabel = currentPath === "" ? "root" : currentPath;
      toastsStore.success(`Added ${importedCount} file(s) to ${folderLabel}`);
      await loadDir(currentPath);
    }
    for (const failure of failures) {
      toastsStore.error(failure);
    }
  }

  function buildBreadcrumbs(path: string): { label: string; path: string }[] {
    const crumbs = [{ label: "root", path: "" }];
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
      toastsStore.success("File saved");
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

  /** Opens the OS file manager with this entry selected, so it can be dragged
   *  out, renamed, or opened in a real editor. */
  async function revealEntry(entry: DirEntry) {
    try {
      await revealItemInDir(resolveServerFilePath(server, entry.relPath));
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
      items.push({
        label: "Open folder",
        icon: FolderOpen,
        tone: "info",
        action: () => openEntry(entry),
      });
    } else {
      items.push({
        label: "Edit file",
        icon: Pencil,
        tone: "info",
        disabled: !isTextFile(entry.name),
        action: () => openEntry(entry),
      });
    }
    items.push(
      {
        label: entry.isDir ? "Show folder in file manager" : "Open file location",
        icon: ExternalLink,
        action: () => revealEntry(entry),
      },
      { label: "Copy name", icon: Copy, action: () => copyToClipboard(entry.name, "Copied name") },
      "separator",
      {
        label: "Delete",
        icon: Trash2,
        danger: true,
        action: () => (confirmingDelete = entry.relPath),
      },
    );
    contextMenuStore.show(event, items);
  }
</script>

<div class="files-tab">
  {#if isDropTargeted && openFile === null}
    <div class="drop-overlay" transition:fade={{ duration: 120 }}>
      <Upload size={28} />
      <p>Drop files into <strong>{currentPath === "" ? "root" : currentPath}</strong></p>
    </div>
  {/if}
  {#if openFile !== null}
    <div class="editor-head">
      <Button variant="ghost" onclick={() => (openFile = null)}>
        <ArrowLeft size={15} /> Back to files
      </Button>
      <code class="editing">{openFile}</code>
      <Button disabled={savingFile} onclick={saveFile}><Save size={15} /> Save</Button>
    </div>
    <div class="editor">
      <div class="gutter" bind:this={gutter} aria-hidden="true">
        {#each editorLineNumbers as lineNumber (lineNumber)}
          <div class="line-number">{lineNumber}</div>
        {/each}
      </div>
      <div class="code-area">
        <!-- Painted text. aria-hidden because the textarea above carries the
             real, accessible copy of the same content. -->
        <pre class="highlight" bind:this={highlightLayer} aria-hidden="true">{@html
            highlightedContents}</pre>
        <textarea
          class="code-input"
          bind:this={editorTextarea}
          bind:value={fileContents}
          onscroll={syncEditorScroll}
          spellcheck="false"
          autocomplete="off"
          autocapitalize="off"
          wrap="off"
          aria-label="File contents"
        ></textarea>
      </div>
    </div>
  {:else}
    <nav class="breadcrumbs">
      {#each breadcrumbs as crumb, index (crumb.path)}
        {#if index > 0}<span class="sep">/</span>{/if}
        <button class="crumb" onclick={() => loadDir(crumb.path)}>{crumb.label}</button>
      {/each}
      <span class="drop-hint">
        {importing ? "adding files…" : "drag files here to add them"}
      </span>
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
              <span class="entry-icon">
                {#if entry.isDir}
                  <Folder size={17} color="var(--feat-files)" />
                {:else if isTextFile(entry.name)}
                  <FileText size={17} color="var(--feat-console)" />
                {:else}
                  <Package size={17} color="var(--feat-mods)" />
                {/if}
              </span>
              <span class="entry-name">{entry.name}</span>
              <span class="entry-size">{entry.isDir ? "" : formatFileSize(entry.sizeBytes)}</span>
            </button>
            {#if confirmingDelete === entry.relPath}
              <Button variant="danger" onclick={() => deleteEntry(entry.relPath)}>Sure?</Button>
              <Button variant="ghost" onclick={() => (confirmingDelete = null)}>No</Button>
            {:else}
              <Button variant="ghost" square onclick={() => (confirmingDelete = entry.relPath)}>
                <Trash2 size={15} />
              </Button>
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
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    height: 100%;
    padding-bottom: 1rem;
  }

  /* The overlay only marks the drop zone — the OS owns the drag itself, so
     it never needs to take pointer events. */
  .drop-overlay {
    position: absolute;
    inset: 0;
    z-index: 5;
    pointer-events: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    color: var(--accent-strong);
    background: color-mix(in srgb, var(--surface) 82%, var(--accent));
    border: 2px dashed var(--accent);
    border-radius: var(--radius-md);
  }

  .drop-overlay p {
    margin: 0;
    font-size: 0.95rem;
  }

  .drop-hint {
    margin-left: auto;
    font-size: 0.8rem;
    color: var(--muted);
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
    padding: 0.15rem 0.4rem;
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  /* Highlight the whole row on hover, not a smaller box inside it. */
  .entries li:hover {
    background: var(--surface-2);
    border-color: color-mix(in srgb, var(--border) 45%, var(--accent));
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
    font-size: 0.95rem;
    text-align: left;
    color: var(--text);
    cursor: pointer;
    /* Tall padding so the click target fills the row height. */
    padding: 0.55rem 0.4rem;
    border-radius: var(--radius-sm);
  }

  .entry-icon {
    display: inline-flex;
    flex-shrink: 0;
    color: var(--muted);
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
    font-size: 0.85rem;
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
    display: flex;
    background: #1a1b1e;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .editor:focus-within {
    border-color: var(--accent);
  }

  /* One font metric shared by the gutter, the painted layer and the real
     input. If these three ever disagree, the highlighting slides out from
     under the caret — so they are set here, once, and inherited. */
  .gutter,
  .highlight,
  .code-input {
    font-family: var(--font-mono);
    font-size: 0.85rem;
    line-height: 1.5;
    tab-size: 4;
  }

  .gutter {
    flex-shrink: 0;
    overflow: hidden;
    padding: 0.75rem 0.6rem 0.75rem 0.75rem;
    text-align: right;
    color: #5c5c66;
    background: #17181b;
    border-right: 1px solid #26272c;
    user-select: none;
  }

  .line-number {
    white-space: pre;
  }

  .code-area {
    position: relative;
    flex: 1;
    min-width: 0;
  }

  .highlight,
  .code-input {
    position: absolute;
    inset: 0;
    margin: 0;
    padding: 0.75rem 0.9rem;
    white-space: pre;
    overflow: auto;
    border: none;
  }

  .highlight {
    pointer-events: none;
    color: #d8d8dc;
    /* The textarea above owns scrolling; this is driven from its handler. */
    overflow: hidden;
  }

  .code-input {
    resize: none;
    background: transparent;
    /* Transparent text with a visible caret: the characters the user reads
       are the painted ones underneath, perfectly aligned with these. */
    color: transparent;
    caret-color: #ffd479;
    outline: none;
  }

  .code-input::selection {
    background: rgba(255, 212, 121, 0.28);
  }

  /* Token colours are fixed rather than themed, matching the always-dark
     editor surface. */
  .highlight :global(.tok-comment) {
    color: #6f7382;
    font-style: italic;
  }
  .highlight :global(.tok-key) {
    color: #8ec9ff;
  }
  .highlight :global(.tok-string) {
    color: #b5e08c;
  }
  .highlight :global(.tok-number) {
    color: #ffb454;
  }
  .highlight :global(.tok-boolean) {
    color: #d4a2ff;
  }
  .highlight :global(.tok-keyword) {
    color: #ff9ec4;
    font-weight: 700;
  }
  .highlight :global(.tok-punct) {
    color: #9a9aa2;
  }
  .highlight :global(.tok-tag) {
    color: #ff9ec4;
  }
  .highlight :global(.tok-attr) {
    color: #8ec9ff;
  }
  .highlight :global(.tok-heading) {
    color: #ffd479;
    font-weight: 700;
  }
  .highlight :global(.tok-level-error) {
    color: #ff6b81;
    font-weight: 700;
  }
  .highlight :global(.tok-level-warn) {
    color: #ffb454;
    font-weight: 700;
  }
  .highlight :global(.tok-level-info) {
    color: #7fd6c1;
  }

  .hint {
    color: var(--muted);
    font-size: 0.9rem;
  }
</style>
