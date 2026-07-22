<script lang="ts">
  import { onMount } from "svelte";
  import { Folder, Gift, Archive, Undo2, Trash2 } from "@lucide/svelte";
  import { createIntroFade } from "../../util/transitions";
  import { api, resolveBackupsDir, type BackupInfo, type ServerConfig } from "../../ipc/api";
  import { onBackupCreated } from "../../ipc/events";
  import { backupsStore } from "../../stores/backups.svelte";
  import { serversStore } from "../../stores/servers.svelte";
  import { toastsStore } from "../../stores/toasts.svelte";
  import { formatFileSize, formatDateTime } from "../../util/format";
  import { FEATURE_COLOR } from "../../util/features";
  import Button from "../../components/Button.svelte";
  import ProgressBar from "../../components/ProgressBar.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  const introFade = createIntroFade();

  let backups = $state<BackupInfo[]>([]);
  // The empty state is held back until the first listing lands, so opening the
  // tab doesn't flash "No backups yet" before the real list arrives.
  let hasListed = $state(false);
  let working = $state(false);
  let confirming = $state<{ action: "restore" | "delete"; fileName: string } | null>(null);

  const status = $derived(serversStore.statusOf(server.id));
  const isStopped = $derived(status === "stopped" || status === "crashed");

  // From the store, so leaving the tab and coming back mid-backup still shows
  // the bar — and still refuses to restore over a backup that is in flight.
  const isBackingUp = $derived(backupsStore.isBackingUp(server.id));
  const backupFraction = $derived(backupsStore.fractionOf(server.id));
  const isBusy = $derived(working || isBackingUp);

  // Re-list only when the folder we're showing actually changes — either a
  // different server, or the same one after its backups location was edited
  // in the Settings tab. serversStore.refresh() hands us a fresh object with
  // the same values, and without this guard that re-listed on every refresh,
  // racing the tab's own in-flight requests.
  let listedDir: string | null = null;
  $effect(() => {
    const dir = resolveBackupsDir(server);
    if (dir === listedDir) {
      return;
    }
    listedDir = dir;
    loadBackups(server.id);
  });

  onMount(() => {
    // Refresh when a backup finishes elsewhere (e.g. right-click "Back up
    // now" while this tab was already open, or a scheduled backup).
    const unlisten = onBackupCreated((serverId) => {
      if (serverId === server.id) {
        loadBackups(server.id);
      }
    });
    return () => {
      unlisten.then((off) => off());
    };
  });

  // Creating a backup and the backup-created event can both trigger a load,
  // so more than one can be in flight at once. Only the newest may write to
  // the list — otherwise a listing taken before the backup finished can
  // resolve last and overwrite the one that contains it, which left the new
  // backup invisible until the tab was remounted.
  let latestLoadId = 0;

  async function loadBackups(serverId: string) {
    const loadId = ++latestLoadId;
    try {
      const listed = await api.listBackups(serverId);
      if (loadId !== latestLoadId) {
        return;
      }
      // Assigning the whole array is safe for the flash the keyed `{#each}`
      // would otherwise cause: rows are keyed by file name, so unchanged
      // backups keep their DOM nodes and only genuinely new ones transition.
      backups = listed;
      hasListed = true;
    } catch (error) {
      if (loadId !== latestLoadId) {
        return;
      }
      toastsStore.error(String(error));
    }
  }

  async function createBackup() {
    // Marked before the call so the bar shows straight away; the store is
    // cleared by the created/failed events, which arrive app-wide whether or
    // not this tab is still mounted.
    backupsStore.start(server.id);
    try {
      const created = await api.createBackup(server.id);
      toastsStore.success(`Backup tucked away safely (${formatFileSize(created.sizeBytes)})`);
      await loadBackups(server.id);
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function confirmAction() {
    if (confirming === null) {
      return;
    }
    const { action, fileName } = confirming;
    confirming = null;
    working = true;
    try {
      if (action === "restore") {
        await api.restoreBackup(server.id, fileName);
        toastsStore.success("Backup restored — world is back!");
      } else {
        await api.deleteBackup(server.id, fileName);
        toastsStore.show("Backup deleted");
      }
      await loadBackups(server.id);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      working = false;
    }
  }
</script>

<div class="backups-tab">
  <div class="head">
    <div class="head-text">
      <p class="hint">
        Backups zip the whole server folder.
        {#if !isStopped}A running server is flushed with <code>save-all</code> first.{/if}
        Change the location in the Settings tab.
      </p>
      <code class="location" title={resolveBackupsDir(server)}>
        <Folder size={13} /> {resolveBackupsDir(server)}
      </code>
    </div>
    <Button disabled={isBusy} onclick={createBackup}>
      {#if isBusy}
        Working…
      {:else}
        <Gift size={15} /> Back up now
      {/if}
    </Button>
  </div>

  {#if isBackingUp}
    <div class="progress">
      <ProgressBar value={backupFraction} />
      <span class="progress-label">
        {#if backupFraction === null}
          Gathering files…
        {:else}
          Zipping — {Math.round(backupFraction * 100)}%
        {/if}
      </span>
    </div>
  {/if}

  {#if backups.length === 0}
    {#if hasListed}
      <div class="empty" in:introFade>
        <span class="face"><Archive size={40} color={FEATURE_COLOR.backups} /></span>
        <p>No backups yet — make your first one, future-you will be grateful!</p>
      </div>
    {/if}
  {:else}
    <ul class="backup-list">
      {#each backups as backup (backup.fileName)}
        <li in:introFade>
          <span class="file">
            <span class="file-name">{backup.fileName}</span>
            <span class="file-meta">
              {formatDateTime(backup.createdAtUnix)} · {formatFileSize(backup.sizeBytes)}
            </span>
          </span>
          {#if confirming?.fileName === backup.fileName}
            <span class="confirm">
              <Button variant="danger" disabled={isBusy} onclick={confirmAction}>
                {confirming.action === "restore" ? "Really restore?" : "Really delete?"}
              </Button>
              <Button variant="ghost" onclick={() => (confirming = null)}>Cancel</Button>
            </span>
          {:else}
            <span class="row-actions">
              <Button
                variant="soft"
                disabled={isBusy || !isStopped}
                title={isStopped
                  ? "Replace the server folder with this backup"
                  : "Stop the server to restore"}
                onclick={() => (confirming = { action: "restore", fileName: backup.fileName })}
              >
                <Undo2 size={14} /> Restore
              </Button>
              <Button
                variant="ghost"
                disabled={isBusy}
                onclick={() => (confirming = { action: "delete", fileName: backup.fileName })}
              >
                <Trash2 size={14} />
              </Button>
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .backups-tab {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding-bottom: 1rem;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  .head-text {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    min-width: 0;
  }

  .location {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    color: var(--text);
    background: var(--surface-2);
    border-radius: var(--radius-sm);
    padding: 0.4em 0.7em;
    /* Scroll rather than wrap: breaking a path mid-word strands single
       characters on their own line. */
    white-space: nowrap;
    overflow-x: auto;
    user-select: text;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
    color: var(--muted);
  }

  .progress {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .progress-label {
    font-size: 0.8rem;
    color: var(--muted);
  }

  .hint code {
    font-family: var(--font-mono);
    background: var(--surface-2);
    border-radius: 6px;
    padding: 0.1em 0.4em;
  }

  .empty {
    text-align: center;
    color: var(--muted);
    padding: 2.5rem 0;
  }

  .face {
    font-size: 2.6rem;
    display: inline-block;
    animation: bob 2.4s ease-in-out infinite;
  }

  @keyframes bob {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-6px);
    }
  }

  .backup-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .backup-list li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-soft);
    padding: 0.6rem 0.9rem;
  }

  .file {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }

  .file-name {
    font-family: var(--font-mono);
    font-size: 0.85rem;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-meta {
    font-size: 0.78rem;
    color: var(--muted);
  }

  .row-actions,
  .confirm {
    display: flex;
    gap: 0.35rem;
    flex-shrink: 0;
  }
</style>
