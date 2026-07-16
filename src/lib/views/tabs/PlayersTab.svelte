<script lang="ts">
  import { fade } from "svelte/transition";
  import { api, type ServerConfig } from "../../api";
  import { serversStore } from "../../stores/servers.svelte";
  import { toastsStore } from "../../stores/toasts.svelte";
  import Button from "../../components/Button.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  let manualName = $state("");

  const status = $derived(serversStore.statusOf(server.id));
  const players = $derived(serversStore.playersOf(server.id));
  const canCommand = $derived(status === "running");

  async function sendPlayerCommand(command: string, successMessage: string) {
    try {
      await api.sendServerCommand(server.id, command);
      toastsStore.success(successMessage);
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  function manualTarget(): string | null {
    const name = manualName.trim();
    if (name === "") {
      toastsStore.show("Type a player name first ✍️");
      return null;
    }
    return name;
  }

  function runManual(commandPrefix: string, label: string) {
    const name = manualTarget();
    if (name === null) {
      return;
    }
    sendPlayerCommand(`${commandPrefix} ${name}`, `${label} ${name} ✨`);
  }
</script>

<div class="players-tab">
  {#if !canCommand}
    <p class="offline-note">Player management needs the server running 🌙</p>
  {:else if players.length === 0}
    <div class="empty" in:fade={{ duration: 120 }}>
      <span class="face">🐑</span>
      <p>No players online right now — the sheep have the place to themselves.</p>
    </div>
  {:else}
    <ul class="player-list">
      {#each players as player (player)}
        <li in:fade={{ duration: 120 }}>
          <img
            src="https://mc-heads.net/avatar/{player}/40"
            alt=""
            width="40"
            height="40"
            loading="lazy"
            onerror={(event) => ((event.currentTarget as HTMLImageElement).style.display = "none")}
          />
          <span class="player-name">{player}</span>
          <span class="player-actions">
            <Button
              variant="soft"
              onclick={() => sendPlayerCommand(`op ${player}`, `Opped ${player} 👑`)}
            >
              👑 Op
            </Button>
            <Button
              variant="ghost"
              onclick={() => sendPlayerCommand(`deop ${player}`, `De-opped ${player}`)}
            >
              De-op
            </Button>
            <Button
              variant="danger"
              onclick={() => sendPlayerCommand(`kick ${player}`, `Kicked ${player} 👢`)}
            >
              👢 Kick
            </Button>
            <Button
              variant="danger"
              onclick={() => sendPlayerCommand(`ban ${player}`, `Banned ${player} 🔨`)}
            >
              🔨 Ban
            </Button>
          </span>
        </li>
      {/each}
    </ul>
  {/if}

  {#if canCommand}
    <div class="manual">
      <h3>Manage any player</h3>
      <p class="hint">Works for offline players too — whitelist, pardon, and more.</p>
      <div class="manual-row">
        <input type="text" bind:value={manualName} placeholder="Player name…" spellcheck="false" />
        <Button variant="soft" onclick={() => runManual("whitelist add", "Whitelisted")}>
          ✅ Whitelist
        </Button>
        <Button variant="ghost" onclick={() => runManual("whitelist remove", "Un-whitelisted")}>
          Remove
        </Button>
        <Button variant="ghost" onclick={() => runManual("pardon", "Pardoned")}>🕊️ Pardon</Button>
        <Button variant="danger" onclick={() => runManual("ban", "Banned")}>🔨 Ban</Button>
      </div>
    </div>
  {/if}
</div>

<style>
  .players-tab {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    padding-bottom: 1rem;
  }

  .offline-note {
    color: var(--muted);
    text-align: center;
    padding: 2.5rem 0 0;
  }

  .empty {
    text-align: center;
    color: var(--muted);
    padding: 2rem 0 0;
  }

  .face {
    font-size: 2.6rem;
    display: inline-block;
    animation: sway 3s ease-in-out infinite;
  }

  @keyframes sway {
    0%,
    100% {
      transform: rotate(-5deg);
    }
    50% {
      transform: rotate(5deg);
    }
  }

  .player-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .player-list li {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-soft);
    padding: 0.55rem 0.9rem;
  }

  .player-list img {
    border-radius: 8px;
  }

  .player-name {
    flex: 1;
    font-weight: 700;
  }

  .player-actions {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .manual {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    padding: 1rem 1.25rem;
  }

  .manual h3 {
    margin: 0 0 0.2rem;
    font-size: 1rem;
  }

  .hint {
    margin: 0 0 0.75rem;
    font-size: 0.85rem;
    color: var(--muted);
  }

  .manual-row {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .manual-row input {
    flex: 1;
    min-width: 160px;
    font-family: inherit;
    font-size: 0.95rem;
    color: var(--text);
    background: var(--surface-2);
    border: 2px solid transparent;
    border-radius: var(--radius-md);
    padding: 0.5em 0.9em;
    outline: none;
    transition: border-color 0.18s ease;
  }

  .manual-row input:focus {
    border-color: var(--accent);
  }
</style>
