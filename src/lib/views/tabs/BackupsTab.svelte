<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { Folder, Gift, Archive, Undo2, Trash2 } from "@lucide/svelte";
  import { api, resolveBackupsDir, type BackupInfo, type ServerConfig } from "../../ipc/api";
  import { onBackupCreated } from "../../ipc/events";
  import { serversStore } from "../../stores/servers.svelte";
  import { toastsStore } from "../../stores/toasts.svelte";
  import { formatFileSize, formatDateTime } from "../../util/format";
  import { FEATURE_COLOR } from "../../util/features";
  import Button from "../../components/Button.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  let backups = $state<BackupInfo[]>([]);
  let working = $state(false);
  let confirming = $state<{ action: "restore" | "delete"; fileName: string } | null>(null);

  const status = $derived(serversStore.statusOf(server.id));
  const isStopped = $derived(status === "stopped" || status === "crashed");

  $effect(() => {
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

  async function loadBackups(serverId: string) {
    try {
      backups = await api.listBackups(serverId);
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function createBackup() {
    working = true;
    try {
      const created = await api.createBackup(server.id);
      toastsStore.success(`Backup tucked away safely (${formatFileSize(created.sizeBytes)})`);
      await loadBackups(server.id);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      working = false;
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
    <Button disabled={working} onclick={createBackup}>
      {#if working}
        Working…
      {:else}
        <Gift size={15} /> Back up now
      {/if}
    </Button>
  </div>

  {#if backups.length === 0}
    <div class="empty" in:fade={{ duration: 120 }}>
      <span class="face"><Archive size={40} color={FEATURE_COLOR.backups} /></span>
      <p>No backups yet — make your first one, future-you will be grateful!</p>
    </div>
  {:else}
    <ul class="backup-list">
      {#each backups as backup (backup.fileName)}
        <li in:fade={{ duration: 120 }}>
          <span class="file">
            <span class="file-name">{backup.fileName}</span>
            <span class="file-meta">
              {formatDateTime(backup.createdAtUnix)} · {formatFileSize(backup.sizeBytes)}
            </span>
          </span>
          {#if confirming?.fileName === backup.fileName}
            <span class="confirm">
              <Button variant="danger" disabled={working} onclick={confirmAction}>
                {confirming.action === "restore" ? "Really restore?" : "Really delete?"}
              </Button>
              <Button variant="ghost" onclick={() => (confirming = null)}>Cancel</Button>
            </span>
          {:else}
            <span class="row-actions">
              <Button
                variant="soft"
                disabled={working || !isStopped}
                title={isStopped
                  ? "Replace the server folder with this backup"
                  : "Stop the server to restore"}
                onclick={() => (confirming = { action: "restore", fileName: backup.fileName })}
              >
                <Undo2 size={14} /> Restore
              </Button>
              <Button
                variant="ghost"
                disabled={working}
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
    overflow-wrap: break-word;
    word-break: break-all;
    user-select: text;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
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
