<script lang="ts">
  import { api, type PlayerDetail } from "../api";
  import { toastsStore } from "../stores/toasts.svelte";
  import { formatDateTime, formatUptime } from "../format";
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";

  interface Props {
    serverId: string;
    playerName: string | null;
    /** Whether player commands can currently be sent (server running). */
    canCommand: boolean;
    onclose: () => void;
  }

  let { serverId, playerName, canCommand, onclose }: Props = $props();

  let detail = $state<PlayerDetail | null>(null);
  let loading = $state(false);

  $effect(() => {
    if (playerName !== null) {
      loadDetail(serverId, playerName);
    } else {
      detail = null;
    }
  });

  async function loadDetail(id: string, name: string) {
    loading = true;
    try {
      detail = await api.getPlayerDetail(id, name);
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      loading = false;
    }
  }

  async function runPlayerCommand(command: string, message: string) {
    try {
      await api.sendServerCommand(serverId, command);
      toastsStore.success(message);
      if (playerName) {
        await loadDetail(serverId, playerName);
      }
    } catch (error) {
      toastsStore.error(String(error));
    }
  }
</script>

<Modal open={playerName !== null} title="Player" {onclose}>
  {#if loading && detail === null}
    <p class="muted">Loading…</p>
  {:else if detail === null}
    <p class="muted">No history for this player yet.</p>
  {:else}
    <div class="head">
      <img
        src="https://mc-heads.net/avatar/{detail.name}/64"
        alt=""
        width="64"
        height="64"
        onerror={(event) => ((event.currentTarget as HTMLImageElement).style.display = "none")}
      />
      <div class="head-text">
        <h2 class="name">{detail.name}</h2>
        <div class="badges">
          {#if detail.online}<span class="badge online">🟢 Online</span>{/if}
          {#if detail.banned}<span class="badge banned">🔨 Banned</span>{/if}
        </div>
      </div>
    </div>

    <div class="stats">
      <div class="stat"><span>Playtime</span><b>{formatUptime(detail.totalPlaySeconds)}</b></div>
      <div class="stat"><span>Joins</span><b>{detail.joinCount}</b></div>
      <div class="stat"><span>Kicks</span><b>{detail.kickCount}</b></div>
      <div class="stat"><span>Messages</span><b>{detail.chatCount}</b></div>
      <div class="stat"><span>Game mode</span><b>{detail.lastGameMode ?? "unknown"}</b></div>
      <div class="stat">
        <span>First seen</span><b>{detail.firstJoinedUnix ? formatDateTime(detail.firstJoinedUnix) : "—"}</b>
      </div>
      <div class="stat">
        <span>Last seen</span>
        <b>{detail.online ? "playing now" : formatDateTime(detail.lastSeenUnix)}</b>
      </div>
    </div>

    <div class="actions">
      {#if detail.banned}
        <Button
          variant="soft"
          disabled={!canCommand}
          title={canCommand ? "" : "Start the server to run this"}
          onclick={() => runPlayerCommand(`pardon ${detail?.name}`, `Pardoned ${detail?.name} 🕊️`)}
        >
          🕊️ Pardon
        </Button>
      {:else}
        <Button
          variant="danger"
          disabled={!canCommand}
          onclick={() => runPlayerCommand(`ban ${detail?.name}`, `Banned ${detail?.name} 🔨`)}
        >
          🔨 Ban
        </Button>
      {/if}
      {#if detail.online}
        <Button
          variant="soft"
          disabled={!canCommand}
          onclick={() => runPlayerCommand(`op ${detail?.name}`, `Opped ${detail?.name} 👑`)}
        >
          👑 Op
        </Button>
        <Button
          variant="danger"
          disabled={!canCommand}
          onclick={() => runPlayerCommand(`kick ${detail?.name}`, `Kicked ${detail?.name} 👢`)}
        >
          👢 Kick
        </Button>
      {/if}
    </div>

    <h3 class="chat-title">💬 Recent chat</h3>
    {#if detail.recentChat.length === 0}
      <p class="muted">Nothing said yet.</p>
    {:else}
      <ul class="chat">
        {#each detail.recentChat as entry, index (index)}
          <li>
            <span class="chat-time">{formatDateTime(entry.atUnix)}</span>
            <span class="chat-msg">{entry.message}</span>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</Modal>

<style>
  .muted {
    color: var(--muted);
    margin: 0.5rem 0;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 0.9rem;
    margin-bottom: 1rem;
  }

  .head img {
    border-radius: 6px;
    image-rendering: pixelated;
  }

  .name {
    margin: 0;
    font-size: 1.2rem;
  }

  .badges {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.3rem;
  }

  .badge {
    font-size: 0.72rem;
    font-weight: 700;
    border-radius: var(--radius-sm);
    padding: 0.2em 0.65em;
  }

  .badge.online {
    color: var(--mint);
    background: var(--mint-soft);
  }

  .badge.banned {
    color: var(--strawberry);
    background: var(--strawberry-soft);
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .stat {
    display: flex;
    justify-content: space-between;
    gap: 0.5rem;
    background: var(--surface-2);
    border-radius: var(--radius-sm);
    padding: 0.5em 0.75em;
    font-size: 0.85rem;
  }

  .stat span {
    color: var(--muted);
  }

  .stat b {
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-bottom: 1rem;
  }

  .chat-title {
    margin: 0 0 0.5rem;
    font-size: 0.95rem;
  }

  .chat {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 200px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .chat li {
    display: flex;
    gap: 0.6rem;
    font-size: 0.82rem;
    padding: 0.2rem 0;
    border-bottom: 1px solid var(--border);
  }

  .chat-time {
    color: var(--muted);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .chat-msg {
    word-break: break-word;
  }
</style>
