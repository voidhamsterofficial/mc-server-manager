<script lang="ts">
  import { fade } from "svelte/transition";
  import { Blocks, Save, Users, Play, Square } from "@lucide/svelte";
  import { api, type ServerConfig } from "../api";
  import { serversStore } from "../stores/servers.svelte";
  import { toastsStore } from "../stores/toasts.svelte";
  import StatusBlob from "./StatusBlob.svelte";
  import Button from "./Button.svelte";
  import Chip from "./Chip.svelte";

  interface Props {
    server: ServerConfig;
    onopen: () => void;
    oncontextmenu?: (event: MouseEvent) => void;
  }

  let { server, onopen, oncontextmenu }: Props = $props();

  let busy = $state(false);

  const status = $derived(serversStore.statusOf(server.id));
  const players = $derived(serversStore.playersOf(server.id));
  const canStart = $derived(status === "stopped" || status === "crashed");
  const canStop = $derived(status === "running" || status === "starting");

  async function togglePower(event: MouseEvent) {
    event.stopPropagation();
    busy = true;
    try {
      if (canStart) {
        await api.startServer(server.id);
      } else if (canStop) {
        await api.stopServer(server.id);
      }
    } catch (error) {
      toastsStore.error(String(error));
    } finally {
      busy = false;
    }
  }
</script>

<div
  class="card"
  in:fade={{ duration: 120 }}
  onclick={onopen}
  {oncontextmenu}
  onkeydown={(event) => event.key === "Enter" && onopen()}
  role="button"
  tabindex="0"
>
  <div class="top">
    <span class="name">{server.name}</span>
    <StatusBlob {status} showLabel />
  </div>
  <div class="chips">
    <Chip tone="info"><Blocks size={13} /> {server.mcVersion}</Chip>
    <Chip>{server.loader}</Chip>
    <Chip tone="warning"><Save size={13} /> {server.memoryMb} MB</Chip>
    {#if status === "running"}
      <Chip tone="success"><Users size={13} /> {players.length} online</Chip>
    {/if}
  </div>
  <div class="actions">
    {#if canStart}
      <Button variant="soft" disabled={busy} onclick={togglePower}>
        <Play size={15} /> Start
      </Button>
    {:else if canStop}
      <Button variant="danger" disabled={busy} onclick={togglePower}>
        <Square size={15} /> Stop
      </Button>
    {:else}
      <Button variant="ghost" disabled onclick={() => {}}>…</Button>
    {/if}
  </div>
</div>

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-soft);
    width: 100%;
    max-width: 560px;
    padding: 1.4rem 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.05rem;
    cursor: pointer;
    transition: border-color var(--duration-fast) var(--ease-out);
  }

  .card:hover {
    border-color: color-mix(in srgb, var(--border) 40%, var(--accent));
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .name {
    font-weight: 700;
    font-size: 1.08rem;
    line-height: 1.3;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.45rem;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
