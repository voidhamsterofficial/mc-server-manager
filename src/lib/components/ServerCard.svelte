<script lang="ts">
  import { fly } from "svelte/transition";
  import { backOut } from "svelte/easing";
  import { api, type ServerConfig } from "../api";
  import { serversStore } from "../stores/servers.svelte";
  import { toastsStore } from "../stores/toasts.svelte";
  import StatusBlob from "./StatusBlob.svelte";
  import Button from "./Button.svelte";
  import Chip from "./Chip.svelte";

  interface Props {
    server: ServerConfig;
    index: number;
    onopen: () => void;
  }

  let { server, index, onopen }: Props = $props();

  let busy = $state(false);

  const status = $derived(serversStore.statusOf(server.id));
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
  in:fly={{ y: 24, duration: 420, delay: index * 70, easing: backOut }}
  onclick={onopen}
  onkeydown={(event) => event.key === "Enter" && onopen()}
  role="button"
  tabindex="0"
>
  <div class="top">
    <span class="name">{server.name}</span>
    <StatusBlob {status} showLabel />
  </div>
  <div class="chips">
    <Chip>🧱 {server.mcVersion}</Chip>
    <Chip>{server.loader}</Chip>
    <Chip>💾 {server.memoryMb} MB</Chip>
  </div>
  <div class="actions">
    {#if canStart}
      <Button variant="soft" disabled={busy} onclick={togglePower}>▶ Start</Button>
    {:else if canStop}
      <Button variant="danger" disabled={busy} onclick={togglePower}>⏹ Stop</Button>
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
    padding: 1.4rem 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.05rem;
    cursor: pointer;
    position: relative;
    transition:
      transform 0.22s var(--ease-bounce),
      box-shadow 0.22s ease;
  }

  /* Lift without scaling, and stack above neighbours, so hovering never
     overlaps adjacent cards' content. */
  .card:hover {
    transform: translateY(-3px);
    box-shadow: var(--shadow-pop);
    z-index: 1;
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
