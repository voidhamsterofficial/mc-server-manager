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
  } from "@lucide/svelte";
  import { api, type ServerConfig } from "../../api";
  import { serversStore } from "../../stores/servers.svelte";
  import { toastsStore } from "../../stores/toasts.svelte";
  import { contextMenuStore, type MenuEntry } from "../../stores/contextMenu.svelte";
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
      {
        label: "Give item…",
        icon: Gift,
        tone: "success",
        disabled: isBedrock,
        action: () => prefill("give "),
      },
      { label: "Set gamemode…", icon: Gamepad2, tone: "info", action: () => prefill("gamemode ") },
      { label: "Difficulty…", icon: Swords, tone: "warning", action: () => prefill("difficulty ") },
      { label: "Op player…", icon: Crown, tone: "warning", action: () => prefill("op ") },
    );
    return entries;
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
    <input
      bind:this={commandInput}
      type="text"
      bind:value={commandText}
      placeholder={canCommand
        ? "Type a command… (e.g. say hi)"
        : "Start the server to send commands"}
      disabled={!canCommand}
      spellcheck="false"
    />
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
