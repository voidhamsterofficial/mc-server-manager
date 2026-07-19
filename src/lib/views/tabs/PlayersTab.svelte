<script lang="ts">
  import { fade } from "svelte/transition";
  import {
    CircleCheckBig,
    ShieldCheck,
    Ban,
    UserX,
    Crown,
    LogOut,
    BookOpen,
    Timer,
    DoorOpen,
  } from "@lucide/svelte";
  import { api, type RosterEntry, type ServerConfig } from "../../ipc/api";
  import { serversStore } from "../../stores/servers.svelte";
  import { toastsStore } from "../../stores/toasts.svelte";
  import { formatDateTime, formatUptime } from "../../util/format";
  import { commandArg, commandText } from "../../ipc/commands";
  import { FEATURE_COLOR } from "../../util/features";
  import { reasonPromptStore } from "../../stores/reasonPrompt.svelte";
  import Button from "../../components/Button.svelte";
  import PlayerDetailModal from "../../components/PlayerDetailModal.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  let manualName = $state("");
  let historyOpen = $state(false);
  let roster = $state<RosterEntry[]>([]);
  let inspectedPlayer = $state<string | null>(null);

  const status = $derived(serversStore.statusOf(server.id));
  const players = $derived(serversStore.playersOf(server.id));
  const canCommand = $derived(status === "running");
  const isBedrock = $derived(server.loader === "bds");
  // Bedrock calls it the allowlist; vanilla BDS has no ban/pardon commands.
  const whitelistCommand = $derived(isBedrock ? "allowlist" : "whitelist");

  $effect(() => {
    // Reload the history when opened, and keep it fresh as players come
    // and go while it is open.
    void players;
    if (historyOpen) {
      loadRoster();
    }
  });

  async function loadRoster() {
    try {
      roster = await api.getPlayerRoster(server.id);
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function sendPlayerCommand(command: string, successMessage: string) {
    try {
      await api.sendServerCommand(server.id, command);
      toastsStore.success(successMessage);
      // Ban/pardon/kick change the roster's live state; the server writes
      // banned-players.json a moment later, so refresh shortly after.
      reloadRosterSoon();
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  function reloadRosterSoon() {
    if (!historyOpen) {
      return;
    }
    setTimeout(() => loadRoster(), 700);
  }

  function manualTarget(): string | null {
    const name = manualName.trim();
    if (name === "") {
      toastsStore.show("Type a player name first");
      return null;
    }
    return name;
  }

  function runManual(commandPrefix: string, label: string) {
    const name = manualTarget();
    if (name === null) {
      return;
    }
    sendPlayerCommand(`${commandPrefix} ${commandArg(name)}`, `${label} ${name}`);
  }

  /** Kick/ban with an optional reason gathered from a small dialog. Cancelling
   *  the dialog aborts; confirming with an empty field records no reason. */
  async function moderateWithReason(
    verb: "kick" | "ban",
    name: string,
    actionLabel: string,
    successMessage: string,
  ) {
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
    await sendPlayerCommand(command, successMessage);
  }

  function kickPlayer(name: string) {
    return moderateWithReason("kick", name, "Kick", `Kicked ${name}`);
  }

  function banPlayer(name: string) {
    return moderateWithReason("ban", name, "Ban", `Banned ${name}`);
  }

  function runManualBan() {
    const name = manualTarget();
    if (name === null) {
      return;
    }
    return banPlayer(name);
  }
</script>

<div class="players-tab">
  {#if canCommand}
    <div class="manual">
      <h3>Manage any player</h3>
      <p class="hint">Works for offline players too — whitelist, pardon, and more.</p>
      <div class="manual-row">
        <input type="text" bind:value={manualName} placeholder="Player name…" spellcheck="false" />
        <Button variant="soft" onclick={() => runManual(`${whitelistCommand} add`, "Whitelisted")}>
          <CircleCheckBig size={15} /> Whitelist
        </Button>
        <Button
          variant="ghost"
          onclick={() => runManual(`${whitelistCommand} remove`, "Un-whitelisted")}
        >
          Remove
        </Button>
        {#if !isBedrock}
          <Button variant="ghost" onclick={() => runManual("pardon", "Pardoned")}>
            <ShieldCheck size={15} /> Pardon
          </Button>
          <Button variant="danger" onclick={runManualBan}><Ban size={15} /> Ban</Button>
        {/if}
      </div>
    </div>
  {:else}
    <p class="offline-note">Player management needs the server running</p>
  {/if}

  {#if canCommand && players.length === 0}
    <div class="empty" in:fade={{ duration: 120 }}>
      <span class="face"><UserX size={40} color={FEATURE_COLOR.players} /></span>
      <p>No players online right now.</p>
    </div>
  {:else if canCommand}
    <ul class="player-list">
      {#each players as player (player)}
        <li in:fade={{ duration: 120 }}>
          <img
            src="https://mc-heads.net/avatar/{encodeURIComponent(player)}/40"
            alt=""
            width="40"
            height="40"
            loading="lazy"
            onerror={(event) => ((event.currentTarget as HTMLImageElement).style.display = "none")}
          />
          <button class="player-name link" onclick={() => (inspectedPlayer = player)}>
            {player}
          </button>
          <span class="player-actions">
            <Button
              variant="soft"
              onclick={() => sendPlayerCommand(`op ${commandArg(player)}`, `Opped ${player}`)}
            >
              <Crown size={15} /> Op
            </Button>
            <Button
              variant="ghost"
              onclick={() =>
                sendPlayerCommand(`deop ${commandArg(player)}`, `De-opped ${player}`)}
            >
              De-op
            </Button>
            <Button variant="danger" onclick={() => kickPlayer(player)}>
              <LogOut size={15} /> Kick
            </Button>
            {#if !isBedrock}
              <Button variant="danger" onclick={() => banPlayer(player)}>
                <Ban size={15} /> Ban
              </Button>
            {/if}
          </span>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="history">
    <button class="history-toggle" onclick={() => (historyOpen = !historyOpen)}>
      <span class="chevron" class:open={historyOpen}>▸</span>
      <BookOpen size={16} /> Player history
      {#if historyOpen && roster.length > 0}
        <span class="count">{roster.length}</span>
      {/if}
    </button>

    {#if historyOpen}
      <div class="history-body" in:fade={{ duration: 120 }}>
        <!-- History stays last: it's reference material, not a daily control. -->
        {#if roster.length === 0}
          <p class="history-empty">No one has visited this server yet.</p>
        {:else}
          <ul class="history-list">
            {#each roster as entry (entry.name)}
              <li>
                <button class="entry-main" onclick={() => (inspectedPlayer = entry.name)}>
                  <img
                    src="https://mc-heads.net/avatar/{encodeURIComponent(entry.name)}/28"
                    alt=""
                    width="28"
                    height="28"
                    loading="lazy"
                    onerror={(event) =>
                      ((event.currentTarget as HTMLImageElement).style.display = "none")}
                  />
                  <span class="entry-name">{entry.name}</span>
                  {#if entry.online}
                    <span class="badge online">Online</span>
                  {/if}
                  {#if entry.banned}
                    <span class="badge banned"><Ban size={11} /> Banned</span>
                  {/if}
                  <span class="entry-stats">
                    <Timer size={12} /> {formatUptime(entry.totalPlaySeconds)}
                    · <DoorOpen size={12} /> {entry.joinCount} join{entry.joinCount === 1
                      ? ""
                      : "s"}
                    {#if entry.kickCount > 0}
                      · <LogOut size={12} /> {entry.kickCount} kick{entry.kickCount === 1
                        ? ""
                        : "s"}
                    {/if}
                  </span>
                </button>
                {#if entry.banned && !isBedrock}
                  <Button
                    variant="soft"
                    disabled={!canCommand}
                    title={canCommand ? "" : "Start the server to pardon"}
                    onclick={() =>
                      sendPlayerCommand(`pardon ${commandArg(entry.name)}`, `Pardoned ${entry.name}`)}
                  >
                    <ShieldCheck size={14} /> Pardon
                  </Button>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  </div>
</div>

<PlayerDetailModal
  serverId={server.id}
  playerName={inspectedPlayer}
  {canCommand}
  onclose={() => (inspectedPlayer = null)}
/>

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
    display: inline-block;
    color: var(--muted);
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
    border-radius: 4px;
    image-rendering: pixelated;
  }

  .player-name {
    flex: 1;
    font-weight: 700;
  }

  /* Clickable player names open the detail page. */
  .link {
    border: none;
    background: transparent;
    font-family: inherit;
    font-size: inherit;
    font-weight: 700;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    padding: 0;
  }

  .link:hover {
    color: var(--accent-strong);
    text-decoration: underline;
  }

  .player-actions {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .history {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    overflow: hidden;
  }

  .history-toggle {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    border: none;
    background: transparent;
    color: var(--text);
    font-family: inherit;
    font-size: 0.95rem;
    font-weight: 700;
    text-align: left;
    padding: 0.9rem 1.25rem;
    cursor: pointer;
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .history-toggle:hover {
    background: var(--surface-2);
  }

  .chevron {
    display: inline-block;
    color: var(--muted);
    transition: transform var(--duration-fast) var(--ease-out);
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .count {
    font-size: 0.78rem;
    font-weight: 700;
    color: var(--accent-strong);
    background: var(--accent-soft);
    border-radius: var(--radius-sm);
    padding: 0.15em 0.65em;
  }

  .history-body {
    padding: 0 1.25rem 1rem;
  }

  .history-empty {
    margin: 0;
    color: var(--muted);
    font-size: 0.88rem;
  }

  .history-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
  }

  .history-list li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0;
    border-bottom: 1px solid var(--border);
  }

  .history-list li:last-child {
    border-bottom: none;
  }

  /* The whole row (except the pardon button) opens the player page. */
  .entry-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 0.7rem;
    border: none;
    background: transparent;
    font-family: inherit;
    text-align: left;
    padding: 0.15rem 0.3rem;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: background-color var(--duration-fast) var(--ease-out);
  }

  .entry-main:hover {
    background: var(--surface-2);
  }

  .history-list img {
    border-radius: 4px;
    flex-shrink: 0;
    image-rendering: pixelated;
  }

  .entry-name {
    font-weight: 700;
    color: var(--text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 0.3em;
    font-size: 0.72rem;
    font-weight: 700;
    border-radius: var(--radius-sm);
    padding: 0.2em 0.65em;
    white-space: nowrap;
  }

  .badge.online {
    color: var(--mint);
    background: var(--mint-soft);
  }

  .badge.banned {
    color: var(--strawberry);
    background: var(--strawberry-soft);
  }

  .entry-stats {
    flex: 1;
    font-size: 0.8rem;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
