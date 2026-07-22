<script lang="ts">
  import { fade } from "svelte/transition";
  import { Blocks, Save, Users, Play, Square } from "@lucide/svelte";
  import { api, type ServerConfig } from "../ipc/api";
  import { serversStore } from "../stores/servers.svelte";
  import { statsStore } from "../stores/stats.svelte";
  import { startServerWithPortCheck } from "../util/startServer";
  import { toastsStore } from "../stores/toasts.svelte";
  import { formatMemory } from "../util/format";
  import StatusBlob from "./StatusBlob.svelte";
  import Sparkline from "./Sparkline.svelte";
  import Button from "./Button.svelte";
  import Chip from "./Chip.svelte";

  interface Props {
    server: ServerConfig;
    onopen: () => void;
    oncontextmenu?: (event: MouseEvent) => void;
  }

  let { server, onopen, oncontextmenu }: Props = $props();

  const BYTES_PER_MB = 1024 * 1024;

  let busy = $state(false);

  const status = $derived(serversStore.statusOf(server.id));
  const players = $derived(serversStore.playersOf(server.id));
  const canStart = $derived(status === "stopped" || status === "crashed");
  const canStop = $derived(status === "running" || status === "starting");

  // Live resource usage for the mini graphs. When the server isn't running
  // there's no sample, so the values read "—" and the sparklines sit empty —
  // the strip still gives the card shape and fills in the moment it starts.
  const stats = $derived(statsStore.of(server.id));
  const isLive = $derived(stats.latest !== null);
  const cpuText = $derived(isLive ? `${stats.latest!.cpuPercent.toFixed(1)} %` : "—");
  const memoryText = $derived(isLive ? formatMemory(stats.latest!.memoryBytes) : "—");
  // Fixed scale relative to the configured heap keeps the line stable; the JVM
  // can exceed -Xmx with off-heap memory, hence the 1.5× headroom.
  const memoryScaleMax = $derived(server.memoryMb * BYTES_PER_MB * 1.5);

  async function togglePower(event: MouseEvent) {
    event.stopPropagation();
    busy = true;
    try {
      if (canStart) {
        await startServerWithPortCheck(server);
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
  <div class="stats" class:live={isLive}>
    <div class="stat">
      <div class="stat-head">
        <span class="stat-label">CPU</span>
        <span class="stat-value">{cpuText}</span>
      </div>
      <Sparkline values={stats.cpuHistory} max={100} color="var(--chart-cpu)" height={30} />
    </div>
    <div class="stat">
      <div class="stat-head">
        <span class="stat-label">Memory</span>
        <span class="stat-value">{memoryText}</span>
      </div>
      <Sparkline
        values={stats.memoryHistory}
        max={memoryScaleMax}
        color="var(--chart-mem)"
        height={30}
      />
    </div>
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
    /* No max-width: the dashboard's fixed two-column grid already stops a
       lone server becoming a screen-wide slab, and capping here would leave
       dead space on a wide window instead of letting the cards grow. */
    width: 100%;
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

  .stats {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.6rem;
  }

  /* Dim the readouts when there's no live process, so an idle server's empty
     graphs read as "asleep" rather than looking broken. */
  .stats:not(.live) {
    opacity: 0.65;
  }

  .stat {
    background: var(--surface-2);
    border-radius: var(--radius-md);
    box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.08);
    padding: 0.55rem 0.7rem 0.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    min-width: 0;
  }

  .stat-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .stat-label {
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }

  .stat-value {
    font-family: var(--font-pixel);
    font-size: 0.92rem;
    color: var(--text);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
