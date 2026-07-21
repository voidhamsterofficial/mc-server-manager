<script lang="ts">
  import {
    Megaphone,
    Gift,
    Gamepad2,
    Swords,
    Crown,
    Save,
    ListChecks,
    Send,
    Zap,
    Sun,
    Moon,
    CloudSun,
    User,
    UserMinus,
    Navigation,
  } from "@lucide/svelte";
  import { api, type ServerConfig } from "../../ipc/api";
  import {
    applyCompletion,
    suggestCompletions,
    usageHint,
    type Suggestion,
  } from "../../util/mcCommands";
  import {
    BEDROCK_DATA_KEY,
    loadCommandData,
    type McCommandData,
  } from "../../util/mcData";
  import { serversStore } from "../../stores/servers.svelte";
  import { toastsStore } from "../../stores/toasts.svelte";
  import { contextMenuStore, type MenuEntry, type MenuItem } from "../../stores/contextMenu.svelte";
  import ConsoleView from "../../components/ConsoleView.svelte";
  import Button from "../../components/Button.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  let commandText = $state("");
  let commandInput = $state<HTMLInputElement | null>(null);
  let consoleEl = $state<HTMLDivElement | null>(null);

  const status = $derived(serversStore.statusOf(server.id));
  const consoleLines = $derived(serversStore.consoleOf(server.id));
  const canCommand = $derived(status === "running" || status === "starting");
  const isBedrock = $derived(server.loader === "bds");

  async function runCommand(command: string) {
    const echoLine = {
      spans: [{ text: `> ${command}`, color: "#ffaa00", bold: true }],
      level: "info" as const,
    };
    serversStore.appendConsole(server.id, [echoLine]);
    try {
      await api.sendServerCommand(server.id, command);
    } catch (error) {
      toastsStore.error(String(error));
    }
  }

  async function sendCommand(event: SubmitEvent) {
    event.preventDefault();
    const command = commandText.trim();
    if (command === "") {
      return;
    }
    commandText = "";
    suggestionsOpen = false;
    await runCommand(command);
  }

  /** Prefills the input with a command that still needs an argument. */
  function prefill(command: string) {
    commandText = command;
    requestAnimationFrame(() => {
      commandInput?.focus();
      const end = commandText.length;
      commandInput?.setSelectionRange(end, end);
    });
  }

  const onlinePlayers = $derived(serversStore.playersOf(server.id));

  // --- Command autocomplete ----------------------------------------------

  let suggestionsOpen = $state(false);
  let highlightedIndex = $state(0);

  /** This server's version's command set, or null when we have no data for
   *  it — completion is per-version, and a version we don't know is one we
   *  say nothing about rather than guess at. */
  let commandData = $state<McCommandData | null>(null);

  $effect(() => {
    const versionKey = isBedrock ? BEDROCK_DATA_KEY : server.mcVersion;
    let isStale = false;

    commandData = null;
    void loadCommandData(versionKey).then((data) => {
      if (!isStale) {
        commandData = data;
      }
    });

    return () => {
      isStale = true;
    };
  });

  const suggestions = $derived(
    suggestionsOpen ? suggestCompletions(commandText, commandData, onlinePlayers) : [],
  );
  const hint = $derived(usageHint(commandText, commandData));

  const commandPlaceholder = $derived.by(() => {
    if (!canCommand) {
      return "Start the server to send commands";
    }
    if (commandData === null) {
      return `Type a command… (no completions for ${server.mcVersion})`;
    }
    return "Type a command… (Tab to complete)";
  });

  function handleInput() {
    suggestionsOpen = true;
    highlightedIndex = 0;
  }

  function acceptSuggestion(suggestion: Suggestion) {
    commandText = applyCompletion(commandText, suggestion.value);
    highlightedIndex = 0;
    // Still open: the next argument usually has completions of its own.
    suggestionsOpen = true;
    commandInput?.focus();
  }

  /** Tab/Enter accept, arrows move, Escape dismisses without clearing the
   *  box. Enter only completes when a suggestion is highlighted, so the
   *  common case — typing a whole command and hitting Enter — still sends. */
  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      suggestionsOpen = false;
      return;
    }
    if (suggestions.length === 0) {
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      highlightedIndex = (highlightedIndex + 1) % suggestions.length;
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      highlightedIndex = (highlightedIndex - 1 + suggestions.length) % suggestions.length;
      return;
    }
    if (event.key === "Tab") {
      event.preventDefault();
      acceptSuggestion(suggestions[highlightedIndex]);
    }
  }

  function quickCommands(): MenuEntry[] {
    const entries: MenuEntry[] = [
      { label: "List players", icon: ListChecks, tone: "info", action: () => runCommand("list") },
      { label: "Save world", icon: Save, tone: "success", action: () => runCommand("save-all") },
      "separator",
    ];
    if (!isBedrock) {
      entries.push(
        { label: "Time: day", icon: Sun, tone: "warning", action: () => runCommand("time set day") },
        { label: "Time: night", icon: Moon, tone: "info", action: () => runCommand("time set night") },
        {
          label: "Weather: clear",
          icon: CloudSun,
          tone: "success",
          action: () => runCommand("weather clear"),
        },
        "separator",
      );
    }
    entries.push(
      { label: "Broadcast (say)…", icon: Megaphone, tone: "info", action: () => prefill("say ") },
      withPlayerPicker("Give item…", Gift, "success", "give ", isBedrock),
      { label: "Set gamemode…", icon: Gamepad2, tone: "info", action: () => prefill("gamemode ") },
      { label: "Difficulty…", icon: Swords, tone: "warning", action: () => prefill("difficulty ") },
      withPlayerPicker("Op player…", Crown, "warning", "op ", false),
      withPlayerPicker("Kick player…", UserMinus, "warning", "kick ", false),
      withPlayerPicker("Teleport to…", Navigation, "info", "tp ", isBedrock),
    );
    return entries;
  }

  /** A quick command whose first argument is a player: clicking it prefills
   *  the bare command, hovering offers the online players directly. */
  function withPlayerPicker(
    label: string,
    icon: MenuItem["icon"],
    tone: MenuItem["tone"],
    prefix: string,
    disabled: boolean,
  ): MenuItem {
    const playerEntries: MenuItem[] = onlinePlayers.map((player) => ({
      label: player,
      icon: User,
      tone: "info",
      action: () => prefill(`${prefix}${player} `),
    }));
    return {
      label,
      icon,
      tone,
      disabled,
      submenu: playerEntries,
      emptySubmenuLabel: "No players online",
      action: () => prefill(prefix),
    };
  }

  function openQuickCommands(event: MouseEvent) {
    contextMenuStore.showAbove(event, consoleEl, quickCommands());
  }
</script>

<div class="console-tab">
  <div class="console" bind:this={consoleEl}>
    <ConsoleView lines={consoleLines} />
  </div>

  <form class="command-row" onsubmit={sendCommand}>
    <Button
      variant="soft"
      square
      disabled={!canCommand}
      onclick={openQuickCommands}
      title="Quick commands"
    >
      <Zap size={18} fill="currentColor" strokeWidth={1.5} />
    </Button>
    <div class="command-field">
      {#if suggestions.length > 0}
        <ul class="suggestions" role="listbox">
          {#each suggestions as suggestion, index (suggestion.value)}
            <li>
              <button
                type="button"
                class="suggestion"
                class:highlighted={index === highlightedIndex}
                role="option"
                aria-selected={index === highlightedIndex}
                onmouseenter={() => (highlightedIndex = index)}
                onmousedown={(event) => {
                  // mousedown, not click: blur fires first and would close the
                  // list before a click ever landed. preventDefault keeps focus
                  // in the input so the caret never leaves.
                  event.preventDefault();
                  acceptSuggestion(suggestion);
                }}
              >
                <span class="suggestion-value">{suggestion.value}</span>
                <span class="suggestion-detail">{suggestion.detail}</span>
              </button>
            </li>
          {/each}
        </ul>
      {:else if hint !== null}
        <p class="usage-hint">{hint}</p>
      {/if}
      <input
        bind:this={commandInput}
        type="text"
        bind:value={commandText}
        placeholder={commandPlaceholder}
        disabled={!canCommand}
        spellcheck="false"
        autocomplete="off"
        oninput={handleInput}
        onkeydown={handleKeydown}
        onblur={() => (suggestionsOpen = false)}
      />
    </div>
    <Button type="submit" disabled={!canCommand}>
      <Send size={16} />
      Send
    </Button>
  </form>
</div>

<style>
  .console-tab {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .console {
    flex: 1;
    min-height: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    padding: 0.75rem;
  }

  /* Anchors the suggestion popup, which floats above the input rather than
     pushing the console around as it appears and disappears. */
  .command-field {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
  }

  .command-field input {
    flex: 1;
    min-width: 0;
  }

  .suggestions,
  .usage-hint {
    position: absolute;
    left: 0;
    bottom: calc(100% + 0.4rem);
    z-index: 20;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-soft);
  }

  .suggestions {
    right: 0;
    max-height: 240px;
    overflow-y: auto;
    list-style: none;
    margin: 0;
    padding: 0.25rem;
  }

  .usage-hint {
    margin: 0;
    padding: 0.4rem 0.6rem;
    font-size: 0.78rem;
    font-family: var(--font-mono);
    color: var(--muted);
    white-space: nowrap;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .suggestion {
    width: 100%;
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    border: none;
    background: transparent;
    font-family: inherit;
    text-align: left;
    color: var(--text);
    cursor: pointer;
    padding: 0.35rem 0.5rem;
    border-radius: var(--radius-sm);
  }

  .suggestion.highlighted {
    background: var(--surface-2);
  }

  .suggestion-value {
    font-weight: 700;
    font-size: 0.9rem;
  }

  .suggestion-detail {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: right;
    font-size: 0.76rem;
    font-family: var(--font-mono);
    color: var(--muted);
  }

  .command-row {
    display: flex;
    gap: 0.6rem;
  }

  .command-row input {
    flex: 1;
    font-family: var(--font-pixel);
    font-size: 0.9rem;
    color: var(--text);
    background: var(--surface);
    border: 2px solid var(--border);
    border-radius: var(--radius-md);
    padding: 0.6em 1.1em;
    outline: none;
    transition: border-color 0.18s ease;
  }

  .command-row input:focus {
    border-color: var(--accent);
  }
</style>
