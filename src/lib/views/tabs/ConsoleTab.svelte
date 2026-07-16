<script lang="ts">
  import { api, type ServerConfig } from "../../api";
  import { serversStore } from "../../stores/servers.svelte";
  import { toastsStore } from "../../stores/toasts.svelte";
  import ConsoleView from "../../components/ConsoleView.svelte";
  import Button from "../../components/Button.svelte";

  interface Props {
    server: ServerConfig;
  }

  let { server }: Props = $props();

  let commandText = $state("");

  const status = $derived(serversStore.statusOf(server.id));
  const consoleLines = $derived(serversStore.consoleOf(server.id));
  const canCommand = $derived(status === "running" || status === "starting");

  async function sendCommand(event: SubmitEvent) {
    event.preventDefault();
    const command = commandText.trim();
    if (command === "") {
      return;
    }
    commandText = "";
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
</script>

<div class="console-tab">
  <div class="console">
    <ConsoleView lines={consoleLines} />
  </div>

  <form class="command-row" onsubmit={sendCommand}>
    <input
      type="text"
      bind:value={commandText}
      placeholder={canCommand
        ? "Type a command… (e.g. say hi)"
        : "Start the server to send commands"}
      disabled={!canCommand}
      spellcheck="false"
    />
    <Button type="submit" disabled={!canCommand}>Send ✉️</Button>
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
    border-radius: 999px;
    padding: 0.6em 1.1em;
    outline: none;
    transition: border-color 0.18s ease;
  }

  .command-row input:focus {
    border-color: var(--accent);
  }
</style>
