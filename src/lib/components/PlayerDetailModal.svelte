<script lang="ts">
  import { Ban, ShieldCheck, Crown, LogOut, MessageSquare } from "@lucide/svelte";
  import { api, type PlayerDetail } from "../api";
  import { toastsStore } from "../stores/toasts.svelte";
  import { formatDateTime, formatUptime } from "../format";
  import { commandArg, commandText } from "../commands";
  import { reasonPromptStore } from "../stores/reasonPrompt.svelte";
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
  // Bumped on every load so a slow response for a previously-viewed player
  // can't overwrite the one we're looking at now.
  let loadToken = 0;

  $effect(() => {
    if (playerName !== null) {
      loadDetail(serverId, playerName);
    } else {
      detail = null;
    }
  });

  async function loadDetail(id: string, name: string) {
    const token = ++loadToken;
    loading = true;
    try {
      const result = await api.getPlayerDetail(id, name);
      if (token === loadToken) {
        detail = result;
      }
    } catch (error) {
      if (token === loadToken) {
        toastsStore.error(String(error));
      }
    } finally {
      if (token === loadToken) {
        loading = false;
      }
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

  /** Kick/ban the shown player with an optional reason from a small dialog. */
  async function moderateWithReason(
    verb: "kick" | "ban",
    actionLabel: string,
    successMessage: string,
  ) {
    const name = detail?.name;
    if (!name) {
      return;
    }
    const reason = await reasonPromptStore.ask({
      title: `${actionLabel} ${name}`,
      actionLabel,
      variant: "danger",
    });
    if (reason === null) {
      return;
    }
    const base = `${verb} ${commandArg(name)}`;
    const command = reason === "" ? base : `${base} ${commandText(reason)}`;
    await runPlayerCommand(command, successMessage);
    reloadDetailSoon();
  }

  function pardon() {
    const name = detail?.name;
    if (!name) {
      return;
    }
    runPlayerCommand(`pardon ${commandArg(name)}`, `Pardoned ${name}`).then(reloadDetailSoon);
  }

  /** The server writes banned-players.json a moment after the command runs, so
   *  reload once more shortly after to pick up the new ban state and reason. */
  function reloadDetailSoon() {
    setTimeout(() => {
      if (playerName) {
        loadDetail(serverId, playerName);
      }
    }, 700);
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
        src="https://mc-heads.net/avatar/{encodeURIComponent(detail.name)}/64"
        alt=""
        width="64"
        height="64"
        onerror={(event) => ((event.currentTarget as HTMLImageElement).style.display = "none")}
      />
      <div class="head-text">
        <h2 class="name">{detail.name}</h2>
        <div class="badges">
          {#if detail.online}<span class="badge online">Online</span>{/if}
          {#if detail.banned}<span class="badge banned"><Ban size={12} /> Banned</span>{/if}
        </div>
      </div>
    </div>

    {#if detail.banned}
      <div class="ban-notice">
        <span class="ban-label"><Ban size={12} /> Ban reason</span>
        <span class="ban-reason">{detail.banReason ?? "No reason recorded"}</span>
      </div>
    {/if}

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
          onclick={pardon}
        >
          <ShieldCheck size={15} /> Pardon
        </Button>
      {:else}
        <Button
          variant="danger"
          disabled={!canCommand}
          onclick={() => moderateWithReason("ban", "Ban", `Banned ${detail?.name}`)}
        >
          <Ban size={15} /> Ban
        </Button>
      {/if}
      {#if detail.online}
        <Button
          variant="soft"
          disabled={!canCommand}
          onclick={() =>
            runPlayerCommand(`op ${commandArg(detail?.name ?? "")}`, `Opped ${detail?.name}`)}
        >
          <Crown size={15} /> Op
        </Button>
        <Button
          variant="danger"
          disabled={!canCommand}
          onclick={() => moderateWithReason("kick", "Kick", `Kicked ${detail?.name}`)}
        >
          <LogOut size={15} /> Kick
        </Button>
      {/if}
    </div>

    <h3 class="chat-title"><MessageSquare size={16} /> Recent chat</h3>
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
    display: inline-flex;
    align-items: center;
    gap: 0.3em;
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

  .ban-notice {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    background: var(--strawberry-soft);
    border-radius: var(--radius-sm);
    padding: 0.6em 0.85em;
    margin-bottom: 1rem;
  }

  .ban-label {
    display: inline-flex;
    align-items: center;
    gap: 0.3em;
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--strawberry);
  }

  .ban-reason {
    font-size: 0.9rem;
    word-break: break-word;
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
    display: flex;
    align-items: center;
    gap: 0.4em;
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
